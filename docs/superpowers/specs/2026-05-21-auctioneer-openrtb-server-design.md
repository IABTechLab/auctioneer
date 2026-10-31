# Auctioneer OpenRTB Server Design on EdgeZero

## M3b Review Contract (2026-10-05)

For M3b, the [current review corrections](../plans/2026-07-10-m3b-multi-adapter-production-fanout.md#current-review-corrections-2026-10-05)
supersede conflicting historical formulas and coordination descriptions
below. In particular, bidder/alias identifiers have a 64-byte UTF-8 limit;
request copies charge syntactic-node heap and preflight repeated supplement
expansion; every probe config resolution is counted; and complete status
bodies have an 8192-byte limit, with diagnosis space reserved first. Probe
output retention has four independently funded slots, held through terminal
egress even before bootstrap. Other route classes cannot consume them; when
probes themselves exhaust the pool, admission aborts without another body.
Config-work permits do not fund retained outputs, and provisional platform
population estimates are not used as a probe-output bound.
Mandatory bootstrap warnings use EdgeZero's independent boot
target filter, while ordinary logging uses the configured level from the
start. Auctioneer APIs may break without compatibility shims. The historical
2026-10-07 integration pinned EdgeZero PR #275 at `ea66eff045512cccc60646d6ec339e83ff44fed7`
(2026-10-07), including deferred Fastly logging diagnostics, validation
of batch driver EOF after the final slot, Axum declared-length response
EOF validation and preservation of adapter-selected platform metadata.
Application decoder/resource caps and provider capability classifications are unchanged;
revision-specific records below remain historical. The current inherited
source is merged M5 (`main` at `b01c309`), whose Cargo manifest and lockfile
pin `28038cce26a499499fd4e050387d4d02e35fa7c7`. M6 conflict resolution
preserves that source and dependency pin without upgrading the SDK.
The SDK revision discussed in this historical record centralizes future-based
outbound batches and
response-body policy and adds opt-in reusable lifecycle helpers. Auctioneer keeps
its existing application lifecycle; this update does not enable retention.
The 2026-10-07 SDK follow-up additionally supplies bounded native HTTP/1
ingress and immutable startup config snapshots, shared UTF-8 config values,
bounded Cloudflare/Fastly config reads and provider qualification tooling.
Native environment overrides must remain within the adapter's funded profile;
the SDK owns serving and config loading. Cloudflare enables `enable_weak_ref`
for bounded-reader finalization. Native config pushes require a server restart.
Spin host config allocation evidence and actual hosted lifecycle runs remain
open; `BestEffort` egress is not confirmed delivery. HTTP/2 is not enabled by
the native server. See [the current SDK update](../plans/2026-10-07-edgezero-bounded-ingress-config-update.md)
for migration verification and per-target Q1/Q3/Q4 boundaries; the
[prior SDK delta review](../plans/2026-10-06-edgezero-shared-policy-update.md)
records evidence at its earlier pin.

The 2026-10-06 follow-up also preflights request-derived denial/status
bookkeeping before impression-ID copies and adds its independently bounded
peak to the envelope. Dropped aliases cannot fall through to compiled-name
routes; catalog eligibility shares the identity bounds. Independent runtime
transport/decoded response caps now resolve per field across host, account,
bidder and alias layers, with funded-envelope clamping, observable code-10020
warnings and an explicit missing-budget invariant. See the current corrections
for the governing formulas and scope, including the development fanout's
buffered-only enforcement. Acceptance enumeration requires the declared total
to equal the expanded named roster; local Fastly logging process regressions
run in CI. None of these changes upgrades hosted-provider guarantees.

The additional 2026-10-06 review contract validates returned `impid`s against
the observable bidder's participating impressions and, when nonempty, the
originating call's metadata. Empty metadata falls back to bidder participation.
Invalid bids are dropped before targeting with one bounded code-4 error per
call, preserving mixed-response valid bids. Targeting prefixes reserve 12 bytes
inside the existing 20-byte cap. Enabled-only discovery honors effective alias
shadowing, while base-only discovery retains compiled-name semantics. Runtime
bidder override keys are bounded before value validation. Accepted aliases
named `prebid` retain their diagnostics alongside auction-wide entries at the
shared wire key, bidder entries first. Config allocation acceptance exercises
production resolution and measures peak live overlap, not only final retention.
Escaped targeting-prefix diagnostics retain their byte bound. Root map
validation does not capture the rejected maps in diagnostic parameters.
See [the review-fix checklist](../plans/2026-10-06-m3b-review-fixes.md).

The 2026-10-08 review additionally requires `PassID=false` to clear the
flattened `User.other` and `UserExt.other` maps: unclassified user fields must not carry identifiers
around the typed scrub. `PassID=true` preserves those fields. Malformed or
conflicting health settings close only health on per-request Worker/Spin boot;
installation conflicts remain errors and never change a frozen policy.
`GclStale` retains code 10019; outbound budget clamps use distinct code 10020.
Only auction-owned privacy warnings feed the deployment health counters.
Capability profiles are adapter-owned static declarations, not runtime
enforcement or a required boot registry. Response metadata selects decoding;
the resolved envelope supplies numeric limits. The authoritative native budget
proof reads the shipped adapter row, not a copied runtime fixture. Provider
integration tests must link their adapter and exercise its timer/cooperator;
fake-transport tests and WASM compilation do not certify hosted delivery,
transport cancellation, or provider memory bounds.

The final review corrections additionally enforce the 64-byte identity limit
on both explicit impression bidder maps before copying parameters or resolving
names, without restricting arbitrary legacy extension keys. Matched non-body
handlers drain under the shared 4096-byte limit and ingress read deadline
before configuration work. Borrowed bidder responses expose body comparison,
not extractable raw bytes or rebudgeting. Object FPD patches normalize absent,
null or scalar targets before applying nested null deletions. Cloudflare's
response handoff funds the source Rust buffer, Rust copy and JavaScript copy.
See [the final repair and qualification tasks](../plans/2026-10-06-m3b-final-review-fixes.md)
for verification evidence and separate Auctioneer/EdgeZero owners. Hosted tests
and calibrated deployment limits remain explicit selected-target release
gates, not claims implied by local compilation. Credentialed live execution
and selected actual deployment Q2-D now belong to M7.6, not M6 completion.

**Status:** Design — rev 81 body; historical M3b sections current through the
round-173 review (2026-09-18). Self-review 2026-09-18: the "round-75"
stamp was ~98 rounds stale and read as if the M3b contracts here had
never been revisited; they have, repeatedly. The M3b-owned sections
(§5.2 fan-out, §5.3 limits, the capability/prerequisite boundaries)
are the ones this stamp now covers — non-M3b sections remain at rev 81
and are NOT implied current by it.
**M6 contract correction (2026-10-05):** sections 5.6, 9.1, the related
config/debug/precedence acceptance rows, and the M6 milestone definition
now defer to the [M6 hardening design](2026-10-05-m6-parity-hardening-design.md).
That document defines new work, not a claim that M6 debug has shipped.
**User-approved execution scope correction (2026-10-15):** M6 retains offline
provider harness/artifact handoff/refusal tests and runbooks, complete native
Q2-A/B/C, reusable Q2-E and all debug/config work. Credentialed real
Cloudflare/Fastly/Spin handler execution, authorized isolated corrupt-config
injection/recovery, selected actual deployment Q2-D and the live limited-bidder
pilot move to [M7.6](../../roadmap.md#8-limited-bidder-publisher-pilot-and-hosted-acceptance),
after M7.5 plus the selected publisher bidder migration wave/host setup, not
full M7 catalog closure or video parity. M6 is local/staging acceptance only;
selected-target safety/privacy/credential/creative-rendering requirements
remain mandatory before real traffic. Historical milestone references below
do not override this split or establish execution evidence.
**User-approved bidder scope correction (2026-10-26):** the
[roadmap correction](../../roadmap.md#2026-10-26-scope-correction) assigns
foundations and full selected-flow acceptance for all 14 priority bidders,
including Sharethrough/Rubicon/APS, to M7. All priority-local branch and
schema/resource requirements remain. M20 retains terminal closure gates;
M7 completion does not await them. The
[2026-10-27 sequencing correction](../../roadmap.md#2026-10-27-post-pilot-bidder-expansion)
adds M7.7 demand-led bidder expansion after selected M7.5/M7.6 Trusted
Server/publisher testing and before M8 onward, with residual migration under
M20. This changes ownership, not behavior/security, the complete source inventory or default/full
bidder-checker semantics, and claims no new implementation or qualification.
Historical reviews, appendices and evidence retain their original scope and
do not override this correction.
**Date:** 2026-05-21
**Scope:** Initial core-auction and privacy milestones; later product scope is assigned in the approved roadmap

> **Execution status — M1/M2 done; M3a MERGED (PR #10); M3b IN
> PROGRESS on `m3b-fanout` (M3b-9, M3b-1a and the round-145 EdgeZero
> pin synchronization to `120dd2aa` landed — round-166 #7: that is the
> PRIOR reference-state sync, NOT task M3b-11, which is still pending
> and will move the pin to the PR head; the repository remains pinned
> to `120dd2aa`; remaining upstream gates are raw-ingress
> head limits / framing validation and response-egress completion —
> round-144 #8 refresh of the round-22 M5 note).** M1 (foundations) and M2 (auction
> core) shipped. M3 was split into M3a + M3b in round-20. **M3a is
> MERGED (PR #10)** — privacy resolver, config overlays, Axum
> production fanout, DevFanout conformance across all four adapters;
> the round-157 #6 correction retires the stale "current in-review"
> wording that contradicted the MERGED status two lines above. M3b
> tracks multi-adapter production fanout, fail-loud config, and
> bounded body cap; its OUTBOUND prerequisites are LANDED at the
> pinned `120dd2aa` (see the M3b plan's prerequisite checklist —
> the raw-ingress items remain ABSENT, and the response-egress items
> are IMPLEMENTED-BUT-NOT-CERTIFIABLE — BestEffort on all four
> adapters at head `4790a140`, which is not a certifiable guarantee,
> so Prereq 9 stays open; self-review 2026-09-18 corrects the earlier
> "absent" for those rows). The historical M2
> platform-readiness prerequisites from earlier drafts (Cloudflare
>
> - Spin wrapper adapter conformance) are RESOLVED against the
>   pinned EdgeZero — verified at `9e661ae`, re-verified at the current
>   pin `docs/outbound-http-spec` @ `120dd2aa` (round-142: the atomic
>   pin move landed the outbound client, typed reasons, store-extraction
>   taxonomy, string-class ingress and app clock in code;
>   `fcefdde3` / `12965da2`-era verification notes below are historical
>   records of those snapshots);
>   `cargo check --target
wasm32-unknown-unknown -p auctioneer-adapter-cloudflare` and the
>   Spin equivalent pass. (Round-66 M1 — the earlier "Historical
>   (pre-round-22 M5)" prerequisites block that claimed the pinned
>   `run_app` takes `manifest_src: &str` and that the CF/Spin wrappers
>   do not compile was FALSE against `9e661ae` and has been removed; the
>   current API takes no such argument and both wrappers build. See §14
>   for the authoritative wrapper status.)
>
> **Target-readiness matrix** (round-70 M6 / round-66 M1 — single
> source of truth across the spec; resolves the §13 / §14 /
> top-of-spec inconsistencies):
>
> | Target     | Wrapper-ready                        | Dev-ready (M2 dev)    | Behavior-complete (M2 release)                                                                                                                                                               | Production-complete (M2 prod)                                                                                                          |
> | ---------- | ------------------------------------ | --------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
> | Axum       | ✓ today                              | ✓ post-dev-primitives | ✓ outbound client + app clock landed at `120dd2aa` (M3b-2b consumes them)                                                                                                                    | ✗ until raw-ingress head limits / framing validation and response-egress completion exist (Unsupported on every adapter at `120dd2aa`) |
> | Cloudflare | ✓ today                              | ✓ post-dev-primitives | ✓ same (round-143 #7)                                                                                                                                                                        | ✗ same                                                                                                                                 |
> | Fastly     | ✓ today                              | ✓ post-dev-primitives | ✓ (round-143 #7) — batch dispatch-before-harvest landed at `120dd2aa`; outbound rows are BestEffort by design (observation-time elapsed, documented deviations) per the M3b capability model | ✗ — blocked on the same raw-ingress + response-egress rows as every target                                                             |
> | Spin       | ✓ today (`unsafe_code` scope landed) | ✓ post-dev-primitives | ✓ same, outbound-deadlines BestEffort                                                                                                                                                        | ✗ same                                                                                                                                 |
>
> Round-66 M1: all four wrappers build against the pinned `9e661ae`
> today (no `manifest_src` fix is needed — the API loads the manifest
> internally; Spin's `#[http_service]` `unsafe_code` allow-scope is
> already in place). Fastly (round-144 #8 refresh): the serialized
> `ProxyClient::send` gap is CLOSED — EdgeZero's batch API
> dispatches every slot before harvesting (BestEffort by design:
> serial harvest, observation-time elapsed); behavior-complete on
> Fastly rides that contract, documented in the M3b capability
> model.
>
> **M2 _development_ on Axum, Cloudflare, Fastly (serialized fan-out per round-72 L5), and Spin can proceed**
> (post-wrapper-fix) using four development-only impls
> distributed across `auctioneer-core` and `auctioneer-adapter-cloudflare`
> (round-34 finding M2 + round-35 finding M1 + round-62 L2 — corrects
> the earlier "all four in `auctioneer-core`" claim; the Cloudflare shims
> live in `auctioneer-adapter-cloudflare` because they need `worker::*`,
> which the library crates and `auctioneer-core` cannot depend on):
>
> 1. **Interim `BidderFanout`** over `ProxyClient` + `join_all` (§5.2,
>    in `auctioneer-core`).
> 2. **Native dev `Clock`** for Axum/wasi targets backed by
>    `std::time::SystemTime::now()` + `std::time::Instant::now()` (in
>    `auctioneer-core`).
> 3. **`worker::Date`-backed dev `Clock` shim for Cloudflare Workers**
>    (wall = `Date::now()`, monotonic = wall-clock delta — sufficient
>    for M2 dev though not strictly monotonic; in
>    `auctioneer-adapter-cloudflare` per §1.2 + §14 prereq block).
> 4. **Dev `Rng`** — **target-specific** seed source per §14 (Axum/Spin
>    seed from `SystemTime` in `auctioneer-core`; Cloudflare seeds from
>    `worker::Date::now()` XOR `Math.random()` in
>    `auctioneer-adapter-cloudflare` because Workers has no
>    `std::time::SystemTime`; round-61 M1 + round-62 L2). Earlier
>    "seeded from `std::time` entropy" was Cloudflare-incompatible.
>
> With these four, plus the **M1.5 adapter wrapper conformance task**
> (round-69 M2 — Cloudflare + Spin `manifest_src` fixes + Spin
> `unsafe_code` lint scope; see §14 M1.5), all of Axum, Cloudflare,
> Fastly (serialized fan-out per round-72 L5), and Spin actually run
> M2 in development (round-73 L4 — Fastly added; matches the
> target-readiness matrix above and §14 M2-dev prereq line). M2 **production / behavior-complete release** is blocked
> until EdgeZero publishes the native primitives — under the dev impls
> Fastly fan-out runs serialized, the Cloudflare `Clock` shim is
> wall-clock-derived (not truly monotonic), and the `BidderFanout`
> conformance suite (§11.2) does not yet pass. Subsequent milestones
> build on M2 and inherit the same caveat (round-33 finding M3).

---

## 1. Purpose & Context

Implement Auctioneer as an OpenRTB-native auction server built on the
**EdgeZero** framework, so a single core codebase deploys unchanged to native
(Axum) and WASM edge runtimes (Cloudflare Workers, Fastly Compute, Spin).

This spec defines the initial core-auction and privacy scope:

1. **Core auction** — `/openrtb2/auction`, a bidder-adapter framework, four
   reference adapters, `/status`, `/info/bidders`.
2. **Privacy (full-fidelity)** — GDPR/TCF v2, CCPA/US-Privacy, GPP, the
   activity-control framework, and `/cookie_sync` + `/setuid` + `/getuids`,
   all behavior-compatible with the pinned reference server baseline.

Later phases (out of scope, see §13): stored requests; AMP/video/native
endpoints; price floors; bid adjustments; currency conversion; creative caching;
hooks/modules; analytics; full metrics; the remaining bidder catalog. The
[approved roadmap's 2026-10-26 scope correction](../../roadmap.md#2026-10-26-scope-correction)
assigns this later scope through M20: M7 owns foundations and all 14 priority
flows. The [2026-10-27 correction](../../roadmap.md#2026-10-27-post-pilot-bidder-expansion)
places additional bidder waves in M7.7 after M7.5/M7.6 Trusted Server/publisher
testing; M20 retains residual migration and full-catalog closure.

### 1.1 Reference baseline

All behavior-baseline statements are pinned to a fixed source snapshot.
The reference defines observable behavior, not an implementation language or
source layout. Auctioneer's documented types, processing order, limits, and policy
invariants are the contract; reference filenames, functions, and line numbers
are not implementation requirements. Source attribution and fixture provenance
remain explicit.

- **Behavioral reference** — [Prebid Server](https://github.com/prebid/prebid-server/tree/d7f8515b86258688304b0d9b6668c6a0e258bc9e),
  by Prebid.org and its contributors,
  git commit **`d7f8515b86258688304b0d9b6668c6a0e258bc9e`** (2026-04-16).
  Reference areas are auction validation and responses, bidder request/response
  processing, targeting and no-bid diagnostics, privacy enforcement and data
  projection, identity synchronization and cookies, and account configuration.
  Shared credits and license notices are in the
  [project acknowledgements](../../../README.md#acknowledgements) and
  [NOTICE](../../../NOTICE).
- **EdgeZero**: [PR #275](https://github.com/IABTechLab/edgezero/pull/275),
  revision `28038cce26a499499fd4e050387d4d02e35fa7c7`, observed on
  2026-10-12. All six workspace dependencies use the same explicit `rev`,
  and `Cargo.lock` resolves the transitive adapter and macro packages to
  that commit too. A remote branch does not float these dependencies.
  This revision contains the portable outbound HTTP, ingress and response
  lifecycle contracts, plus deployment-time Fastly logical store binding.
  Fastly opens the baked logical aliases for both health and request
  dispatch, and resolves logging through `Hooks::logging_for("fastly")`;
  the removed runtime-environment Config Store is not part of the current
  integration. Auctioneer still installs its logging sink before validating
  the boot envelope.
  Historical plans and evidence describe their recorded snapshots, not
  current API availability. Reference EdgeZero symbols by name, not
  dependency-checkout line numbers. Pin changes require fresh native and
  provider-WASM verification; compilation is not hosted runtime evidence.

When either pin is bumped, this section is updated and affected behavior
re-verified.

### 1.2 Design principles

- **EdgeZero integration is confined to `auctioneer-core` and the four
  `auctioneer-adapter-*` crates** (round-43 M5 — earlier "EdgeZero in
  auctioneer-core only / adapter crates unchanged" wording contradicted
  the rev 48 plan for a `worker::Date` dev clock shim in
  `auctioneer-adapter-cloudflare`). Every **library** crate
  (`auctioneer-openrtb`, `auctioneer-config`, `auctioneer-runtime`,
  `auctioneer-privacy`, `auctioneer-bidder`, `auctioneer-exchange`) is **pure** —
  no `edgezero-*`, no `tokio`/`reqwest`/`fastly`/`worker`/`spin-sdk`.
  The library crates reach the host environment only through small
  traits (`BidderFanout`, `Clock`, `Rng`) whose production
  implementations live in `auctioneer-core`. The four `auctioneer-adapter-*`
  crates are allowed two narrow capabilities: (a) target-specific
  EdgeZero wiring (the framework's adapter contract — that's what
  these crates are _for_), and (b) **target-specific host-primitive
  shims** when the EdgeZero target lacks a native impl — concretely,
  the dev `Clock` shim for Cloudflare Workers in
  `auctioneer-adapter-cloudflare` (top-of-spec, M2 development). When
  EdgeZero ships native `Clock`/`Rng`/fan-out, those shims go away
  and the adapter crates return to "wiring only."
- **Single-threaded async — precise statement.** **Portable core
  futures are `?Send`** — every `async fn` body inside `auctioneer-openrtb`
  / `auctioneer-config` / `auctioneer-runtime` / `auctioneer-privacy` /
  `auctioneer-bidder` / `auctioneer-exchange` / `auctioneer-core` is `?Send`
  (cooperative concurrency at the edge runtimes, no threads).
  **Adapter services may be `Send` where the host requires it**
  (round-67 L5 — earlier "no threads" was too broad for Axum): the
  Axum target wraps auctioneer's `?Send` `Future` in Tower's `Send`-bounded
  service interface and Tokio's multi-threaded runtime; the adapter
  crate handles the `Send` boundary (typically via
  `LocalSet`-equivalent or by treating the per-request future as
  `Send` since its captured state is `Send`). Cloudflare, Fastly, and
  Spin runtimes are inherently single-threaded per request and need
  no such adapter. **Trait objects shared across the auction** carry
  `Send + Sync` (`edgezero_core::proxy::ProxyClient` is declared
  `Send + Sync`; the bidder registry, `Box<dyn Bidder>`, and the
  host-primitive traits are all `Send + Sync`). _Portable core
  futures_ are `?Send`; _shared services_ are `Send + Sync`;
  _adapter services_ may add a `Send` requirement on the future the
  service yields.
- **No runtime filesystem.** All configuration ships as embedded
  defaults at build time (§6) plus optional runtime `config_store`
  overrides for host/account (§9.1, round-43 L6). No file I/O at
  request time.
- **No ambient time/entropy.** Wall time and randomness are taken only
  through the `Clock` / `Rng` traits (§5.7) — never `SystemTime::now()` or an
  ambient RNG inside library crates.
- **Security by construction.** Outbound requests target only config-derived
  endpoints/hosts (§5.4, §8.6); request bodies are size-bounded (§5.3); debug
  output is redacted (§5.6).
- **Deterministic output.** Bidder processing order and response
  serialization are deterministic (Appendix E.7) so snapshot tests are
  stable.
- **Tested at two layers — both required.** Every module ships **unit
  tests** (fast, no I/O, per-function); cross-crate and runtime-relevant
  behavior ships **integration tests** (including end-to-end against
  Mocktioneer). TDD: tests are written before the implementation they
  cover, and a milestone (§14) is "done" only when both layers (§11.1,
  §11.2) are green — neither layer optional.

---

## 2. Architecture & Crate Layout

```
auctioneer (workspace)
├── crates/auctioneer-openrtb      OpenRTB 2.6 + auction extension types (serde).
├── crates/auctioneer-gvl-schema   GVL manifest + vendor-list validators (round-43 B4; no build.rs, no auctioneer deps).
├── crates/auctioneer-config       Config types + build.rs codegen (DATA ONLY).
├── crates/auctioneer-runtime      Host-environment seams: Clock, Rng, body_caps (round-65 M5).
├── crates/auctioneer-bidder       `Bidder` trait, registry, 4 reference adapters.
├── crates/auctioneer-privacy      TCF v2 / CCPA / GPP / activity gate + scrubbing.
├── crates/auctioneer-exchange     Auction engine: fan-out, collection, targeting.
├── crates/auctioneer-core         EdgeZero `app!` + handlers + primitive bindings.
└── crates/auctioneer-adapter-*    axum / cloudflare / fastly / spin — wiring-only except temporary target-specific dev shims (round-43 M5 + round-61 L2 — Cloudflare carries the `worker::Date`-backed dev `Clock` + dev `Rng` until EdgeZero ships native primitives; see §1.2).
```

### 2.1 Dependency graph (explicit "depends on", acyclic)

| Crate               | Depends on                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | EdgeZero dependency                                                                       |
| ------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| `auctioneer-openrtb`    | `serde` + **`serde_json`** (`RawValue` passthrough, §3.1; round-43 L8) + `url` (URL parsing for site default) + `publicsuffix` (eTLD+1) + `ipnetwork` (IANA IP classification for client IP sanitization). `prepare()` (§3.2) needs URL/eTLD/IP-classification capabilities — no longer "serde only" (round-29 finding H2). All deps are pure-Rust, no I/O, no runtime-specific code; the crate stays WASM-portable and EdgeZero-free.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              | none                                                                                      |
| `auctioneer-gvl-schema` | `serde` + `serde_json` + `serde_path_to_error` + `time` (round-43 B4 — NEW crate holding `validate_gvl_json` / `validate_manifest` + their DTOs. Extracted from `auctioneer-config` so the GVL refresh tool can depend on the validator WITHOUT depending on `auctioneer-config`, whose `build.rs` fails on a stale archive — the cycle that would otherwise make a broken archive unrepairable. Has **no `build.rs`** and no auctioneer deps, so it always builds.)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            | none                                                                                      |
| `auctioneer-config`     | **`auctioneer-gvl-schema`** (round-43 B4 — in BOTH `[dependencies]` and `[build-dependencies]`; a build-dep on a sibling path crate is legal and cycle-free because that sibling has no build script. Replaces the round-26 `include!("src/gvl_schema_shared.rs")` workaround), `serde` + `serde_json` (host/account settings use typed overlays, §9.1; no generic deletion-patch engine); `toml` + `brotli` + `hex` + `sha2` + `time` in `build.rs` only (round-43 B4/B9 — brotli powers the compressed-bundle gate, sha2+hex the archive digest pin, time the freshness SLA). **Declares `insecure-bidders = []` feature** (round-75 M1 — its `build.rs` reads `CARGO_FEATURE_INSECURE_BIDDERS` to gate the syncer-URL HTTPS check per §6 + §8.6; forwarded in from `auctioneer-bidder` via `insecure-bidders = ["auctioneer-config/insecure-bidders"]`).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     | none                                                                                      |
| `auctioneer-runtime`    | **`time`** (`OffsetDateTime` for the `Clock` trait return type, §5.7; round-43 L8 — `Clock::now() -> time::OffsetDateTime` was undocumented in the dep row); **`http`** (round-71 M6 — `body_caps::match_route(method, path, &RouteTable)` takes `&http::Method`; pure-Rust, WASM-portable; the only `http` types used are `Method` and possibly `StatusCode` for the ingress hook return shape, no I/O surface); **`async-trait`** (round-15 M4 — M3 adds a separate `Timer` trait next to `Clock` at `auctioneer-runtime/src/timer.rs`, using `#[async_trait(?Send)]` so `dyn Timer` is object-safe alongside the existing `dyn Clock`; `Clock` itself is untouched).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 | none                                                                                      |
| `auctioneer-privacy`    | `auctioneer-openrtb`, `auctioneer-config`, **`auctioneer-gvl-schema`** (round-43 B4 — the runtime lowering path in `Gvl::load_from_json` calls the shared validator directly rather than through `auctioneer_config::gvl_schema`), `auctioneer-runtime` (round-88 LOW / round-104 H9 — an M2 carryover; **M3 privacy still takes no Clock DEPENDENCY**, but `PrivacyContext::resolve` now RECEIVES `now: time::OffsetDateTime` as a passed VALUE (the six-arg form, round-99 B1) for the GCL authenticity checks — CMP `deleted_date <= now` and GCL expiry; round-109 H5: the GVL VENDOR `deleted_date` is ALSO parsed now — at validation in `auctioneer-gvl-schema` (RFC 3339 via the shared helper; malformed fails validation, build-gate and runtime alike) onto the runtime `Vendor.deleted_date: Option<OffsetDateTime>`; the earlier "opaque/unparsed" contract is retired. The two deletions stay DISTINCT artifacts with the same comparator: GVL-vendor deletion drives Task 9 AUTHORIZATION denial via `Vendor::is_deleted_at(now)`, GCL-CMP deletion drives Task 8 consent-USABILITY, both `deleted_date <= now`), (round-91 L6 — **no `serde` and no `serde_json` for GVL**: loading delegates to the shared `auctioneer_gvl_schema::validate_gvl_json` parse and lowers `ValidatedGvl` into plain runtime structs with no serde derives), **`lru`** + **`once_cell`** (round-40 H3 — lazy-parse `GvlStore`), **`time`** (round-88 LOW — used for the TCF **core-segment** `Created`/`LastUpdated` timestamps and manifest freshness; **round-109 H5: the GVL vendor `deleted_date` is parsed too** (the round-104 H9 "stays opaque" note is retired) — the parse lives in `auctioneer-gvl-schema`'s validator, so THIS crate consumes the already-parsed `Option<OffsetDateTime>` on the lowered `Vendor` and compares it in `is_deleted_at(now)`), **`log`** (round-91 L6 — GVL cache-load errors ONLY; the per-vendor deleted-vendor runtime WARN is REMOVED — round-90 H4 moved deleted-vendor observability to a BUILD-TIME `cargo:warning` aggregate in `build.rs`, so `log` no longer carries deleted-vendor lines). Round-43 B4: `serde_path_to_error` moves OUT of this row — it now lives on `auctioneer-gvl-schema`, which owns the validator that uses it. Pure-Rust, WASM-portable, no I/O. | none                                                                                      |
| `auctioneer-bidder`     | `auctioneer-openrtb`, `auctioneer-config`, **`serde`** + **`serde_json`** (round-65 L6 — typed `make_requests` / `make_bids` adapter params + `RawValue` passthrough; was implicit via `auctioneer-openrtb` re-export but should be a direct dep) + `url` + `percent-encoding` (for `EndpointBuilder`, §5.4 — adapter-owned URL assembly with safe macro encoding and `host_allowlist` validation; round-30 finding M3)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         | none                                                                                      |
| `auctioneer-exchange`   | `auctioneer-openrtb`, `auctioneer-config`, `auctioneer-runtime`, `auctioneer-bidder`, `auctioneer-privacy`, **`async-trait`** (round-65 L6 — the dyn-safe `BidderFanout` trait at §5.2 uses `#[async_trait(?Send)]`; was named in §2.1 prose but missing from this row), **`flate2` with `default-features = false, features = ["rust_backend"]`** (round-63 M3 — pure-Rust `miniz_oxide` backend; WASM-portable, no C/zlib dependency; needed by the outbound gzip path at §5.2 + bidder `endpoint_compression = "gzip"` per round-22 H2 / round-25 M6). Compression-level fixed at `Compression::default()` (level 6) for byte-stable snapshot tests.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 | none                                                                                      |
| `auctioneer-core`       | all of the above                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    | `edgezero-core` (`app!`, extractors, `RequestContext`, fan-out + host-primitive bindings) |

**`auctioneer-exchange` is EdgeZero-free (review findings 2, 3).** The auction
engine takes `&dyn BidderFanout`, `&dyn Clock`, `&dyn Rng` — neutral traits.
Their production implementations (which touch EdgeZero) live in `auctioneer-core`,
the single integration seam. `Clock`/`Rng` live in `auctioneer-runtime` (below
`auctioneer-exchange`), so there is no `auctioneer-core → exchange → core` cycle.
`auctioneer-exchange` does depend on the `async-trait` proc-macro crate (for the
object-safe `BidderFanout`, §5.2) — a build-time macro, not an EdgeZero or
runtime dependency, so "pure" still holds.

**No codegen cycle:** `auctioneer-config`'s `build.rs` generates **data tables
only** — never adapter instances. The bidder _registry_ is assembled in
`auctioneer-bidder`; `auctioneer-config` never depends on `auctioneer-bidder`.

### 2.2 Endpoints (registered in `edgezero.toml`)

**Phase-1 final route inventory.** All eight routes are registered in
the phase-1 build (round-41 M1: the earlier "seven" number predated the
M3a Task 4a `/_auctioneer/health/gvl` endpoint that round-40 H2 added). The
`Milestone` column tags when each handler is _reachable_ (round-45 M4 —
earlier rounds split sync ownership into M5 per §14; the route table
now mirrors that). Routes ship when their milestone ships —
earlier-milestone `edgezero.toml` excludes the not-yet-ready sync
registrations rather than registering 501-stubs.

| Method | Path                   | Handler                               | Milestone                                                                                                                                                                                                                                                                                                                                                                                                      |
| ------ | ---------------------- | ------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| POST   | `/openrtb2/auction`    | `auctioneer_core::handlers::auction`      | **M2**                                                                                                                                                                                                                                                                                                                                                                                                         |
| GET    | `/status`              | `auctioneer_core::handlers::status`       | **M2**                                                                                                                                                                                                                                                                                                                                                                                                         |
| GET    | `/info/bidders`        | `auctioneer_core::handlers::info_bidders` | **M2**                                                                                                                                                                                                                                                                                                                                                                                                         |
| GET    | `/info/bidders/{name}` | `auctioneer_core::handlers::info_bidder`  | **M2**                                                                                                                                                                                                                                                                                                                                                                                                         |
| GET    | `/_auctioneer/health/gvl`  | `auctioneer_core::handlers::health_gvl`   | **M3a** (round-40 H4 — Task 4a. Round-50 H4 auth contract: **fail-closed**. The route returns **401** unless the adapter installed a `HealthAccessPolicy` at boot; `Token` requires a matching `Authorization: Bearer <t>`; `UnauthenticatedAcknowledged` (deploy opt-in `allow_unauthenticated_health = true`) serves without a token, in which case deployers SHOULD also restrict `/_auctioneer/*` at ingress.) |
| POST   | `/cookie_sync`         | `auctioneer_core::handlers::cookie_sync`  | **M5** (round-40 H1 — 40 here is a milestone-review round number, not a review round of this document)                                                                                                                                                                                                                                                                                                         |
| GET    | `/setuid`              | `auctioneer_core::handlers::setuid`       | **M5**                                                                                                                                                                                                                                                                                                                                                                                                         |
| GET    | `/getuids`             | `auctioneer_core::handlers::getuids`      | **M5**                                                                                                                                                                                                                                                                                                                                                                                                         |

The demo handlers in the current `edgezero.toml` (`root`, `echo`, `stream`,
`headers`, `echo_json`, `proxy_demo`) are removed.

---

## 3. `auctioneer-openrtb` — OpenRTB Types

A hand-defined Rust subset of **OpenRTB 2.6 only** plus upstream extensions,
modeled on `github.com/prebid/openrtb` as used by the pinned baseline.

- **Core types:** `BidRequest`, `Imp`, `Banner`, `Video`, `Native`, `Audio`,
  `Site`, `App`, `Dooh`, `Publisher`, `Device`, `Geo`, `User`, `Eid`,
  `Regs`, `Source`, `BidResponse`, `SeatBid`, `Bid`.
- **OpenRTB 2.6 only — no upconverter; 2.5 privacy fields rejected.** auctioneer
  reads only native 2.6 fields for privacy and routing (`regs.gdpr`,
  `regs.us_privacy`, `regs.gpp`, `regs.gpp_sid`, `regs.coppa`,
  `user.consent`, `user.eids`). **`prepare()` (§3.2) rejects requests
  carrying the legacy 2.5 privacy fields** `regs.ext.gdpr`,
  `regs.ext.us_privacy`, or `user.ext.consent` — silently dropping them
  would be a privacy regression (round-13 finding H1). Other 2.5-style ext
  fields with no privacy meaning pass through opaquely via `RawValue`. This
  is an intentional divergence from the pinned baseline (§15). OpenRTB 2.6
  is the supported wire contract; legacy privacy inputs are rejected explicitly.

### 3.1 Extension representation

Two explicit categories:

- **Typed ext** — for every `ext` the server reads or writes: a typed struct
  with `#[serde(flatten)] other: serde_json::Map<String, Value>` capturing
  unknown keys so re-serialization loses nothing.
- **Passthrough ext** — `Option<Box<serde_json::value::RawValue>>` for any
  `ext` the server never inspects. Lossless, forward-only.

`RawValue` is never used for an ext the server mutates. Round-trip fidelity is
a unit-tested invariant (§11).

**Concrete typed-ext inventory** (the implementation plan refines this into a
field-by-field type list; authoritative starting set):

| Location                | Typed fields the server reads/writes                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| ----------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `BidRequest.ext.prebid` | `aliases`, `aliasgvlids`, `targeting` (`ExtRequestTargeting`), `debug`, `channel`, `nosale`, `bidderparams` (merged into imp.ext.bidder per §5.1 step 7), `returnallbidstatus` (gates `seatnonbid`, Appendix E.6). **Typed pass-through (round-trip only, no behavior in phase 1):** `multibid` (round-22 M8; reference multi-bid limits clamp + reference auction handling bid limiting deferred), `data` and `data.eidpermissions` (round-24 H2 / round-25 L8; request-level first-party-data resolution deferred), `bidderconfig` (round-24 H2 / round-25 L8; upstream bidder-FPD application deferred). The per-imp `imp.ext.prebid.imp` merge is the only FPD path that takes effect (§5.1 step 7). |
| `Imp.ext`               | `tid` (transaction id; scrubbed when `transmitTid` denied, Appendix C.2); `bidder` (top-level map name → `RawValue` params — **written** by the exchange when hoisting per §5.1 step 7); legacy `<bidder>` (top-level map at `imp.ext.<bidderName>` — **read** by the exchange; supplemented with request-level `bidderparams` per §5.1 step 7, finding M6 round-26)                                                                                                                                                                                                                                                                                                                                     |
| `Imp.ext.prebid`        | `bidder` (map name → `RawValue` params); `imp` (map bidder-name → bidder-specific FPD merged into the bidder-scoped `Imp` at §5.1 step 7 — round-21 finding M3); `storedrequest` (parsed; stored-requests are a later phase)                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `Bid.ext.prebid`        | `targeting`, `type`, `meta`, `bidid`, `dealpriority`, `dealtiersatisfied`, `targetbiddercode`; plus top-level `origbidcpm`, `origbidcur`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `Site.ext`              | `amp` (0/1; set during implicit-field fill, §5.1 step 5)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `Publisher.ext.prebid`  | `parentAccount` (consulted during account resolution, §9.1)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| `Regs` (native 2.6)     | `gdpr`, `us_privacy`, `gpp`, `gpp_sid`, `coppa`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| `Regs.ext`              | `dsa`, `gpc` (`Sec-GPC` propagation, §5.2 / round-18 M6)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `User` (native 2.6)     | `consent`, `eids`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `User.ext`              | `data`; `eids` (deleted by `scrub_gdpr_id` per Appendix C.2 — round-26 finding M6)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `Device` (native 2.6)   | `lmt`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `Device.ext`            | `atts` (iOS App Tracking Transparency; read-only — drives the iOS LMT override per §7.5 / §5.1 step 5)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| `Source` (native 2.6)   | `tid`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `Source.ext`            | `schain`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |

- **Validation:** `BidRequest::validate()` returns structured **errors only**
  (no warnings; round-33 finding M4): ≥1 imp; each imp ≥1 media type and
  unique id; **exactly one of `site` / `app` / `dooh` is present** (matches
  Upstream reference auction handling; round-18 finding H1); `tmax` sanity. **Consent-string
  well-formedness is NOT checked here and NOT in `prepare()`** (round-37
  H3 + round-38 M2): it lives in `auctioneer-privacy`'s
  `PrivacyContext::resolve` (§7.7) for the auction path and
  `resolve_sync_privacy` (§7.8) for the sync endpoints. `validate()` does
  not look at consent strings.

### 3.2 `NormalizedBidRequest` — the shared, type-enforced auction input

A newtype around `BidRequest`, constructed **only** by one shared helper, so
every caller — EdgeZero handler, trusted-server caller, tests — runs the same
validation/normalization and `run_auction` (§5.8) cannot be called with a
non-normalized request:

```rust
/// All HTTP-header signals implicit fill needs. Trusted-server callers
/// without an HTTP request set every field to None (round-28 finding H1).
pub struct RequestHeaders<'a> {
    pub referer:          Option<&'a str>,
    pub user_agent:       Option<&'a str>,
    pub dnt:              Option<&'a str>,
    pub sec_gpc:          Option<&'a str>,
    pub true_client_ip:   Option<&'a str>,
    pub x_forwarded_for:  Option<&'a str>,
    pub x_real_ip:        Option<&'a str>,
    pub remote_addr:      Option<&'a str>,
}

pub struct NormalizedBidRequest {
    inner:    BidRequest,                    // private; consumed by prepare()
    warnings: Vec<NonFatalWarning>,          // round-26 M5 + round-37 H3 —
                                             // non-fatal warnings raised
                                             // during prepare ONLY (e.g. a
                                             // future implicit-fill warning).
                                             // Consent-syntax warnings are
                                             // emitted by PrivacyContext
                                             // (§7.7), NOT by prepare.
                                             // Currently empty in M1; the
                                             // field is kept so the engine's
                                             // surfacing path (below) is
                                             // already wired when prepare-
                                             // time warnings are added.
}
impl std::ops::Deref for NormalizedBidRequest { /* -> &BidRequest */ }
impl NormalizedBidRequest {
    pub fn warnings(&self) -> &[NonFatalWarning] { &self.warnings }
}

/// Single reusable preparation gate (round-28 finding H1). Performs
/// implicit-field fill AND normalization, so a trusted-server caller cannot
/// skip implicit fill: there is no callable type-safe path that produces a
/// NormalizedBidRequest without going through this function.
pub fn prepare(
    req: BidRequest,
    headers: &RequestHeaders<'_>,
    limits: &PrepareLimits,   // round-125 #5 (M3b): carries
                              // max_impressions_per_auction from the
                              // boot envelope; over-limit requests are
                              // rejected as BadInput before per-imp
                              // work. M3a call sites migrate in
                              // M3b-2's mechanical pass; until then
                              // the M3a two-argument form is the
                              // shipped shape.
) -> Result<NormalizedBidRequest, ValidationError>;
```

`struct PrepareLimits { pub max_impressions: u16 }` (round-126 #9 —
the M3b envelope-derived input; `ValidationError` gains
`TooManyImpressions { actual, limit }`, wire body
`Invalid request: too many impressions (<n> > <limit>)\n`).

`prepare` performs **in this exact order** (round-27 H1 + round-28 H1 —
implicit fill + normalize are one type-safe gate; no path produces a
`NormalizedBidRequest` without running these; round-126 #9 inserts
step 0: **imp-count check against `limits.max_impressions` BEFORE any
per-imp work** — it precedes implicit fill, so an over-limit request
does zero per-imp allocation):

1. **Implicit-field fill** from `headers` (matches reference auction handling;
   round-17 H1 + round-18 M5 + round-20 M5/M6 + round-22 H1; **only fill
   when empty, never overwrite client-supplied values, except for the IP
   sanitization sub-rule below**):
   - **Site default** — when `app == nil && dooh == nil`: **always
     materialize a `site` object** (even when `headers.referer` is
     missing — matches reference auction handling;
     round-29 finding M5). Populate each of `site.page`, `site.domain`,
     `site.publisher.domain` from `headers.referer` **only if both the
     header is non-empty AND the corresponding field is currently empty**;
     `site.ext.amp = 0` is set whenever absent. When `referer` is missing,
     auctioneer falls back to the existing `site.page` (if the client supplied
     one) to populate `site.domain` (= parsed URL `Host`, full host, not
     eTLD-stripped) and `site.publisher.domain` (= eTLD+1). The
     materialized empty `site` is enough to pass validation; downstream
     enrichment (page/domain/publisher.domain) is best-effort.
   - **Client IP** (round-34 finding M5 — exact rules, matches upstream
     reference HTTP context extraction):
     1. **Sanitize first**: if `device.ip` / `device.ipv6` is non-empty
        but unparseable / in IANA private/reserved/loopback/link-local
        space, **clear it**.
     2. **Fill chain**, in priority order. The first parsed _valid
        public IP_ wins; later sources are not consulted. Empty headers
        and parse failures are silently skipped (no warning). For each
        candidate, ASCII-trim, strip ports, then parse with
        `IpAddr::from_str`:
        a. `true_client_ip` — single value.
        b. `x_forwarded_for` — split on `,`, walk **left-to-right** (the
        client / first-hop end is first; trim whitespace each hop;
        IPv4 strip `:port` suffix; IPv6 strip bracket+port form
        `[::1]:port` → `::1`).
        c. `x_real_ip` — single value, same strip rules.
        d. `remote_addr` — transport-layer value, typically `host:port`
        for IPv4 and `[v6]:port` for IPv6; same strip rules.
     3. **Public-IP requirement** is the same predicate as the sanitize
        step: parseable + not in IANA private/reserved/loopback/
        link-local space. Route a chosen IPv6 to `device.ipv6` and an
        IPv4 to `device.ip`.
        If no candidate passes, `device.ip` / `device.ipv6` stay `None` —
        auctioneer does not invent a value.
   - `device.ua` from `user_agent` (only if empty); `device.dnt` from
     `dnt` (only if absent).
   - **`regs.ext.gpc`** — **`sec_gpc == "1"` _always overwrites_ this
     field to `"1"`** (round-31 finding H1: a privacy-protective signal
     from the header must not be silently suppressed by a conflicting
     body value like `regs.ext.gpc = "0"`). Other header values (or an
     absent header) do not touch the field; a client-supplied JSON
     `regs.ext.gpc = "1"` is left as-is. Outbound propagation (§5.2)
     fires iff the resulting `regs.ext.gpc == "1"` — so a `Sec-GPC: 1`
     header reliably propagates, regardless of the body value.
   - **`at` defaults to `1`** (first-price) when **absent OR `0`**
     (round-31 finding M3 — matches reference auction handling, which treats
     `AT == 0` as unset). The default is a hard-coded constant.
   - **iOS LMT override** — `device.lmt` set per §7.5 / iOS 14+ ATT
     based on `device.ext.atts` already in the request (read-only).
   - **`ext.prebid.channel` default** — when `app` is set and
     `ext.prebid.channel` is absent, fill `channel.name = "app"`.
2. `BidRequest::validate()` — structural validation (≥1 imp, media types,
   **exactly one of `site` / `app` / `dooh`** per §3 round-18 finding H1,
   `tmax` sanity).
3. **2.6-only privacy gate** (§3): reject the request with a
   `ValidationError` if it carries `regs.ext.gdpr`, `regs.ext.us_privacy`,
   or `user.ext.consent`.
4. **`ext.prebid.channel` validation**: if `ext.prebid.channel` is
   _present_ but `channel.name` is explicitly empty (`""`), reject with
   `ValidationError` (`400 Invalid request: channel must have a name`).
   (The _absent → fill with "app"_ default ran in step 1 above; rejection
   here only fires when the client explicitly sent `""`.)
5. **No `cur` defaulting** — `prepare` does **not** default empty `cur`
   to `["USD"]`; matches upstream's late defaulting (§5.1 step 10 applies
   the USD assumption when collecting responses).
6. **No consent-string parsing in `prepare()`** (round-37 H3). TCF v2,
   US-Privacy, and GPP string well-formedness checks are **NOT** performed
   here — they live in `auctioneer-privacy` (§7), which already owns the full
   semantic decoders. Putting them here would force `auctioneer-openrtb` to
   depend on consent parsers and create a cycle (`auctioneer-privacy` depends
   on `auctioneer-openrtb`, §2.1). Non-fatal consent warnings are emitted
   inside `PrivacyContext::resolve` during §5.1 step 6 (see §7.7).

**Where warnings surface.** Inside `run_auction`, when constructing the
`BidResponse` object (between §5.1 step 10 collect and step 12 caller
serialization — i.e. during the engine-domain response build),
`NormalizedBidRequest.warnings()` is **iterated and each entry cloned**
(the borrow is `&NormalizedBidRequest`, so the slice cannot be drained
in place — round-37 H2) onto `ext.warnings.prebid[]` of the returned
`BidResponse`, matching upstream's auction-wide diagnostic placement placement
(Appendix E.5). `prepare` itself never writes the response; serialization
of that response into bytes is the caller's step 12 (round-31 finding
L5 + round-46 L6 — step number corrected after the round-43 H1 host
insertion).

**Alias resolution is _not_ performed here** (round-13 finding H3) —
`auctioneer-openrtb` knows nothing about the bidder registry (§2.1).
Round-106 H8 (aligning this paragraph with the round-37 B1 ownership
move that §7.6 already records): aliases are resolved by the
**HANDLER**, before `AuctionContext` construction, and carried on
`AuctionPlan` — the handler order is wire-signal fatal gate →
`prepare` → `resolve_aliases(&normalized, registry)` →
`AuctionPlan::new_m3a(...)` → context build → `run_auction(&req,
&ctx, &plan, &mut audit)`, and `run_auction` reads `plan.aliases` rather than
re-resolving (M3a Task 13 Steps 1a/2; the ordering is pinned by the
Task 13 handler-order test). The pre-round-37 text placing
resolution inside `run_auction` step 7 is retired.

Privacy and adapter behavior cannot diverge by caller — a single
normalization point is the type-system contract.

---

## 4. `auctioneer-bidder` — Adapter Framework

### 4.1 The `Bidder` trait

Pure and synchronous — no I/O, no async, no EdgeZero types. **The
implemented trait uses the enforcing sink and retained metadata
(round-106 B6 / round-202 #2):**

```rust
pub trait Bidder: Send + Sync {
    /// Requests are pushed into the sink — the ONLY constructor of
    /// `BidderHttpRequest` (fields pub(crate) + read-only accessors).
    /// The sink enforces max_outbound_raw_bytes via `CappedWriter`,
    /// OUTBOUND_META_BYTES via the byte pool-backed meta builder, and the
    /// AUCTION-GLOBAL call budget via a shared `CallBudget` consumed
    /// in §5.9 registry order then per-bidder request order
    /// (deterministic exhaustion; a budget-exhausted bidder stays
    /// live with the calls that fit + one code-10999 warning per the
    /// §9.1 max_outbound_requests contract — N = rejected attempts,
    /// one unified message string). The sink's terminal API is
    /// `into_outcome()` / `into_outcome_discarding_accepted()`, both
    /// returning the complete `SinkOutcome` (round-108 B1). The Result channel stays
    /// SEMANTIC: business failures return Err exactly as before, and
    /// a semantic Err discards that bidder's accepted requests while
    /// the sink's typed build errors still report per-bidder.
    fn make_requests(&self, request: &BidRequest, info: &BidderRequestInfo,
        sink: &mut OutboundRequestSink<'_>) -> Result<(), Vec<BidderError>>;
    fn make_bids(&self, request: &BidRequest, sent: &OutboundRequestMeta,
        response: &BidderHttpResponse) -> Result<BidderResponse, Vec<BidderError>>;
}
```

The consuming `finalize_for_transport` runs once before `BidderCall`
construction. It returns retained metadata alongside the typed per-slot
result, including rejected encoded slots. Only after the final reserved
header appends does it share the immutable metadata pool via `Arc`.
Transport conversion consumes `into_parts()` and moves the body `Vec`
into `Body::from_bytes`; there is no retained or cloned request body.
`make_bids` borrows the retained method, URL, headers and covered impression
IDs. Timeout and encoded-failure `seatnonbid` use that same metadata.

The exchange owns the network call between the two methods. Every adapter is
unit-tested with fixture request/response pairs — no async, no mocking (the
sink is a plain struct; tests construct one directly).

- `BidderHttpRequest` — private pooled metadata and body, with read-only
  accessors. Only the sink constructs it. The exchange chooses GPC and
  compression for the single bounded finalizer; `BidderCall` retains only
  timeout and response-size policy, not an in-drive compression instruction.
- `BidderHttpResponse` — `status`, `headers`, `body: Vec<u8>`. Plain struct.
- `BidderResponse` — `Vec<TypedBid>` (bid + classified media type), `currency`,
  and `errors: Vec<BidderError>` for bid-local failures. Classification consumes
  parsed bids by value and keeps valid siblings across seats, returning one error
  for each rejected bid. If every bid is rejected, return those errors in `Err`;
  whole-body parse or protocol failures also remain fatal. The exchange folds
  both error channels into the observable bidder's `ext.errors` without dropping
  eligible siblings. All adapters share standard `bid.mtype` and named-type
  classification against the matching post-FPD, privacy-scrubbed impression;
  reviewed legacy hints must agree with standard hints. Construction and response
  decoding receive the same bidder context and host `tmax`. M3b reconstructs one
  context at a time during response processing within its sequential working-copy
  allowance, rather than retaining a full request for each bidder through fanout.
  Adapter-specific capability exclusions remain local.
- **`BidderRequestInfo`** (round-27 finding M3) — context passed to
  `make_requests` so an adapter has stable, typed access to non-`BidRequest`
  fields it might need without reaching for global state. Phase-1 contract:
  ```rust
  pub struct BidderRequestInfo<'a> {
      pub bidder:          BidderName,        // alias or base, as resolved
      pub core_name:       BidderName,        // base bidder name
      pub account_id:      &'a str,           // ResolvedAccount.id (§5.8) —
                                              //   drives {{.AccountID}} macros
                                              //   (round-29 finding H3)
      pub endpoint_template: &'a str,         // BidderInfo.endpoint as the
                                              //   base TEMPLATE (round-30 L5);
                                              //   the adapter constructs the
                                              //   final URL via EndpointBuilder
                                              //   (§5.4) — this is not the
                                              //   final outbound URL.
      pub endpoint_macros: &'a [&'a str],     // macro names permitted by this
                                              //   bidder's endpoint template
                                              //   (round-29 H3)
      pub host_allowlist:  Option<&'a [&'a str]>,
                                              // for bidders with host-varying
                                              //   endpoints (§5.4); None when
                                              //   the endpoint host is fixed
                                              //   (round-29 H3)
      pub gpc:             Option<&'a str>,   // regs.ext.gpc value if set
      pub entry_point:     EntryPoint,        // Auction | (later) Amp/Video
      pub currency:        &'a [String],      // request cur (unchanged here)
  }
  pub enum EntryPoint { Auction }             // Amp/Video/Native added in
                                              // their later phases (§13)
  ```
  Fields intentionally **omitted in phase 1** but reserved for later:
  currency-conversion rates, preferred-media hints, tmax-adjustments. The
  struct is `#[non_exhaustive]` so adding fields later is non-breaking.

### 4.2 Registry

- `BidderName` — interned, alias-aware bidder identifier.
- `BidderRegistry` — `name -> (Box<dyn Bidder>, &'static BidderInfo)`,
  assembled in `auctioneer-bidder` by pairing `auctioneer-config` codegen data (§6)
  with adapter constructors.
- `BidderInfo` — `endpoint`, `endpoint_macros`, `host_allowlist`,
  `endpoint_compression` (per round-22 finding H2; `"gzip"` or unset —
  Pubmatic's `bidder-info` sets `endpointCompression: gzip`), `gvl_vendor_id`,
  `capabilities`, `maintainer`, `syncer` config, `openrtb.gpp_supported`,
  `debug.allow` (bidder-level debug gate; round-24 finding M5 — when
  `false`, bidder HTTP debug entries are suppressed even when account
  `debug_allow` is true, and a `BidderLevelDebugDisabled` warning (code
  **10003**) is recorded in `ext.warnings[bidder]`; matches upstream
  reference bidder request/response processing), and `disabled` (bidder enablement, from the bidder-info
  TOML). `disabled` is **build-time data, not runtime-overridable** (§9.1).
  When `endpoint_compression == "gzip"`, the exchange gzip-encodes outbound
  bodies and sets `Content-Encoding: gzip` (matches reference bidder request/response processing).

**Disabled-bidder + alias semantics** (round-24 M3 + round-25 M3/M4;
matches reference request validation / reference auction handling):

- A bidder named in `imp.ext.prebid.bidder.<name>` whose `BidderInfo.
disabled == true` is **removed from that imp with a warning**
  `BidderTemporarilyDisabled` (numeric code 6, but **severity = warning**,
  not fatal — round-25 finding M3; appears in `ext.warnings[<bidder>]`).
- If an imp ends up with **zero remaining bidders** after disabled-bidder
  removal, the request is **rejected** with `AuctionError::BadInput(...)`
  → `400 Invalid request: imp <impid> has no valid bidders`.
- **Alias validation** (registry-dependent, round-25 finding M4 —
  rejected at §5.1 step 7 with `AuctionError::BadInput(...)` → `400`):
  - **Unknown base bidder** in `ext.prebid.aliases` → `Invalid request:
alias <alias> targets unknown bidder <base>`.
  - **Base bidder disabled** → `Invalid request: alias <alias> targets
disabled bidder <base>`.
  - **No-op alias** (alias name equals base name) → `Invalid request:
alias <alias> is a no-op`.
  - **`aliasgvlids` entry for an unknown alias** → `Invalid request:
aliasgvlids names unknown alias <alias>`.
  - **`aliasgvlids[<alias>] < 1`** → `Invalid request: aliasgvlids
value for <alias> must be >= 1`.
- **Params validation:** each adapter defines a typed serde `Params` struct;
  deserialization of `imp.ext.prebid.bidder.{name}` _is_ the validation.
  Embedding `static/bidder-params/*.json` schemas is deferred (open item 6).

### 4.3 Four reference adapters

| Adapter             | Demonstrates                                                                                                                                                                                  |
| ------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Generic OpenRTB** | Near-verbatim OpenRTB transform. "Generic" is the _transform_, not the destination — each bidder using it still has a build-time-fixed `BidderInfo.endpoint` (§5.4); no runtime-supplied URL. |
| **AppNexus**        | Custom request transform — placement-id params, request-level customization.                                                                                                                  |
| **Pubmatic**        | Custom response transform — publisher-level params, response media-type inference.                                                                                                            |
| **Mocktioneer**     | Thin adapter targeting the local Mocktioneer bidder. Deterministic pricing → snapshot E2E backend (§11).                                                                                      |

AppNexus/Pubmatic are representative exemplars, swappable at planning time.

---

## 5. `auctioneer-exchange` — Auction Engine

### 5.1 `POST /openrtb2/auction` flow

The `auction` handler in `auctioneer-core` receives a `RequestContext`, constructs
the host-primitive implementations, **resolves host config**, **resolves
the account**, **calls `auctioneer_openrtb::prepare(req, headers, &limits)`** (a
single gate covering implicit fill + validate + normalize, §3.2;
`limits` from the boot envelope — round-126 #9; the M3a-shipped
two-argument form migrates in M3b-2's mechanical pass), and
delegates to **`auctioneer_exchange::run_auction`** (§5.8) — the same
reusable library function any trusted-server caller invokes. **Steps
6–11 below execute inside `run_auction`; steps 1–5** (capture, body
read, parse, host resolution, account resolution, prepare) **and step
12** (serialize) are the caller's responsibility (round-44 M1 —
renumbered after round-43 H1 inserted host as step 3). The `prepare()`
gate is the single point where a `NormalizedBidRequest` can be
constructed (round-28 H1) — a trusted-server caller cannot skip
implicit-field fill and still call `run_auction`. The host, account, and
parse boundaries are caller-owned so callers can use whatever stores and
transports fit their environment, but normalization is mandatory and
type-enforced (§3.2) so privacy and adapter behavior cannot diverge
between call sites.

1. **Capture `request_start = clock.now_monotonic()`** — anchor for the
   whole-auction deadline (§5.3). Round-117 P0-1 (M3b): once EdgeZero
   Prereq 10 lands, the capture happens in EDGEZERO INGRESS before the
   bounded body read (so slow-upload time is inside tmax) and arrives
   via `RequestContext`; the handler statement below then reads the
   injected instant rather than sampling a second one. Until that
   prerequisite, per round-82 review Low: the capture
   must be the first executable statement of the `#[action]` handler
   (M3b override — round-123 #14: once EdgeZero's ingress seam lands,
   admission AND this capture both move pre-routing per M3b Prereq 10;
   the statement below describes the M3a/dev shape)
   after `clock_handle(&ctx)` (extractor plumbing that returns an
   `Arc` handle with no wall-clock cost). `rng_handle(&ctx)`, fanout
   resolution, config load, and every other setup happens **after**
   `request_start` so its cost counts against tmax. A later edit that
   re-inserts work in front of the capture violates the "whole-
   auction deadline" invariant of §5.3. Then, **before reading the body**,
   inspect the `Content-Encoding` request header — a non-empty value
   (gzip, br, etc.) is **rejected with `400 Invalid request: Content-
Encoding of type <encoding> is not supported`** (exact upstream
   reference auction handling / plain-text error output wording — round-25 H2 + round-29 L6).
   Then **read body, bounded** with
   `Body::into_bytes_bounded(MAX_AUCTION_BODY)` (§5.3). Oversize → `400`.

   **EdgeZero ingress requirement** (round-63 H1 + round-64 H1 —
   body-size cap must be guaranteed at TWO layers; a per-path cap
   table alone is insufficient if the adapter has already read the
   body). The cap is enforced as **defense in depth** across three
   layers — at least one of which must hard-stop the read **before**
   memory allocation proportional to the request body has occurred:

   1. **Platform-runtime limit** (the outer wall). All four target
      runtimes already cap inbound request bodies independently of
      auctioneer:
      - Cloudflare Workers: 100 MiB cap per the Workers platform.
      - Fastly Compute@Edge: platform-configured cap (default
        applies; per-service tunable).
      - Spin: per-`spin.toml` configurable limit (Spin's
        `EdgeZero` adapter delegates to this; see its
        `into_core_request` comment).
      - Axum (native): the operator's reverse proxy / process
        manager owns the outer limit (e.g. nginx
        `client_max_body_size`); auctioneer does not run "naked"
        Axum in production.
        The platform limit is necessary but FAR larger than auctioneer's
        route caps (`MAX_AUCTION_BODY` = 256 KiB,
        `MAX_COOKIESYNC_BODY` = 32 KiB), so it does NOT replace the
        next two layers.

   2. **EdgeZero ingress policy hook** (round-64 H1 + round-65 H1 —
      reassigned to EdgeZero, since `auctioneer-adapter-*` crates only
      forward to `edgezero_adapter_*::run_app::<auctioneer_core::App>`
      and have no place to intercept the body before EdgeZero's
      `into_core_request` materializes it). The hook is what
      auctioneer actually needs from EdgeZero for
      production-completion — **identified during M2 design, but its
      implementation/release owner is M3b Task M3b-10** (round-80 M2;
      M3b-1..9 stay behavior-complete without it) — alongside the fan-out
      / clock / entropy primitives (open item 14). Required shape:

      - EdgeZero exposes a per-route ingress policy hook on `App`
        (or equivalent — the exact API shape is EdgeZero's call,
        analogous to its existing `extractors` mechanism). The
        hook is invoked **before** `into_core_request` materializes
        the body, receiving the **request method, path, headers, a
        normalized `FramingState` (round-74 H1), and a
        registration-issued `RouteResolution`** (round-136 #3 —
        REVERSES round-72 L6's hook-owns-classification rule:
        classification is REGISTRATION-owned; the hook consumes
        `Matched(RouteId) | MethodNotAllowed { allowed } | NotFound`
        and NEVER re-matches — `body_caps::match_route` survives only
        as the M3a/dev-path fallback where no registration ran;
        round-74 H1 — the framing is NORMALIZED by
        the adapter because the platform parser consumes the raw
        framing headers before the hook, so the hook cannot re-derive
        duplicate/conflicting lengths from the `HeaderMap`),
        and returning a `Decision` (round-72 B2 — richer than the
        earlier `Continue { bounded_read }`): `Reject(Rejection)` |
        `Dispatch` | `BoundedRead(BoundedReadPolicy { max_bytes,
on_exceeded, on_success })`, where a bounded read that crosses
        the cap yields the route-specific 400 `on_exceeded` and a
        within-cap read yields `on_success` (`Dispatch`, or the
        405/404 `Reject`). auctioneer-core registers a hook that:
        - **Method/route-aware cap lookup** (round-69 H1 — earlier
          `body_caps::for_path(&str) -> Option<usize>` was
          path-only, which would (a) give `GET /openrtb2/auction`
          the 256 KiB auction cap even though it's the wrong
          method, and (b) hand out the 32 KiB `/cookie_sync` cap
          in M2-M4 builds where that route doesn't exist yet). The
          accessor is now method + path + milestone-scoped:

          ```rust
          // auctioneer-runtime::body_caps
          pub enum RouteMatch {
              Matched { route_id: &'static str, cap_bytes: usize },
              MethodNotAllowed { cap_bytes: usize }, // path matches a
                                                     // known route but
                                                     // method does not
                                                     // — cap = default
                                                     // (4 KiB)
              NotFound        { cap_bytes: usize }, // no route matches
                                                    // path — cap =
                                                    // default (4 KiB)
          }
          pub fn match_route(
              method: &http::Method,
              path: &str,
              active_routes: &RouteTable,    // milestone-scoped
          ) -> RouteMatch;
          ```

          The `RouteTable` is constructed from the milestone's
          `edgezero.toml` registration set (M2: four auction-shaped
          routes; M3a adds `/_auctioneer/health/gvl` (five); M5:
          adds `/cookie_sync`, `/setuid`, `/getuids` for the
          full eight per round-42 M1 sweep). The accessor
          never returns a per-route cap for a route that isn't
          actually wired in the current build — so `POST
/cookie_sync` in an M2 build returns `NotFound { cap_bytes:
4096 }`, NOT `Matched { route_id: "cookie_sync",
cap_bytes: 32768 }`. This closes the milestone-scope
          loophole.

        - **Content-Encoding rejection** (round-66 M2 + round-69 M3
          — applies uniformly across ALL three `RouteMatch` arms,
          not just `/openrtb2/auction` and `/cookie_sync`): if
          Content-Encoding is non-empty, `Reject(400, "Invalid
request: Content-Encoding of type <encoding> is not
supported\n")` (Appendix F canonical body, round-69 L4
          — trailing `\n` made explicit everywhere). This check
          runs **before** the Content-Length / chunked checks
          below so a malformed Content-Encoding never reaches the
          body read. Same body on `Matched`, `MethodNotAllowed`,
          and `NotFound` arms — uniform handling closes the
          /missing + `Content-Encoding: gzip` ambiguity (round-69
          M3): always 400 with the Content-Encoding body, never
          404/405, because the encoding rejection short-circuits
          before the route-match-based status mapping.
        - **Per-arm body-cap + dispatch behavior** (round-69 H1 —
          replaces the earlier four-bullet path-only sequence):
          - **`Matched { route_id, cap_bytes }`**: if
            `Content-Length` exceeds `cap_bytes` → `Reject(400,
body)`. Otherwise: if `Content-Length` is missing OR
            `Transfer-Encoding: chunked` → `Continue` AND signal
            EdgeZero that `into_core_request` must use a **bounded
            read** capped at `cap_bytes`, returning 400 the moment
            the cap is exceeded. Body strings (exact upstream bytes;
            **trailing `\n` always** per round-69 L4 + Appendix F
            canonical):
            - `route_id == "auction"` (`POST /openrtb2/auction`,
              256 KiB): `Invalid request: request size exceeded
max size of 262144 bytes.\n`.
            - `route_id == "cookie_sync"` (`POST /cookie_sync`,
              32 KiB; only reachable in M5+ builds per round-69
              H1): `cookie_sync request size exceeded max size of
32768 bytes.\n`.
            - **Non-body routes** (`route_id ∈ {"status",
"info_bidders", "info_bidder", "getuids"}`; M2 ships
              `status`, `info_bidders`, `info_bidder`; M5+ adds
              `getuids`; round-71 M5 — these routes have
              `cap_bytes: 4096` since they don't read a body in
              the happy path): same 4 KiB cap rule. If
              `Content-Length` ≤ 4 KiB OR absent (bounded read
              under cap) → `Continue` and dispatch normally (the
              handler reads the request but ignores any body). If
              `Content-Length` > 4 KiB → `Reject(400, "request
size exceeded max size of 4096 bytes.\n")` — same
              body as the default-cap unmatched-path path.
          - **`MethodNotAllowed { cap_bytes: 4096 }`** (e.g. `GET
/openrtb2/auction` — path known, wrong method): same
            cap rules as above with the 4 KiB default body
            (`request size exceeded max size of 4096 bytes.\n`).
            If the body check passes, dispatch returns
            `Reject(405, "")` with no body.
          - **`NotFound { cap_bytes: 4096 }`** (e.g. `POST
/missing`, OR `POST /cookie_sync` in an M2-M4 build
            where the route isn't wired): same 4 KiB cap and 4
            KiB default oversize body. If the body check passes,
            dispatch returns `Reject(404, "")` with no body.
          - **Content-Encoding precedence** (round-69 M3 —
            Content-Encoding rejection above wins over
            method/route mapping; a `POST /missing` with
            `Content-Encoding: gzip` returns 400 with the
            Content-Encoding body, NOT 404). The order in the
            hook is: Content-Encoding → Content-Length →
            bounded-read → dispatch status; the first one to
            fire short-circuits the rest. Practically, the hook
            implementation looks like: classify the request via
            `match_route(method, path, active_routes)`; if not
            found / wrong method, treat the matched-route value
            as the `NotFound` / `MethodNotAllowed` arm (4 KiB cap)
            and run the same Content-Encoding / Content-Length /
            bounded-read pipeline; on success, return the 404 or
            405 instead of dispatching.
      - Without this hook, auctioneer-core can only enforce the cap
        AFTER EdgeZero has already buffered the full body (each
        adapter's `into_core_request` uses `to_bytes(_,
usize::MAX)` (Axum) / `req.bytes()` (Cloudflare) /
        `read_to_end` (Fastly) / `req.into_body().await` (Spin)
        per the pinned EdgeZero SHA — round-64 H1 + round-74 L2;
        Spin added to match the body-materialization list in
        §12 open item 14). That defeats layer-2's "before
        allocation" intent on adversarial traffic.
      - **Interim posture until EdgeZero ships the hook** (M2
        development only): auctioneer relies solely on layer 1
        (platform runtime limits — Cloudflare 100MiB / Fastly /
        Spin / reverse proxy on Axum) plus layer 3 (auctioneer-core
        embedded read). The development build documents this gap
        in its banner; production-complete release is blocked on
        the hook per §13/§14 + open item 14.

   3. **auctioneer-core embedded read** (defense-in-depth). After the
      adapter delivers a `Body`, auctioneer-core still calls
      `Body::into_bytes_bounded(MAX_AUCTION_BODY)` as a final
      check. This catches: a buggy adapter (forgot the
      pre-check), a future EdgeZero version that exposes a true
      streaming `Body` (so auctioneer-core can enforce the cap
      directly), or a runtime whose platform-limit changes.

   **Current adapter state** (round-64 H1 + round-65 H1 — verified
   against the `edgezero` pin (`docs/outbound-http-spec`
   @ `12965da2` since round-138; formerly `fcefdde3`, `56796415`,
   `c8d6e414`,
   `9e661ae`, before that `2277072`)): each
   `auctioneer-adapter-*` crate is a thin shim that forwards to
   `edgezero_adapter_<target>::run_app::<auctioneer_core::App>` (see
   each adapter's `lib.rs` / `main.rs` — no body handling at the
   auctioneer layer). EdgeZero's `into_core_request` then materializes
   the full body unconditionally (Axum `to_bytes(_, usize::MAX)`;
   Cloudflare `req.bytes()`; Fastly `read_to_end`; Spin
   `req.into_body().await` with an explicit comment that it
   delegates to platform-runtime limits — round-70 L10 corrects
   the earlier `body.bytes()` citation). Since the auctioneer adapter shims have
   no place to intercept the body, **layer 2 is an EdgeZero
   responsibility** (open item 14 / §13 prereq list); auctioneer's
   share of the work is:
   - **`auctioneer-runtime::body_caps::match_route` accessor** (round-69
     H1 + round-70 M2 — corrects stale `for_path` references) ships in
     M1 (round-65 M5) — the cap table EdgeZero's hook will read
     from.
   - **auctioneer-core ingress hook registration** — round-71 H2: the
     CANONICAL owner is **M3b Task M3b-10** (production-completion,
     blocked on EdgeZero Prereq 5), NOT M2. The earlier "ships in M2"
     note was aspirational: registration cannot ship until EdgeZero
     publishes the INGRESS hook API — still absent at `12965da2`
     (round-137: PR 275 landed the OUTBOUND foundation and an
     all-route admission seam, but the route-aware
     classification / grant-transfer / parser-cap surfaces this
     hook needs remain the open G1/G16 residue). M2
     ships only the `match_route` accessor (above); the hook that
     consumes it is M3b-10's deliverable. (This is the single owner —
     the §14 production-complete boundary and M3a's non-goals both
     defer to M3b-10.)
   - Until EdgeZero ships the hook (Prereq 5) and M3b-10 lands, the
     adapter does not pre-cap; layer 1 (platform) + layer 3
     (auctioneer-core post-buffer cap) are the operational defenses, and
     the development banner records the gap.

   §11.2 ingress tests (round-65 M3 — Axum-only end-to-end since
   that's the only adapter target with handler-level integration
   coverage; see Cross-target test scope below): when EdgeZero
   ships the hook, the Axum integration suite asserts:
   - 400 with no body-read metrics when `Content-Length` exceeds
     the route cap (cheap path).
   - 400 with bounded memory growth when `Content-Length` is
     absent and a streamed body would exceed the cap (bounded-
     read path).
   - 400 also fires when chunked encoding sends > cap bytes (no
     way to pre-check via header).
     Cloudflare/Fastly/Spin remain build-only verification per the
     round-36 L4 cross-target scope decision (§11.2); per-adapter
     ingress contract tests against those targets are a later-phase
     parity-hardening task.

2. **Resolve host config** from one validated runtime blob and baked
   defaults using the typed overlay in §9.1. Shared config extraction and
   effective-host resolution precede the handler-local bounded body read;
   earlier EdgeZero ingress/admission refusals still win at their boundary.
   Malformed runtime data, including any account overlay, → **500**
   `Malformed host configuration`; transient store failures retain 503
   with Retry-After. Clamp runtime resource settings to the boot-funded
   envelope before constructing consumers. No second config read is allowed.
3. **Bounded syntax/node check, then resolve account** → `ResolvedAccount`
   (§9.1). Body bytes remain bounded by step 1's limits; before account
   discovery or typed allocation, count all syntactic nodes using the
   effective host node cap. Invalid JSON or node-budget overflow → **400**,
   even if the body also names a disabled account. Only then discover raw
   `app|site|dooh.publisher.{ext.prebid.parentAccount,id}` identity and
   resolve the typed account overlay. For syntax-valid, within-budget input,
   account-disabled → **503** before full parse/prepare. Malformed runtime
   account data is already a whole-blob loader failure, not a separately
   fetched `MalformedAcct` outcome. M6 section 7 pins competing-error tests.
4. **Parse** → `openrtb::BidRequest`. auctioneer accepts **OpenRTB 2.6
   only**; 2.5-style legacy fields under `regs.ext.*` /
   `user.ext.*` are not upconverted (§3). Failure → `400 Invalid
request: <err>` (Appendix F).
5. **Prepare** — call `auctioneer_openrtb::prepare(req, headers,
&limits)` (§3.2; round-127 — the three-argument M3b form, step 0
   being the impressions-limit check; M3a's two-argument call is the
   shipped shape until M3b-2's mechanical migration). This single function performs implicit-field fill +
   validate + 2.6 privacy gate + channel validation, producing a
   `NormalizedBidRequest` (consent-syntax warnings are NOT done here —
   they live in `PrivacyContext::resolve`, §7.7 / round-37 H3). Both
   call sites _must_ invoke `prepare`; the type system enforces that
   there is no other path to `NormalizedBidRequest`. Failure → `400
Invalid request: <err>` (Appendix F).
6. **Privacy preprocessing** — build `PrivacyContext` (§7).
7. **Consume resolved aliases + GROUP participation** — round-106 H8 /
   round-108 H6 (steps 7–9 are ONE authoritative
   DECIDE → MATERIALIZE → SCRUB sequence; the pre-round-108 text made
   step 8 re-apply privacy and step 9 re-clone — those were leftovers
   of the retired all-clones-first shape). Alias RESOLUTION happens at
   HANDLER entry — `resolve_aliases(&normalized, registry)` maps each
   `ext.prebid.aliases` entry to its base bidder and merges
   `ext.prebid.aliasgvlids`, and the result rides `AuctionPlan.aliases`
   into `run_auction` — this step READS `plan.aliases`; it never
   re-resolves. (Resolution still cannot live in `prepare`:
   `auctioneer-openrtb` knows nothing about the bidder registry —
   round-13 finding H3.) This step then GROUPS participation — the
   per-bidder name set + per-bidder imp membership (indices), one
   owned `request_value`, and the disabled-bidder warnings — WITHOUT
   materializing any bidder-scoped clone. The following merge/hoist
   semantics are DEFINED here and EXECUTE at step 9's
   materialization. For each imp: (a) **merge `ext.prebid.bidderparams.{name}`
   (request-level) into both `imp.ext.bidder` and
   `imp.ext.prebid.bidder.{name}`** before hoisting — **imp-level values
   take precedence** over request-level (matches reference auction handling;
   round-22 finding H3). **The merge supplements only — it never creates
   bidder participation** (round-23 finding H1; matches upstream
   reference auction handling): if neither `imp.ext.<name>` nor
   `imp.ext.prebid.bidder.<name>` is already present on an imp, the
   request-level params for that bidder are skipped for that imp; only
   missing keys are filled for existing bidder entries. (b) hoist
   `imp.ext.prebid.bidder.{name}` to `imp.ext.bidder`; (c) **also merge
   bidder-specific FPD from `imp.ext.prebid.imp.{name}`** into the
   bidder-scoped `Imp` (round-17 finding M6). **The bidder-specific
   FPD lookup uses object indexing on
   `imp.ext.prebid.imp.<bidder-name>`, NOT a JSON Pointer
   `/ext/prebid/imp/<bidder-name>`** — since `BidderName` is a free
   `Box<str>` and alias validation does not reject `/` or `~`
   metacharacters, a pointer-based path would misparse alias names
   like `my/alias` (round-80 review Medium). (d) **strip every
   competitor's params from the bidder-scoped ext**:
   `imp.ext.prebid.bidder`, `imp.ext.prebid.imp`, AND every top-level
   `imp.ext.<other-bidder-name>` entry (spec §5.1 step 7 competitor
   contract). A key is a "bidder name" here iff it resolves as a
   registered bidder OR a request-declared alias — free-form
   passthrough top-level keys survive scoping unchanged (round-80
   review High). Re-validate the bidder-scoped imp; **failure is
   fatal** — the auction returns an
   `AuctionError::InvalidImpFirstPartyData` (code 15), caller emits
   **`500`** (matches reference auction policy / reference auction processing fatal severity,
   and the auction endpoint's reference auction handling mapping of non-`BadInput`
   exchange errors to `500`; round-20 finding H1). auctioneer does not
   down-grade this to a per-bidder drop. (Executed at step 9 — only
   bidders that survive step 8 are ever materialized and validated; a
   malformed FPD object scoped to a non-surviving bidder no longer
   fails the auction, round-107 B4.)
   FPD must also preserve the original `imp.id`: replacement or deletion
   is code 15 before scrubbing or either adapter hook, even on an expired
   auction. Repeating the same ID and merging context remain valid. This
   invariant keeps response validation and timeout rows within the
   original participation scope without restoring an unsanitized request.
8. **DECIDE per bidder** (names only — no clone exists yet):
   `fetchBids` denied or GDPR `AllowBidRequest` false → drop the
   bidder (warning). For survivors, RECORD the scrub decisions
   (`PassID`/`PassGeo`, §7.6) and whether the bidder lacks
   `openrtb.gpp_supported` (step 9 back-fills for those). Then
   sampling, then the `max_bidders_per_auction` cap — in that order
   (§5.9/§9.1), all over the step-7 name set.
9. **MATERIALIZE + SCRUB + fan out** — for each step-8 survivor:
   build the bidder-scoped `BidRequest` from the step-7 grouping
   (EXECUTING the step-7 merge/hoist/competitor-strip semantics and
   the code-15 validation — fatal); then scrub per the RECORDED
   step-8 decisions (§7.6) and back-fill _empty_ legacy privacy
   fields from GPP for non-`gpp_supported` bidders (§7.4 — never
   overwrite a populated field).
   **Adapter-visible `BidRequest.TMax` vs auction
   execution deadline are now distinct** (round-27 finding H2; matches
   Upstream reference auction handling + reference auction deadline adjustments):
   - **Adapter-visible `BidRequest.TMax`** (what the bidder sees in the
     outbound payload) = the **defaulted** request tmax in milliseconds
     (raw `req.tmax`, or `tmax_default` if absent/0). **Not** the
     host-capped value. Tmax-adjustments are **out of scope in phase 1**
     (open item; recorded in §15) — when added, that feature applies a
     bidder-adjusted reduction here, not the cap.
   - **Execution deadline** = the host-capped `AuctionDeadline.deadline_at`
     (§5.3), which the M3b fan-out composes ONCE PER CALL
     (min with the bidder timeout) as a `MonotonicInstant` on the
     ONE application clock (`ctx.monotonic_clock()`, the same clock
     `request_start` is read from) and passes as an ABSOLUTE
     upstream `Deadline::at_instant(composed_at)` (round-147 #1:
     EdgeZero exposes only `Deadline::after` and
     `Deadline::at_instant`; the round-135/136 `ClockBridge` /
     `instant_of` projection and the `Deadline::at(..)` name are
     RETIRED from this spec — there is no second instant domain to
     project into. Round-135 #5 still stands: never reconstruct a
     relative `Deadline::after(remaining)`, because time elapsing
     between the two clock reads silently extended the deadline).
     The cap is enforced server-side; bidders are not told about it.
     Construction and every queued transport finalization check that same
     absolute deadline. Expiry stops further encodings, releases the
     partial batch and remaining bodies without fanout, and retains the
     existing timeout/error/warning bookkeeping for original impression IDs.
   - Then `make_requests` → outbound calls via `BidderFanout` (§5.2) →
     `make_bids`. Subject to §5.3.
10. **Collect** `SeatBid`s. MVP enforces single currency USD; a seatbid whose
    currency is not USD has its bids dropped with an `ext.warnings` entry
    (Appendix E.5).
11. **Targeting & winners** — price granularity, `hb_*` keys, per-imp winner
    selection — exact algorithm in **Appendix E**.
12. **Respond** — serialize `BidResponse` JSON (`200`); shape in Appendix E.
    `ext.debug` only when permitted _and_ requested (§5.6).

### 5.2 Outbound fan-out (EdgeZero fan-out primitive)

> **M3 adapter-tier note (round-19 C4; round-36 H5 Spin
> correction).** The pinned EdgeZero proxy layer is not
> uniformly async across adapters. Fastly's
> `pending_request.wait()` blocks and cannot be dropped mid-
> await to cancel. Spin's HTTP body reader is async, but its
> drop-cancellation guarantees are unverified in the pinned
> shim — until M3b Task M3b-4 confirms drop-cancellability
> (round-20 correction), Spin behaves the same as Fastly for
> M3 planning purposes. In M3a the live
> `EdgeZeroFanout` (mid-fanout deadline + per-call timeout +
> per-bidder cancellation) ships **only on Axum**, which installs
> `RaceTimersStrategy`. **Cloudflare, Fastly, and Spin all install
> `DelegateToDevStrategy` under M3a** (`EdgeZeroFanout` construction
> is present so tests type-check, but it delegates to `DevFanout`
> semantics: no per-call cancellation; deadline honored at outer
> batch grain only) — this matches the canonical per-adapter table
> below. The **M3b per-adapter spike (M3b Task M3b-3 for Cloudflare;
> M3b-4 Spin, M3b-5 Fastly)** — NOT any M3a Task 11 step (Task 11
> starts after Task 9.5 and has no Step 0a; round-65 L1 corrects a
> dangling reference) — investigates whether Cloudflare's outbound
> primitive is drop-cancellable enough to earn a live race strategy in
> M3b; nothing in M3a flips Cloudflare live. Bucket B conformance tests under the
> three delegating adapters are documented `#[ignore]`d — not a bug,
> a milestone-gated deferral. Production-shipping a live race on
> Cloudflare/Fastly/Spin requires upstream EdgeZero to expose a
> drop-cancellable nonblocking outbound primitive, confirmed
> per-adapter in M3b.

EdgeZero **may** later provide a native fan-out primitive that issues many
concurrent outbound HTTP calls in a single await. **Round-36 H5 canonical
architecture** — that primitive is NOT what auctioneer uses today; the
implementation lands as a `FanoutStrategy` trait selected per adapter, with
Axum installing `RaceTimersStrategy` (biased three-arm race over the pinned
`ProxyClient::send`) and Cloudflare / Fastly / Spin installing
`DelegateToDevStrategy` until per-adapter cancellation primitives land in
M3b. See M3a Task 9.5 for the strategy shape; §5.2 continues to describe the
`BidderFanout` trait that auctioneer-exchange consumes, but the production
CONSTRUCTION now flows through the strategy install path, not a hypothetical
a hypothetical direct client call. The batch illustration below shows the
CURRENT upstream
shape (`start_batch_until` + `OutboundBatchNext`); the equivalent auctioneer
call site is `EdgeZeroFanout::send_all` on the `BidderFanout` trait,
which is auctioneer's OWN method and is not going away.

The API is not final, but EdgeZero's current proposal is concrete enough to
design against:

```rust
// Proposed EdgeZero API — shape not final.
let http = ctx.http_client()
    .ok_or_else(|| EdgeError::internal("outbound http unavailable"))?;
// auction_deadline.deadline_at was anchored to request_start (§5.1 step 1) +
// the effective tmax budget (§5.3). Both are `MonotonicInstant`s on the
// ONE application clock (`ctx.monotonic_clock()`); round-147 #1: there
// is no bridge and no projection — the composed instant is handed to
// EdgeZero as-is. Round-135 #5: never reconstruct a relative
// `after(remaining)`, where time elapsing between the subtraction and
// the wrap silently extends the deadline.
// per call: composed_at = min(fanout_entry_at + call.timeout,
//                             auction_deadline.deadline_at)
let deadline = Deadline::at_instant(composed_at);
// The expired-budget short-circuit happens in the exchange (run_auction,
// §5.8) before reaching the fan-out — BidderFanout never sees an
// already-expired composed deadline. By this point composed_at > now is
// an invariant.

// Each adapter has already produced a BidderHttpRequest (method, url,
// headers, body) via its make_requests. The exchange maps that 1:1 to
// an OutboundRequest — preserving the adapter's exact method, URL, and
// raw body bytes (round-23 finding M3). Round-109 B2 — ONE owner for
// compression + entity headers: `auctioneer_bidder::finalize_for_transport`
// (§4.1), called by the EXCHANGE once per accepted call before
// BidderCall construction (not inside the adapter — round-24 H1; not
// inside the fanout drive — the M3a-era in-drive application is
// retired at M3b-2). Gzip is applied there when the bidder's
// `BidderInfo.endpoint_compression` (read by the exchange at finalize
// time — round-150 #6) is set, writing through the bounded
// CappedWriter. There is NOTHING to overwrite: `Content-Length`,
// `Content-Encoding`, and `Sec-GPC` (round-147 #2: `Content-Length`
// is RESERVED but NEVER WRITTEN by auctioneer — EdgeZero's
// `normalize_for_dispatch` strips `Host` and `Content-Length` from
// every outbound request immediately before platform conversion
// (`edgezero-core/src/outbound.rs` @120dd2aa L824-826 →
// `strip_hop_by_hop(.., strip_request_framing = true)` L1209-1212)
// and the provider transport generates framing from the body it is
// handed; a auctioneer-written value would be discarded at best and a
// framing conflict at worst) — plus, round-110 H4, the RFC
// 9110 routing/framing set: `Host`/`:authority`, `Transfer-Encoding`,
// `TE` (round-111 H5 — RFC 9110 §7.6.1), `Connection`, `Trailer`,
// `Upgrade`, `Proxy-Connection`,
// `Keep-Alive` — are RESERVED header names the sink REJECTS at push
// time (case-insensitive), so adapters cannot supply stale entity
// metadata, override routing, or smuggle conflicting framing at all (supersedes round-25 M6's
// rewrite-on-apply rule); the finalizer APPENDS exactly TWO into its
// byte pool reserve — `Content-Encoding` (when gzip applied) and
// `Sec-GPC` (when gpc == "1") — and never a Content-Length.
// Encoded-size failure is FinalizeError::OversizeEncoded → the
// positional FanoutError::OversizeRequest { stage: Encoded }.
// Round-110 B3: BidderCall.request is ALREADY FINALIZED — the
// exchange ran `finalize_for_transport` (gzip through the bounded
// writer, Content-Encoding + Sec-GPC appended into the byte pool reserve)
// BEFORE constructing the BidderCall. The transport conversion is a
// pure 1:1 mapping on auctioneer's side: no re-compression, no header
// filtering, no Content-Length computation here (the round-25 M6
// strip-and-rewrite loop this snippet used to show is RETIRED —
// construction-time reserved-header rejection made it dead). EdgeZero
// then runs its OWN framing pass (`normalize_for_dispatch`) that
// removes Host/Content-Length and lets the provider transport frame
// the body — that pass is upstream-owned and auctioneer neither
// duplicates nor tests it.
// Round-148 #2 / round-149 #2-#4 / round-150 #2-#4: the conversion is
// TYPED, FALLIBLE PER SLOT, COPY-FREE for the body, and consumes the
// FACADE's `BoundedOutboundRequest` — built ONCE by `compose_all` at
// FANOUT ENTRY (round-155 #3: NOT at wave entry — see the algorithm
// below; composing per wave would re-anchor a call that lost gate
// contention and silently extend its deadline): `deadline_at =
// min(fanout_entry_at + call.timeout, auction.deadline_at)`,
// `selected_cause` stored AT COMPOSITION (equality ⇒ Bidder). A conversion failure is THAT
// slot's `BoundedSlotError::Preflight` (nothing sent, never a silently
// altered request) and the valid siblings still dispatch. There is
// no `?` inside the loop.
fn to_outbound(call: BoundedOutboundRequest, limits: &ResolvedIsolateLimits)
    -> Result<OutboundRequest, BoundedSlotError>
{
    let BoundedOutboundRequest { request, policy, deadline } = call; // BY VALUE
    let (meta, body) = request.into_parts();  // meta: OutboundRequestMeta
                                              // (byte pool; &str accessors);
                                              // body: Vec<u8> — MOVED, the
                                              // ONLY body allocation is the
                                              // sink's (round-149 #3)
    let preflight = |_| BoundedSlotError::Preflight;
    let method: Method = meta.method().parse().map_err(preflight)?;
    let uri: Uri = meta.url().parse().map_err(preflight)?;
    let mut req = OutboundRequest::new(method, uri).map_err(preflight)?
        .body(Body::from_bytes(body));        // owned Vec<u8>, no clone
    for (k, v) in meta.headers() {
        req = req.header(k, v).map_err(preflight)?; // the platform's own
    }                                               // HeaderMap — charged as
                                                    // OUTBOUND_HEADER_MAP_CHARGE
                                                    // (round-150 #5)
    Ok(req.deadline(Deadline::at_instant(deadline.deadline_at)) // round-137 #3 /
                                           // round-147 #1: ONE
                                           // precomposed absolute
                                           // app-clock instant per
                                           // call — min(bidder_at,
                                           // auction_at). NO
                                           // .timeout(): a relative
                                           // timeout RE-ANCHORS when
                                           // a later wave dispatches
                                           // and corrupts BudgetSource
                                           // attribution; auctioneer's
                                           // stored selected_cause is
                                           // authoritative.
          // REQUEST side (round-148 #2): EXACTLY the envelope's
          // max_outbound_request_bytes. EdgeZero enforces its own
          // request-body cap at dispatch (default
          // DEFAULT_OUTBOUND_REQUEST_BODY_BYTES = 8 MiB → slot
          // BadRequest); an envelope may set the auctioneer cap above
          // that, and the finalizer has ALREADY enforced it, so
          // setting the two equal makes the dispatch check
          // unreachable for an accepted request. Pinned by
          // `outbound_request_body_cap_equals_envelope_cap()`.
          // ROUND-159 #3 — CACHE BYPASS IS MANDATORY ON BIDDER CALLS.
          // EdgeZero defaults outbound requests to
          // `OutboundCachePolicy::PlatformDefault`, which lets a
          // platform intermediary cache a bid response. Bid responses
          // are request-specific and carry privacy-sensitive,
          // single-use data (per-impression pricing, user signals), so
          // a cache hit is both a correctness bug — a stale bid served
          // for a different auction — and a privacy leak across
          // requests. Every auction call sets Bypass; no bidder call
          // is ever cacheable.
          .cache_policy(OutboundCachePolicy::Bypass)
          // AUTHORITY OVERRIDE IS NOT SET, deliberately: auctioneer
          // REJECTS caller-supplied authority (the `Host` /
          // `:authority` pair is in the sink's reserved-header set)
          // and the outbound authority derives from the validated
          // destination URL. There is nothing to override.
          .max_request_body_bytes(limits.max_outbound_request_bytes)
          // RESPONSE side: every field of the per-call policy —
          // none is left at an EdgeZero default.
          .max_encoded_response_bytes(policy.max_transport_bytes)
          .max_decoded_response_bytes(policy.max_decoded_bytes)
          .max_response_header_bytes(u64::from(policy.max_header_bytes))
          .max_response_header_count(u64::from(policy.max_header_count))
          .max_brotli_window_bits(policy.max_brotli_wbits)
          .max_brotli_decoder_bytes(policy.max_brotli_decoder_bytes)
          .max_chunk_bytes(policy.max_chunk_bytes)   // NonZeroU64 (round-149 #4):
                                                     // the policy STORES the
                                                     // NonZeroU64 constant
                                                     // MAX_CHUNK_BYTES (8 KiB),
                                                     // not envelope-configurable,
                                                     // so no zero can arise
          // buffered mode's FINAL collection limit is the max() of the
          // two caps (the five-clause facade contract: max() bounds
          // only the buffered memory, each cap is still enforced
          // independently during the read)
          .max_response_bytes(policy.max_transport_bytes
                               .max(policy.max_decoded_bytes)))
}

// Round-152 #2 — TWO functions, TWO anchors. `compose_all` runs ONCE
// at fanout entry and fixes every call's IMMUTABLE absolute deadline;
// `run_wave` runs once per acquired lease with its own batch-entry
// instant (the anchor for preflight elapsed) and dispatches AT MOST
// the acquired k calls. The wave scheduler of the plan's §3 (race
// `acquire_wave(k)` against the earliest pending deadline; inclusive
// `deadline_at <= now` recheck after acquisition; RAII lease released
// on wave completion INCLUDING auctioneer's passthrough decode) sits
// between them and is not re-derived here.
// Round-151 #5 — composition is ERROR-BEARING: an unrepresentable
// instant (`checked_add` overflow) is the OUTER
// `FanoutInternalError::Internal` → 500 before any dispatch.
fn compose_all(calls: Vec<BidderCall>, auction: &AuctionDeadline,
               fanout_entry_at: MonotonicInstant)
    -> Result<Vec<BoundedOutboundRequest>, FanoutInternalError>
{
    calls.into_iter().map(|call| {
        let bidder_at = fanout_entry_at.checked_add(call.timeout).ok_or_else(|| {
            FanoutInternalError::Internal("deadline composition overflow".to_owned())
        })?;
        let (deadline_at, selected_cause) = if bidder_at <= auction.deadline_at {
            (bidder_at, TimeoutCause::Bidder)              // equality ⇒ Bidder
        } else {
            (auction.deadline_at, TimeoutCause::AuctionDeadline)
        };
        Ok(BoundedOutboundRequest {
            policy: call.policy,
            deadline: ComposedDeadline { deadline_at, selected_cause },
            request: call.request,
        })
    }).collect()
}

// Round-152 #1, CORRECTED round-153 #1/#2 — each slot carries its
// `ComposedDeadline` AND its `DecodePolicy` from composition through
// dispatch to decode. Round-153 #2: the ONLY auctioneer-owned deadline
// recheck is around AUCTIONEER'S OWN decode work. The upstream phase's
// terminal outcome is EdgeZero's to report — each slot has its own
// terminal `elapsed` and EdgeZero does not copy the vector's return
// time across slots — so a batch-time or publication-time recheck
// would retroactively rewrite a bidder that genuinely finished on
// time (a sibling delaying the batch, a later wave, or a slow
// publication loop would be blamed on it). Expiry that upstream
// itself observed arrives as `GatewayTimeout` and classifies through
// the stored `selected_cause`; nothing else is rewritten.
struct DecodePolicy {                 // round-153 #1: the auctioneer-owned
    max_decoded_bytes: u64,           // decode caps. Copied from the
    max_brotli_wbits: u8,             // call's ResponseBodyPolicy at
    max_brotli_layers: u8,            // composition, because
    max_brotli_decoder_bytes: u64,    // `to_outbound` CONSUMES the call
    max_chunk_bytes: NonZeroU64,      // and the decoder runs after the
}                                     // slot returns
struct Pending { index: usize, deadline: ComposedDeadline,
                 decode: DecodePolicy, request: OutboundRequest }
struct Slot    { result: BoundedSlotResult, deadline: ComposedDeadline,
                 decode: DecodePolicy }

async fn run_wave(
    wave: Vec<(usize, BoundedOutboundRequest)>, // ≤ k members, ORIGINAL indices,
    out: &mut [Option<Slot>],                   // input order (plan §3 (i))
    batch_entry_at: MonotonicInstant,
    clock: &dyn Clock, http: &HttpClient, limits: &ResolvedIsolateLimits,
    yielder: &dyn DecodeCooperator,             // round-154 #7: injected,
) -> Result<(), FanoutInternalError> {          // never ambient
    // Preflight compaction (round-149 #2 / round-150 #2): failed slots
    // keep their index and their deadline, carry the auctioneer-measured
    // elapsed from BATCH entry (zero is legal — round-151 #6); only the
    // valid siblings go upstream; an all-invalid wave never constructs
    // an upstream batch.
    let mut pending: Vec<Pending> = Vec::with_capacity(wave.len());
    for (index, call) in wave {
        let deadline = call.deadline;
        let decode = DecodePolicy::from(&call.policy);  // round-153 #1: BEFORE
                                                        // to_outbound consumes it
        match to_outbound(call, limits) {
            Ok(request) => pending.push(Pending { index, deadline, decode, request }),
            Err(err) => out[index] = Some(Slot {
                result: BoundedSlotResult {
                    elapsed: clock.now_monotonic().saturating_duration_since(batch_entry_at),
                    outcome: Err(err),                       // Preflight — round-153
                },                                           // #2: FINAL, never
                deadline, decode,                            // rewritten later
            }),
        }
    }
    if pending.is_empty() {
        return Ok(());
    }
    let expected = pending.len();
    let mut keys = Vec::with_capacity(expected);
    let mut reqs = Vec::with_capacity(expected);
    for Pending { index, deadline, decode, request } in pending {
        keys.push((index, deadline, decode));
        reqs.push(request);
    }
    // ⚠ ROUND-163 #1 — AWAITING THE M3b-0 REWRITE. The batch API has
    // moved twice since this illustration was written; at head
    // `3d87d5bc` `collect()` fails with `OutboundBatchFailure
    // { error, slots }`, which PRESERVES completed slots, so neither
    // neither this batch loop nor a naive `.await?` expresses the
    // contract. M3b-0 replaces this block.
    // ⚠ ROUND-159 #1 — PIN-SCOPED: this call is the contract at the
    // PINNED EdgeZero `120dd2aa`. At head `61457d7d`
    // `OutboundHttpClient::send_all` is HARD-REMOVED in favour of
    // `start_batch_until(..) -> OutboundBatch` (`next()` / `cancel()`
    // / ordered `collect()`). This algorithm is therefore correct
    // ONLY for the pinned revision; the M3b plan's P0 banner owns the
    // rebase, which is a SEMANTIC redesign (completed slots stay
    // terminal; unresolved-at-cutoff slots become
    // `Timeout(selected_cause)`; `None` at cutoff is not a contract
    // violation) and must land before the pin moves.
    // Dispatch-before-harvest on every runtime: each SUCCESSFUL
    // dispatch completes before the first harvest. Whether requests
    // are simultaneously in flight is the provider's timing, not a
    // auctioneer claim — Fastly harvests serially in input order with
    // BestEffort slot isolation.
    let mut batch = http.start_batch_until(reqs, cutoff);
    let mut slots: Vec<Option<OutboundSlotResult>> = vec![None; expected];

    loop {
        match batch.next().await {
            // TERMINAL: a resolved slot is never revisited or rewritten.
            Ok(OutboundBatchNext::Item(item)) => {
                slots[item.index] = Some(item.result);
            }
            // Every dispatched slot resolved, so a still-unfilled slot
            // is a genuine positional breach. A RUNTIME check with a
            // TYPED outer error — never debug_assert, never
            // zip-truncation, never expect.
            Ok(OutboundBatchNext::Finished(OutboundBatchTermination::Completed)) => {
                let got = slots.iter().filter(|s| s.is_some()).count();
                if got != expected {
                    return Err(FanoutInternalError::BatchContractViolation {
                        expected,
                        got,
                    });
                }
                break;
            }
            // The observation window closed. Unfilled slots become
            // positional timeouts; completed siblings are untouched,
            // and this is NOT a contract violation.
            Ok(OutboundBatchNext::Finished(OutboundBatchTermination::Cutoff)) => break,
            // BATCH-LEVEL failure (duplicate or out-of-range index,
            // premature driver termination). Items already recorded are
            // terminal and valid, but auctioneer publishes a wave whole or
            // not at all, so this maps to the outer error and 500. The
            // batch is poisoned; it is not polled again.
            Err(_) => return Err(FanoutInternalError::Internal),
            // A future variant of either enum is Err-equivalent, never
            // a silent per-slot default: a new upstream variant is a
            // pin-move review item.
            _ => return Err(FanoutInternalError::Internal),
        }
    }

    // Cutoff leaves holes; each becomes a positional timeout carrying
    // the cause selected at dispatch. The wave lease releases by RAII
    // on every path above.
    let slots: Vec<OutboundSlotResult> = slots
        .into_iter()
        .map(|slot| slot.unwrap_or_else(|| OutboundSlotResult::timeout(selected_cause)))
        .collect();

    for ((index, deadline, decode), slot) in keys.into_iter().zip(slots) {
        // OutboundSlotResult { elapsed, outcome: Result<OutboundResponse,
        // EdgeError> } → BoundedSlotResult { elapsed, outcome:
        // Result<BoundedProxyResponse, BoundedSlotError::Edge(..)> }.
        // Round-153 #2: the upstream outcome is TAKEN AS TERMINAL. An
        // upstream expiry is already `GatewayTimeout` in it; auctioneer adds
        // NO post-batch recheck, because `next()` yields each slot as it
        // the whole batch and a slow sibling must never convert an
        // on-time bidder into a Timeout.
        let mut result = lift_slot(slot);
        // Passthrough decode UNDER THE WAVE LEASE and UNDER THE ORIGINAL
        // DEADLINE (plan cap-ownership row 3), with THIS slot's own
        // `DecodePolicy` (round-153 #1 — decoded-byte, layer, brotli
        // window and decoder-memory caps; the memory proof charges
        // exactly these). This is the ONE recheck auctioneer owns, over work
        // auctioneer itself performs: the decoder is COOPERATIVE and returns
        // `Err(Expired)` the moment `deadline_at` passes, so decode
        // expiry can never surface as DecoderFailure / OversizeResponse.
        // Decode time is added to `elapsed` (spec §5.7).
        if matches!(&result.outcome, Ok(resp) if resp.cap_row == CapRow::RawPassthrough) {
            let decode_start = clock.now_monotonic();
            result.outcome =
                decode_passthrough(result.outcome, &decode, &deadline, clock, yielder).await;
            result.elapsed = result.elapsed
                .saturating_add(clock.now_monotonic().saturating_duration_since(decode_start));
        }
        out[index] = Some(Slot { result, deadline, decode });
    }
    Ok(())
}

// Round-153 #4, dataflow completed round-154 #7 — the decoder's
// cooperative yield. OWNERSHIP: it is a HOST PRIMITIVE, installed by
// each adapter through `bootstrap_adapter(make_clock, make_rng,
// make_timer, make_strategy, make_profile, make_decode_cooperator)`
// — a sixth `OnceLock`-backed installer argument, idempotent like the
// others, with `decode_cooperator_handle()` as the reader and
// `TestDecodeCooperatorHandle` as the extensions-injected test
// override (exactly the pattern clock/rng/timer/strategy/profile
// already use). `EdgeZeroFanout` takes it at construction from
// `fanout_from_context` and threads it into every `run_wave` call as
// the `yielder` parameter above; nothing reads it ambiently.
// Reading the clock
// between always-ready decoded chunks is NOT sufficient on Cloudflare:
// Workers freezes `Date.now`/`performance.now` across CPU-only
// execution, so a pure-CPU decode loop sees ONE instant no matter how
// long it runs. EdgeZero's own decoder yields through a private host
// event; auctioneer holds raw passthrough bytes and cannot reach it. So
// the decoder takes a `&dyn DecodeCooperator` from the adapter:
//
//   #[async_trait(?Send)]
//   pub trait DecodeCooperator {
//       /// Awaits a CLOCK-REFRESHING host boundary. Cloudflare's impl
//       /// MUST await a real host EVENT — `worker::Delay::from(
//       /// Duration::ZERO)` (a `setTimeout(0)`-backed future, which
//       /// returns control to the JS event loop and so refreshes the
//       /// frozen clock). Round-154 #7: an already-resolved promise
//       /// or a bare `yield_now()` does NOT refresh it and is
//       /// FORBIDDEN on Cloudflare. The native and WASI impls are
//       /// `yield_now()` (their clocks are not frozen across CPU
//       /// work, so the yield only bounds poll monopolization).
//       /// Must be cheap:
//       /// it is called once per `yield_every_bytes` of decoded output.
//       async fn yield_to_host(&self);
//       /// Decoded-byte interval between yields. Bounded so a hostile
//       /// stream cannot monopolize a poll: the default is 64 KiB, and
//       /// `yield_every_bytes <= max_chunk_bytes * 8` is validated.
//       fn yield_every_bytes(&self) -> NonZeroU64;
//   }
//
// `decode_passthrough` awaits `yield_to_host()` every
// `yield_every_bytes` of decoded output and re-reads the clock AFTER
// the await — so the deadline check runs on a REFRESHED clock. Zero
// decoded bytes between yields is impossible (the interval is
// NonZero), so the loop always terminates. It is installed beside the
// other host primitives (`bootstrap_adapter`), and the frozen-clock
// conformance test asserts that a decode which would exceed the
// deadline returns `Expired` under a clock that only advances at
// `yield_to_host()`.

// THE DRIVER (plan §3 wave loop, normative there; shown here only for
// the anchors and the data flow):
let fanout_entry_at = clock.now_monotonic();
let calls = compose_all(bidder_calls, &auction_deadline, fanout_entry_at)?;
let total = calls.len();
let mut out: Vec<Option<Slot>> = std::iter::repeat_with(|| None).take(total).collect();
let mut pending: Vec<(usize, BoundedOutboundRequest)> = calls.into_iter().enumerate().collect();
while !pending.is_empty() {
    // earliest = min(deadline_at over pending); k = min(gate capacity, |pending|);
    // race acquire_wave(k) against earliest. This wait IS auctioneer-owned work,
    // so its expiry check stays (round-153 #2). Expiry while WAITING emits, for
    // every pending call with deadline_at <= now, a Slot whose outcome is
    // Err(Expired) and whose elapsed is now − fanout_entry_at (outcome class
    // (c): it measures the wait), removes them, and re-enters the loop.
    // Acquisition runs the INCLUSIVE recheck first (drop the lease if any
    // pending call is due), then:
    // Round-154 #6 — the acquisition is RACED against `earliest` on
    // targets with a real host timer (Axum, Cloudflare, Spin), so a
    // wave that cannot get permits before the earliest deadline
    // emits its positional timeouts instead of waiting. On FASTLY
    // there is no guest-visible sleep between dispatch and harvest,
    // so this wait is BestEffort: a bare `.await` cannot re-read the
    // clock until a permit frees it, and the wait can therefore
    // overrun a slot deadline. Fastly's loop instead re-checks the
    // clock on every permit wake (bounded iterations, no busy spin)
    // and declares the limitation in its capability row — it is a
    // documented BestEffort gap, not an enforced deadline.
    // ROUND-156 #1 — THE OUTER CANCELLATION RACE IS RETIRED. The
    // round-155 #7 design raced this future against a timer at
    // `max(deadline_at)` and, on timer win, DROPPED it and
    // synthesized `Expired` for every member. That is unimplementable
    // against the real API and self-contradictory: upstream
    // the batch handle yields each slot as it resolves, so a slot
    // completes, so dropping it destroys every result the batch had
    // already computed — slots that answered successfully before a
    // slow sibling, local preflight failures the facade recorded, and
    // each one's own `elapsed` and error class. It would do exactly
    // what round-153 #2 forbids: let a slow sibling convert an
    // on-time bidder into a timeout, and worse, for the entire wave.
    //
    // ROUND-174 #1 — WHERE CANCELLATION IS BESTEFFORT: on handler
    // teardown the batch is dropped and Cloudflare's adapter issues
    // `AbortController::abort()` per Fetch. That abort is REQUESTED,
    // not proven finite — `outbound-batch-cancellation` is BestEffort
    // on Cloudflare, Fastly and Spin (Native only on Axum) — so
    // auctioneer guarantees only its OWN half: nothing late is published
    // as on-time and the wave lease releases. Origin-side termination
    // is never asserted.
    //
    // THE CONTRACT INSTEAD: the wave future is AWAITED TO COMPLETION.
    // Per-request expiry is EdgeZero's own job — every
    // `OutboundRequest` carries `Deadline::at_instant(composed_at)`,
    // and the adapter expires that slot (Cloudflare aborts its Fetch)
    // while its siblings keep running. Each slot's outcome, expired
    // or not, comes back in the vector and is TERMINAL. Auctioneer adds
    // no batch-level timer.
    //
    // WHAT STILL CANCELS THE BATCH: handler teardown only — the
    // client went away or the auction future was dropped, so nothing
    // in this wave has a consumer. Dropping the future then loses
    // nothing anyone can observe, and Cloudflare's adapter REQUESTS
    // an abort for each in-flight Fetch — requested, not proven
    // finite: `outbound-batch-cancellation` is BestEffort there
    // (round-175 #1 corrects "aborts each in-flight Fetch", which
    // read as a guarantee). Pinned by
    // `cf_besteffort_cancellation_suppresses_late_results_and_releases_lease`
    // (renamed from the retired `cf_native_…`),
    // which asserts only auctioneer-owned outcomes. The wave lease
    // releases by RAII on either path.
    //
    // CONSEQUENCE ON BESTEFFORT TARGETS — round-175 #1 adds
    // CLOUDFLARE, which this list omitted: at head Cloudflare
    // reports BOTH `outbound-deadlines` AND
    // `outbound-batch-cancellation` as BestEffort, so it belongs
    // here beside Fastly and Spin. Only AXUM is Native for
    // cancellation. (Cloudflare, Fastly, Spin): a host that
    // does not honor the per-request deadline can hold the batch
    // past `max(deadline_at)`. Auctioneer does NOT cut it short — it
    // reports each slot as the host returned it, with auctioneer's stored
    // cause for classification, and declares the overrun in the
    // target's capability row (`outbound-deadlines: BestEffort`).
    // Trading a whole wave's real results for a synthetic timeout
    // would be strictly worse. Closing the gap needs an upstream
    // batch handle that can cancel PENDING slots while collecting
    // COMPLETED ones; that ask is recorded as feedback G17, and no
    // auctioneer task waits on it.
    let lease = gate.acquire_wave(k).await;          // RAII
    let batch_entry_at = clock.now_monotonic();
    let wave: Vec<_> = pending.drain(..k).collect(); // FIRST k, input order
    run_wave(wave, &mut out, batch_entry_at, clock, &http, &limits, yielder).await?;
    drop(lease);                                     // wave complete: batch + decode
}
// PUBLICATION (round-153 #2: NO recheck here). Every slot's outcome is
// already terminal — upstream reported its own, auctioneer's decode
// checked its own on a refreshed clock — and publication happens once
// for the whole auction, so a clock read here would blame the last
// wave's duration on the first wave's fastest bidder.
let mut results: Vec<BidderCallResult> = Vec::with_capacity(total);
for entry in out {
    let Some(Slot { result, deadline, .. }) = entry else {
        return Err(FanoutInternalError::BatchContractViolation { // an unfilled
            expected: total,                                      // index is the
            got: results.len(),                                   // same contract
        });                                                       // breach
    };
    results.push(BidderCallResult {
        elapsed: result.elapsed,
        result: classify(result.outcome, deadline.selected_cause),
        // classify is EXHAUSTIVE over BoundedSlotError (round-150 #3 /
        // round-152 #1):
        //   Err(Expired)                        → Timeout(selected_cause)   // auctioneer's DECODE clock
        //   Err(Preflight)                      → Transport(TransportError { kind: Preflight })
        //   Err(Edge(GatewayTimeout { .. }))    → Timeout(selected_cause)   // auctioneer's cause
        //   Err(Edge(other))                    → the plan's round-130/147 mapping table
        //   Ok(response)                        → Ok(BidderHttpResponse)
    });
}
Ok(results)
```

Properties the exchange relies on:

- **`start_batch_until(Vec<OutboundRequest>, Deadline) -> OutboundBatch`** —
  polled with `next()` until `Finished`,
  dispatch-before-harvest on every runtime **including Fastly**: the
  guarantee is that every SUCCESSFUL dispatch call completes before the
  first harvest begins (round-152 #5 — NOT that all requests are
  simultaneously in flight: dispatch is sequential, and an early request
  may finish before a later one is dispatched; overlap is the provider's
  timing, never a auctioneer claim); Fastly then harvests serially in input
  order, so its per-slot `elapsed` is observation-time; supersedes the old
  `ProxyClient::send` + `wait()` serialization problem.
  **Bounded concurrency (round-43 B1, REVISED round-105 B8).** M3b's
  StreamGate caps simultaneously-live body streams per isolate at the
  ENVELOPE-DERIVED `max_active_body_streams`. **Round-153 #7 — every
  per-target NUMBER below is ILLUSTRATIVE, not a certified default:
  the M3b plan's 2a.FINAL gates all four target envelopes on the §8
  native measurements and on the Cloudflare table's pending
  rederivation, so these figures show the SHAPE of the derivation and
  will be replaced by the measured values.** The fixed "6 on
  Cloudflare" promise is retired: with GuestDecoded per-stream charges
  (~6 MB each) six streams cannot fit the 128 MB isolate alongside the
  GVL and auction terms, so the CF worked row derives **2**
  (concurrent, never serialized — the M3b plan's worked table funds it
  by lowering the CF default `max_calls` to 12; Cloudflare's own
  6-connection window remains the PLATFORM ceiling the envelope value
  may never exceed) — and 6 on
  Fastly/Spin (pre-correction figures — their rows re-derive), and
  4 on Axum (illustrative likewise — round-140 #1: upstream's audited
  16 MiB fixed brotli charge puts per_stream at ≈ 20.77 MiB, funding 4
  streams at max_calls 64; the round-129 figure of 16 rode the undercharged
  model and is retired, as the 32 before it was). Round-123 #14 corrects the queueing claim:
  the upstream batch is ONE atomic dispatch and queues NOTHING inside
  auctioneer's gate. Bounded concurrency is auctioneer's own WAVE-LEASE
  scheduler (M3b plan §3, round-123 #4): `StreamGate::acquire_wave(k)`
  grants k permits all-or-nothing FIFO (no partial hold, so no
  hold-and-wait deadlock; no gate bypass, so the cap is real), the
  fanout dispatches exactly one batch of k per lease, and
  waves repeat until the call list drains. Per-call deadlines are
  ABSOLUTE instants composed once at FANOUT ENTRY (round-156 #4 —
  `compose_all(calls, auction, fanout_entry_at)` above; the retired
  "first-wave batch-entry" wording was a second anchor that
  would extend a starved call's deadline past gate contention), never
  re-anchored, so
  a call starved into a late wave times out against the SAME composed
  `min(bidder, auction)` instant it always had — the surfaced cause is
  whichever source won AT COMPOSITION, equality ⇒ `Bidder` (round-129
  #8 aligns this sentence with the M3a Task 9.5 rule; the earlier
  blanket `Timeout(AuctionDeadline)` claim is retired). This is bounded concurrency, not serialization. (Rounds 41-42 briefly
  labelled CF/Fastly/Spin `M3b-partial` on the belief that a 24 MiB
  decoder reservation forced 1-stream admission; round-43 B1
  established that reservation was a self-imposed Brotli-window
  policy rather than a platform constraint, and the label is
  withdrawn.)
- **Positional alignment** — `results[i]` corresponds to `calls[i]`.
- **Partial-failure isolation** — each `OutboundSlotResult` is independently
  a success or an error; one bidder's 5xx / timeout never fails the
  others, and (round-149 #2) a slot that fails auctioneer's own preflight
  conversion never prevents its siblings from dispatching.
- **Builder-carried limits** — the per-call composed absolute
  `.deadline()` (never `.timeout()` — round-137 #3), the request-body
  cap, and BOTH response caps plus the header/brotli/chunk limits ride
  on each `OutboundRequest` (the conversion above sets every one), so
  tmax enforcement and response-size bounding are handled by the
  primitive (§5.3). Round-136 #2 / G5 LANDED: EdgeZero selects and
  enforces the governing response cap after response headers reveal the
  encoding; auctioneer cannot choose it at request build, and the singular
  `.max_response_bytes()` is only buffered mode's final collection
  limit, set to `max(transport, decoded)`.

**`Sec-GPC` propagation.** Outbound `Sec-GPC: 1` fires iff
`regs.ext.gpc == "1"` after `prepare()`. Because `prepare()` **always
overwrites** `regs.ext.gpc` with `"1"` when the inbound `Sec-GPC: 1`
header was present (round-31 H1), a header value reliably propagates
regardless of any conflicting body value. The JSON-only case (no header
but `regs.ext.gpc == "1"`) also forwards — recorded as an intentional
divergence in §15. The header is added in the exchange's `BidderCall`
construction; adapters do not need to set it.

**Insulation seam.** The exchange uses a `auctioneer-exchange`-owned trait,
`BidderFanout`, in auctioneer's own neutral types so the auction logic and the
`Bidder` adapters never depend on EdgeZero types and tests inject a mock:

```rust
struct BidderCall {
    request:             BidderHttpRequest,   // ALREADY finalized (gzip +
                                              // Sec-GPC applied by the
                                              // exchange, §4.1) — round-150
                                              // #6: no `endpoint_compression`
                                              // field; its only consumer was
                                              // the retired in-drive gzip, and
                                              // the exchange reads
                                              // `BidderInfo.endpoint_compression`
                                              // at finalize time BEFORE this
                                              // struct exists (the M3a field
                                              // is deleted in M3b-2b)
    timeout:             Duration,            // per-bidder; the FANOUT composes
                                              // it with the auction deadline
                                              // into the facade's
                                              // `ComposedDeadline { deadline_at,
                                              // selected_cause }` at wave entry
                                              // (round-150 #2)
    max_response_bytes:  usize,               // Current per-call decoded cap.
                                              // The wave scheduler copies
                                              // ResponseBodyPolicy from boot
                                              // limits, then narrows decoded
                                              // bytes by this value. Transport,
                                              // header, and decoder ceilings
                                              // remain the installed limits.
}
enum Compression { Gzip }                     // consumed by `finalize_for_transport`
                                              // (§4.1 `TransportFinalize.compression`)

/// Implementation distinction (M4 integration): independent transport and
/// decoded response-cap overlays are NOT current runtime configuration fields.
/// The implemented call shape is above; `narrow_to_call` in the wave scheduler
/// lowers the installed decoded cap while leaving its transport cap unchanged.
///
/// Retained design requirement from rounds 30-34, not delivered by M4:
/// independent caps flow from baked host defaults through runtime host,
/// account, and per-bidder overlays into AuctionPlan.budgets: ResolvedBudgets,
/// then a per-call ResponseBodyPolicy. Each configured cap must be in
/// 1 KiB..=100 MiB, with the legacy response cap supplying both defaults
/// when neither is overridden. This requires explicit configuration/API
/// implementation and tests; operators cannot use those proposed overlay
/// keys in the current build. No unbounded transport is permitted.

/// Whole-auction deadline: anchored to `request_start` (§5.1 step 1) plus the
/// effective tmax budget (§5.3). Carrying an **absolute monotonic instant**
/// rather than a relative budget means preprocessing time is automatically
/// counted — fan-out reads `deadline_at.saturating_duration_since(clock
/// .now_monotonic())` for remaining time and hands `deadline_at` itself to
/// `Deadline::at_instant`. `source` records how the underlying budget was
/// derived. Round-148 #1: `deadline_at` is a `MonotonicInstant`
/// (`web_time::Instant`, EdgeZero's own instant type) — the M2/M3a
/// `Duration` field is retired by the plan's M3b-2b.0 migration.
struct AuctionDeadline { deadline_at: MonotonicInstant, source: TmaxSource }
enum   TmaxSource { Request, HostDefault, CappedByHost }

/// Per-call result from the outbound batch. `elapsed` is monotonic call duration —
/// fast bidders carry their own time; slow bidders do not poison the batch
/// (true completion time on Axum/Cloudflare/Spin; observation time on
/// Fastly, written to `responsetimemillis` as-is — round-148 #6, §5.7).
/// Round-136 #8 — the semantics are OUTCOME-CLASSED, and the
/// discontinuity is deliberate: for DISPATCHED calls (Ok or Err),
/// `elapsed` is the per-slot upstream time (G15) — StreamGate queue
/// wait is EXCLUDED (attributed to no bidder), and passthrough decode
/// time is added locally for Ok slots; for a call that EXPIRES
/// BEFORE DISPATCH, `elapsed` is the synthesized wait
/// (now − fanout_entry_at at emission). The M3b plan's facade block
/// defines all four outcome classes.
struct BidderCallResult {
    result:  Result<BidderHttpResponse, FanoutError>,
    elapsed: Duration,
}

/// Per-call failure. Maps to `ext.errors[bidder]` codes (Appendix G) and to
/// `seatnonbid` statuscodes (Appendix E.6) per upstream's reference no-bid reason mapping.
///
/// **Round-26 H4 correction**: `TimeoutCause` ships in **M3a** Task 11,
/// not M3b. M3a's Axum `EdgeZeroFanout` needs the discriminator to raise
/// the global `ext.errors.prebid` code-12 entry when the whole-auction
/// deadline elapsed (spec §5.2 line 1309). M2 callers see the plain
/// `Timeout` variant; from M3a on the wire enum is
/// `Timeout(TimeoutCause::Bidder | TimeoutCause::AuctionDeadline)`. Pre-M3a
/// crates that pattern-match on `FanoutError::Timeout` will need to be
/// updated by Task 11.
/// Round-39 H3 canonical shape (retires the round-17 M5
/// draft which listed only `Unreachable` as a bare variant
/// and had `OversizeResponse` unstructured). M3b's typed
/// error surface splits transport errors into a
/// `TransportError { kind: TransportErrorKind }` struct so
/// classifier variants can vary independently from wire
/// codes; the round-39 B7 fix additionally forbids Debug
/// derivation to prevent internal-detail leak into
/// `ext.errors`. See M3b Task M3b-2 "Typed error surface"
/// for the full shape.
enum FanoutError {
    Timeout(TimeoutCause),                          // code 1   → seatnonbid 101 (M3a Task 11)
    OversizeResponse { source: OversizeSource },    // code 4   → seatnonbid 100 (round-39 H3: structured)
    BadServerResponse,                              // code 4   → seatnonbid 100
    Other(String),                                  // code 999 → seatnonbid 100 (uncategorised)
    Transport(TransportError),                      // varies — see TransportErrorKind mapping
    UnsupportedContentEncoding { actual: String },  // code 4   → seatnonbid 100 (M3b M4)
    /// Round-101 H10 / round-102 B6 — stage + ownership, ONE contract:
    /// `Raw` = crossed `max_outbound_raw_bytes` DURING construction
    /// (`CappedWriter`, pre-fanout): an ADAPTER-SIDE preparation
    /// failure recorded per-bidder in `ext.errors[bidder]` (code 5)
    /// BEFORE fanout, like a `make_requests` failure — never a
    /// positional `BidderCallResult` slot. `Encoded` = the FINAL
    /// encoded body crossed `max_outbound_request_bytes` in
    /// `finalize_for_transport` at the exchange's finalize step
    /// (round-108 B2 — M3b retires the in-drive
    /// `prepare_outbound_request`) — a PER-CALL
    /// error in the positional results. Multi-request bidders are
    /// PARTIAL by design: each call fails or proceeds independently.
    /// (`enum OversizeRequestStage { Raw, Encoded }` — defined beside
    /// this enum.)
    OversizeRequest { stage: OversizeRequestStage }, // code 5 → seatnonbid 100
                                                     // (Raw: pre-fanout per-bidder;
                                                     //  Encoded: positional per-call)
    // Round-102 B6: the enum CONTINUES — the earlier split into a
    // separate `_FanoutErrorContinued` was a doc artifact; these are
    // variants of FanoutError itself:
    TooManyEncodingLayers { actual: usize },        // code 4   → seatnonbid 100 (M3b M4)
    DecoderFailure { layer: ContentCoding, source: String },  // code 4 → seatnonbid 100
}
enum OversizeSource { Transport, Decoded,
    Headers }  // round-134 #6 — response initial/1xx/trailer
               // field-block over-limit (the plan's round-114 B1
               // classification); code 4 → seatnonbid 100 like the
               // body variants
enum TransportErrorKind {
    Unreachable,                          // code 999 → seatnonbid 103
    Initial,                              // code 4   → seatnonbid 100
    MidStream { decoded_bytes: u64 },     // code 4   → seatnonbid 100
    ChunkTooLarge { observed: u64, limit: u64 },  // code 4 → seatnonbid 100
    Preflight,                            // code 4   → seatnonbid 100 — round-150 #3:
                                          // auctioneer's own transport conversion
                                          // rejected the request (method / URI /
                                          // header at the http layer); nothing sent
}
// Adapter-side make_requests failure (not a fan-out concern) → code 5
// (FailedToRequestBids). Note: seatnonbid 300 (Rejected-General) is reserved
// for actual bid-response rejections (DSA, creative-size, secure) — not
// transport failures. Phase 1 only emits 100/101/103 from the fan-out.

/// Batch-level failure: the fan-out itself could not run (e.g. no outbound
/// HTTP capability available). Distinct from per-call FanoutError, which is
/// non-fatal to the batch.
enum FanoutInternalError {
    NoOutboundCapability,   // ctx.http_client() returned None
    /// M3b round-129 #2: the batch facade violated its positional
    /// contract — `send_bounded_all` returned `EmptyBatch` for a batch
    /// the fanout built non-empty, or `Ok(out)` with
    /// `out.len() != calls.len()`. Batch-fatal like the other
    /// variants: the exchange maps to `AuctionError::FanoutUnavailable`
    /// and the caller emits `500`.
    BatchContractViolation { expected: usize, got: usize },
    Internal(String),
}

#[async_trait(?Send)]     // dyn-safe: the exchange holds &dyn BidderFanout
trait BidderFanout: Send + Sync {
    /// Contract:
    /// - `Ok(results)` ⇒ `results.len() == calls.len()` and each `results[i]`
    ///   corresponds positionally to `calls[i]` (no reordering, no drops).
    /// - `Err(_)` ⇒ no per-call results are produced; the exchange maps to
    ///   `AuctionError::FanoutUnavailable` and the caller emits `500`.
    async fn send_all(&self, calls: Vec<BidderCall>, deadline: AuctionDeadline)
        -> Result<Vec<BidderCallResult>, FanoutInternalError>;
}
```

`BidderFanout` is used as `&dyn BidderFanout`, so it carries
`#[async_trait(?Send)]` — native `async fn` in traits is not object-safe, and
the futures are `?Send` (§1.2). `Bidder`, `Clock`, and `Rng` are synchronous
and need no such attribute. `auctioneer-exchange` therefore depends on the
`async-trait` crate — a build-time proc-macro, not an EdgeZero or
runtime-specific dependency, so the crate stays pure (§2.1).

**Body-source API (round-40 H4, re-based round-130 #7).** The M3b
fan-out transport is BUFFERED: facade `Ok` values carry the complete
collected body under `ResponseMode::Buffered { max_bytes }`, and
per-call failures are each slot's typed `Err` (the earlier
"stream items are typed `Result<Bytes, TypedBodyError>`" phrasing
described the round-36 requirements shape, which the M3b plan's
alignment section now maps onto upstream's buffered vocabulary;
streaming survives only on non-fanout raw-body paths).

**Consolidated M3b facade facts (round-127, replacing scattered
amendments):** `ProxyClientApi` — with single-call `send_bounded` and
the positional per-call-policy batch `send_bounded_all` — is the
AUCTIONEER-OWNED facade trait (implemented by
`auctioneer_core::outbound::BoundedFetch` over EdgeZero's client); the
sink constructor is the three-argument
`OutboundRequestSink::new(limits, call_budget, endpoint_validator)`;
the per-stream charge is transport + decoded +
max(auctioneer's passthrough decoder chain, the adapter-reported
`guest_decoder_bound_bytes`) — round-134 #6 replaces the per-mode
charging: one mode cannot describe an adapter that guest-decodes
gzip while auctioneer decodes deflate/stacked, and the max covers every
disposition either way;
`Rejection` bodies render into the 256-byte static-class buffer, never
an unbounded `String`. The static body-source capability description
is a AUCTIONEER-SIDE per-adapter constant table (round-140 #3: no
`supported_body_sources()` method exists upstream — the historic
name survives only in requirements records) — there is NO
per-request body-source negotiation (round-133 #6 retired the
strategy selection; the response's actual disposition is read from
retained `Content-Encoding`, round-139 #3). Canonical three-way shape (round-39 B8, round-40
B2 — retires the two-way `RawCompressed | RuntimeDecoded`
of earlier rounds):

```rust
enum BodySource {
    RawCompressed,               // auctioneer runs its own decoder pipeline
    GuestDecoded,                // adapter runs decoder inside the guest;
                                 //   admission charges per_stream_bytes(..) per stream
    HostDecoded,                 // decoder lives outside the guest (currently
                                 //   no adapter honours this; reserved)
}

// Adapter capability map (round-40 B2 measured):
//   Cloudflare → &[RawCompressed, GuestDecoded]     (round-144 #3: bare gzip/br
//                                                    decoded in the guest; unknown /
//                                                    parameterized / repeated /
//                                                    stacked encodings pass through
//                                                    RAW and auctioneer decodes them —
//                                                    guest per pinned proxy.rs:137)
//   Fastly     → &[RawCompressed, GuestDecoded]
//   Spin       → &[RawCompressed, GuestDecoded]
//   Axum       → &[RawCompressed, GuestDecoded]
```

The admission gate memory formula charges the canonical
FIVE-param `per_stream_bytes(max_transport, max_decoded,
max_brotli_wbits, max_brotli_layers, guest_decoder_bound_bytes)`
(round-107 H6 / round-134 #6 — the M3b-2 signature: the mode
parameter is RETIRED, the layer count is an explicit input, and
`guest_decoder_bound_bytes: u64` is the adapter-reported in-guest
bound, 0 where none exists) per active stream. Every stream charges
the transport+decoded buffers; the DECODER-STATE component is the
checked max(auctioneer passthrough chain, guest bound) (round-46 H2:
the fixed `MAX_DECODER_STATE = 24 MiB` constant is retired —
per-stream cost derives from upstream's audited 16 MiB fixed
decoder charge plus the WBITS-22 ring, `MAX_BROTLI_LAYERS = 1` —
≈ 20.77 MiB per stream (round-140 #1: the earlier window+512 KiB
model undercharged by ~16 MiB), not a flat 24 MiB).
Full mechanics live in M3b Task M3b-2, whose admission /
memory-proof track follows the M3b plan's **round-98 design**
(try-acquire `AdmissionGate` + isolate-wide `StreamGate` bounded
waves + one validated `ResolvedIsolateLimits` charging decoder state PER
ACTIVE STREAM — the per-stream accounting this section requires);
this section carries the canonical enum so implementers don't
reinvent it from skimming the plans.

**Correlation (review finding 8).** At the `BidderFanout` boundary,
`results[i]` corresponds to `calls[i]`. The exchange's `PreparedSlot` list is
broader: each finalization attempt retains its bidder identity,
`Arc<OutboundRequestMeta>`, and an optional synthetic rejection. Only
successfully finalized requests enter `calls`; their bodies move into
transport ownership. Result processing walks the full slot list in order,
using a slot's synthetic rejection or consuming the next transport result.
It must not directly index transport results by the full slot index after
an encoded rejection. `make_bids` receives the correlated metadata, never a
retained request body. Raw sink failures remain pre-fanout errors and create
no slot. No call IDs are needed.

**`responsetimemillis[bidder]` aggregation** (round-15 finding H2 — fixes
inflation under one global batch). Wrapping the global batch await
would charge fast bidders for the slowest bidder's wall time. Instead, the
per-bidder metric is composed from sources that are _independent of the
batch_:

```rust
// Per bidder, around adapter CPU only:
let t_mr = clock.now_monotonic();
// One auction-global call budget is shared by all per-bidder sinks.
// Take the outcome on both arms; semantic failure discards accepted calls
// and refunds their reservations rather than committing them.
let mut sink = OutboundRequestSink::new(limits, &call_budget, &validator);
let outcome = match bidder.make_requests(&req, &info, &mut sink) {
    Ok(()) => sink.into_outcome(),
    Err(errors) => {
        record_semantic(errors);
        sink.into_outcome_discarding_accepted()
    }
};
let mr_elapsed = clock.now_monotonic().saturating_duration_since(t_mr);
// Finalize accepted requests, retain only Arc<OutboundRequestMeta> for
// correlation, and move their bodies into the global batch.

// After the batch finishes, for this bidder:
let max_call = results_for(&bidder_name)
    .iter().map(|r| r.elapsed).max().unwrap_or(Duration::ZERO);
                              // BidderCallResult.elapsed is per-call (§5.2 trait),
                              // measured by the runtime — independent of when
                              // the batch as a whole returned.

let t_mb = clock.now_monotonic();
// For each successful response and its correlated sent metadata:
let bids = bidder.make_bids(&req, sent_metadata.as_ref(), &response)?;
let mb_elapsed = clock.now_monotonic().saturating_duration_since(t_mb);

let elapsed = mr_elapsed + max_call + mb_elapsed;
response.ext.responsetimemillis.insert(bidder_name, elapsed.as_millis() as u32);
```

This composes the bidder's own adapter CPU (`make_requests` + `make_bids`)
with **the bidder's longest individual HTTP call** — not the global batch
wall time — so a fast bidder is not penalized by a slow neighbour in the
same batch.

**Approximation vs upstream (round-16 finding H2).** upstream processes each
HTTP response as it arrives, potentially running bid response conversion on call 1's
response while call 2 is still in flight (reference bidder request/response processing). Under that
streaming model, work overlaps with in-flight calls; total elapsed can be
slightly less than `max_call + make_bids`. auctioneer's post-batch synchronous
model **deliberately over-counts that overlap** by a small amount —
typically within milliseconds since `make_bids` is fast. **Listed as an
intentional divergence (§15).**

**Note: this depends on `BidderCallResult.elapsed` being _per-call_
completion time** — a hard requirement on the EdgeZero fan-out primitive
FOR THE TARGETS WHOSE SLOT ISOLATION IS NATIVE (Axum, Cloudflare, Spin),
recorded as part of the conformance suite, §11.2. Round-148 #6 — Fastly
is SCOPED OUT of that requirement, by design, not by gap: Fastly's
the batch dispatches every slot before the first harvest but harvests
serially in input order, so a completed slot may be observed late.
On Fastly `elapsed` is OBSERVATION time (the plan's `elapsed_observed`
diagnostic name), it IS written to `responsetimemillis[<bidder>]`
as-is — an upper bound on the bidder's true completion time, never an
under-count, and the wire field has no way to say "degraded" — and
the capability row plus Fastly deployment documentation declare that
meaning. There is no cross-target degraded fallback: each target
reports its own semantic and nothing reports one as the other.

**Binding.** The **production** `BidderFanout` impl lives in `auctioneer-core` and
is the only code that touches the EdgeZero fan-out API: it maps `BidderCall`
→ `OutboundRequest`, drives the batch handle, maps `OutboundSlotResult` back.
`auctioneer-exchange` itself stays EdgeZero-free.

**The M2/M3a interim `BidderFanout` (`ProxyClient` + `join_all`) is
HISTORICAL** (round-150 #8): its "known semantic gaps" list is moved
to the M3b historical appendix §9, because the native primitive is
PUBLISHED at the pinned EdgeZero (`120dd2aa`: `send_all`,
`Deadline::at_instant`, dual response caps, per-slot elapsed) and the
production `EdgeZeroFanout` in `auctioneer-core` is the one path M3b
implements. What survives of that section is the normative two-budget
contract below.

**Two independent response budgets (round-28 H4; standalone
since round-150 #8).** The historic `max_bidder_response_bytes`
is a single value covering "the response body auctioneer consumes,"
which conflates transport bytes with decoded bytes. Under M3b,
the BOUNDED COLLECT honours TWO orthogonal limits (round-134 #6
— enforced DURING the adapter's read loop per the
bounded-buffering contract, never collect-then-check):

- `max_transport_bytes` (compressed / on-wire) — defaults to
  `max_bidder_response_bytes`. Cap that the reader tracks
  against bytes-read-from-socket; exceeding it →
  `FanoutError::OversizeResponse { source:
OversizeSource::Transport }`.
- `max_decoded_bytes` (decoded output) — defaults to
  `max_bidder_response_bytes`. Cap that the reader tracks
  against decoded output after gzip / brotli / deflate /
  identity; exceeding it → `FanoutError::OversizeResponse
{ source: OversizeSource::Decoded }`.

Both bounds are observed to within one `MAX_CHUNK_SIZE`
(default 8 KiB), so worst-case memory during a single bidder
call is bounded by (round-39 H3 canonical formula — retires
round-35 H5 which conflated baseline with a full decoded
buffer, and round-37 H3 which used a subtracted baseline):

```
// baseline_empty = allocator peak on a NO-response run.
// No baseline term is ADDED to the bound; the check uses
// the delta.
peak.checked_sub(baseline_empty)
    <= per_stream_bytes(max_transport_bytes, max_decoded_bytes,
                        max_brotli_wbits, max_brotli_layers, 0)
// round-134 #4: the canonical FIVE-parameter max-rule form —
// the mode parameter is retired; guest bound 0 here because the
// allocator harness measures auctioneer's own pipeline (the chain
// arm). The earlier DecodeMode spelling is superseded.
```

where `per_stream_bytes(..)` is the single canonical helper
(round-46 H2 — the bound is that call directly, not a
hand-written `+ LAYERS * MAX_CHUNK_SIZE + MAX_DECODER_STATE`
sum, which double-counted chunk staging and used a retired
constant). It accounts for `LAYERS_MAX = 3`, the
`MAX_BROTLI_LAYERS = 1` brotli state at the WBITS-22 default
window (≈ 20 MiB: upstream's audited 16 MiB fixed decoder
charge + the 4 MiB ring — round-141 #3 removes the stale
4.5 MiB figure here; the round-33 "24 MiB flat" figure stays
retired for a different reason: it was a guess, this is an
audit), `flate2` state for
the remaining layers, and chunk staging — ≈ 20.77 MiB total at
the shipped 256 KiB budgets. A deployer raising
`max_brotli_wbits` toward 24 trades concurrency for a larger
window, visible at boot. M3b Task M3b-7 runs the worst-case
allocator harness on the RawCompressed path (Axum, native —
the one target where a counting global allocator is reliable);
GuestDecoded adapters (Cloudflare, and Fastly/Spin when they
negotiate GuestDecoded) decode OUTSIDE auctioneer's allocator, so
their decoder bound is the adapter-reported Prereq-8 value, not
a auctioneer allocator assertion — Cloudflare is GuestDecoded, NOT
HostDecoded (round-46 H2 corrects the stale "Cloudflare
HostDecoded only" line here). `Content-Encoding` values auctioneer supports
(gzip / br / deflate / identity + absent header treated as
identity); unknown values → `FanoutError::Unsupported
   ContentEncoding`; multi-layer chains apply decoders
**right-to-left** per RFC 9110 §8.4 — the header lists codings
in the order the sender applied them, so the receiver must
reverse that stack. Example: `Content-Encoding: gzip, deflate`
means the sender gzip-ped first, then deflate-ped; the
receiver decodes deflate FIRST (undo the last-applied coding),
then gzip. Upper bound of 3 layers
(`FanoutError::TooManyEncodingLayers` beyond that).
M3a Task 12 stays deferred to M3b; the M3b prereq 2 spells out
the full contract.

The production `BidderFanout` must pass the `BidderFanout` conformance
suite (§11) before it is considered behavior-complete (round-150 #8:
there is no interim path to swap out any more — see the historical
appendix §9).

### 5.3 Resource limits

The auction bounds its blast radius with hard limits. Body size caps are
**embedded constants** (the body must be bounded before host config can be
resolved); all other limits are host-config values. Defaults below are
advisory pending load testing (open item 10):

| Limit                        | Default                                  | Enforcement                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| ---------------------------- | ---------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `MAX_AUCTION_BODY`           | 256 KiB                                  | **embedded constant** (NOT host-config; round-20 finding M4 — the body is read before host config is resolved); `Body::into_bytes_bounded`; oversize → `400`                                                                                                                                                                                                                                                                                                                                                                          |
| `MAX_COOKIESYNC_BODY`        | 32 KiB                                   | **embedded constant** (same reason); bounded read; oversize → `400`                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| `max_bidder_response_bytes`  | 256 KiB                                  | Host-config value carried as `BidderCall.max_response_bytes`. The current wave scheduler narrows the boot policy's decoded ceiling by this cap; the transport ceiling remains the boot value. Independent host/account/bidder overlays for both response caps remain the explicit design requirement in §5.2, not supported runtime keys.                                                                                                                                                                                             |
| `max_outbound_raw_bytes`     | 256 KiB                                  | round-100 H6 / round-102 H13 / round-107 H6 — cap on an adapter-built outbound body, enforced DURING construction by the §4.1 `OutboundRequestSink` (`push_request` hands the adapter a `CappedWriter`; the earlier `OutboundLimits` parameter + `bounded_json_body` shape is superseded by the sink, which is the sole `BidderHttpRequest` constructor); over-cap → `OversizeRequest { stage: Raw }`, an adapter-side PRE-FANOUT per-bidder `ext.errors` entry (code 5), never a positional slot. Host-config value → boot envelope. |
| `max_outbound_request_bytes` | 256 KiB                                  | round-100 H6 — cap on the FINAL ENCODED outbound body (post-serialization + compression; gzip expansion counted); over-cap → per-call `FanoutError::OversizeRequest` (code 5). Host-config value → boot envelope.                                                                                                                                                                                                                                                                                                                     |
| `response_write_deadline_ms` | 10 000 ms                                | The current `EgressDeadline` is anchored at request admission/start, not egress entry; config and auction work consume its allowance before permit queueing, bounded serialization, and transmission. It is distinct from tmax. Host-config value, codegen range `100..=120_000`, NOT account/runtime-overlayable; carried on `ResolvedIsolateLimits`.                                                                                                                                                                                |
| `max_bidders_per_auction`    | 50                                       | applied after alias resolution, over bidders in registry order (§5.9 — lexicographic over base names ∪ request-defined alias names ∪ config alias names participating in this auction): the first N are kept; each bidder past N is dropped with a non-fatal warning per the contract below (round-72 M3)                                                                                                                                                                                                                             |
| `max_outbound_requests`      | 100                                      | cap on total `OutboundRequest`s, counted in §5.9 registry order then per-bidder request order. Round-106 B6: enforced AT CONSTRUCTION by the auction-global `CallBudget` shared by every per-bidder `OutboundRequestSink` (§4.1) — `push_request` past the budget returns `CallBudgetExhausted` before any body is built (the M2 materialize-then-drop truncation is retired); the bidder gets a non-fatal warning — the auction still proceeds (partial-bidder semantics below)                                                      |
| `bidder_timeout`             | per-bidder (`BidderInfo`)                | folded into the per-call composed absolute deadline (`min(fanout_entry_at + timeout, auction deadline_at)`) — NEVER `OutboundRequest::timeout()`, which re-anchors at dispatch (round-137 #3 / round-147 #1)                                                                                                                                                                                                                                                                                                                          |
| `tmax`                       | validated vs `tmax_max` / `tmax_default` | absolute monotonic deadline anchored at `request_start` (§5.1 step 1); the M3b fan-out composes one absolute `MonotonicInstant` per call on the application clock and passes `Deadline::at_instant(..)` (round-135 #5 retired the relative `after(remaining)` form; round-147 #1 retired the bridge/projection and the `Deadline::at` name); advisory only under the interim impl                                                                                                                                                     |

**`max_bidders_per_auction` warning contract** (round-72 M3 — was
missing while `max_outbound_requests` had a full contract right
below; the two share the same code and message-shape pattern):
Each bidder dropped past the cap is recorded under
`ext.warnings[<bidder>]` with **code 10999 `Unknown`** (same
catchall as `max_outbound_requests`; Appendix G) and message
**`bidder dropped: max_bidders_per_auction (N) exceeded`** where
`N` is the resolved cap value at the time of the request. The
warning is attributed to the **dropped bidder name** (the alias
or base name that was over the cap, per the §5.9 registry order
used for the cap). No `responsetimemillis[<bidder>]` entry is
recorded because the bidder never ran. No `seatnonbid[<imp>]`
entry is recorded either — the cap is a request-shaping decision,
not a bidder-side rejection. `ext.errors[<bidder>]` is untouched.
The cap-drop warning is emitted on **both** completion paths —
the normal post-fanout path AND the tmax-expired short-circuit
(`remaining == Duration::ZERO` after cap application). Skipping
the emission on the short-circuit would silently drop the cap
signal whenever tmax happens to expire before fanout starts.

**`max_outbound_requests` partial-bidder semantics** (round-14 finding M4;
message unified round-108 B1). For a multi-call bidder whose attempts are
only partially over the budget: the bidder **remains live with the calls
that fit**; the rejected attempts' imp ids do **not** produce `seatnonbid`
entries (the bidder was never asked); a single non-fatal warning is
recorded under `ext.warnings[<bidder>]` (code **10999** `Unknown`, message
EXACTLY "`N` outbound request attempt(s) rejected by
`max_outbound_requests`" where N = the sink's `rejected_call_count` — the
attempts actually made past the budget; an adapter that stops at its first
rejection yields N = 1. The pre-round-108 "truncated by" wording described
the retired materialize-then-drop enforcement and is replaced — this
string is THE one message, identical in the M3b plan); `responsetimemillis[<bidder>]` reflects only the
calls that ran. Bidders entirely beyond the cap are dropped with the same
warning shape but no `responsetimemillis` entry.

**Units.** OpenRTB `tmax` is **milliseconds** (per OpenRTB spec); host config
`tmax_default` / `tmax_max` are also milliseconds. auctioneer converts to
`Duration` via `Duration::from_millis(...)` at the resolution boundary.

**tmax has four distinct values; preprocessing time counts against the whole
auction:**

- **Request tmax** — `req.tmax` as received (milliseconds); may be absent or
  `0`.
- **Effective budget** — resolved from `req.tmax`: absent or `0` →
  `tmax_default` (`TmaxSource::HostDefault`); exceeds `tmax_max` → clamped to
  `tmax_max` (`TmaxSource::CappedByHost`); otherwise the request value
  (`TmaxSource::Request`). Always a positive `Duration`.
- **Whole-auction deadline** — `AuctionDeadline.deadline_at = request_start +
effective_budget` (a `MonotonicInstant`; round-154 #9: `checked_add`
  overflow is an INTERNAL ERROR → 500, never a saturation to
  `DEADLINE_FAR_FUTURE` — the same ONE policy the fan-out's per-call
  composition uses), where `request_start =
clock.now_monotonic()` is captured at handler entry (§5.1 step 1; under
  M3b it is `ctx.request_start()`, the same instant type). This **absolute
  monotonic instant** means preprocessing time (parse, validation, privacy,
  split, adapter request build) is counted against tmax along with the
  fan-out. The exchange's expired-budget short-circuit reads `remaining =
deadline_at.saturating_duration_since(clock.now_monotonic())`; the
  fan-out passes the composed per-call INSTANT itself to
  `OutboundRequest::deadline(Deadline::at_instant(..))` — never a
  reconstructed relative value (round-135 #5 / round-148 #1).
- **`ext.tmaxrequest`** in the response — `BidRequest.TMax` echoed back
  (matches upstream's the request timeout budget, reference auction processing).
  Because reference configuration defaults _defaults `tmax` when zero before
  the response is built_, an omitted request `tmax` is echoed as the
  resolved default (`tmax_default`), **not as `0`**. auctioneer applies the
  same convention: `ext.tmaxrequest` is the post-default request tmax
  in milliseconds (round-19 finding M4).

A small-but-positive budget is honored as-is — bidders that miss it produce
timeout signals on three distinct surfaces (Appendix G + Appendix E.6):

- `ext.errors[bidder]` — **code 1** (`Timeout`) for each bidder whose own
  per-bidder timeout / deadline elapsed.
- `ext.errors.prebid` — **code 12** (`TmaxTimeout`) once, when the
  whole-auction deadline (`AuctionDeadline.deadline_at`) elapsed (this is
  auction-wide and distinct from per-bidder).
- `ext.prebid.seatnonbid[...].statuscode` — **101** (`Error-Timeout`) for each
  imp the timed-out bidder would have bid on.

**How the three surfaces interact** (round-13 finding M4):

- **code 12 is an auction-wide marker, alone.** It is emitted at most once
  per auction, in `ext.errors.prebid`. It does **not** produce per-imp
  `seatnonbid` entries by itself — only code-1 entries map to statuscode
  101 (Appendix E.6). **Round-151 #1 — its ONE trigger is the exchange's
  own clock:** after the batch finishes, code 12 is emitted iff
  `clock.now_monotonic() >= auction_deadline.deadline_at`. A per-call
  `Timeout(TimeoutCause::AuctionDeadline)` is PROVENANCE (the auction
  budget was the smaller operand when that call's deadline was composed),
  never proof the auction deadline elapsed — on BestEffort targets a slot
  can carry that cause for an earlier platform-phase timeout while the
  auction still has budget, and upstream's `BudgetSource` is likewise
  provenance. Such a slot yields its per-bidder code 1 only.
- **code 1 is per-bidder.** When the auction-wide deadline elapses, every
  in-flight bidder produces a code-1 entry in its own
  `ext.errors[bidder]`; those code-1 entries are what generate
  `statuscode 101` `seatnonbid` rows.
- Per-bidder code-1 timeouts can also occur **independently** of a
  whole-auction expiry (the bidder's own `timeout` ran out while the
  auction still had budget); in that case code 12 is **not** emitted.

There is no separate "too small budget" rejection. If `remaining == 0`
**before fan-out** (budget already exhausted by preprocessing), the
**exchange short-circuits inside `run_auction` (§5.8) without calling
`BidderFanout`** — it emits code 12 once and a code-1 entry for every
bidder that would have been called (each treated as having a 0 ms slot),
which in turn produces the corresponding `statuscode 101` `seatnonbid`
rows (if `ext.prebid.returnallbidstatus` is set, Appendix E.6).
`BidderFanout::send_all` never sees a zero `remaining` budget.

### 5.4 Outbound safety / SSRF prevention

Applies to bidder fan-out (syncer-URL safety is §8.6).

- **Primary guarantee: every outbound host is build-time-fixed.** The set of
  hosts auctioneer will ever connect to is fully determined at build time — each
  bidder's endpoint host(s) come from `BidderInfo`, generated from vendored
  bidder-info TOML (§6). **No request, account, or `config_store` data can
  introduce, alter, or select a host outside that build-time set** (§9.1).
  This is the load-bearing defense: edge runtimes do not expose the
  DNS-resolved IP to the guest, so auctioneer cannot do post-resolution
  private-IP filtering — therefore the host set must be closed at build time.
- **URL construction is adapter-owned; validation is exchange-owned**
  (round-28 finding H2). The flow is:
  1. The adapter's `make_requests` constructs the final
     `BidderHttpRequest.url` using `BidderRequestInfo.endpoint_template`
     (round-31 L6 — it is a template, not the final URL) as the base
     (path/query templating + macro substitution per the rules below).
  2. The exchange runs `validate_endpoint(url, bidder_info)` on every
     `BidderHttpRequest` _after_ `make_requests` returns and _before_
     the call reaches the fan-out (§5.2). (M3b override — round-124
     #8: under the M3b sink, validation moves INSIDE the sink's
     reservation transaction via the exchange-owned
     `EndpointValidator` handle passed at sink construction, so an
     invalid endpoint never consumes a committed call slot; this
     post-`make_requests` pass describes the M2/M3a shape and remains
     the semantic authority for WHAT is validated.) **Round-118 F01 — the
     validated URL is the ONLY destination the transport may contact:
     every bounded transport (Prereqs 2-3) MUST disable automatic
     redirect following** (round-120 ADOPT — upstream's outbound design
     §3.1.4 "Redirects: not followed automatically" makes this the
     BASELINE on all four adapters and names the exact mechanics:
     reqwest `redirect::Policy::none()`; Workers
     `worker::RequestInit { redirect: worker::RequestRedirect::Manual }`
     — the ENUM, not the string `"manual"` this block previously named;
     Spin's hand-built `wasi:http` request and Fastly's SDK do not
     auto-follow, so no opt-out exists or is needed there), because
     default policies re-POST the body — and Workers forwards headers —
     to a 3xx target that never passed this validation. Round-119 F6 fixes the classification the round-118
     text got wrong: a RECEIVED 3xx is a RESPONSE-side failure —
     **code 4 `BadServerResponse` + seatnonbid 100**, matching the
     response-error model (code 5 stays pre-dispatch URL-validation's).
     This needs an explicit classifier change: the generic response
     classifier rejects only >= 500 today, so without a named 3xx arm
     a redirect would fall through to bid parsing. A 3xx body is NEVER
     parsed — the fixture set includes a 307 carrying valid-looking
     bid JSON, asserted rejected with the exact code-4/seatnonbid-100
     result. Conformance rows:
     302/307/308 from an allowed bidder with ZERO requests observed at
     the redirect target, per adapter. Deliberate redirect support, if
     ever wanted, needs per-hop validation + credential policy + shared
     budgets — not a default. Round-120 MATCH on the F6 half: upstream
     delivers a received 3xx as `Ok(OutboundResponse)` with `Location`
     preserved ("non-2xx is not an error" — outbound design §3.4.3
     table), so the code-4 / seatnonbid-100 classification of received
     3xx is auctioneer-side classifier work by upstream's own contract,
     exactly as this block already requires. A failure here drops that
     bidder with a per-bidder error code 5 `FailedToRequestBids`
     (adapter-side; Appendix G).
  3. `auctioneer-bidder` provides an `EndpointBuilder` helper that adapters
     use to safely percent-encode path/query macro values, look up
     `host_allowlist` membership, and assemble the final URL; using it
     is recommended but not type-enforced, since adapters need control
     over format-specific templating. `validate_endpoint` is the
     authoritative gate either way.
- **Macros never touch scheme or authority.** Scheme + host are taken
  verbatim from `BidderInfo.endpoint` (or, for host-varying endpoints,
  from a member of the build-time `host_allowlist`). Endpoint macros
  (`{{.PublisherID}}`, `{{.AccountID}}`, zone/placement ids from
  `imp.ext` params, region keys) may appear **only** in the path or
  query, are percent-encoded for their position by the adapter (with
  `EndpointBuilder` if used), and cannot inject a scheme, authority,
  or path traversal. `{{.AccountID}}` resolves to
  `AuctionContext.account.id` — the **`ResolvedAccount` identity from
  §5.8**, post-fallback. `{{.PublisherID}}` resolves to the raw
  `site.publisher.id` / `app.publisher.id` (whichever is present on
  the request); the two are intentionally distinct.
- **Macro syntax is a build-time gate, not a runtime one.**
  `auctioneer-config`'s `render_endpoint_macros` calls
  `endpoint_unclosed_macro_prefix` before macro extraction so an
  endpoint template with an unclosed `{{...` (no matching `}}`) fails
  codegen with a clear "unclosed template macro starting at …"
  message. A malformed template cannot bake into the wasm binary.
  The syncer path has the equivalent gate inside `MacroScan::Unclosed`.
- **`{{.Host}}` is authority-strict.** The token is defined as the
  endpoint's authority-substitution slot and MAY NOT appear in the
  path or query. `render_endpoint_macros` rejects any endpoint where
  `endpoint_has_host_macro_outside_authority` fires, and
  `validate_host_allowlist` uses
  `classify_endpoint_authority(endpoint) == HostVarying` as the
  host-varying discriminator instead of a substring scan. Without
  this rule, a template like `https://fixed.example/rtb/{{.Host}}`
  plus a declared `hostAllowlist` would misclassify a fixed-host
  bidder as host-varying and let the runtime splice a chosen host
  into the path (round-84 review Medium).
- **Host-varying endpoints** (e.g. regional hosts) declare every permitted
  host in a build-time `host_allowlist` in `BidderInfo`. The selected host
  must be a member; a value outside it is rejected. The allowlist is
  build-time data, not runtime-overridable (§9.1). Round-82 review
  Medium: adapters call the shared helper
  `auctioneer_bidder::adapters::url::build_outbound_url_with_host(info, host)`
  which invokes `EndpointBuilder::select_host` (the allowlist check)
  before macro expansion. Open-coding `select_host` per adapter would
  drift the guard as new host-varying bidders ship, so the shared
  helper is the only supported path — a fixed-host bidder (no
  `host_allowlist`) that calls the helper is rejected with
  `UnknownMacro("Host")`, and an off-allowlist host is rejected with
  `HostNotInAllowlist`. Both fall through to code-5
  `FailedToRequestBids` (Appendix G).
- **Secondary checks.** Resolved URLs must be `https://`; a host that is a
  private/loopback/link-local IP literal or `localhost` is rejected. These
  are defense-in-depth on top of the build-time host set — _not_ the primary
  defense (a configured hostname could still resolve to a private IP, which
  auctioneer cannot observe).
- **`allow_insecure` is compiled out of production builds.** The only way to
  reach an `http://` or loopback target is the cargo feature
  `insecure-bidders` (off by default, never enabled in release/production
  builds). The relaxation code is `#[cfg(feature = "insecure-bidders")]`-
  gated — in a default build it does not exist, so no runtime data (config,
  params, a stray `allow_insecure` field) can re-enable insecure
  destinations. Used only for local testing (e.g. Mocktioneer against
  `http://127.0.0.1`).

  **Feature ownership and propagation** (round-63 M2 + round-72 H2 —
  Cargo features are package-scoped, so the spec must name where the
  feature lives and how it reaches every gate that needs it. The
  workspace today has only `auctioneer-core` + four adapter crates per
  the root `Cargo.toml` — `auctioneer-bidder` / `auctioneer-exchange` /
  `auctioneer-privacy` / `auctioneer-openrtb` / `auctioneer-config` /
  `auctioneer-runtime` are M1 deliverables that haven't been added to
  the workspace yet, so the entire forwarding chain below is an
  **M2 wiring deliverable**, not something runnable today):
  - **Canonical declaration** (M2 deliverable): `auctioneer-bidder`
    owns the feature (`auctioneer-bidder/Cargo.toml` declares
    `insecure-bidders = []`). This crate owns `EndpointBuilder` +
    `validate_endpoint` (§5.4), which are the primary
    outbound-safety gates the feature relaxes.
  - **Forwarding features** propagate the flag to every other
    crate whose code needs the `#[cfg(feature =
"insecure-bidders")]` relaxation (all M2 wiring):
    - **`auctioneer-config/Cargo.toml`**: `insecure-bidders = []`
      (round-75 M1 — Cargo features are package-scoped, and
      `auctioneer-config`'s `build.rs` (§6) runs the syncer-URL
      template-validation pass; without this declaration the
      build script never sees `CARGO_FEATURE_INSECURE_BIDDERS`
      and the §8.6 feature-on codegen fixture cannot compile
      cleanly even though the runtime validators would allow
      localhost). The build script reads
      `env::var("CARGO_FEATURE_INSECURE_BIDDERS").is_ok()` and
      relaxes the `https://`-only check on syncer URLs and
      bidder endpoints with a localhost-only allowlist (same
      rule as §5.4 runtime; round-71 H1).
    - `auctioneer-bidder/Cargo.toml`: extend the canonical
      declaration to forward into config: `insecure-bidders =
["auctioneer-config/insecure-bidders"]` (round-75 M1 —
      auctioneer-bidder depends on auctioneer-config per the dep graph
      §2.1, so this forwarding chain reaches the build script).
    - `auctioneer-exchange/Cargo.toml`: `insecure-bidders = ["auctioneer-
bidder/insecure-bidders"]` (the exchange runs
      `validate_endpoint` per outbound, §5.2; the chain through
      auctioneer-bidder also reaches auctioneer-config).
    - `auctioneer-core/Cargo.toml`: `insecure-bidders = ["auctioneer-
bidder/insecure-bidders", "auctioneer-exchange/insecure-
bidders"]` (the sync URL validator §8.6 reuses the same
      scheme rule, so its gate must follow the same feature).
    - The four `auctioneer-adapter-*` crates each forward via
      `insecure-bidders = ["auctioneer-core/insecure-bidders"]` so a
      single `--features insecure-bidders` on `cargo build -p
auctioneer-adapter-<target>` reaches every gate, including
      auctioneer-config's build.rs.
  - **Current workspace state and runnable commands** (round-72 H2 +
    round-73 M2 — documents what works TODAY vs what becomes runnable
    when M2 wires the chain; the round-72 H2 claim "none of them
    carry `[features]`" was too broad — Cloudflare's, Fastly's, and
    Spin's adapter `Cargo.toml`s do declare target/runtime
    `[features]` blocks, just not `insecure-bidders`): the
    workspace today contains only `auctioneer-core` + four adapter
    crates per the root `Cargo.toml`. **None of those five packages
    declares the `insecure-bidders` feature** (each adapter's
    `[features]` block, where it exists, is for its target runtime
    only: `cloudflare`, `fastly`, `spin`). Consequently, **no
    local-test command with `--features insecure-bidders` is
    runnable today** — the package that will own the feature
    (`auctioneer-bidder`) has not been added to the workspace yet. The
    forwarding chain above (canonical declaration + forwarding
    through exchange, core, adapters) is a **M2 wiring
    deliverable**; the rounds 66/70/71 attempts to point the
    local-test command at `auctioneer-adapter-axum` / `auctioneer-core` /
    `auctioneer-bidder` were each wrong because none of those packages
    declare the `insecure-bidders` feature today. Until M2 adds `auctioneer-bidder` to the workspace with
    `insecure-bidders = []`, the local-test command is a
    placeholder — see CI commands below for the M2-and-later
    form.
  - **No workspace-wide feature aggregation**: Cargo's "workspace
    features" RFC is unstable and inconsistently supported across
    Compute@Edge / Spin toolchains. The forwarding chain above
    works on stable Cargo today.
  - **CI commands** (round-63 M2 + round-64 M3 — one off, one on,
    both enforced; the OFF check now uses `cargo build` so there
    are actual release artifacts to inspect — `cargo check`
    produces no rlib/wasm output for grep):
    - Production / default — **per-target release builds with the
      proper target triple AND each adapter's actual feature set**
      (round-64 M3 + round-65 M4 + round-66 H1 — corrects the
      earlier blanket `--features default`: not every adapter
      Cargo.toml declares a `default` feature, and the per-adapter
      `[features]` sets differ). The exact commands are:
      - `cargo build --release -p auctioneer-adapter-axum` (host
        triple; auctioneer-adapter-axum's Cargo.toml has no
        `[features]` block, so no flags are needed — round-66 H1).
      - `cargo build --release -p auctioneer-adapter-cloudflare
--target wasm32-unknown-unknown` (its default features
        include `cloudflare`, which is what the wasm32 build
        needs — round-62 M1).
      - `cargo build --release -p auctioneer-adapter-fastly
--target wasm32-wasip1` (uses the crate's own default
        features).
      - `cargo build --release -p auctioneer-adapter-spin
--target wasm32-wasip2` (uses the crate's own default
        features).
        All four must succeed against the workspace's actual
        adapter wrappers. **Gate progression** (round-66 H1 +
        round-70 M4 — corrects the earlier "wait for ingress hook"
        claim; wrapper conformance is M1.5 and has NO EdgeZero
        primitive dependency, while the ingress hook was IDENTIFIED
        during M2 design but its implementation/release owner is
        **M3b Task M3b-10** — production-completion only, and
        M3b-1..9 stay behavior-complete without it, round-79 M2):
      - **M1+ library `cargo check`** matrix (round-68 H1) runs
        on every PR from M1 onward; doesn't touch adapter
        wrappers.
      - **M1.5 adapter wrapper conformance** lands the
        Cloudflare + Spin `manifest_src` fixes and the Spin
        `unsafe_code` lint scope (see §14 M1.5). This is
        independent of any EdgeZero primitive (the wrappers just
        need to call the pinned `run_app` signature correctly).
      - **M1.5+ adapter-wrapper + `auctioneer-core` `cargo check`**
        matrix becomes mandatory once M1.5 ships — every PR.
      - **M2 production-complete release-artifact gate**
        (per-target `cargo build --release` + `cargo expand
--release -p auctioneer-bidder` snapshot test) runs on
        release tags only and becomes mandatory at M2
        production-complete. The EdgeZero ingress hook (open
        item 14) is NOT part of this build gate and NOT M1.5 — it was
        identified during M2 design, but its implementation/release
        owner is M3b Task M3b-10 (round-80 M2).
        Then a static-analysis check via `cargo expand --release
-p auctioneer-bidder` snapshot test in §11.2 — strengthened per
        round-70 M8 since "no `cfg(...)` blocks survive" is a weak
        assertion (inactive `cfg` blocks naturally disappear from
        expansion regardless of source correctness; the test was
        mostly verifying Cargo behavior). The §11.2 snapshot now
        asserts THREE things:
      1. **Default build** (no feature): the expanded output
         contains **no** function whose name matches the
         insecure path (`allow_insecure_scheme`,
         `validate_insecure_endpoint`, etc.) AND **no** literal
         string matching `http://127.0.0.1`, `http://[::1]`, or
         `http://localhost` (round-72 M4 — IPv6 loopback included
         so the snapshot contract matches the runtime allowlist
         in §5.4 / §8.6)
         in the expanded source. Both items pinned to the actual
         function/literal names the feature gates.
      2. **`--features insecure-bidders` build**: the same
         expansion DOES contain those function names and at
         least one allowed-localhost literal — the gated path is
         not dead and is reachable from the public API.
      3. **Default build still passes its own test suite** —
         catches the case where removing a feature breaks
         non-feature code paths.
         The triple-assertion form is more reliable than grepping
         compiled wasm/rlib for `http://` literals (which can
         appear in unrelated strings) and guards against the
         regression where the feature gate is forgotten on a new
         insecure code path.
    - Local testing — **M2+ command, not runnable today** (round-70
      H1 + round-71 H2 + round-72 H2 — successive attempts to point
      this at `-p auctioneer-adapter-axum`, `-p auctioneer-core`, then `-p
auctioneer-bidder` all failed because none of those packages
      declare the feature in the current workspace; the workspace
      members are just `auctioneer-core` + four adapter crates per the
      root `Cargo.toml`). The M2 form, once `auctioneer-bidder` is
      added to the workspace with `insecure-bidders = []` and the
      forwarding chain is wired through `auctioneer-exchange` →
      `auctioneer-core` → `auctioneer-adapter-axum`:

      ```bash
      # Feature OFF (must return code 5 FailedToRequestBids
      # because http://127.0.0.1 fails validate_endpoint):
      cargo test -p auctioneer-bidder -- mocktioneer_local
      # Feature ON (must succeed; runs against Mocktioneer at
      # http://127.0.0.1):
      cargo test -p auctioneer-bidder --features insecure-bidders \
                 -- mocktioneer_local
      # Same test from the adapter (proves the forwarding chain):
      cargo test -p auctioneer-adapter-axum --features insecure-bidders \
                 -- mocktioneer_local
      ```

      Today (round-73 L3 + round-74 L3 — current workspace
      state; exact error strings updated to match what Cargo
      actually emits):
      - First command (`cargo test -p auctioneer-bidder --
mocktioneer_local`): `error: package ID specification
'auctioneer-bidder' did not match any packages` (auctioneer-bidder
        isn't a workspace member).
      - Second command (`cargo test -p auctioneer-bidder --features
insecure-bidders -- mocktioneer_local`): `error: cannot
specify features for packages outside of workspace`
        (round-74 L3 — Cargo reports the feature-flag error
        before the package-ID error when both apply).
      - Third command (`cargo test -p auctioneer-adapter-axum
--features insecure-bidders -- mocktioneer_local`):
        `error: the package 'auctioneer-adapter-axum' does not
contain this feature: insecure-bidders` (round-74 L3 —
        the earlier "failed to select a version for ..." prefix
        does NOT appear in current Cargo output; the message
        starts directly with `the package ... does not contain
this feature`). The adapter's `[features]` block, where
        it exists, declares only the target-runtime feature
        (`cloudflare` / `fastly` / `spin`), not `insecure-
bidders`.
        The local-test deliverable rides in with M2 wiring; M1
        acceptance does not require it.

- `validate_endpoint(url, bidder_info)` runs on every `BidderHttpRequest`
  before it reaches the fan-out. Explicit rules (round-31 finding M2 —
  these enforce the §5.4 primary guarantee even when adapters bypass
  `EndpointBuilder`):
  - **Fixed-host endpoint** (`host_allowlist == None`): the final URL's
    host **must equal `BidderInfo.endpoint`'s host exactly** (after
    normalization — same scheme, same authority). Adapters cannot
    "freelance" a different host even if it looks plausible.
  - **Host-varying endpoint** (`host_allowlist == Some(set)`): the
    final URL's host **must be a member of `host_allowlist`**.
  - Scheme must be `https` (or `http` only when the `insecure-bidders`
    cargo feature is enabled — §5.4 `allow_insecure`).
  - **If the final host is an IP literal** (round-71 H1 — feature-on
    exception added so `insecure-bidders` can actually reach
    Mocktioneer per §5.4 / round-70 M7): default builds reject any
    IANA private / loopback / link-local IP literal, and reject
    `localhost` and its common aliases (`127.0.0.1.nip.io`, etc.).
    With `#[cfg(feature = "insecure-bidders")]` enabled, the
    rejection narrows to a **localhost-only allowlist**: exactly
    `127.0.0.1`, `[::1]`, and `localhost` are permitted; all other
    private / link-local IP literals stay rejected even with the
    feature on (the feature is for local Mocktioneer testing, not
    for opening up the whole RFC 1918 space). Edge runtimes do not
    expose DNS-resolved addresses to the guest, so auctioneer cannot
    do post-resolution filtering — this is a literal-form check
    only, secondary to the build-time host set (round-32 finding
    L5).
    Failure produces a per-bidder code-5 `FailedToRequestBids` (Appendix G).

### 5.5 Error model

**Caller-domain errors (HTTP-level, surfaced by the handler / trusted-server
caller, not by `run_auction`):**

- Body parse failure / oversize → `400` (caller's bounded read, §5.1
  steps 1–2).
- Account errors — **endpoint-specific** (round-18 finding M7):
  - **Auction:** disabled account → `503` (code 7 `AccountDisabled`).
  - **Sync endpoints (`/cookie_sync`, `/setuid`):** disabled account →
    the existing `400` account-disabled body.
  - Independently fetched malformed-account mappings are M8 design work,
    not M6 runtime-blob outcomes. Malformed account data inside the current
    single blob fails through the host-config loader before account selection.
- Malformed runtime blob / effective host config → `500` (§9.1); transient
  config-store failures retain the existing `503` with Retry-After.
- **`prepare()` `ValidationError`** (round-33 finding L7 — this is
  caller-domain, not engine-domain): `prepare(req, headers, &limits)`
  (§3.2 / §5.1 step 5; the M3b three-argument form — M3a shipped the
  two-argument shape) returns a structured `ValidationError` for structural /
  normalization failure → caller emits `400` with a `text/plain` body
  in the upstream `Invalid request: <err>` format (Appendix F).
  `run_auction` cannot see an unvalidated request (type-enforced); the
  engine-domain `AuctionError::BadInput(...)` block below is a
  separate, registry-aware failure path.

**Engine-domain errors (returned by `run_auction` as `AuctionError`; the
caller maps to HTTP):**

- **`AuctionError::BadInput(...)` — registry-dependent validation
  failure** (round-25 finding H1): some validations require the bidder
  registry and therefore can only run inside `run_auction` after the
  `AuctionContext` is built — disabled-bidder removal with imp-no-bidders
  (§4.2), unknown alias targets, no-op aliases, invalid `aliasgvlids`
  (§5.1 step 7 / §7.3). These return `AuctionError::BadInput(...)` →
  caller emits **`400`** with the `Invalid request: <err>` body.
- **`AuctionError::InvalidImpFirstPartyData(...)` — fatal, non-`BadInput`**
  (round-26 finding H1, resolves the §5.1 step 7 vs §5.5 contradiction in
  rev 31): bidder-scoped FPD merge that produces an invalid `Imp` is
  fatal — caller emits **`500`** (matches upstream's non-`BadInput`
  exchange-error mapping in reference auction handling, code 15 `InvalidImpFirstPartyData`
  in Appendix G). This is **distinct** from the registry-validation
  `BadInput` cases above.
- Fan-out batch-level failure → `AuctionError::FanoutUnavailable` (e.g.
  `BidderFanout::send_all` returns `FanoutInternalError::NoOutboundCapability`)
  → caller emits `500`. This is distinct from per-bidder fan-out errors,
  which are non-fatal.
- Genuine internal faults → `AuctionError::Internal(...)` → caller emits
  `5xx`.
- Bidder errors are **non-fatal** — collected into per-bidder `ext.errors`;
  other bidders still bid. A fully-failed auction returns `Ok(BidResponse)`
  — exact shape per case in Appendix E.5.

Shared taxonomy (`BidderError`, `AuctionError`) distinguishes fatal errors,
invalid-input outcomes, and non-fatal warnings. Numeric diagnostic codes are
defined in Appendix G; HTTP body formats are defined in Appendix F.

### 5.6 Debug surface & redaction

Auction `ext.debug` stays suppressed through M5, with no debug-denial
warnings during that suppression window. M6 enables it only with the
complete [M6 hardening contract](2026-10-05-m6-parity-hardening-design.md),
sections 3-6; this document does not claim it is already implemented.

- **Requested:** OR of `test == 1`, `ext.prebid.debug == 1` or boolean
  `true`, the first decoded `?debug=1`, and a single
  `X-Auctioneer-Debug: 1` header. Query/header parsing is bounded and does
  not provide a force-enable path; exact invalid/duplicate rules are in
  M6 section 3. The engine combines normalized body signals with the
  caller flag so direct calls do not accidentally lose a body request.
- **Account gate:** requested + account `debug_allow == false` yields
  one 10002 warning under `ext.warnings.prebid[]`, omits `ext.debug`,
  and performs no capture. The warning survives nonfatal early engine
  exits; caller/engine fatal errors keep their existing HTTP disposition.
- **Base-bidder gate:** check permission before tentative capture; only
  finalized calls entering fanout produce its denial warning. A compiled
  base's false `BidderInfo.debug_allow` suppresses
  HTTP-call entries for its aliases too and yields one 10003 warning
  per observable bidder name under `ext.warnings[<bidder>]`. Account
  denial never also yields 10003. Bidders dropped before this stage do
  not receive a misleading debug-denial warning.
- **Wire shape:** `ext.debug = {httpcalls, truncated, dropped_calls}`.
  HTTP-call arrays use observable alias keys and positional fanout order,
  with fixed safe error categories and existing per-call timing. Body
  strings are sanitized decoded projections, never raw fallback text.
  The complete field/omission schema is pinned in M6 section 4.
- **Redaction:** separate request/response allowlists; native and legacy
  consent, IPs, IDs/IFA/EIDs/device hashes, precise geo, demographics,
  transaction IDs, credentials, cookies, arbitrary extension/text values,
  and creative/tracking content are masked or omitted. Only safe origin
  and normalized allowlisted header values are displayed. Unknown formats
  never bypass redaction. Consent permitting bidder delivery does not
  permit debug disclosure. Bid/outbound bytes remain unchanged.
- **Ownership:** `auctioneer-exchange::debug` owns gates and sanitized DTOs;
  capture after privacy/adapter projection, preserve unique metadata
  finalization and sealed bounded response parsing, and correlate through
  existing fanout slots. M6 section 5 defines exact stages.
- **Resources:** fixed per-body/call/auction caps, an explicit prepaid
  response-charge term, bounded parsing/writing, fallible reservation, and
  deterministic omission. Before HTTP output, discard the whole debug
  object if adding it would exceed the final-response cap; never truncate
  JSON, remove bids, or extend deadlines. Existing base-response and
  required-warning overflow behavior remains unchanged.
- **HTTP/log privacy:** recognized debug requests use no-store through the
  existing response path. Do not log raw or sanitized payloads. No new
  middleware config read or adapter-local router is introduced.

### 5.7 Host-environment primitives — Clock & Rng

EdgeZero owns host-environment access. Beyond the fan-out primitive (§5.2),
the design needs two more, both **assumed to be EdgeZero primitives** (M2
**production-complete** prerequisites — see §14 and open item 13; M2
_development_ uses the dev-only impls described at the top of this spec):

- **Clock** — exposes both a wall clock (`now`) and a monotonic source
  (`now_monotonic`). Wall clock for `uids` cookie expiry as RFC3339
  timestamps (§8.5) and `ext.prebid.auctiontimestamp`; monotonic for
  `responsetimemillis` and any interval timing, so wall-clock jumps don't
  distort measurement.
- **Rng** — an entropy source. Needed for cookie-sync bidder shuffling
  (Appendix B.3), generated bid IDs, and the `timesplit` price-rounding mode
  (Appendix E.2).

Both are small traits in `auctioneer-runtime`:

```rust
/// Round-148 #1 (M3b): ONE monotonic instant type across auctioneer and
/// EdgeZero. `web_time::Instant` is `std::time::Instant` on native
/// targets and a `performance.now()`-backed instant on
/// wasm32-unknown-unknown; EdgeZero's `edgezero_core::time::
/// MonotonicInstant` is the SAME alias, so a auctioneer instant is an
/// EdgeZero instant by type identity — no bridge, no projection.
pub type MonotonicInstant = web_time::Instant;
pub trait Clock: Send + Sync {
    fn now(&self)           -> time::OffsetDateTime;   // wall clock — RFC3339, cookies
    fn now_monotonic(&self) -> MonotonicInstant;       // monotonic — for intervals
                                                       // and absolute deadlines
}
pub trait Rng: Send + Sync { fn next_u64(&self) -> u64; }
```

`now_monotonic` returns a `MonotonicInstant`; only differences and
comparisons are meaningful (`saturating_duration_since`, `+ Duration`).
The M2/M3a shape returned a `Duration` since an impl-defined epoch; that
shape is RETIRED by the M3b-2b.0 migration (plan) because the fan-out
hands the composed absolute instant to `Deadline::at_instant(..)`
unchanged. `responsetimemillis` and any interval timing use this
source — never `now()`.

Round-149 #1 — ONE clock SOURCE under M3b, not just one instant type:
`auctioneer-core` builds a request-scoped composite `RequestClock` (plan
M3b-2b.0) whose monotonic half IS EdgeZero's injected
`ctx.monotonic_clock()` and whose wall half is the boot-installed host
clock; the same `Arc<RequestClock>` reaches `AuctionContext.clock` and
`EdgeZeroFanout`, so `request_start`, deadline composition, the wave
race and `Deadline::at_instant` are all readings of the clock EdgeZero
itself uses. `MonotonicInstant::now()` is never sampled on the auction
path.

- `auctioneer-exchange` and `auctioneer-core` take `&dyn Clock` / `&dyn Rng`; library
  crates never reach for ambient time or entropy.
- **Production impls live in `auctioneer-core`**, bound to the EdgeZero
  primitives — the same single-seam pattern as `BidderFanout`.
- **Tests** inject a fixed `FakeClock` and a seeded `Rng`, so cookie expiry,
  timestamps, shuffling, and `timesplit` rounding are fully deterministic.
- **Cloudflare monotonic time** (round-36 M3 caveat RETIRED by
  round-150 #8): the `worker::Date`-delta `now_monotonic` shim is
  replaced in M3b-2b.0 by `web_time::Instant::now()`
  (`performance.now()`-backed on wasm32-unknown-unknown, the same
  source EdgeZero's `MonotonicClock` uses), so timing-conformance
  assertions run unexempted on Cloudflare. The fully deterministic
  guarantees apply under `FakeClock` in tests.

`auctioneer-runtime` provides two platform-independent helpers on top of
`next_u64()`, with pinned signatures so every randomized call site uses the
same code and a given `Rng` seed yields byte-identical output on every target:

```rust
pub fn shuffle<T>(slice: &mut [T], rng: &dyn Rng);          // Fisher-Yates
pub fn gen_range(rng: &dyn Rng, upper: NonZeroU64) -> u64;  // [0, upper),
                       // Lemire rejection sampling — no modulo bias
```

All randomized behavior (cookie-sync shuffle, `timesplit`, bid ids) goes
through these helpers.

### 5.8 Reusable entry point — `run_auction`

The auction engine is exposed as a single library function in
`auctioneer-exchange` so the same code runs from `auctioneer-core`'s `#[action]`
handler **and** from any other trusted-server context — a standalone Rust
service, an internal RPC caller, a test harness — with no modification. The
engine is EdgeZero-free (§2.1); only its host-primitive seams differ between
call sites.

`run_auction` performs **§5.1 steps 6–11** (privacy preprocess, split,
privacy-per-bidder, fan-out, collect, targeting & winners). Steps 1–5
(capture / Content-Encoding gate / body read, parse, **host
resolution**, account resolution, **prepare** — the single gate that
subsumes implicit-field fill + validate + normalize, §3.2 / round-28
H1) and step 12 (serialize) are caller-owned (§5.1; round-44 M1
corrected stale "steps 1-4 + step 11" numbering).

```rust
/// Caller-resolved account identity (§9.1). The exchange takes this
/// pre-resolved so callers can use whatever account-store fits their
/// environment (EdgeZero `config_store`, an in-memory map, an RPC).
pub struct ResolvedAccount<'a> {
    pub id:     String,            // **Preserves the request-supplied id**
                                   // (or "unknown" if empty/absent) even on
                                   // DefaultFallback — used for debug/log
                                   // attribution (§9.1, round-14 finding L6).
                                   // The behaviour is driven by `config`.
    pub source: AccountSource,
    pub config: &'a AccountConfig, // Embedded default when source = DefaultFallback.
}
pub enum AccountSource { Provided, DefaultFallback, Configured }
// `Configured` (round-69 H1 — added to match M3a Task 14): the account
// matched a `[accounts.<id>]` entry in the single-TOML `AuctioneerAppConfig`.

#[non_exhaustive]   // external crates cannot construct via field-literal
                    // syntax — they MUST call ::with_request_start, which
                    // enforces the request_start anchor invariant (round-14
                    // finding L7). Tests inside auctioneer-exchange may
                    // construct directly via field literals.
pub struct AuctionContext<'a> {
    pub request_start:    MonotonicInstant,     // monotonic anchor for tmax (§5.3);
                                                // round-150 #1: the app-clock instant
    pub host:             &'a HostConfig,
    pub account:          &'a ResolvedAccount<'a>,
    pub bidders:          &'a BidderRegistry,
    pub fanout:           &'a dyn BidderFanout,
    pub clock:            &'a dyn Clock,
    pub rng:              &'a dyn Rng,
    /// Versioned GVL store. `PrivacyContext::resolve` parses the
    /// TC string first, extracts `(tcf_policy_version,
    /// vendor_list_version)`, and calls `gvl_store.get(spec,
    /// list)` to obtain the exact `&Gvl` that CMPs consulted.
    /// A single `&Gvl` here would have forced the resolver to
    /// substitute a different revision when the exact pair is
    /// unbundled — IAB requires the exact revision. Round-37 B4:
    /// replaces the earlier `gvl: &'a Gvl` field which is now
    /// incompatible with `GvlStore` selection. **Round-81 H1 —
    /// field order matches the real struct + constructor: added
    /// immediately before `debug_requested`, after `rng`.**
    pub gvl_store:        &'a GvlStore,
    /// Round-106 B2 (M3a rounds 98–105): the baked Global CMP List
    /// (`auctioneer_privacy::gcl::Gcl`, from `Gcl::baked()`) and the
    /// wall-clock instant (`clock.now()`, captured by the handler).
    /// `PrivacyContext::resolve(req, account, host, gvl_store, gcl,
    /// now)` consumes both — CMP authenticity/deletion checks against
    /// `gcl.cmps` and GCL staleness against `gcl.expires_at` (§7,
    /// warning 10019). Added immediately after `gvl_store`,
    /// before `debug_requested`.
    pub gcl:              &'a Gcl,
    pub now:              time::OffsetDateTime,
    pub debug_requested:  bool,    // union of req.test==1, ext.prebid.debug==1/true,
                                   // and caller signals (?debug=1, X-Auctioneer-Debug: 1).
                                   // The caller computes the union — query/header
                                   // sources are caller-domain (§5.6).
    /// Round-81 H1 — the real struct also carries these two fields
    /// (they exist in the M2 code today); `with_request_start`
    /// defaults them, `with_request_start_ext` sets them explicitly.
    pub features:                ExchangeFeatures,
    pub bidder_timeout_override: &'a BTreeMap<BidderName, u32>,
}

/// Round-34 H7: `AuctionContext` intentionally does NOT carry
/// request-derived scratch values like resolved aliases,
/// per-bidder budgets, or the disabled-bidder set. Those live
/// on `AuctionPlan` (defined in
/// `crates/auctioneer-exchange/src/plan.rs`), a sibling handle
/// that **the HANDLER constructs (round-69 H1 / M3a Task 13 —
/// after resolving the disabled set + aliases) and passes into
/// `run_auction` as a sibling `&plan` argument**; `run_auction`
/// reads `plan.aliases` rather than re-resolving. (Earlier spec
/// text said `run_auction` constructs it; round-37 B1 moved
/// construction to the handler.) It threads through fanout,
/// split, and response assembly. Keeping the two separate
/// prevents the lifetime tangles that would arise if
/// `AuctionContext` referenced
/// values derived from the request body it never sees.

/// Round-37 B1: canonical construction + lifecycle.
///
/// `AuctionPlan` is `#[non_exhaustive]` so downstream crates
/// cannot construct it via a `Self { .. }` field literal — every
/// call site MUST go through `AuctionPlan::new_m3a(aliases,
/// disabled_bidders, bidder_sample_rates)` (M3a — 3 args, no
/// budgets) or `AuctionPlan::new_m3b(aliases, disabled_bidders,
/// budgets, bidder_sample_rates)` (M3b — 4 args). Round-43 M1:
/// both signatures corrected here to match the definitions
/// below; the round-39 H3 `bidder_sample_rates` addition never
/// reached this doc comment. Field-literal construction is
/// confined to `auctioneer-exchange` itself where
/// `#[non_exhaustive]` does not apply (same-crate rule).
///
/// **Construction sequence** (§5.1 step 5.5, inserted between
/// prepare + auction handler-owned resolutions and `run_auction`
/// invocation). Round-45 B1 canonical ordering (retires
/// round-43 H1's `validate_regs_gdpr`, which checked
/// `regs.gdpr ∈ {0,1}` UNCONDITIONALLY — that 400'd
/// `{gpp_sid:[2], gdpr:2}` even though §7.1 rule 1 says a
/// non-empty gpp_sid overrides regs.gdpr entirely, so the
/// invalid value is never consulted. Malformed TCF/GPP is
/// still NEVER fatal per §7.7):
///   0. handler calls
///      `auctioneer_privacy::validate_wire_signal_input(&req)`
///      FIRST. It is a thin, PRECEDENCE-AWARE wrapper over
///      the same `resolve_wire_signal` the resolver uses:
///      it surfaces only the `Err` side. Because the
///      resolver checks `gpp_sid` before `regs.gdpr`, an
///      invalid `regs.gdpr` accompanied by a non-empty
///      `gpp_sid` yields `Ok` here — no 400. An invalid
///      `regs.gdpr` with absent/empty `gpp_sid` →
///      `Err(PrivacyError::InvalidRegsGdpr)`, which the
///      exchange boundary maps to `AuctionError::BadInput`
///      → 400, BEFORE any disabled/alias work so an alias
///      failure can't preempt it. It does NOT treat
///      `regs.gpp` or `user.consent` as fatal — those are
///      §7.7 warn-and-continue inputs and belong to
///      `PrivacyContext::resolve`, which still runs inside
///      `run_auction` after §5.1 preprocess.
///   1. handler resolves `disabled_bidders` from the runtime
///      overlay (`app_cfg.bidders` entries whose
///      `enabled == Some(false)`). Empty set when no overlay
///      TOML is loaded.
///   2. handler resolves `aliases` via `resolve_aliases(&req,
///      &registry, &disabled_bidders, &host_aliases)`
///      (round-69 H1 — argument order is
///      `(req, registry, disabled, host_aliases)`, matching M3a
///      Tasks 16/18; `disabled` is computed in step 1 above, so
///      it precedes the host-alias argument).
///   3. handler resolves `budgets` (M3b only) via
///      `resolve_budgets(...)` — the per-bidder map keyed by
///      base bidder name; alias entries are NOT keys, they
///      resolve through `aliases` at lookup time.
///   4. `AuctionPlan::new_*(aliases, disabled_bidders, [budgets,]
///      bidder_sample_rates)` returns the constructed handle
///      (round-81 M1 — `new_m3a` is 3-arg: `aliases,
///      disabled_bidders, bidder_sample_rates`; `new_m3b` is 4-arg,
///      inserting `budgets` before `bidder_sample_rates`).
///   5. handler passes `&plan` alongside `&ctx` into
///      `run_auction`. Downstream code (split, fanout, response
///      builder) borrows `&plan`; no downstream owns the plan.
#[non_exhaustive]
pub struct AuctionPlan {
    pub aliases:              ResolvedAliases,
    pub disabled_bidders:     BTreeSet<BidderName>,
    /// M3b Task M3b-2 populates this. Under M3a the field is
    /// `ResolvedBudgets::default()` (empty map).
    pub budgets:              ResolvedBudgets,
    /// Round-39 H3: per-request sample-rate map (Task 17).
    /// M3a canonical field. Empty for tests that don't sample.
    pub bidder_sample_rates:  BTreeMap<BidderName, f64>,
    // future fields added as milestones need them
}

impl AuctionPlan {
    /// M3a constructor. Round-39 H3: takes `bidder_sample_rates`
    /// (Task 17). Callers that don't sample pass
    /// `BTreeMap::new()`.
    pub fn new_m3a(
        aliases: ResolvedAliases,
        disabled_bidders: BTreeSet<BidderName>,
        bidder_sample_rates: BTreeMap<BidderName, f64>,
    ) -> Self {
        Self {
            aliases, disabled_bidders,
            budgets: ResolvedBudgets::default(),
            bidder_sample_rates,
        }
    }

    /// M3b constructor: adds resolved per-bidder budgets.
    pub fn new_m3b(
        aliases: ResolvedAliases,
        disabled_bidders: BTreeSet<BidderName>,
        budgets: ResolvedBudgets,
        bidder_sample_rates: BTreeMap<BidderName, f64>,
    ) -> Self {
        Self { aliases, disabled_bidders, budgets, bidder_sample_rates }
    }
}

impl<'a> AuctionContext<'a> {
    /// Construct with an **explicit** `request_start` captured by the caller
    /// at request receipt (§5.1 step 1) — *before* body read, parse,
    /// account resolution, and `prepare`. The context is usually built
    /// late, after preprocessing, so capturing `request_start` inside this
    /// constructor would silently exclude preprocessing time from tmax
    /// (round-13 finding H2). Direct field construction stays available
    /// for tests.
    ///
    /// Round-37 B4: takes `gvl_store: &'a GvlStore` (not `gvl:
    /// &'a Gvl`) — the resolver selects the exact GVL revision
    /// per TC string via `gvl_store.get(spec, list)`.
    /// Round-81 H1 / round-106 B2 — CANONICAL final argument order:
    /// `gvl_store, gcl, now` sit immediately before `debug_requested`
    /// (after `rng`), preserving the M2 `fanout, clock, rng` ordering.
    /// This is the ONE signature (eleven args); every call site and
    /// fixture uses exactly this. The handler passes `Gcl::baked()`
    /// and `clock.now()`.
    pub fn with_request_start(
        request_start: MonotonicInstant,          // round-150 #1
        host: &'a HostConfig,
        account: &'a ResolvedAccount<'a>,
        bidders: &'a BidderRegistry,
        fanout: &'a dyn BidderFanout,
        clock: &'a dyn Clock,
        rng: &'a dyn Rng,
        gvl_store: &'a GvlStore,
        gcl: &'a Gcl,
        now: time::OffsetDateTime,
        debug_requested: bool,
    ) -> Self {
        // features + bidder_timeout_override take their defaults; the
        // empty override map is a `static EMPTY_OVERRIDES:
        // OnceLock<BTreeMap<..>>` so the `&'a` borrow is `'static`.
        // `with_request_start_ext` sets both explicitly.
        static EMPTY_OVERRIDES: OnceLock<BTreeMap<BidderName, u32>> = OnceLock::new();
        Self {
            request_start,
            host, account, bidders, fanout, clock, rng,
            gvl_store, gcl, now, debug_requested,
            features: ExchangeFeatures::default(),
            bidder_timeout_override: EMPTY_OVERRIDES.get_or_init(BTreeMap::new),
        }
    }

    /// Round-81 H1 — `with_request_start_ext` is `with_request_start`
    /// plus exactly TWO explicit args beyond it (NOT "five"): `features`
    /// and `bidder_timeout_override`. Same leading argument order.
    pub fn with_request_start_ext(
        request_start: MonotonicInstant,          // round-150 #1
        host: &'a HostConfig,
        account: &'a ResolvedAccount<'a>,
        bidders: &'a BidderRegistry,
        fanout: &'a dyn BidderFanout,
        clock: &'a dyn Clock,
        rng: &'a dyn Rng,
        gvl_store: &'a GvlStore,
        gcl: &'a Gcl,
        now: time::OffsetDateTime,
        debug_requested: bool,
        features: ExchangeFeatures,
        bidder_timeout_override: &'a BTreeMap<BidderName, u32>,
    ) -> Self {
        Self {
            request_start,
            host, account, bidders, fanout, clock, rng,
            gvl_store, gcl, now, debug_requested,
            features, bidder_timeout_override,
        }
    }
}

pub async fn run_auction(
    req: &NormalizedBidRequest,                 // §3.2 — type-enforced
    ctx: &AuctionContext<'_>,
    plan: &AuctionPlan,                         // round-37 B1
    audit: &mut DisclosureAudit,                // caller-owned, prepaid aggregate
) -> Result<BidResponse, AuctionError>;
```

- **Input is `&NormalizedBidRequest`** (§3.2) — the type system enforces
  that every caller has run the shared `prepare` helper. Privacy and
  adapter behavior cannot diverge by caller (round-12 finding H1).
- **Return type is `BidResponse` directly** — no separate debug envelope.
  `ext.debug` (when emitted per §5.6) is populated **inside
  `response.ext.debug`** by `run_auction` itself, including redaction. One
  source of truth: a caller cannot forget to inject debug or to redact it.
- `run_auction` performs §5.1 **steps 6–11** (privacy preprocess, split,
  privacy-per-bidder, fan-out, collect, targeting, winners; round-45 M1
  — corrects stale "5–10" numbering after round-43 H1 inserted host
  resolution as step 3). It does **not** read the request body, parse
  JSON, **resolve host config** (step 3, caller-owned), **resolve the
  account** (step 4), **call `prepare()`** (step 5 — which subsumes
  implicit-field fill + validate + normalize, round-28 H1 / round-29
  H1), or serialize the response (step 12) — those are all the
  caller's responsibility.
  - **Account resolution is caller-owned** because (a) the resolution
    mechanism varies by environment (EdgeZero `config_store` vs an
    in-memory map vs RPC) and (b) the resolved identity feeds outbound
    macros (`{{.AccountID}}`, §5.4) — passing a pre-resolved
    `ResolvedAccount` keeps that identity unambiguous.
- Returned future is `?Send`-friendly (matches §1.2).
- The **expired-deadline short-circuit** is `run_auction`'s responsibility:
  if `remaining == 0` after preprocessing, it returns a `BidResponse`
  carrying the three timeout surfaces (§5.3) without ever calling
  `BidderFanout`. The fan-out trait never sees a zero `remaining`.

**Call sites.** Both supply their own host primitives (`BidderFanout`,
`Clock`, `Rng`) and their own `request_start` from `clock.now_monotonic()`
at request receipt:

- **EdgeZero handler** (`auctioneer-core`): receives `request_start` and
  the admission grant FROM THE INGRESS SEAM (round-128 #7 / M3b
  Prereq 10: ingress captures `request_start = clock.now_monotonic()`
  before any body byte and injects it into the context; the handler
  takes the grant via `ctx.take_ingress_grant()` and
  downcast-verifies it — under M3a's shipped shape, with no ingress
  seam, the handler itself captures `request_start` at handler entry,
  before any preprocessing),
  rejects non-empty inbound `Content-Encoding` (§5.1 step 1), reads the
  body bounded (§5.3 — under the ingress lease and the
  `max_ingress_read_ms` deadline where Prereq 10 is present), parses
  to `BidRequest`, **resolves the host
  config** (round-43 H1 — per-request merge + validate per §9.1;
  malformed → `500`), **resolves the account** from `config_store`
  (§9.1) into a `ResolvedAccount`, builds a `RequestHeaders` view from
  the inbound HTTP headers, **calls `auctioneer_openrtb::prepare(req,
headers, &limits)`** (the M3b three-argument form; M3a shipped the
  two-argument shape) to produce `NormalizedBidRequest` (implicit fill +
  validate + normalize are one type-safe gate — round-28 H1 /
  round-29 H1), computes `debug_requested` from the _normalized_
  request (Deref) plus bounded caller-side signals (`?debug=1`,
  `X-Auctioneer-Debug: 1`, M6 design §3),
  **builds the `AuctionPlan`** via `AuctionPlan::new_m3a(aliases,
disabled_bidders, bidder_sample_rates)` (round-81 M1 — the
  three-arg M3a constructor; `bidder_sample_rates` is `BTreeMap::new()`
  until Task 17) after resolving aliases + reading the
  overlay's disabled-bidder set (round-37 B1 — plan construction
  is handler-owned, happens before `AuctionContext` construction,
  and threads through as `&plan` alongside `&ctx`), builds the
  context with `AuctionContext::with_request_start(request_start,
host, account, bidders, fanout, clock, rng, gvl_store,
Gcl::baked(), clock.now(), debug_requested)` (round-81 H1 /
  round-106 B2 — `gvl_store, gcl, now` immediately before
  `debug_requested`), invokes
  `run_auction(&req, &ctx, &plan, &mut audit)`, serializes per Appendix F.
- **Trusted-server caller**: captures its own `request_start` at request
  receipt (same anchor rule), parses the `BidRequest` however it likes,
  **resolves the host config** (round-43 H1 — caller-owned; can be
  embedded-default-only if no `config_store` is wired), **resolves the
  account**, **constructs a `RequestHeaders`** from whatever
  HTTP-header sources are available (all `None` is permissible —
  defaults are then driven by request JSON only), **calls
  `prepare(req, headers, &limits)`** (mandatory — the only callable
  path that produces a `NormalizedBidRequest`; implicit fill cannot
  be skipped; the trusted-server caller supplies its own
  `PrepareLimits`, M3a's two-argument call being the shipped shape
  until M3b-2),
  computes `debug_requested`, **builds the `AuctionPlan`** via
  `AuctionPlan::new_m3a(...)` (round-37 B1), builds the context
  with `with_request_start(..., gvl_store: &GvlStore, gcl: &Gcl,
now: OffsetDateTime, ...)` (its own GCL handle + wall-clock
  instant — round-106 B2),
  invokes `run_auction(&req, &ctx, &plan, &mut audit)`, emits the
  `BidResponse` in its own transport. It is responsible for body
  limits and authentication appropriate to its environment.

Tests invoke `run_auction` directly with a mock `BidderFanout`, `FakeClock`,
and seeded `Rng` (§11.2).

The same factoring is the goal for `/cookie_sync` and `/setuid` (an
analogous `run_cookie_sync` / `run_setuid` in `auctioneer-exchange` or a sibling
`auctioneer-usersync` crate) — **only the trusted-server reusable entry
point factoring is deferred** (round-41 L6 — earlier wording read like
the sync endpoints themselves were deferred, which they aren't). The
`/cookie_sync` / `/setuid` / `/getuids` _handlers_ ARE in phase-1 scope
(§13) and ship in M5 (§14); they just run as `auctioneer-core` handlers, not
through a reusable library function. A trusted-server caller that wants
to invoke the sync flow programmatically (without going through EdgeZero)
is out of phase 1 unless that becomes a requirement. The auction
(`run_auction`) is the only entry point with that reusable factoring in
phase 1.

### 5.9 Aliases — observable identity contract

A request alias declared in `ext.prebid.aliases` (e.g. `"myalias":
"appnexus"`) maps to a base bidder. The alias is a first-class **observable
identity**: it appears in every user-facing field of the response while the
**base adapter** runs the I/O. Matches upstream (round-14 finding M3).

| Field                                   | Identity used |
| --------------------------------------- | ------------- |
| `seatbid.seat`                          | alias name    |
| `ext.errors[<bidder>]`                  | alias name    |
| `ext.warnings[<bidder>]`                | alias name    |
| `ext.responsetimemillis[<bidder>]`      | alias name    |
| Targeting key suffix (`hb_pb_<bidder>`) | alias name    |
| `ext.debug.httpcalls[<bidder>]`         | alias name    |
| `seatnonbid.seat`                       | alias name    |

The **adapter code** (`Box<dyn Bidder>`) is the **base** bidder's. The
**`BidderInfo`** is the base's, with two alias-specific overrides applied:
`gvl_vendor_id` from `ext.prebid.aliasgvlids[<alias>]` (if present) — used
for the GDPR vendor lookup (§7.3) — and the `BidderName` for all observable
fields above. All other capability / syncer / endpoint / `host_allowlist`
fields come from the base. The fan-out's `BidderName` in correlation
metadata is the alias.

Phase 1 supports request-defined aliases; host/config-declared aliases
appear in `/info/bidders` (§8.1) but request-defined ones do not (they are
auction-local, §8.1).

**Alias position in registry order** (round-40 M2). The phrase
"registry order" — used by `max_bidders_per_auction` (§5.3),
`max_outbound_requests` (§5.3), winner tie-breaking (Appendix E.4), and
deterministic serialization (Appendix E.7) — refers to a **single
lexicographic sort over the set of bidder names that participate in
_this_ auction**, where the set is the union of:

1. **Base bidders** registered for the request (those with at least one
   imp's `imp.ext.bidder.<name>`, `imp.ext.prebid.bidder.<name>`, or the
   legacy top-level `imp.ext.<name>` slot per §3.1 — after alias
   resolution and FPD merge, §5.1 step 7), keyed by their base
   `BidderName`. The public helper `auctioneer_exchange::participating_bidders`
   walks the same three sources plus request-declared aliases so
   external callers can never observe a different participation set
   than the one `split::process_one_imp` actually drives fan-out from.
   Unregistered top-level passthrough keys (`imp.ext.tid`,
   `imp.ext.prebid`, arbitrary FPD-adjacent slots) are excluded —
   only names that resolve against `BidderRegistry` or against
   `ext.prebid.aliases` count. **An alias declared in
   `ext.prebid.aliases` but not referenced by any imp does NOT
   participate.** Fanout would ignore it, so the public participation
   set must too; otherwise external tooling would observe an
   auction-participation claim the fanout contract silently drops.
2. **Request-defined aliases** from `ext.prebid.aliases`, keyed by the
   **alias name** (not the base). An alias and its base appear as two
   distinct entries when both have imps targeting them; the auction
   treats them as independent observable identities (table above).
3. **Host/config-declared aliases** (if any) that have imps in this
   request, keyed by the config alias name. Same rules as request
   aliases.

The combined set is sorted ascending by **byte-lexicographic comparison
of UTF-8 names**. That single ordering is the "registry order" everywhere
in the spec — caps, fan-out emission, tie-breaking, `seatbid`
serialization, `ext.*` map serialization. Two consequences pinned for
clarity:

- **Caps** (`max_bidders_per_auction`, `max_outbound_requests`) operate
  on this combined set. An alias counts as one bidder for cap-counting,
  even though it shares the base's adapter code (§5.9 — the alias has
  its own per-bidder warning slot and `responsetimemillis`).
- **Tie-breaking** on equal bid price uses this same ordering: the
  earlier entry (lexicographically smaller name) wins the auction
  (Appendix E.4 / round-26 finding M2). Aliases participate as
  first-class names in this comparison.

`/info/bidders` (§8.1) is unaffected — it lists only registered base
bidders + host-declared aliases, in lexicographic order; request aliases
remain auction-local and do not appear there.

---

## 6. `auctioneer-config` — Build-Time Config Generation

WASM edge runtimes have no filesystem, so **embedded defaults** for
all configuration are generated at build time. Host and account config
also support optional runtime overrides via EdgeZero's `config_store`
(§9.1; round-43 L6 — the "all configuration" qualifier above means "no
runtime _filesystem_ config", not "no runtime overrides at all").
Bidder data is build-time-only — no runtime override (§9.1, the SSRF
guarantees rest on this).

- **Vendored inputs** (committed into the auctioneer repo — hermetic build):
  - `data/bidder-info/*.toml` — one per supported bidder (from reference server
    `static/bidder-info/`).
  - `data/gvl_archive/` — **(round-63 B3 — replaces the singular
    `data/gvl.json`)** a versioned IAB Global Vendor List **archive**:
    per-`(spec, vendorListVersion)` blobs under `spec2/` / `spec3/`
    plus a `manifest.json` that SHA-256-pins each entry, refreshed by
    the `crates/auctioneer-refresh-gvl-archive` tool (see the M3a plan Task 4 /
    Task 4a-v24 for the manifest schema, the fetch-and-pin-at-execution
    refresh, and its `gvl_archive.lock` advisory-lock + crash-recovery
    protocol). The old single-blob `data/gvl.json` + `gvl_raw()`
    accessor are DELETED; the runtime iterates the archive via
    `GvlStore`.
  - `data/host-defaults.toml` — host config defaults.
  - `data/account-defaults.toml` — the embedded default-account config (§9.1).
    **⚠ M3b-1d FOLDS BOTH INTO `auctioneer.toml` under explicit
    namespaces (round-194 #1).** They become its `[baked.host]` and
    `[baked.account]` tables, today's contents move under
    `[runtime]`, and the two `data/` files are deleted — so one file
    shows every knob and its default while the rebuild/push boundary
    stays visible. `build.rs` reads the two `[baked.*]` tables into
    the unchanged `HostDefaultRaw` / `AccountRaw`; `auctioneer-cli`
    extracts `[runtime]` and pushes that, so the blob schema,
    `AuctioneerAppConfig` and `resolve.rs` are unchanged and no deployed
    blob is invalidated. See §6.1 for the full proposal. **This row
    and the M3b plan's M3b-1d must change in ONE commit**; until
    that task lands the two `data/` files above are accurate.
  - `data/eea-countries.txt` — EEA list for the geo-aware GDPR default.
- **`build.rs`** parses these → generates Rust into `OUT_DIR`: **data tables
  only** — bidder data table (name → endpoint, macros, `host_allowlist`,
  **`endpoint_compression`** (round-23 finding M2), GVL vendor id,
  capabilities, syncer config, `gpp_supported`, `disabled`, **`debug.allow`**
  (round-25 finding M5; defaults to **`true`** when missing from the
  bidder-info TOML, matching reference bidder request/response processing)), host-config and
  default-account defaults, GVL lookup tables, EEA set. `auctioneer-config`
  `include!`s the generated file and exposes typed accessors. A `gzip`
  bidder (Pubmatic) is in the §11.2 conformance fixtures.
- The build emits **no adapter instances** — the registry is assembled in
  `auctioneer-bidder`.
- Plain `build.rs`; generated code satisfies the workspace clippy gate via
  scoped `#![allow]`s.
- **`insecure-bidders` feature gate in `build.rs`** (round-75 M1 —
  the build script honors the same scheme-relaxation rule as §5.4 /
  §8.6 runtime gates; without this gate on `auctioneer-config`, the §8.6
  feature-on codegen fixture cannot compile cleanly even though the
  forwarded chain reaches runtime crates). The script reads
  `env::var("CARGO_FEATURE_INSECURE_BIDDERS").is_ok()`; when true,
  the template-validation pass for bidder endpoints AND syncer URLs
  applies the localhost-only allowlist (`127.0.0.1`, `[::1]`,
  `localhost`) instead of HTTPS-only. The feature is declared on
  `auctioneer-config/Cargo.toml` as `insecure-bidders = []` and forwarded
  in from `auctioneer-bidder/Cargo.toml` via
  `insecure-bidders = ["auctioneer-config/insecure-bidders"]` per the
  §5.4 forwarding chain.
- **Phase-1 bidder universe.** Phase 1 vendors and generates config for
  **only the four reference bidders — they are the entire bidder universe in
  phase 1**. `/info/bidders`, cooperative-sync candidate construction, and
  `host_allowlist` all operate over exactly those four. Scaling toward the
  full ~266 is the mechanical phase-6 step: drop the bidder-info TOML +
  register an adapter; no change to the generator.

### 6.1 Runtime overlay — `auctioneer.toml` + `[stores.config]`

> **⚠ PROPOSED (M3b-1d) — ONE CONFIG FILE. This section describes
> the CURRENT two-file arrangement; the proposal below replaces it,
> and the two must change in ONE commit.**
>
> **The problem.** A knob's DEFAULT and its OVERRIDE live in
> different files today, under different section names. The five
> auction limits are the clearest case: `auctioneer.toml` carries
> `[auction] default_tmax_ms` commented out, while the live default
> `1000` sits in `data/host-defaults.toml` under
> `[resource_limits] default_tmax_ms`. `resolve.rs` maps the two
> one-to-one, so they are the SAME FIELD under two names in two
> files. Worse, `[gdpr]`, `[ccpa]`, `[lmt]` and `[host_cookie]`
> exist ONLY in the baked file, so `auctioneer.toml` never shows an
> operator the settings that most affect privacy behaviour.
>
> **The proposal.** `auctioneer.toml` becomes the single source for
> every auctioneer knob AND its default. `build.rs` reads its defaults
> from that file; `auctioneer-cli config push` pushes its `[runtime]`
> subtree (round-195 #3 — an earlier line said "the same file",
> which contradicted the namespaced layout below).
> `data/host-defaults.toml` and `data/account-defaults.toml` are
> deleted.
>
> **Why this is safe: the baked and runtime schemas stay SEPARATE,
> so nothing has to reconcile** (round-195 #3 — an earlier line
> claimed the two schemas were "already aligned", which is true only
> for the host `gdpr` pair and false in general: host
> `gdpr.enabled` is `bool` while account `gdpr.enabled` is a
> tri-state `String`). Each namespace deserializes into the struct
> it already has, so no type is re-defined and the merge rule below
> is unchanged.
>
> **Layout — two explicit namespaces in one document.** An earlier
> revision put the defaults at the top level and under
> `[accounts.default]`; **withdrawn (round-194 #1)**, because
> `resolve_account` does `app.accounts.get(&id)`, so a request whose
> publisher id is literally `"default"` would select the defaults
> entry as a configured account — something no build-time assertion
> can prevent. The layout is:
>
> ```toml
> [baked.host]     # was data/host-defaults.toml   — REBUILD REQUIRED
> [baked.account]  # was data/account-defaults.toml — REBUILD REQUIRED
> [runtime]        # was the whole file — pushed, effective next request
> ```
>
> The namespaces make the rebuild/push boundary VISIBLE rather than
> implied, and they resolve the tension with the paragraph below:
> `[runtime]` holds exactly the fields "an operator legitimately
> tunes without a rebuild", while `[baked.*]` holds the rest, so no
> value has to satisfy both descriptions.
>
> **The `gdpr.enabled` type clash disappears.** Host
> `[baked.host.gdpr].enabled` is `bool`; account
> `[baked.account.gdpr].enabled` is a tri-state `String`. They
> collided only because the two `data/` files were both document
> ROOTS; under distinct namespaces they never meet.
>
> **`[baked.host]` keeps `[resource_limits]`; `[runtime]` keeps
> `[auction]`** (round-195 #3 — an earlier line said `[auction]`
> absorbs `[resource_limits]`, which contradicted "`[baked.host]`
> deserializes into the UNCHANGED `HostDefaultRaw`"). They are the
> same five fields under two names, but they now sit in DIFFERENT
> namespaces — the baked default and its runtime override — so
> neither struct changes and no projection is needed. The
> duplication that motivated this proposal is gone because the two
> are adjacent in ONE file and their relationship is visible.
>
> **A complete, valid example (round-196 #4 — transcribed from the
> shipped `host-defaults.toml` / `account-defaults.toml`, not
> invented; the earlier version omitted required sections and used
> `enabled = "0"`, which is not a `TriState` value):**
>
> ```toml
> [baked.host]
> status_response = ""
> [baked.host.gdpr]
> enabled = true                    # bool — host semantics
> default_value = "0"
> host_vendor_id = 0
> non_standard_publishers = []
> [baked.host.gdpr.tcf2]
> enabled = false
> purposes = {}
> special_features = {}
> [baked.host.ccpa]
> enforce = false
> [baked.host.lmt]
> enforce = false
> [baked.host.host_cookie]
> ttl_days = 90
> [baked.host.resource_limits]
> default_tmax_ms = 1000
> max_tmax_ms = 2000
> max_bidder_response_bytes = 262144
> max_bidders_per_auction = 50
> max_outbound_requests = 100
> [baked.host.aliases]
>
> [baked.account]
> disabled = false
> debug_allow = false
> bid_rounding = "down"
> targeting_prefix = "hb"
> [baked.account.gdpr]
> enabled = "unset"                 # TriState: unset | enabled | disabled
> purposes = {}
> special_features = {}
> basic_enforcement_vendors = []
> [baked.account.gdpr.channel_enabled]
> web = "unset"
> app = "unset"
> amp = "unset"
> video = "unset"
> dooh = "unset"
> [baked.account.privacy.ipv4]
> anon_keep_bits = 24
> [baked.account.privacy.ipv6]
> anon_keep_bits = 56
>
> [runtime]
> # Every key here is an Option<_> OVERRIDE and is absent by default.
> # A present key PINS its value and stops inheriting future baked
> # defaults, so leave it commented unless you mean to pin it.
> # [runtime.auction]
> # default_tmax_ms = 1500
> ```
>
> `[baked.host]` and `[baked.account]` are REQUIRED and strict;
> `[runtime]` is required but its every field is optional. The two
> `gdpr.enabled` values coexist because they are different paths and
> different types. An operator who uncomments
> `[runtime.auction].default_tmax_ms` overrides
> `[baked.host.resource_limits].default_tmax_ms` through the
> existing merge in `resolve.rs`, unchanged — and only then does
> `AUCTIONEER__RUNTIME__AUCTION__DEFAULT_TMAX_MS` reach it, because the
> env overlay only touches keys the file already sets.
>
> **Who reads what.** `build.rs` deserializes `[baked.host]` into
> the unchanged `HostDefaultRaw` and `[baked.account]` into the
> unchanged `AccountRaw`, keeping `deny_unknown_fields` on each,
> plus every guard it runs today (resource-limit validation, the
> alias-vs-registry check, the typed render checks, the
> build-failure harness). **`auctioneer-cli` passes the WRAPPER type
> `AuctioneerPushSource` as `C`** (round-197 #1 — this said it
> "extracts `[runtime]` and pushes that through
> `run_config_push_typed::<AuctioneerAppConfig>`", which contradicts the
> wrapper contract stated below and is not how the API works:
> `run_config_*_typed::<C>` takes only command args and
> deserializes the file root itself, so the projection must happen
> INSIDE `C`). Because the wrapper's `Serialize` emits exactly
> today's `auctioneer.toml` content, the blob schema,
> `AuctioneerAppConfig` and `resolve.rs` are **entirely unchanged** and
> no deployed blob is invalidated — the withdrawn variant would
> have needed new overlay fields AND a merge change, since
> `resolve.rs` deliberately passes `ccpa`, `lmt` and `host_cookie`
> through untouched.
>
> **What does NOT move.** `runtime-bootstrap.toml` stays separate.
> Its one key, `allow_missing_config`, decides what to do when the
> store holds NO blob; folding it into the pushed source would be
> circular. Three M3b values are likewise consumed before any store
> read exists (`max_config_fetch_ms` bounds the fetch;
> `terminal_response_permits` and `refusal_write_deadline_ms` are
> process-globals installed while `configure`, `build_app` and the
> admission policy are all synchronous and hold no store handle).
> They are DECLARED in `auctioneer.toml` with the other host settings
> and BAKED from it, and their doc comments say **REBUILD
> REQUIRED**: `config push` cannot change them and neither can a
> restart, since the same binary re-reads the same constant.
>
> **`auctioneer.toml` is still never read at runtime and never baked
> wholesale into the binary.** It is the SOURCE artifact: `build.rs`
> extracts DEFAULT VALUES from it at build time (exactly as it does
> from `host-defaults.toml` today), and the runtime reads the pushed
> STORE BLOB.
>
> **Acceptance is invariance.** `host_default()` and
> `account_default()` keep their name, type and value, so the four
> call sites need no edit, and the pushed envelope's `data` matches
> what today's `auctioneer.toml` produces. **ONE existing test changes**
> (round-196 #7): `config_reference_examples.rs` `include_str!`s the
> whole file and parses it as `AuctioneerAppConfig`, which the
> namespaced root makes impossible; it is split into a
> runtime-projection case and a baked-examples case. Every other
> test stays green unedited.
>
> **The CLI passes a auctioneer WRAPPER type, not `AuctioneerAppConfig`**
> (round-196 #2/#3/#5). `run_config_*_typed::<C>` takes only command
> args and deserializes the file root itself, so the projection
> happens in `C`: `AuctioneerPushSource` deserializes the combined root
> and serializes ONLY `[runtime]`, with hand-written `Serialize`,
> `Validate` and `AppConfigMeta` (the `AppConfig` derive would both
> conflict with a manual `AppConfigMeta` and prefix every secret
> path with `runtime.`). The shared root DTO lives in a NEW
> dependency-neutral crate, `auctioneer-config-schema` — NOT in
> `auctioneer-core`, which `auctioneer-config` already depends on, so
> putting it there would make the build script cyclic. The precedent
> is `auctioneer-gvl-schema`. **`config validate` migrates to
> `run_config_validate_typed`** so it stops reporting "valid" for a
> document `push` would reject.

Every configurable knob has **two layers** and one merge rule:

1. **Baked layer** — the build-time TOML tables described above,
   compiled into the wasm binary. Immutable at runtime.
2. **Overlay layer** — the pushed blob living in
   `[stores.config].default` (declared in `edgezero.toml`). Deployer
   pushes via `auctioneer-cli config push --adapter <name>`; typed schema
   is `auctioneer_core::config::AuctioneerAppConfig`.
   **⚠ AFTER M3b-1d (round-198 #1): the PAYLOAD is `auctioneer.toml`'s
   `[runtime]` SUBTREE, not the whole file.** `auctioneer.toml` is the
   single SOURCE document and carries `[baked.host]`,
   `[baked.account]` and `[runtime]`; the CLI's wrapper type
   serializes only `[runtime]`, so the blob's bytes and schema are
   exactly what they are today. Calling the payload "the pushed
   `auctioneer.toml`" was accurate before the fold and is not after.
3. **Merge rule** — `resolved_host()` / `resolved_account()` (in
   `auctioneer_core::config::resolve`) take the pushed `AuctioneerAppConfig`
   and the build-time default, then return an owned effective
   `HostConfig` / `AccountConfig`. `Option` fields absent → baked
   value flows through; `Option` fields present → overlay wins.

**What lives in `[runtime]`** (round-198 #1 — this said "in
`auctioneer.toml`", which after the fold names the whole source document
including the baked tables; the rule below governs the RUNTIME
namespace only). **Only fields an operator legitimately
tunes on a live deployment without a rebuild** — mirroring the "no
runtime _filesystem_ config, but yes runtime overrides" caveat of
this section's opening paragraph. Fields that are spec-defined or
codegen-driven never appear.

- `[auction]` — resource limits: `default_tmax_ms`, `max_tmax_ms`,
  `max_bidders_per_auction`, `max_outbound_requests`,
  `max_bidder_response_bytes`, and (round-101 B5, M3b)
  `max_outbound_raw_bytes` + `max_outbound_request_bytes`. Each
  overrides its counterpart in `HostConfig.resource_limits`; under
  M3b's boot envelope the outbound-cap overrides clamp DOWNWARD only.
  Round-109 H3: `response_write_deadline_ms` is deliberately NOT in
  this overlay DTO — it is host-baked only; an overlay carrying the
  key is rejected as unknown.

- `[aliases]` — the root-level runtime host-alias overlay map
  (round-103 H7: entry count is bounded by the M3b envelope's
  `max_aliases`, default 64; an overlay with MORE entries is REJECTED
  at config validation — fail-loud, never silently truncated).
- `[targeting]` — `prefix` (overrides `AccountConfig.targeting_prefix`
  when present; length capped at 8 so the Appendix E.3 `MaxKeyLength
= 20` invariant holds with a `_pb_<bidder>` suffix) and
  `truncate_target_attribute` (12..=20).
- `[status]` — `banner` string that overrides `HostConfig.status_response`.
  Deployers set an env-name / build-id here so `/status` probes carry
  per-deployment provenance.
- `[features]` — kill-switches: `gzip_outbound`,
  `returnallbidstatus_default`. Both `Option<bool>`; `None` means
  "use conservative default". Resolved values flow into
  `AuctionContext.features: ExchangeFeatures`, which `run_auction`
  reads at every decision point (`build_call_list` strips
  `endpoint_compression` when `gzip_outbound = false`;
  `returnallbidstatus` falls back to the feature default when the
  request omits it). Non-USD currency handling is intentionally NOT
  a runtime knob in M2 — the response builder hardcodes `cur =
"USD"` and the wire shape carries no per-seat currency column, so
  admitting foreign-currency bids would emit mis-labelled prices.
  M3 fx wires currency-aware bundles + revisits the toggle.
- `[bidders.<name>]` — per-bidder overrides: `enabled` (kill-switch;
  M2 logs a warning if the baked `BidderInfo.disabled` bit doesn't
  match, M3 wires the request-scoped registry overlay), `timeout_ms`
  (overrides `BidderInfo.bidder_timeout_ms` via
  `AuctionContext.bidder_timeout_override`; the exchange's timeout
  resolution chain is `alias override → base-bidder override →
baked BidderInfo → bidder_visible_tmax_ms` — a request-declared
  alias inherits the base bidder's `timeout_ms` when the alias has
  no override of its own), `sample_rate`
  (`#[validate(range(min=0.0,max=1.0))]`; value is validated +
  stored, sampler wires in M3). Unlisted bidders default to enabled.
  Every entry runs through `BidderOverrides::validate()` via a
  custom validator on the map (validator 0.20 does not descend into
  `BTreeMap` values automatically).

**What does NOT live in `auctioneer.toml`** (explicit non-goals — avoid
scope creep):

- Bidder catalog (endpoints, media types, macros) — codegen from
  TOML.
- ~~GDPR / CCPA / LMT enforcement flags — spec-defined, TOML.~~
  **Amended round-21 H3 for M3a.** GDPR enforcement flags DO live
  in `auctioneer.toml` under M3a as the `[gdpr]` host overlay + the
  `[accounts.<id>.gdpr]` per-account overlays. Baked TOML remains
  the source of the DEFAULT values (conservative "everything
  off"); `auctioneer.toml` overlays turn enforcement on for a specific
  deployment. This is the same overlay-over-baked pattern
  `auctioneer.toml` uses for auction limits and status body. CCPA /
  LMT stay TOML-only until M4.
- Deployment identity strings (env name, region, build id) — vendor
  platforms already expose these; env vars or `option_env!` do the
  job with no store round-trip.
- Secrets — belong in `[stores.secrets]`, referenced from `auctioneer.toml`
  by store-key name only.

**⚠ ROUND-195 #2 — THE ENV OVERLAY ONLY REACHES KEYS THE FILE
ALREADY SETS, SO EVERY EXAMPLE BELOW IS A NO-OP TODAY.** Upstream's
loader states it: "The overlay only overrides keys that ALREADY
EXIST in the parsed tree" (`edgezero-core/src/app_config.rs`), and
`auctioneer.toml` currently has **ZERO uncommented keys** —
`grep -cE '^[a-z_]+ *=' auctioneer.toml` returns 0. So
`AUCTIONEER__AUCTION__DEFAULT_TMAX_MS=1500` is silently ignored. This is
a SHIPPED defect in this document, not a consequence of the M3b-1d
proposal.
**⚠ ROUND-197 #1 — M3b-1d DOES NOT "FIX" IT, AND AN EARLIER
REVISION HERE OVERSTATED WHAT THE FOLD BUYS.** It said the folded
file would carry live values and that `AUCTIONEER__BAKED__HOST__...`
would affect the build. Both are wrong, and the plan's later
contract is binding:

- **`[runtime]` keys stay ABSENT.** They are `Option<_>` overrides;
  a present value deserializes as `Some(..)` and PINS the knob,
  stopping the blob inheriting future baked defaults. So
  `AUCTIONEER__RUNTIME__AUCTION__DEFAULT_TMAX_MS` reaches a key only
  once an operator deliberately uncomments it — a visible choice to
  pin, not a workaround.
- **There is no `AUCTIONEER__BAKED__...` override at all.** `build.rs`
  reads the file with `fs::read_to_string` + `toml::from_str` and
  applies NO env overlay; the CLI discards the `[baked]` subtree.
  **Baked values change by editing `auctioneer.toml` and rebuilding,
  full stop.**
  Filed upstream as G27 (populate absent typed `Option` fields from
  the overlay). Until it lands, treat every env example in this
  paragraph as applying ONLY to keys the file already sets.

**Env-var overlay — applied at PUSH time, not at request time
(execution amendment, post-review round 3).**
`edgezero_core::app_config::load_app_config` applies
`AUCTIONEER__<SECTION>__…__<KEY>` on top of the TOML before
validation — but that loader runs inside `auctioneer-cli config push`
on the operator's machine. The runtime read path
(`AppConfig::from_store`) consumes the stored envelope verbatim
and reads no instance environment. So the overlay changes what
the NEXT PUSH contains; changing a deployed instance requires
re-running the push with the variable set. Example:
`AUCTIONEER__RUNTIME__AUCTION__DEFAULT_TMAX_MS=750 auctioneer-cli config
push …` (round-198 #2 — the path gains its namespace after the
fold, and it reaches the key only once `default_tmax_ms` is
uncommented, per the overlay rule above)
bakes the override into the pushed blob. A true per-instance
runtime overlay would need the edge platforms' env surface at
request time and is not an M3a capability.

**Handler read path — M3a behaviour** (round-26 H5 scope tag —
earlier "runtime path never 500s" was a pre-fail-loud statement
that contradicts §9.1 and M3b Task M3b-1). Every handler that
today reads `auctioneer_config::host_default()` /
`account_default()` calls
`AppConfig::<AuctioneerAppConfig>::from_store(&ctx, "auctioneer_runtime",
None)`. Under M3a as ORIGINALLY SPECIFIED, every extractor error
degraded to `AuctioneerAppConfig::default()` with a WARN.
**Execution amendment (Task 15 review + post-review rounds 1-3):
the shipped M3a contract is stricter, per route.** On the auction
and `/info/*` routes: `EdgeError::Internal` (envelope parse / SHA
mismatch / unknown version) and `EdgeError::BadRequest` (invalid
or overlong store key — reachable through the binding's
environment-resolved `default_key`, NOT guarded by the store-id
const) → `RuntimeConfigError::MalformedHost` → HTTP 500;
`EdgeError::ServiceUnavailable` → `ConfigStoreUnavailable` →
HTTP 503 + `Retry-After`. Only the umbrella
`EdgeError::ConfigOutOfDate` (missing blob / deserialize /
validate, indistinguishable at the pinned EdgeZero) still
degrades with the WARN — that residual fail-open is the recorded
`dev-complete` gap the promotion barrier exists for, and M3b's
total match over the landed `StoreExtractionReason` taxonomy
(round-141 #2) closes it. Round-117 P2-11: under
M3b, `MissingKey` does NOT unconditionally default — a
`runtime.bootstrap` TOML (an IMMUTABLE boot-time file baked with
the deploy, schema: `[runtime] allow_missing_config: bool`,
default false; precedence: bootstrap file only, no env, no store
— it is what decides how store errors are treated, so it cannot
itself come from the store) must set `allow_missing_config =
true` for a missing key to default silently; otherwise
`MissingKey` joins the fail-loud arm. The M3b plan's Task M3b-1
carries the full schema and the config-inventory row. `/status` alone keeps the
permissive degrade on EVERY class (it is a liveness probe and
carries the milestone header regardless). The rationale for the
tightening: a corrupt pushed blob silently disabling GDPR was a
privacy fail-open, unacceptable even at dev-complete. This is deliberate
FOR M3a: the pinned EdgeZero (`docs/outbound-http-spec`, verified
at the `fcefdde3`-era alignment; pin now `12965da2`, where the
categorized-store-error DESIGN exists but the code still) does NOT
distinguish the three failure classes auctioneer needs for a
correct fail-loud decision (missing key vs unparseable envelope
vs failed validator) — all three surface as `EdgeError::ConfigOutOfDate`
from `extract_from_handle`
[extractor.rs:766-820](https://github.com/IABTechLab/edgezero/blob/9e661ae/crates/edgezero-core/src/extractor.rs#L766).
Other classes DO have distinct EdgeZero signals today
(`ConfigStoreError::InvalidKey`, `ConfigStoreError::Unavailable`,
`SecretError::NotFound`, `SecretError::Unavailable`) but they
are collapsed into `EdgeError::internal` / `service_unavailable`
by the pinned `From` impls, and auctioneer does not yet unwrap
them. 500'ing on the umbrella `ConfigOutOfDate` under M3a would
kill `curl -X POST /openrtb2/auction` against a fresh checkout
(round-27 M1 correction to the earlier "every store failure
becomes ConfigOutOfDate" claim, which was too broad).

**M3b supersedes this behaviour — against the REAL landed
taxonomy (round-141 #2: `EdgeError::StoreExtraction { reason:
StoreExtractionReason }`, a sixteen-variant `#[non_exhaustive]`
enum at the candidate EdgeZero head; the `StoreAppConfigError`
variant roster that stood here is HISTORY — the earlier "no
upstream design exists" framing is false now, and around the
privacy fail-closed path a stale taxonomy is dangerous).** The
handler match is TOTAL per the M3b plan's round-140 #4 mapping:
`MissingBlob` → the `allow_missing_config` rule;
`BackendUnavailable` / `SecretBackendUnavailable` /
`DeadlineExceeded` → **`RuntimeConfigError::ConfigStoreUnavailable`**

(HTTP 503 + `Retry-After` — all transient); the **ELEVEN**
malformed-family variants (`BackendFailure`, `Deserialization`,
`IntegrityMismatch`, `InvalidSecretValue`,
`MalformedEnvelope`, `MissingRegistry`, `MissingSecret`,
`UnknownStore`, `UnsupportedVersion`, `Validation`,
`ValueTooLarge`) → **`RuntimeConfigError::MalformedHost`** (HTTP
500); and the `_` wildcard for future variants → 500,
fail-closed.

**⚠ ROUND-203 #5 — `InvalidKey` LEAVES THE MALFORMED FAMILY.** It
maps to **`RuntimeConfigError::StoreKeyRejected`**, which is why
the count above is eleven rather than twelve. **Its HTTP
disposition is UNCHANGED — 500, fail-closed, no `Retry-After`** —
so this is a REPORTING split, not a policy change: a rejected key
means no blob can ever be pushed where this deploy looks for one,
and retrying cannot fix it. The split exists so `/status` can
report `store_key_rejected` distinctly from `malformed_config`
(§6.1); without it both faults collapse to one code and an
operator cannot tell a corrupt blob from a refused key. **This
paragraph and the M3b plan's M3b-1c change in ONE commit.**

**`EdgeError::BadRequest` — the store REJECTING the bound key —
maps to `RuntimeConfigError::StoreKeyRejected`, and its disposition
is HTTP 500, fail-closed, exactly like `MalformedHost`. It is NOT
transient and carries no `Retry-After`** (round-191 #7: this mapping
was previously inserted inside the paragraph above, whose trailing
qualifier reads "all transient", leaving its status ambiguous
between 503 and 500). A rejected key means no blob can ever be
pushed where this deploy looks for one — retrying cannot fix it, so
the transient treatment would be wrong. **The ONE exception is
`/status`**, which per §6.1 answers degraded `200` with code
`store_key_rejected` rather than any 5xx, because a liveness probe
reaches no privacy or auction decision. The variant exists to let
`/status` name that fault distinctly; on every other route it
behaves as `MalformedHost` does, which is why this is a REPORTING
split and not a policy change. Round-33 M2 refresh: `AuctionError` variants
were removed by v32 M4 (config loading occurs before
`run_auction`, so auction-level variants are unreachable
here); the canonical route error is the core-owned
`RuntimeConfigError` from `crates/auctioneer-core/src/config/
runtime.rs`. The M3a "runtime path never 500s"
guarantee holds only for M3a; M3b intentionally 500s on
malformed / broken-deployment paths so operators see the
error rather than have privacy policy silently disabled.
Note: no `AuthFailure` variant — the pinned upstream lacks a
typed auth-failure signal on both `SecretError` and
`ConfigStoreError`; permission-denied surfaces as
`SecretError::Internal` and lands in the malformed-family arm
(`StoreExtractionReason::InvalidSecretValue` /
`BackendFailure` → 500) under M3b (round-141 #2).

Round-120 status, SUPERSEDED round-141 #2: the gap this
paragraph recorded (G4 — "no upstream design yet") is CLOSED in
design AND code at the candidate EdgeZero head: the
sixteen-variant `#[non_exhaustive]`
`StoreExtractionReason` landed, and this section's contract is
the round-140 #4 total mapping over it. What remains
prereq-gated is only the ATOMIC pin move that makes the enum
importable, plus the G4-residue distinctions the M3b plan's
matrix tracks.

**Push-time is the fail-fast surface** (M3a AND M3b).
`auctioneer-cli config push` runs
`run_typed_preflight::<AuctioneerAppConfig>` before writing, so a
schema-invalid blob never reaches the store; `auctioneer-cli config
push --dry-run` produces the same diff without writing.
Operators inspect the WARN log (M3a) or 500 response (M3b) at
any regression to trace which push (or env-overlay) mis-fired,
then re-push with a corrected blob.

Merged types flow into `AuctionContext::with_request_start_ext` —
exactly TWO args beyond the base constructor (round-81 H1 — corrects
the stale "five"): `features: ExchangeFeatures` and
`bidder_timeout_override: &BTreeMap<BidderName, u32>`. Both are computed
by `auctioneer_core::config::resolve::{resolved_features,
resolved_bidder_timeouts}` from the same pushed `AuctioneerAppConfig`.

**tmax hard-ceiling guarantee.** `resolve_tmax` clamps the effective
deadline to `max_tmax_ms` even when the request omits `tmax`. Without
this clamp, a pushed config with `default_tmax_ms > max_tmax_ms`
would silently run past the supposed hard cap. The adapter-visible
`tmax` (what the bidder sees in `BidRequest.TMax`) passes through
verbatim — it advertises the bidder's slice, not the exchange's
execution budget.

**Dependency-purity invariant preserved.** `AuctioneerAppConfig` lives
in `auctioneer-core` (not `auctioneer-exchange`); resolvers produce owned
`HostConfig` / `AccountConfig` that the exchange consumes without
learning about the config store. Task 41's dependency-purity test
still passes — the library crates remain free of `edgezero-*`.

---

## 7. `auctioneer-privacy` — Privacy & Consent (baseline-fidelity with documented input caps)

A pure parsing + decision crate, behavior-compatible with the pinned baseline
(`gdpr/`, `privacy/`). Truth tables in **Appendix A** (GDPR), **Appendix C**
(activities / scrubbing); GPP precedence and error rules in **Appendix D**.

**Compatibility constraints (round-42 H2 canonical,
retires round-41 M1's "≤ 24 publisher restrictions"
figure — the TCF wire encodes the restriction count in
a 12-bit field per IAB Tech Lab TCF specification, so
the wire ceiling is 4,095 records, not 24. The 24-bit
`PurposeConsent` bitfield inside each restriction record
was the source of the round-41 confusion):**

The M3a Task 4/5/6 landing enforces these input caps:

- **Publisher restrictions ≤ 2,048 per TC string.** The
  TCF wire's 12-bit count field allows up to 4,095;
  auctioneer's decoded ceiling is set at 2,048 as an
  implementation limit that fits the M3a resolver's
  allocation budget. TC strings encoding more are
  rejected with
  `TcfParseError::TooManyPublisherRestrictions`.
  Legitimate CMPs never exceed a few dozen — the cap
  is far above the practical envelope but well under
  the wire maximum.
- Purpose IDs are validated to the 1-24 range at parse
  time (the wire allocates a 24-bit `PurposeConsent`
  bitfield); values > 24 are rejected.
- Vendor IDs > 65_535 are rejected (u16 storage keeps
  the vendor map compact).
- Special-feature IDs > 12 are rejected (wire is 12
  bits).

Legitimate TC strings from all conformant CMPs fit
inside these bounds. The word "full-fidelity" is
retired; auditors reading this heading know the M3a
resolver rejects some technically-parseable strings
that exceed the practical envelope. Future milestones
may loosen these caps if a CMP starts emitting values
outside them.

### 7.1 GDPR "applies" signal resolution

`Signal` ∈ {`Yes`(1), `No`(0), `Ambiguous`(-1)}. Resolution order:

1. `regs.gpp_sid` non-empty: `Yes` if section 2 (TCFEU2) present, else `No`.
   **GPP SID overrides `regs.gdpr` entirely.**
2. Else `regs.gdpr` if set: `0` → `No`, `1` → `Yes`, **any other
   value** → hard `Err(PrivacyError::InvalidRegsGdpr)` (round-46
   M1 — the privacy crate returns `PrivacyError`, NOT
   `AuctionError`, since `AuctionError` lives in `auctioneer-exchange`
   and a privacy→exchange edge is a Cargo cycle; the exchange
   boundary maps it to `AuctionError::BadInput` → `400` via
   `From<PrivacyError>`). Note this branch is only reached when
   `regs.gpp_sid` is empty (step 1 overrides `regs.gdpr`
   entirely), so `{gpp_sid:[2], gdpr:2}` is NOT rejected here.
   Round-37 B4: the earlier "`Ambiguous` + `BadInput`" phrasing
   conflated two paths; there is exactly one — the invalid value
   never yields a wire signal, the resolver short-circuits at the
   handler boundary.
3. Else `Ambiguous`.

**Geo-aware default:** start from host `gdpr.default_value`
(`"0"`→`No`, else `Yes`); if a device/user geo country is present, EEA → `Yes`,
well-formed non-EEA ISO-3 → `No`. **Precedence (round-86 B1):** the
`user.geo` OBJECT wins over `device.geo` — checked object-first, not
country-first. A PRESENT `user.geo` object SELECTS user-geo and blocks
the device-geo fallback: if its `country` is well-formed it decides
(EEA→`Yes` / non-EEA→`No`), and if its `country` is absent/malformed the
signal falls to the host `default_value` WITHOUT consulting `device.geo`.
`device.geo` is consulted only when the `user.geo` object is ABSENT.

**Enforcement — two NAMED signals, one for "applies" and one for
permissions** (round-71 H3 + round-72 H1 — names split so the two
distinct rules can't drift back into "same name two meanings"):

- **`auction_geo_signal: Signal`** (round-72 H1 — was the
  ambiguously-named "geo-aware applies") uses the **geo-aware**
  default ladder:
  - Start from `Signal::Ambiguous` (from §7.1 steps 1-3).
  - **Object-first, user-before-device (round-86 B1).** If the
    `user.geo` OBJECT is present it is SELECTED (device-geo not
    consulted): a well-formed `country` decides (EEA → `Yes`, non-EEA
    ISO-3 → `No`), an absent/malformed `country` falls to the host
    default. Else if the `device.geo` object is present, the same
    country test applies.
  - Otherwise (neither geo object present, or the selected object's
    country is absent/malformed), fall through to the **raw host
    `default_value`** (`"0"` → `No`, anything else → `Yes`).
    Drives `gdpr_applies = signal == Yes || (signal == Ambiguous &&
auction_geo_signal == Yes)`, and `gdpr_enforced = gdpr_applies
&& channel_enabled && host.gdpr.enabled`. **Auction-only** —
    this is the "is GDPR in scope?" question.
- **`sync_permissions_signal: Signal`** (round-72 H1 — what
  §7.8.1's `normalized_request_signal` exposes; the rename clears
  up that this signal NEVER consults geo) uses ONLY the **raw host
  `default_value`** when the wire signal is `Ambiguous`, NEVER a
  geo hint. Matches reference permission evaluation's subtlety (open item 12) and what reference TCF permission evaluation actually consults for
  `HostCookiesAllowed` / `BidderSyncAllowed`. Drives the §7.8.1
  `gdpr_permissions_enabled` boolean and the sync-endpoint
  parse-or-skip decision.

**Worked example** (round-72 H1 — replaces the earlier USA example
which was internally inconsistent; the geo-aware default DOES NOT
return Yes for USA + host default "1" — it returns No because
non-EEA wins over the host default, not the other way around):

- Request: `regs.gdpr` absent (parsed signal = Ambiguous);
  `device.geo.country = "USA"` (non-EEA); `host.gdpr.
default_value = "1"`.
- `auction_geo_signal`: geo ladder runs first — USA is well-formed
  non-EEA → **`No`**. Host default is NOT consulted because geo
  resolved cleanly. So `gdpr_applies = Ambiguous == Yes || (true
&& No == Yes) = false`. Auction proceeds with GDPR not in
  scope.
- `sync_permissions_signal`: NEVER touches geo. Raw host
  `default_value = "1"` → **`Yes`**. So
  `gdpr_permissions_enabled = host.gdpr.enabled && channel_enabled
&& Yes`. Sync endpoints (`/cookie_sync` / `/setuid`) treat the
  request as in-scope for the host-cookie permission path, even
  though the auction path treats it as out of scope.
- This is **the intended divergence**: upstream's auction path
  consults geo; upstream's sync path doesn't (sync only ever sees
  the host config and the wire signal). The two signals must
  stay distinct, and observable outcomes can differ on the same
  request.

**Observable on `/cookie_sync` and `/setuid`**: this matters for
the malformed / empty consent paths in §7.8.1 (round-72 H1) — a
request with `regs.gdpr` absent + USA geo + host default `"1"` +
empty `gdpr_consent` triggers /cookie_sync's empty-consent 400
(per the §7.8.1 + round-52 H2 ambiguous + normalized-Yes rule)
even though the auction path treats the same request as
out-of-scope. The §11.1 sync-endpoint matrix already covers this
case under the "absent gdpr + host default 1" fixture.

**Conflict warning** — when `regs.gdpr` is set _and_ `regs.gpp_sid` is
**non-nil** (including a present-but-empty array), auctioneer emits a non-fatal
warning `InvalidPrivacyConsent` (code 10001) noting that GPP takes
precedence. Matches reference auction handling warning (round-16 finding L6).
The signal itself is decided by §7.1 step 1 (non-empty SID drives signal;
empty array falls through to `regs.gdpr` but still produces this warning if
both are set).

`channel_enabled` resolves account `gdpr.channel_enabled.{web|app|amp|video|
dooh}` → account `gdpr.enabled` → host `gdpr.tcf2.enabled`.

**Channel classifier (round-87 B1 — corrects round-86 B2).** The channel
key is derived from the TRUSTED request type (route + inventory shape)
ONLY; `ext.prebid.channel.name` is request-controlled and MUST NOT be
consulted for the privacy gate (otherwise an `app` request could claim
`web` to reach a `channel_enabled.web = disabled` path and bypass `app`
enforcement — upstream derives this key from the trusted request type). On
`/openrtb2/auction` the key is inventory-derived: `dooh` present → `dooh`;
else `app` → `app`; else → `web`. `amp`/`video` are ROUTE-derived by their
dedicated endpoints (M5+), never from the request body. A request may carry
only one distribution-channel object (§3.2), so conflicting
`site`/`app`/`dooh` is a `prepare()` 400 and unreachable here.

### 7.2 TCF v2 consent string

- Parse the base64 TCF v2 string; **only encoding version 2** accepted.
- **Core-segment contents only; STRICT trailing-segment set
  (round-88 B1 as amended by rounds 108–110).** auctioneer splits on the
  first `.` and parses ONLY the leading core segment's CONTENTS for
  retention. The current IAB format defines exactly three segment
  types — Core (0), Disclosed Vendors (1, MANDATORY), Publisher TC
  (3) — so the trailing set must be EXACTLY `{DV}` or
  `{DV, PublisherTC}` (order-agnostic; `MAX_TRAILING_SEGMENTS = 2`).
  Every trailing segment is bounded-decoded and FRAMING-VALIDATED
  (DV: SegmentType + MaxVendorId + IsRangeEncoding + complete
  bitfield / well-formed bounded range list — a first-char-only
  check would let a truncated segment masquerade, round-109 B1;
  PublisherTC: the 57 + 2N-bit layout with < 8 pad bits). REJECTED,
  each fail-closed as unusable consent (`default_permissions()` row
  - the `invalid_privacy_consent` warning — never a 400): zero DV
    (`MissingDisclosedVendorsSegment`), duplicate DV or PubTC
    (`DuplicateDisclosedVendorsSegment` /
    `DuplicatePublisherTcSegment`), truncated/malformed framing
    (`MalformedDisclosedVendorsSegment` /
    `MalformedPublisherTcSegment`), empty segments
    (`EmptyTrailingSegment`), more than two trailing segments
    (`TooManyTrailingSegments`), and any other type — the obsolete
    type 2 (Allowed Vendors/OOB) included
    (`UnknownSegmentType`). Validated trailing CONTENTS are discarded
    (DV membership is retained starting M4 for special-purpose
    disclosure enforcement; publisher-TC enforcement is a later
    milestone). The 64 KiB `MAX_INPUT_LEN` cap is measured on the CORE
    substring.
- **Stricter-than-upstream core-segment validations (round-89 H3 — these
  are OBSERVABLE divergences from upstream, which performs none of them;
  each is enumerated in §15 with its exact error):**
  - **Policy-version cutoff:** a `tcfPolicyVersion < 4` string whose
    core `Created` is on/after 2023-10-01 → reject
    (`PolicyVersionTooOldForCreated`) — round-90 M6: the cutoff is
    `< 4` (all pre-v4 policy versions), matching the M3a plan's Task 3
    rule; the earlier "`== 3`" wording here under-specified it. upstream
    accepts it.
  - **Timestamp equality + day-truncation:** the core `Created` and
    `LastUpdated` must be EQUAL and day-truncated (no sub-day component)
    → else reject (`CreatedLastUpdatedMismatch` / `TimestampNotDayTruncated`).
  - **`IsServiceSpecific == 0` rejected** unconditionally
    (`NotServiceSpecific`).
  - The 64 KiB `MAX_INPUT_LEN` core cap and the decoded/range caps (the
    M3a plan's Task 3 "Caps derived from wire-format capacity" table —
    round-90 M6: NOT spec Appendix D, which is the GPP precedence/error
    section) also reject strings upstream would accept.
- Decoded model exposes `PurposeAllowed(id)`, `PurposeLITransparency(id)`,
  `VendorConsent(id)`, `VendorLegitInterest(id)`, `SpecialFeatureOptIn(id)`,
  `PurposeOneTreatment()`, `CheckPubRestriction(purpose, type, vendor)`,
  policy version (GVL spec version 2 vs 3).
- **TCF v2.4 (present in the published IAB TCF specification; round-69
  M1 — do not cite PR #443's merge state, which is not the source of
  truth) removes the legacy Special-Purpose-only-vendor
  Legitimate-Interest workaround** — a vendor declaring ONLY special purposes (no regular
  purposes) no longer needs to also assert the Vendor
  Legitimate-Interest bit to be valid. auctioneer's model already treats
  **special purposes as requiring no legal basis** (they are always
  allowed and never gate on `VendorConsent`/`VendorLegitInterest`),
  so auctioneer never implemented the removed workaround and no code
  change is required — recorded here so the v2.4 audit is closed, not
  silently skipped. The v2.4 core-segment ENCODING limits (range caps,
  `PurposeId 1..=24`) are unchanged from v2.3 (round-88 B1 — `MAX_SEGMENTS`
  no longer applies since auctioneer parses core-only).
- **Empty / malformed consent** (round-48 H1 + round-52 H1+H2 +
  round-53 M3 + round-73 H1 — endpoint + signal split per §7.1's
  two-signal taxonomy: auction paths key off `auction_geo_signal` /
  `gdpr_enforced`; sync paths key off `sync_permissions_signal` /
  `gdpr_permissions_enabled`. The two can diverge on the same
  request, see the §7.1 USA worked example):
  - **Auction: `!gdpr_enforced`** (i.e. `!gdpr_applies` from
    `auction_geo_signal`, or channel disabled, or
    `!host.gdpr.enabled`) → allow-all on the auction; TCF never
    parsed for the auction path (reference TCF permission evaluation
    short-circuit).
  - **Sync: `sync_permissions_signal != Yes` OR
    `!gdpr_permissions_enabled`** → sync endpoints **silently
    ignore** malformed TCF (no 400, no privacy block); TCF never
    parsed for the sync path.
  - **Auction: `gdpr_enforced` + empty consent** →
    `default_permissions()`. Auction never returns a hard error
    for empty consent — this is a privacy decision, not a request
    error.
  - **Sync: `sync_permissions_signal == Yes` +
    `gdpr_permissions_enabled` + empty consent** → split
    (round-52 H2):
    - `/cookie_sync` → **`400`** when `parsed_request_signal ==
Yes` OR (`Ambiguous` + normalized Yes) — matches
      reference cookie-sync handling + reference cookie-sync handling fixtures.
    - `/setuid` → **`400`** only when `parsed_request_signal ==
Yes` (matches reference UID-write handling); Ambiguous + normalized Yes
      falls into the host-cookie permission path.
  - **Auction: `gdpr_enforced` + malformed (non-empty) consent** →
    `default_permissions()`. Auction-only — sync paths handled
    below using their own signal.
  - **Sync: `sync_permissions_signal == Yes` +
    `gdpr_permissions_enabled` + malformed (non-empty) consent**:
    - `/cookie_sync` → per-syncer privacy denial (200 + empty
      `bidder_status`, round-47 H1).
    - `/setuid` → `400` from the permission path **only when ALL of**
      `host_vendor_id != 0` AND `sync_permissions_signal == Yes`
      AND `gdpr_permissions_enabled` AND activity already allowed
      (round-53 H2 + round-58 M1 — full preconditions; the
      permission path runs in §8.3 step 11 AFTER activity at step
      10, so `syncUser`-denied wins with 451, not 400 — matches
      reference UID-write handling). Otherwise `200` (host_vendor_id ==
      0 via reference GDPR permission evaluation short-circuit, or host disables GDPR via
      reference GDPR permission evaluation AlwaysAllow) or `451` (activity denied).
      See §7.8 for the full policy.

### 7.3 Per-purpose enforcement, legal basis, permissions

- **Per-purpose config:** `enforce_algo` (`full`|`basic`), `enforce_purpose`,
  `enforce_vendors`, `vendor_exceptions`. Account overrides tri-state.
  `basic_enforcement_vendors` is account-only.
- **Full enforcement** consults the embedded GVL + TCF publisher
  restrictions. **Basic enforcement** never touches the GVL _within the
  per-purpose legal-basis walk_ (Appendix A.3) — this describes the
  algorithm's inputs, NOT an exemption from the fail-closed gate
  (round-86 B3). When enforcement is active and the CMP-declared `(spec,
list)` GVL is unavailable (or the TC string declares a policy version
  NEWER than the bundled blob's — the asymmetric check ratified at
  execution: `tcf_policy_version > gvl.tcf_policy_version()`; see the
  §15 divergence row. An OLDER string against a newer list is the
  normal forward-compatible case and stays usable. The earlier
  symmetric "disagrees with the TC
  string), `resolve` sets `consent_usable = false` and `permissions()`
  short-circuits to the Appendix A.4 default row BEFORE the A.2/A.3 walk
  runs (round-52 Critical), so a basic-enforced or `vendor_exceptions`
  bidder is still denied `PassID`. A full-enforced purpose is
  auto-downgraded to basic for any bidder in `basic_enforcement_vendors`.
- Legal-basis algorithms (full / basic) — Appendix A.2 / A.3.
- Publisher restrictions (full and basic): matching `NotAllowed`, or both
  `RequireConsent` and `RequireLegitInterest` for the same purpose/vendor,
  deny before disabled enforcement, vendor exceptions, basic-list bypasses,
  or the LI-transparency override. Duplicate same-type matches and restrictions
  on a different purpose/vendor are not conflicts. Only full enforcement
  remaps the legal basis for a single `RequireConsent` or
  `RequireLegitInterest`; basic still does not consult GVL declarations.
- Basic's LI-transparency override requires a nonzero resolved vendor ID.
  This does not add that requirement to its other allow rows (Appendix A.3).
  These safeguards are an owner-approved Auctioneer policy tightening
  (2026-10-12), not a normative IAB mandate or legacy Basic compatibility.
- **`AuctionPermissions`** per bidder: `AllowBidRequest` (Purpose 2),
  `PassGeo` (Special Feature 1), `PassID` (OR of legal basis over Purposes
  2–10). `default_permissions()` = `{AllowBidRequest: !P2_enforced, PassGeo:
!SF1_enforced, PassID: false}`.
- Vendor id from `BidderInfo.gvl_vendor_id` or request
  `ext.prebid.aliasgvlids`; id 0 → both consent and LI fail under full
  enforcement.

### 7.4 GPP

- Parse the GPP string + `gpp_sid`. **Decoded/enforced sections:** 2 (TCFEU2
  → GDPR), 6 (USPV1 → US-Privacy). Sections 3 and 7–12 are recognized and
  passed through within `regs.gpp` but **not decoded or enforced** (matches
  Upstream v4; open item 4).
- **`gpp_sid` consumers (review finding 7).** The section-id list is used by
  (a) GDPR signal resolution — section 2 presence as parsed-int membership
  (§7.1), and (b) usersync `SkipWhen.gpp_sid` — **exact-string match** of
  the raw request `gpp_sid` against each configured skip-string (not
  parsed-section membership; matches upstream, Appendix B.3 step 9; round-15
  finding M5). It does **not** feed config-driven activity rules: upstream
  v4's activity conditions match only `componentName` / `componentType`
  (Appendix C.1) — there is no GPP condition in activity config. GPP
  therefore gates GDPR and usersync skip, not activities directly.
- **Precedence — "GPP wins"** when the relevant section/SID is present;
  legacy fields produce non-fatal conflict warnings (Appendix D).
- **Parse errors** — Appendix D error table.
- **Legacy downgrade — fills empty fields only (review finding 3).** For
  bidders without `openrtb.gpp_supported`, back-fill the legacy fields from
  GPP **only when the legacy field is empty/absent**: set `regs.gdpr` only if
  `nil`; `user.consent` only if empty; `regs.us_privacy` only if empty. A
  populated legacy field is **never overwritten**.

### 7.5 CCPA / US-Privacy & LMT / COPPA

- **CCPA:** the 4-character US-Privacy string. Index 0 must be `'1'`; indices
  1–3 ∈ {`Y`,`N`,`-`}. **Only index 2 (opt-out-of-sale) drives
  enforcement for valid strings.** A selected invalid nonempty string causes
  a nonfatal warning and enabled CCPA ID/geo scrubbing; invalid legacy bytes
  are cleared without fabricating a valid opt-out value. Source:
  `regs.us_privacy` / GPP USPV1 (Appendix D). No-sale list from
  `request.ext.prebid.nosale` (`["*"]` = all bidders). `ShouldEnforce(bidder)
= enabled && (valid_sale_opt_out || selected_usp_malformed) && bidder ∉ no_sale`;
  `enabled` from account `ccpa.channel_enabled` → account `ccpa.enabled` → host `ccpa.enforce`
  (default false).
  Account settings use tri-state inheritance; trusted inventory/route selects
  channel, not `ext.prebid.channel.name`. No-sale names match the observable
  bidder/alias exactly, including validated aliases dropped only for disablement.
  Unknown names and mixed wildcard lists are `400` even when CCPA is disabled
  or USP is malformed. Only `["*"]` exempts all; exemptions do not affect other
  policies. See the [M4 design](2026-09-26-m4-privacy-design.md) for the complete
  validation and source-selection contract.
- **LMT:** `ShouldEnforce = lmt.enforce && device.lmt == 1` — host
  `lmt.enforce` (default `false`) gates the request-wide scrub; the iOS
  ATT override (`device.ext.atts` / iOS-14 IFA rules) sets `device.lmt`
  Upstream in `prepare()` step 1 (§3.2). Matches upstream's
  `EnabledPolicyEnforcer{Enabled: cfg.LMT.Enforce}`.

  **iOS ATT → `device.lmt` mapping table** (round-34 finding M4 — the
  normative rule, only applied when `app` is set and
  `device.os.eq_ignore_ascii_case("ios")`):

  | `device.ext.atts` | Status        | Action on `device.lmt`                                |
  | ----------------- | ------------- | ----------------------------------------------------- |
  | `0`               | NotDetermined | **set to `1`** (overrides any prior value)            |
  | `1`               | Restricted    | **set to `1`** (overrides any prior value)            |
  | `2`               | Denied        | **set to `1`** (overrides any prior value)            |
  | `3`               | Authorized    | **set to `0`** (overrides any prior value)            |
  | other / missing   | unknown       | **no change** — preserve client-supplied `device.lmt` |

  The override **always wins** over a client-supplied `device.lmt` for
  ATT codes 0–3 — this is an exception to the general "never overwrite
  client-supplied values" rule in §3.2 step 1, because the ATT signal is
  authoritative for iOS LMT and a client-side mismatch is suspect.

- **COPPA:** `regs.coppa == 1` → unconditional request-wide scrub with full
  geo and named user-FPD removal. No toggle, no exception. The stricter FPD
  and malformed-USP treatment above are approved Auctioneer policies, not IAB
  deletion or invalid-input mandates.

### 7.6 Activity-control framework & scrubbing

- **Activities** (reference activity evaluation): `syncUser`, `fetchBids`,
  `enrichUfpd`, `reportAnalytics`, `transmitUfpd`, `transmitPreciseGeo`,
  `transmitUniqueRequestIds`, `transmitTid`. **Phase 1 enforces the five live
  in the auction/sync paths:** `syncUser`, `fetchBids`, `transmitUfpd`,
  `transmitPreciseGeo`, `transmitTid`. `reportAnalytics` is plumbed but inert
  until the analytics phase; `enrichUfpd` and `transmitUniqueRequestIds` are
  inert in upstream v4 too.
- **Config:** `account.privacy.allowactivities.{activity}` = `{ default:
bool?, rules: [{ condition: { componentName[], componentType[] }, allow:
bool }] }`.
- **Evaluation:** default = **allow** when unset; rules in order, first
  non-abstaining wins (no deny-override); a rule abstains if any clause fails
  (case-insensitive, empty = match-all). `Component = { type ∈ {bidder,
analytics, rtd, general}, name }`.
- **API:**
  ```rust
  /// Round-37 B4: canonical shape.
  /// - `gvl_store: &GvlStore` (not `&Gvl`) — resolver picks the
  ///   exact `(spec, list)` per TC string; an unbundled pair FAILS
  ///   CLOSED by setting `consent_usable = false` so `permissions()`
  ///   returns the Appendix A.4 default row (enforced-but-empty)
  ///   directly, plus a `gvl_version_unavailable` warning
  ///   (round-52 Critical — NOT the withdrawn `gvl_unavailable`
  ///   constructor; see the assembly note below).
  /// - `Result<_, PrivacyError>` (round-45 H1 — NOT
  ///   `AuctionError`, which lives in `auctioneer-exchange`;
  ///   `auctioneer-exchange` already depends on `auctioneer-privacy`,
  ///   so returning the exchange error from the privacy crate
  ///   is a Cargo cycle. `auctioneer-privacy` owns `PrivacyError`;
  ///   `auctioneer-exchange` provides `From<PrivacyError> for
  ///   AuctionError`, mapping `InvalidRegsGdpr` →
  ///   `BadInput`). Invalid `regs.gdpr` (a value other than
  ///   0/1) propagates as `Err(PrivacyError::InvalidRegsGdpr)`
  ///   ONLY when `regs.gpp_sid` is absent/empty — a non-empty
  ///   `gpp_sid` overrides `regs.gdpr` entirely (§7.1 rule 1),
  ///   so the invalid value is never consulted and resolve is
  ///   `Ok`. §5.1 step 6.5 maps the `Err` to HTTP 400 at the
  ///   handler via the `From` impl. Empty consent, valid
  ///   consent, and unavailable GVLs are all `Ok(_)`; the
  ///   unusable-consent cases (missing/malformed TCF, unavailable
  ///   GVL) yield the ENFORCED-BUT-EMPTY row plus a warning —
  ///   never allow-all (round-50 B2).
  /// M4: normalized source selection precedes independent policy resolution.
  /// Options carry a narrow GDPR exemption and validated NoSale identities;
  /// neither bypasses activity, CCPA, LMT, COPPA, or terminal input limits.
  /// The result owns the context and borrows legacy-projection inputs from req.
  PrivacyContext::resolve<'a>(&'a BidRequest, &AccountConfig, &HostConfig,
                              &GvlStore, &Gcl, time::OffsetDateTime,
                              ResolveOptions)
      -> Result<ResolvedPrivacy<'a>, PrivacyError>
  /// Test-only convenience for structural auction tests. Live auctions always
  /// resolve privacy, including GDPR-disabled/out-of-scope/NSP requests.
  PrivacyContext::allow_all_no_scrub() -> PrivacyContext
  /// **Round-50 B2 (SECURITY): unavailable `(spec, list)` FAILS
  /// CLOSED.** The predecessor `default_permissions_with_warning`
  /// yielded M2-permissive (allow-all) purpose resolution, which
  /// handed bidders IDs and precise geo that no consent record
  /// supported whenever the archive lagged a GVL revision. When
  /// enforcement is active and the CMP-declared GVL is absent,
  /// auctioneer cannot evaluate Purpose 2 or Special Feature 1, so it
  /// applies the SAME enforced-but-empty row it already applies to
  /// missing/malformed TCF (Appendix A.4 / Appendix E):
  ///   AllowBidRequest = !P2_enforced
  ///   PassGeo         = !SF1_enforced
  ///   PassID          = false
  /// This is NOT AllowNone — bidding still proceeds unless the
  /// operator enforces P2 — but it never leaks identifiers.
  /// **Round-52 Critical: the fail-closed guarantee is the
  /// `consent_usable = false` SHORT-CIRCUIT, not an empty GVL.**
  /// `resolve` sets `consent_usable = false` (for empty/absent,
  /// malformed-TCF, AND GVL-unavailable) and `permissions()` returns
  /// the A.4 default row BEFORE the A.2/A.3 walk. The round-51 claim
  /// that substituting `Gvl::empty()` "makes every lookup miss so
  /// the walk yields enforced-but-empty" was WRONG — `vendor_
  /// exceptions` and basic enforcement bypass the GVL and would
  /// still grant `PassID`. The empty GVL is now just a placeholder;
  /// the short-circuit is the guarantee. The `gvl_version_
  /// unavailable` warning is pushed onto the ALREADY-accumulated
  /// warning vector (preserving any earlier gpp_sid-conflict
  /// warning). The round-50
  /// `gvl_unavailable(&EffectiveEnforcement, ..)` signature is
  /// withdrawn — `EffectiveEnforcement` was never defined.
  /// Out-of-scope traffic skips GDPR authorization/GVL selection only.
  /// M4 still normalizes sources and resolves every independent policy.
  PrivacyContext::take_warnings(&mut self) -> Vec<NonFatalWarning>  // §7.7
  PrivacyContext::permissions(core_name, bidder) -> AuctionPermissions
  PrivacyContext::activity_allow(activity, component) -> bool
  PrivacyContext::bidder_decision(core_name, bidder) -> BidderDecision
  /// Exchange-owned scrub composition runs after bidder FPD projection.
  /// Both adapter hooks receive the same sanitized, legacy-projected request.
  ```
- **Scrubbing** — Appendix C. Primitives mirror upstream's
  reference privacy projection.

### 7.7 Consent-syntax warnings (auction path)

`prepare()` (§3.2) deliberately does **not** parse consent strings — that
would require `auctioneer-openrtb` to depend on consent parsers and induce a
`auctioneer-openrtb → auctioneer-privacy → auctioneer-openrtb` cycle (round-37 H3).
Instead, **`PrivacyContext::resolve` emits the non-fatal consent-syntax
warnings** during §5.1 step 6:

- Selected TCF syntax failure emits code 10001 with source-specific scope:
  `user.consent` for legacy, `regs.gpp` with a fixed section-2 message for
  active GPP. Empty/invalid authoritative section 2 does not retry granting
  legacy consent. Inactive TCF is not authorization-parsed. Existing M3
  authenticity and authorization checks still apply to the selected source.
- Invalid nonempty legacy USP emits code 10001 at `regs.us_privacy`, even
  when CCPA is disabled, and is cleared before adapter construction. An
  invalid selected GPP USP has a distinct fixed section-6 message scoped to
  `regs.gpp`; a nonempty invalid active value does not fall back to legacy.
- Within-limit malformed GPP framing emits a fixed code-10001 warning at
  `regs.gpp` and permits the specified legacy fallback. Unknown section
  payloads stay opaque. Hard envelope limits and arithmetic overflow instead
  return terminal `PrivacyError::GppLimitExceeded`, mapped to HTTP 400 with
  zero adapter calls/sends; they never become warning-only fallback.
- Structural wire warnings and selected-source conflicts retain their fixed
  ordering and provenance. USP conflict warnings do not disable enforcement.
  Invalid `nosale` is independently a terminal HTTP 400, not a warning.

The [M4 design, section 9](2026-09-26-m4-privacy-design.md#9-errors-observability-and-resources)
defines warning order, deduplication, scopes, and fixed text. Diagnostics do
not interpolate raw consent strings or parser errors. The legacy OpenRTB 2.5
privacy fields remain rejected by `prepare()` before this stage (§3).

`run_auction` collects these via
`PrivacyContext::take_warnings()` (consume-on-build) and merges them with
`NormalizedBidRequest.warnings()` (cloned, §3.2) when populating
`ext.warnings.prebid[]`. The structural-test convenience constructor
`allow_all_no_scrub` emits no warnings and is not a live-auction bypass.
GDPR permissions retain Appendix A.4 semantics; independent M4 policies
continue to apply even when GDPR itself is disabled or out of scope.

### 7.8 Sync-endpoint privacy API — `resolve_sync_privacy`

The Rust struct/callback block in this section records the historical
design sketch, not the current M5 signatures. The live API is the typed
`SyncPrivacyInput` / `SyncPrivacyContext` in
[`auctioneer-privacy::sync`](../../../crates/auctioneer-privacy/src/sync/mod.rs), with
closed extraction errors and host, candidate, and `/setuid` permission
methods. In particular, the M2-M4 allow-all callbacks are not used by
registered sync handlers. The endpoint decision rules below remain the
behavioral contract unless an M5 clarification explicitly supersedes them.

`PrivacyContext::resolve` is auction-shaped (takes a `&BidRequest`),
which does not fit `/cookie_sync` and `/setuid` (round-37 M4 — those
endpoints carry query/body fields, not a `BidRequest`). Sync endpoints
have **endpoint-specific hard-error and permission policies** (round-57
M1 — corrects the earlier blanket "malformed consent → hard 400"
claim, which was wrong after round-47 H1 + round-53 H2 narrowed both
endpoints separately):

- **Request-extraction hard-400s** apply uniformly to bad GPP, bad
  `gpp_sid`, and unresolved `Invalid` legacy `gdpr`. Empty consent
  request-extraction-400s differ between endpoints (§7.8.1 + round-52
  H2): /cookie_sync 400s on parsed Yes OR (Ambiguous + normalized
  Yes); /setuid 400s only on parsed Yes.
- **Malformed non-empty TCF** is NOT a request-extraction 400 on
  either endpoint. /cookie_sync converts it to per-syncer privacy
  denial (200 + empty `bidder_status`, round-47 H1 + round-49 H2);
  /setuid routes it through the host-cookie permission path
  (round-53 H2 + round-56 H1 + round-60 L1 — full dependency list)
  which yields 400 / 200 / 451 depending on four conditions:
  (a) `host_vendor_id`, (b) `normalized_request_signal`,
  (c) `gdpr_permissions_enabled`, AND (d) **the prior `syncUser`
  activity result** (the permission path runs in §8.3 step 11
  AFTER activity at step 10; `syncUser`-denied wins with 451). See
  the per-endpoint matrices below + §10 / Appendix F for the full
  outcome table.

The sync endpoints use a separate, narrower API. The API must
also support the per-syncer decisions §8.2 / Appendix B.3 actually
requires — `BidderSyncAllowed` (GDPR), `SkipWhen.gpp_sid` exact-string
matching, `syncUser` activity per component — so the returned context
exposes both **request-wide** state and **two per-syncer callbacks**:
`sync_allowed` for /cookie_sync (chooser ladder) and `setuid_allowed`
for /setuid (host-cookie GDPR + disabled-syncer only) (round-38 H1 +
round-54 H1 + round-57 L4).

```rust
pub struct SyncPrivacyInput<'a> {
    /// /cookie_sync body schema (B.1) sends `"gdpr": 0` as a JSON number;
    /// /setuid query schema (B.4) sends `gdpr=0` as a string. The caller
    /// pre-coerces the wire value into `GdprSignalIn` — round-46 H1
    /// REVISES this to match upstream's GPP-precedence behavior:
    ///   - Recognized values (0 / 1 / absent) → Zero / One / Absent.
    ///     JSON null and an empty /setuid query value also mean Absent;
    ///     a JSON string, including "", remains Invalid.
    ///   - **Unrecognized value** (any other shape: `2`, `"yes"`, an
    ///     object, etc.) → `GdprSignalIn::Invalid(<raw wire bytes>)`,
    ///     NOT a hard-400 at the wire surface. Whether `Invalid` causes
    ///     a 400 is decided **inside `resolve_sync_privacy` after GPP
    ///     /SID precedence runs**: if `regs.gpp_sid` resolved a signal
    ///     (TCFEU2 section present or non-empty SID), `Invalid` is
    ///     silently ignored (matches upstream reference cookie-sync handling
    ///     where `gpp_sid` is checked before `IntSignalParse`, and
    ///     reference UID-write handling which only rejects malformed legacy
    ///     `gdpr` when GPP did not already provide a signal). Otherwise
    ///     `resolve_sync_privacy` returns `Err(MalformedConsent { scope:
    ///     "gdpr" })` and the caller emits the endpoint-specific 400
    ///     body (round-44 M5 / round-45 M2).
    pub gdpr:        GdprSignalIn,
    pub consent:     Option<&'a str>,   // TCF v2 string (gdpr_consent)
    pub us_privacy:  Option<&'a str>,   // US-Privacy 4-char
    pub gpp:         Option<&'a str>,   // GPP string
    pub gpp_sid:     Option<&'a str>,   // **raw** request string, preserved
                                        // verbatim — SkipWhen does
                                        // exact-string matching, NOT
                                        // parsed-section membership
                                        // (Appendix B.3 step 9 / round-15
                                        // M5). Caller passes the raw query
                                        // / body value; the API never
                                        // re-stringifies a parsed list.
    pub account:     &'a AccountConfig,
    pub host:        &'a HostConfig,
    /// Which sync endpoint is calling. Affects multiple policies —
    /// implementers must NOT collapse the two endpoints to a single
    /// path (round-47 H1 + round-52 H2 + round-53 M4 — earlier
    /// "only affects malformed-TCF policy" comment was incomplete).
    /// Endpoint-specific behaviors:
    /// 1. **Empty consent** (round-60 L2 — taxonomy-correct phrasing):
    ///    - /cookie_sync 400s when `parsed_request_signal == Yes`
    ///      OR (`parsed_request_signal == Ambiguous` AND
    ///      `normalized_request_signal == Yes`) — reference cookie-sync handling
    ///    - /setuid 400s only when `parsed_request_signal == Yes`
    ///      — reference UID-write handling.
    /// 2. **Malformed non-empty TCF** (round-57 M2 — full
    ///    precondition list; earlier "when host_vendor_id != 0"
    ///    short-form omitted three of the four conditions and
    ///    could mislead implementers into building the wrong
    ///    branch):
    ///    - /cookie_sync → per-syncer privacy denial (200 + empty
    ///      bidder_status), as required by the per-syncer privacy decision.
    ///    - /setuid → 400 from the permission path (NOT request
    ///      extraction) **only when ALL four conditions hold**:
    ///      (a) host.gdpr.host_vendor_id != 0,
    ///      (b) normalized_request_signal == Yes,
    ///      (c) gdpr_permissions_enabled (host.gdpr.enabled AND
    ///          channel_enabled),
    ///      (d) the syncUser activity check has ALREADY ALLOWED
    ///          the request (the permission path runs in §8.3
    ///          step 11, AFTER activity at step 10 — so
    ///          syncUser-denied wins with 451, not 400 — round-
    ///          54 H3).
    ///      If any of (a)-(c) is false the parse is skipped → 200
    ///      (host_vendor_id == 0 via AllowHostCookies wrapper,
    ///      reference GDPR permission evaluation; or AlwaysAllow via reference GDPR permission evaluation).
    ///      reference UID-write handling + reference GDPR permission evaluation + reference TCF permission evaluation.
    /// 3. **`sync_allowed` vs `setuid_allowed` decision shape**:
    ///    - /cookie_sync uses `sync_allowed` (chooser ladder:
    ///      BidderSyncAllowed + SkipWhen.gpp_sid + disabled syncer
    ///      + activity).
    ///    - /setuid uses `setuid_allowed` (narrower: host-cookie
    ///      GDPR + disabled syncer; NO BidderSyncAllowed, NO
    ///      SkipWhen.gpp_sid) — reference UID-write handling.
    /// All other fields (gdpr GPP-precedence, GPP/sid hard-400,
    /// USP silent-clear, account/host lookup) behave identically
    /// across both endpoints.
    pub endpoint:    SyncEndpoint,
    pub gvl:         &'a Gvl,           // round-38 H1 — full GDPR
                                        // enforcement (§7.3) needs vendor
                                        // declarations + publisher
                                        // restrictions; sync_allowed()
                                        // consults the GVL like the
                                        // auction path does.
}

pub enum GdprSignalIn {
    Absent,
    Zero,
    One,
    /// Wire value the caller couldn't coerce to 0/1 — preserved here
    /// so `resolve_sync_privacy` can decide whether to emit
    /// `MalformedConsent { scope: "gdpr" }` (GPP-precedence rule;
    /// round-46 H1). The wrapped string is the raw bytes used in
    /// error-body formatting.
    Invalid(String),
}

pub enum SyncEndpoint { CookieSync, SetUid }   // round-47 H1

pub enum SyncPrivacyError {
    /// **Request-extraction** consent errors only (round-55 M3 —
    /// narrowed after round-53 H2 moved malformed non-empty TCF to
    /// the downstream permission path):
    /// - `scope: "gdpr"` — `GdprSignalIn::Invalid(_)` AND GPP/SID
    ///   did not override (round-46 H1).
    /// - `scope: "gdpr_consent"` — empty consent under
    ///   `parsed_request_signal == Yes` for /setuid; for
    ///   /cookie_sync, also under `Ambiguous` + normalized Yes
    ///   (round-52 H2). Malformed NON-empty TCF does NOT use this
    ///   variant — it returns `Ok(_)` and the parse decision lives
    ///   in `HostCookiesAllowed` (per-syncer denial on
    ///   /cookie_sync via per-syncer privacy evaluation; permission-path 400
    ///   on /setuid **only when ALL of** host_vendor_id != 0 AND
    ///   normalized_request_signal == Yes AND
    ///   gdpr_permissions_enabled AND activity already allowed —
    ///   round-58 M1; see SyncPrivacyInput::endpoint doc for the
    ///   full precondition list).
    /// - `scope: "gdpr_consent"` (alt) — malformed TCF on /setuid
    ///   is NOT raised here; see §8.3 step 11.
    /// USP malformation is NOT an error here (silently cleared,
    /// round-41 M3). GPP / `gpp_sid` malformation uses their own
    /// variants below.
    MalformedConsent { scope: &'static str },   // → 400
    MalformedGpp,                               // → 400 (envelope or section)
    MalformedGppSid,                            // → 400 (invalid applicability encoding)
    BadInput(String),                           // → 400 (other request-shape errors)
}

/// Identifies a syncer slot to the per-syncer decision method. The
/// caller resolves bidder-name → BidderInfo + Syncer config from the
/// build-time registry (§6) and passes them in. round-38 H1 + round-39
/// M3.
///
/// **No request aliases on sync endpoints.** Unlike `/openrtb2/auction`,
/// neither `/cookie_sync` (Appendix B.1) nor `/setuid` (Appendix B.4)
/// carries `ext.prebid.aliases` or `ext.prebid.aliasgvlids` — the wire
/// schemas have no place for them. The only aliases the sync endpoints
/// recognize are **host/config-declared aliases** baked into the
/// build-time bidder registry (the same aliases `/info/bidders` reflects
/// per §8.1). `BidderInfo.gvl_vendor_id` is therefore the sole vendor-id
/// source for `sync_allowed`'s GDPR enforcement; the auction-path
/// `aliasgvlids` override (§5.1 step 7) does not apply here.
pub struct SyncerLookup<'a> {
    pub bidder:      &'a BidderInfo,    // gvl_vendor_id resolved from the
                                        // build-time registry; no request
                                        // aliasgvlids override on sync.
    pub syncer_key:  &'a str,           // syncer family key
    pub syncer:      &'a SyncerConfig,  // `enabled`, `skip_when`, etc.
}

pub enum SyncDecision {
    Allowed,
    BlockedByPrivacy,           // → StatusBlockedByPrivacy (B.3 step 6/7)
    BlockedByDisabledUsersync,  // → StatusBlockedByDisabledUsersync (B.3 step 8)
    BlockedByRegulationScope,   // → StatusBlockedByRegulationScope (B.3 step 9)
}

pub struct SyncPrivacyContext<'a> {
    // === §7.8.1 signal taxonomy (round-51 M2 + round-54 H1 — these
    // fields ship as part of the API surface; the earlier "kept for
    // back-compat" notes referred to gdpr_signal / gdpr_enforced
    // which are aliases for normalized_request_signal /
    // gdpr_permissions_enabled).
    pub parsed_request_signal:     Signal,   // wire-only: GPP-SID
                                             // precedence then legacy
                                             // gdpr; never consults host
                                             // defaults. Drives the
                                             // empty-consent 400 trigger
                                             // (reference cookie-sync handling,
                                             // reference UID-write handling).
    pub normalized_request_signal: Signal,   // (a.k.a.
                                             // `sync_permissions_signal`
                                             // per §7.1 / round-72 H1)
                                             // parsed + RAW host
                                             // default_value, NEVER geo;
                                             // matches §7.1
                                             // sync_permissions_signal
                                             // and reference TCF permission evaluation's
                                             // HostCookiesAllowed gate
                                             // default substitution on
                                             // Ambiguous (§7.1).
    pub gdpr_permissions_enabled:  bool,     // host.gdpr.enabled
                                             // && channel_enabled
                                             // && normalized == Yes.
                                             // When false → permissions
                                             // builder returns
                                             // AlwaysAllow (reference GDPR permission evaluation).

    // Back-compat aliases for §8.2 step 5 prose written before the
    // taxonomy was named (round-51 M2). Implementations may expose
    // these as `Deref`-style getters that return the corresponding
    // taxonomy field above.
    pub gdpr_signal:               Signal,   // alias for normalized_request_signal
    pub gdpr_enforced:             bool,     // alias for gdpr_permissions_enabled

    pub ccpa_enforce:   bool,            // host-syncer scope; per-syncer
                                         // decisions still go through
                                         // sync_allowed() / setuid_allowed().
    // No GPC field on sync (round-39 L5). Neither /cookie_sync (B.1) nor
    // /setuid (B.4) wire schemas carry Sec-GPC, and upstream's sync code
    // does not consult GPC either; the auction path's GPC handling
    // (§5.1 step 5 / §5.2 outbound forward) is auction-only.
    pub raw_gpp_sid:    Option<&'a str>, // exposed so callers can also
                                         // log / surface the raw value
                                         // without re-stringifying.

    // Host-cookie GDPR gate uses these directly (no per-syncer call).
    // /cookie_sync uses this directly at §8.2 step 8; /setuid feeds
    // it into setuid_allowed via the host-cookie permission path
    // (§8.3 step 11).
    pub host_cookie_sync_allowed: bool,  // matches prebid
                                         // reference cookie-sync handling's host-cookie
                                         // GDPR check using the
                                         // host's own syncer; gated by
                                         // host.gdpr.host_vendor_id
                                         // (0 → always true via
                                         // AllowHostCookies wrapper,
                                         // reference GDPR permission evaluation).

    // Per-component activity check (matches §7.6 truth table). Used by
    // /setuid for `activity_allow(syncUser, Component::bidder(name))`.
    // HRTB so callers can pass request-borrowed bidder-name slices
    // (round-40 L5 — matches sync_allowed below).
    pub activity_allow:
        Box<dyn for<'c> Fn(Activity, Component<'c>) -> bool + 'a>,

    // === Per-syncer decisions — endpoint-split (round-52 H3 +
    // round-54 H1; only one of these two is consulted per request,
    // determined by the SyncEndpoint discriminator on
    // SyncPrivacyInput).

    // /cookie_sync chooser ladder — covers Appendix B.3 steps 6-9.
    // Internally consults: `syncUser` activity, `BidderSyncAllowed`
    // (GDPR vendor / publisher restrictions via `gvl`), syncer
    // `enabled`, and the `SkipWhen.gpp_sid` exact-string match
    // against `raw_gpp_sid`. The caller still owns B.3 steps 1-5
    // (registry lookup, dedupe, filters, already-synced) — those are
    // non-privacy. NOT consulted by /setuid (matches upstream
    // reference syncer selection which is /cookie_sync-only).
    pub sync_allowed:
        Box<dyn for<'l> Fn(&SyncerLookup<'l>) -> SyncDecision + 'a>,

    // /setuid narrower decision — covers §8.3 step 11. Internally
    // consults: host-cookie GDPR (via host_cookie_sync_allowed
    // above) + syncer `enabled`. Does NOT invoke `BidderSyncAllowed`
    // and does NOT consult `SkipWhen.gpp_sid` — those are
    // /cookie_sync-only (matches upstream reference UID-write handling which only
    // checks host-cookie GDPR). Returns SyncDecision::Allowed,
    // BlockedByPrivacy, or BlockedByDisabledUsersync only;
    // BlockedByRegulationScope is never returned here.
    pub setuid_allowed:
        Box<dyn for<'l> Fn(&SyncerLookup<'l>) -> SyncDecision + 'a>,
}

pub fn resolve_sync_privacy<'a>(input: SyncPrivacyInput<'a>)
    -> Result<SyncPrivacyContext<'a>, SyncPrivacyError>;
```

Semantic contracts:

- **Hard rejection vs clear-and-continue — per-field split** (round-41
  M3 + round-46 H1 — matches reference cookie-sync handling behavior;
  earlier "TCF/USP/GPP all hard-400" was too strict):
  - **Legacy `gdpr` field malformed** (`GdprSignalIn::Invalid(_)`) —
    decision is **deferred until after GPP/SID precedence runs**
    inside `resolve_sync_privacy` (round-46 H1, matches upstream
    reference cookie-sync handling + reference UID-write handling). If
    `regs.gpp_sid` resolves the GDPR signal (TCFEU2 present or
    non-empty SID), the malformed legacy `gdpr` value is **silently
    ignored**, `Ok(_)` returns. If GPP did NOT resolve the signal,
    `Err(MalformedConsent { scope: "gdpr" })` → caller emits
    endpoint-specific 400. This is the only field whose error policy
    depends on a _different_ field's value.
  - **TCF v2 malformed** (non-empty `gdpr_consent` that fails parse)
    — **gated on `normalized_request_signal` per §7.8.1, then split
    by endpoint** (round-47 H1 + round-48 H1 + round-56 M2 —
    "effective GDPR signal" wording dropped here; the gate is the
    normalized signal because the permissions builder normalizes
    Ambiguous via host `default_value` at reference GDPR permission evaluation BEFORE
    deciding whether to parse. Matches reference TCF permission evaluation's
    `HostCookiesAllowed` / `BidderSyncAllowed` short-circuiting to
    `true` before parsing when the normalized signal `!= SignalYes`
    at, with parsing deferred to `allowSync`):
    - **If `normalized_request_signal` is NOT Yes** (round-51 H1+M2 +
      round-52 H1 — keyed off the **normalized** signal because the
      permissions builder normalizes Ambiguous via host
      `default_value` at reference GDPR permission evaluation _before_ deciding whether to
      parse TCF): malformed TCF is **silently ignored** on both
      endpoints (TCF never parsed by reference TCF permission evaluation) → `Ok(_)`
      returns; `host_cookie_sync_allowed == true` and every
      `sync_allowed(...)` returns `Allowed` (subject to non-TCF
      checks). /setuid does NOT 400 on `gdpr=0&gdpr_consent=bad`;
      /cookie_sync does NOT privacy-block. **Separately**, when
      `gdpr.enabled=false` (regardless of signal), the permissions
      builder returns an allow-all permission result (reference GDPR permission evaluation) — same effect.
      Note that `parsed_request_signal == Ambiguous` no longer
      short-circuits here: if host `default_value="1"` (so
      `normalized_request_signal == Yes`), TCF parsing DOES run via
      the permission path, even with `gdpr` absent from the wire
      (round-52 H1 — earlier "parsed Ambiguous → silent ignore" was
      wrong).
    - **If `normalized_request_signal == Yes` AND
      `gdpr_permissions_enabled`** (round-56 M2 — uses §7.8.1
      taxonomy throughout; the earlier mixed "effective GDPR signal
      IS Yes" wording conflated two distinct gates). The
      malformed-TCF policy below differs from the empty-consent
      request-extraction rule in §7.8: empty consent on /setuid
      keys off `parsed_request_signal == Yes` (matches reference UID-write handling), but malformed non-empty TCF _parsing_ runs only when
      `normalized_request_signal == Yes` (because
      `HostCookiesAllowed` checks the normalized signal at
      reference TCF permission evaluation before parsing). `gdpr.enabled=false`
      bypasses both — the permissions builder returns an allow-all permission result
      (reference GDPR permission evaluation); the request signal is preserved but no parse
      runs.
      - **/setuid** (round-52 H1+H2 — split per-rule):
        - **Request-extraction hard-400 (empty consent)**: upstream
          only hard-400s empty consent when the **parsed request
          signal is Yes** (reference UID-write handling) — i.e. `gdpr=1` was on
          the wire, or GPP-SID resolved Yes from the wire. Empty
          consent under host-default Yes (parsed=Ambiguous,
          normalized=Yes) does NOT 400 here; it falls into the
          permission path below.
        - **Permission-time block — malformed non-empty TCF can be
          400 (NOT 451) ONLY when activity already allowed and
          parsing actually runs; NEVER from request extraction**
          (round-52 H1 + round-53 H2 + round-59 M1 — upstream
          reference UID-write handling only copies and emptiness-checks
          `gdpr_consent` at extraction; TCF parsing happens
          exclusively inside `HostCookiesAllowed`; parse errors
          surface as 400 via reference UID-write handling
          the consent-error response). The 400 fires
          **only when ALL FOUR** of the following hold; if any
          fails the outcome is 200 (parse skipped) or 451 (activity
          denied):
          (a) `host_vendor_id != 0`,
          (b) `normalized_request_signal == Yes`,
          (c) `gdpr_permissions_enabled`,
          (d) the `syncUser` activity check at §8.3 step 10
          already allowed the request (the permission path
          runs in step 11 AFTER activity, so syncUser-denied
          wins with 451 — round-54 H3).
          Other branches:
          - `host_vendor_id == 0` → host-cookie GDPR permission short-circuits
            to `true` without parsing (reference GDPR permission evaluation + reference TCF permission evaluation)
            → /setuid continues to **`200`**.
          - `gdpr.enabled=false` (or `normalized_request_signal !=
Yes`) → permissions builder returns an allow-all permission result
            (reference GDPR permission evaluation); parse skipped; /setuid → **`200`**.
          - `syncUser`-denied at step 10 → **`451`** empty body
            (Appendix F); permission path never runs.
      - **/cookie_sync** → **separate host-cookie gate and per-syncer
        gate** (round-49 H2 — earlier "host_cookie_sync_allowed =
        false" was unconditional; upstream actually splits these):
        - **`host_cookie_sync_allowed`** depends on
          `host.gdpr.host_vendor_id`. If `host_vendor_id == 0` →
          `host_cookie_sync_allowed == true` always (matches upstream
          reference GDPR permission evaluation host-cookie GDPR permission wrapper +
          reference TCF permission evaluation — host-company GDPR check is
          skipped entirely). If `host_vendor_id != 0` →
          `host_cookie_sync_allowed == false` (the host vendor's
          consent couldn't be parsed).
        - **Per-syncer `sync_allowed`** returns `BlockedByPrivacy`
          regardless of `host_vendor_id` (matches upstream
          reference bidder-sync permission at reference cookie-sync handling + reference syncer selection — bidder-sync check fails
          on parse error even when host-cookie check succeeds).
        - Net effect: in BOTH `host_vendor_id` cases the response
          is **`200`** with `bidder_status` empty (all per-bidder
          decisions blocked); only the per-bidder gate differs
          between the two `host_vendor_id` paths. The **top-level
          JSON `status`** field is always derived from the request
          cookie's live-sync count per Appendix B.2 (round-71 M9 —
          corrects earlier "reflects host_cookie_sync_allowed"
          phrasing; status = "ok" iff ≥1 live sync, else
          "no_cookie", regardless of the host or per-bidder
          privacy gate outcomes). The host gate only affects
          `bidder_status`.
          The caller selects endpoint policy via a
          `SyncPrivacyInput::endpoint: SyncEndpoint { CookieSync, SetUid }`
          discriminator (below). **Empty consent — endpoint-split rule**
          (round-52 H2 — corrects round-51's blanket "parsed gdpr=1" claim;
          Upstream endpoints split):
    - **`/cookie_sync`**: `Err(MalformedConsent)` → **`400`** when
      **`parsed_request_signal == Yes` OR (`parsed_request_signal ==
Ambiguous` AND `normalized_request_signal == Yes`)** — i.e.
      explicit `gdpr=1` OR (absent `gdpr` + raw host
      `default_value="1"`). Sync paths NEVER consult geo (round-74
      L1 — corrects the earlier "non-EEA / no geo override" hint,
      which was a leftover from before the §7.1 / round-72 H1
      two-signal split; `sync_permissions_signal` uses only the
      raw host default per §7.1). Matches reference cookie-sync handling + reference cookie-sync handling fixtures which exercises the
      ambiguous-falls-to-host-default-Yes case.
    - **`/setuid`**: `Err(MalformedConsent)` → **`400`** **only when
      `parsed_request_signal == Yes`** (matches reference UID-write handling which checks the parsed signal directly).
      `parsed_request_signal == Ambiguous` + host-default Yes does
      NOT request-extract 400; it falls into the
      `HostCookiesAllowed` permission path with the following
      outcomes (round-56 H1 — corrects the earlier "may produce a
      400" wording, which conflated empty-consent and
      malformed-non-empty-TCF paths):
      - `host_vendor_id == 0` → 200 (host-cookie GDPR permission
        short-circuits to true, reference GDPR permission evaluation).
      - `host_vendor_id != 0` + `gdpr_permissions_enabled` →
        **451** (matches reference UID-write handling `BlockedByPrivacy`:
        empty consent makes `HostCookiesAllowed` return false at
        reference TCF permission evaluation). NOT 400 — 400 only fires when the
        TCF _parse_ fails on a malformed non-empty string
        (reference UID-write handling); empty consent is a "not allowed"
        permission outcome, not a parse error.
      - `host_vendor_id != 0` + `!gdpr_permissions_enabled` → 200
        (permissions builder returns an allow-all permission result, reference GDPR permission evaluation).
  - **Endpoint discriminator** (round-47 H1 + round-52 H2 + round-53
    M4 — see the full enumeration in the `SyncPrivacyInput::endpoint`
    field doc-comment above). `SyncPrivacyInput` gains
    `pub endpoint: SyncEndpoint` with variants `CookieSync` and
    `SetUid`. The discriminator affects **three** distinct
    behaviors (round-60 L2 — taxonomy-correct phrasing):
    (a) malformed-TCF policy under `normalized_request_signal ==
Yes` + `gdpr_permissions_enabled`;
    (b) empty-consent 400 trigger (parsed-Yes-only vs
    parsed-or-normalized-Yes); (c) per-syncer decision shape
    (`sync_allowed` chooser ladder vs the narrower
    `setuid_allowed`). All other fields (GPP-precedence, GPP/sid
    hard-400, USP silent-clear, account/host lookup) behave
    identically across both endpoints.
  - **GPP malformed** (envelope or required-section parse failure) →
    `Err(MalformedGpp)` → caller emits **`400`**. Matches upstream.
  - **`gpp_sid` malformed** under the sync applicability grammar in
    Appendix B.1 -> `Err(MalformedGppSid)` -> caller emits **`400`**.
    Standalone `-1` is valid and means no section applies, not absent
    applicability. Positive IDs are widened beyond the pinned helper's
    signed-int8 range; mixed/repeated sentinel lists are rejected explicitly.
  - **US-Privacy malformed** → `Ok(_)` with the `us_privacy` value
    **silently cleared** (treated as absent for the rest of the
    resolution); the request continues, just without CCPA enforcement
    derived from the bad string. Matches reference cookie-sync handling which
    parses CCPA after privacy extraction and clears invalid values
    without erroring. The caller does NOT emit 400 on bad USP alone.
  - Auction-path's non-fatal warning for malformed USP (§7.7) is
    intentionally NOT mirrored on sync — sync emits no warning on
    silently-dropped USP (matches upstream).
- **No `BidRequest` shape.** Inputs are the wire-level query/body fields
  the syncer endpoints actually carry (§8.2 / §8.3 / Appendix B.1 / B.4).
  No `NormalizedBidRequest`, no `prepare()` — the type-safe gate of §3.2
  applies only to the auction endpoint.
- **Endpoint mapping.**
  - `/cookie_sync` uses `host_cookie_sync_allowed` for the host-cookie
    GDPR gate (matches reference cookie-sync handling's host-syncer GDPR check)
    and calls `sync_allowed(&SyncerLookup{ ... })` once per candidate
    syncer in the §8.2 ladder (covers Appendix B.3 steps 6-9).
  - `/setuid` (round-53 H1 — narrowed; earlier wording leaked the
    /cookie_sync chooser ladder onto /setuid): uses
    `activity_allow(syncUser, Component::bidder(name))` for the
    activity gate (round-31 M4), then a narrower
    **`setuid_allowed(SyncerLookup{...})`** that consults **only**
    the host-cookie GDPR result (from `HostCookiesAllowed`, see
    §8.3 step 11) plus the syncer's `enabled` flag — NOT
    `BidderSyncAllowed` and NOT `SkipWhen.gpp_sid` (those are
    /cookie_sync chooser-ladder decisions per Appendix B.3 steps
    6-9; reference UID-write handling confirms /setuid only checks
    host-cookie GDPR). Outcomes:
    `BlockedByPrivacy` → **`451`** with the pinned `gdpr_consent
string prevents…` body; `BlockedByDisabledUsersync` → **`400`**
    body `The bidder syncer is disabled` (Appendix F; intentional
    divergence per §15 row "/setuid disabled-syncer rejection").
    `BlockedByRegulationScope` is **not reachable** on /setuid.
  - **Empty consent — endpoint-split per §7.8.1 taxonomy** (round-52
    H2 + round-55 H1 — corrects the earlier blanket "400 on both
    endpoints"):
    - `/cookie_sync` → **400** when `parsed_request_signal == Yes`
      OR (`parsed_request_signal == Ambiguous` AND
      `normalized_request_signal == Yes`) — matches reference cookie-sync handling + reference cookie-sync handling fixtures.
    - `/setuid` request-extraction → **400** only when
      `parsed_request_signal == Yes` (matches reference UID-write handling).
      Ambiguous + normalized Yes falls into the host-cookie
      permission path: `host_vendor_id == 0` → 200;
      `host_vendor_id != 0` AND `gdpr_permissions_enabled` → 451
      (`HostCookiesAllowed` returns false on empty consent per
      reference TCF permission evaluation → reference UID-write handling `BlockedByPrivacy`).
  - **Malformed (non-empty) TCF** (round-47 H1 + round-48 H1 +
    round-49 H2 + round-58 M1 — full preconditions, not the
    earlier shorthand):
    - /cookie_sync → **`200` + empty `bidder_status`** via
      per-syncer privacy denial (per-syncer privacy evaluation at
      reference cookie-sync handling).
    - /setuid → permission-path outcomes depending on conditions:
      - `host_vendor_id == 0` → `200` (AllowHostCookies skip,
        reference GDPR permission evaluation).
      - `host_vendor_id != 0` AND `normalized_request_signal ==
Yes` AND `gdpr_permissions_enabled` AND `syncUser`
        activity allowed → **`400`** body `gdpr_consent was
invalid. <parser error>` (reference UID-write handling). The
        activity-allowed precondition matters because the
        permission path runs in §8.3 step 11, AFTER step-10
        activity; `syncUser`-denied wins with 451 (round-54 H3).
      - `host_vendor_id != 0` AND `!gdpr_permissions_enabled`
        (host disabled GDPR) → `200` (AlwaysAllow, reference GDPR permission evaluation).
      - `host_vendor_id != 0` AND `normalized_request_signal !=
Yes` → `200` (parse skipped, reference TCF permission evaluation).
        Net: /setuid only 400s on the narrow precondition list
        above; everything else is 200 (or 451 if activity-denied).
  - **Malformed TCF when `normalized_request_signal != Yes`** → no
    error, no privacy block on either endpoint (TCF never parsed).

`auctioneer-privacy` owns both `PrivacyContext::resolve` (auction) and
`resolve_sync_privacy` (sync). The two share the underlying parsers, GVL
plumbing, and GDPR/CCPA/GPP signal logic; only their inputs and
error-policy differ.

**Milestone ownership** (round-39 M1 + round-40 H1 — pinned, no
hand-off contradiction with §14):

- **M2-M4 historical baseline:** an unreachable, permissive sync API
  stub kept downstream crates compiling; no user-sync routes or UID
  cookie lifecycle were active. Its `SyncerLookup` and allow-all
  callbacks are not a current M5 API or permission claim.
- **M5 contract:** `resolve_sync_privacy` uses real TCF/USP/GPP parsing,
  GVL-backed GDPR decisions, exact raw-SID `SkipWhen` comparison, and
  endpoint-specific host/candidate/`setuid` permissions. The narrower
  `/setuid` path does not run the `/cookie_sync` chooser ladder. The
  typed M5 inputs and errors replace the old stub types where needed;
  neither API-signature preservation nor permissive compatibility
  wrappers are required. M5 also owns the `uids` codec and merge,
  `/cookie_sync` / `/setuid` / `/getuids` routes, URL safety, and cookie
  write/eviction rules (Appendix B). Their presence in a worktree does
  not establish hosted-provider or real-browser qualification.

The historical stub's sole job was to let `auctioneer-privacy` compile and
unit-test before M5; it must not be exposed by a registered sync route.

### 7.8.1 Signal taxonomy (round-51 M2)

Earlier rounds collapsed several distinct concepts under the phrase
"effective GDPR signal," which led to category errors (notably treating
`gdpr.enabled=false` as a signal value). The three concepts are
**separate** and `resolve_sync_privacy` exposes all three:

- **`parsed_request_signal: Signal`** — the GDPR signal derived from
  the **wire** alone: GPP-SID precedence (`Yes` if section 2 present,
  `No` if non-empty SID without section 2) then legacy `gdpr` field
  (`Yes`/`No`/`Ambiguous`). NEVER consults host defaults. This is
  what upstream's request-extraction hard-400 paths key off — empty
  consent at reference cookie-sync handling / reference UID-write handling — because those
  rejections happen before the permissions builder runs.
- **`normalized_request_signal: Signal`** — `parsed_request_signal`
  with the **raw host `default_value`** substituted on `Ambiguous`
  (round-72 H1 + round-73 H1 — corrects earlier "geo-aware"
  wording that contradicted §7.1's two-signal split; this field
  is the sync alias for `sync_permissions_signal`, which NEVER
  consults geo). The **auction path** uses `auction_geo_signal`
  (geo-aware, defined in §7.1) for the in-scope decision and for
  what gets stamped into outbound bidder requests, NOT this
  field; this field is sync-path-only.
- **`gdpr_permissions_enabled: bool`** = `host.gdpr.enabled &&
channel_enabled && normalized_request_signal == Yes` (§7.1
  resolution). When this is `false`, the permissions builder
  returns an allow-all permission result (reference GDPR permission evaluation); when `true`, it builds the
  per-purpose / per-vendor permissions object. **`gdpr.enabled=
false` is NOT a signal value** — it only affects this boolean;
  the request signal is preserved unchanged in the response and in
  outbound macros.

`SyncPrivacyContext` exposes all three:

```rust
pub struct SyncPrivacyContext<'a> {
    pub parsed_request_signal:    Signal,   // round-51 M2 — wire-only
    pub normalized_request_signal: Signal,  // round-51 M2 + round-71 H3 +
                                            // round-72 H1 — (a.k.a.
                                            // sync_permissions_signal per
                                            // §7.1); wire + RAW host
                                            // default_value; NEVER geo
    pub gdpr_permissions_enabled: bool,     // round-51 M2 — host.enabled
                                            // AND normalized == Yes
                                            // AND channel enabled
    pub gdpr_signal:              Signal,   // alias for
                                            // normalized_request_signal; kept
                                            // for back-compat with §8.2 step 5
                                            // pre-round-51 callers.
    pub gdpr_enforced:            bool,     // alias for
                                            // gdpr_permissions_enabled.
    // ... rest of the struct (ccpa_enforce, raw_gpp_sid,
    // host_cookie_sync_allowed, activity_allow, sync_allowed,
    // setuid_allowed) — see the canonical definition in §7.8.
    // round-54 H1: the canonical struct now includes setuid_allowed
    // as a separate /setuid decision callback (narrower than
    // sync_allowed); see §7.8 for the full definition.
}
```

The hard-400 contracts in §7.8 split by endpoint (round-52 H2 +
round-53 M3 — corrects earlier "key off parsed_request_signal"
blanket which was true for /setuid but not /cookie_sync):

- **Empty consent**:
  - `/cookie_sync` 400s when `parsed_request_signal == Yes` OR
    (`parsed_request_signal == Ambiguous` AND
    `normalized_request_signal == Yes`) — matches reference cookie-sync handling.
  - `/setuid` 400s only when `parsed_request_signal == Yes` —
    matches reference UID-write handling.
- **Malformed legacy `gdpr`** (`GdprSignalIn::Invalid(_)`) — both
  endpoints 400 when GPP/SID did not override (round-46 H1),
  regardless of normalized signal.

The permission-time blocks (host-cookie GDPR, per-syncer GDPR) key
off `gdpr_permissions_enabled` AND `normalized_request_signal == Yes`
(both must hold for TCF to actually be parsed).

---

## 8. `auctioneer-core` — Endpoint Handlers

All handlers are `#[action]` functions wired through `edgezero_core::app!`.
User-sync schemas in **Appendix B**. `auctioneer-core` constructs the production
`BidderFanout` / `Clock` / `Rng` and passes them into the library crates.

For M5, the [user-sync design](2026-09-30-m5-usersync-design.md) sections
3 and 9 refine handler integration and browser response headers. The three
sync endpoints plus `OPTIONS /cookie_sync` use manifest-declared aux routes,
the generated EdgeZero App, one validated config snapshot, and
`with_aux_egress`; provider adapters do not add their own routing. CORS
decoration applies to application responses after config resolution, not
pre-config refusals or framework/transport failures. All Auctioneer-owned sync
responses carry `Cache-Control: no-store`, including the bounded framework
renderer and admission/fallback responses. Preserve static header bounds
and completion-owned permits when extending those shared builders.
The admitted `/cookie_sync` handler owns pre-read rejection of nonempty
Content-Encoding, followed by declared Content-Length over 32768 bytes;
the bounded stream reader remains independently required. Route registration
and the aux admission class do not perform those checks automatically.

### 8.1 Auction & info endpoints

| Endpoint                   | Behavior                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `POST /openrtb2/auction`   | §5; response Appendix E.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `GET /status`              | **HEALTHY:** `204 No Content` with empty body when `host.status_response` is unset/empty; `200` with the configured body when non-empty. **DEGRADED (runtime config unloadable, malformed or unfunded):** ALWAYS `200`, never `204` and never `500`, with the baked `status_response` (if any) plus a final line `config: degraded (<code>)`. **Every answer carries `x-auctioneer-config-status: ok \| degraded` and the milestone stamp.** The closed snake_case codes are `malformed_config \| store_key_rejected \| store_unavailable \| unfunded_extraction`; the fourth is the [approved R0 initialization boundary](2026-10-17-m7-r0-wasm-resource-envelope-design.md#42-approval-and-scope-enforcement). `RuntimeConfigError::detail` is never published. The degraded path also covers malformed merged host data, since `resolved_host` is infallible. |
| `GET /info/bidders`        | Configured bidder names; honors `enabledonly`, `baseadaptersonly` (each must be `true` or `false` — any other value → `400`, matches reference bidder-list handling; round-20 finding L7).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| `GET /info/bidders/{name}` | That bidder's `BidderInfo`; `404` if unknown. The special name `all` returns the aggregate (all bidders keyed by name; matches reference bidder-detail handling).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |

**`/info/bidders`** returns a JSON array of bidder names — the four phase-1
bidders plus any configured aliases. `?enabledonly=true` filters to enabled
bidders; `?baseadaptersonly=true` excludes aliases. **`/info/bidders/{name}`**
returns a JSON object mirroring upstream's bidder-detail shape:

```json
{
  "status": "ACTIVE",
  "usesHttps": true,
  "maintainer": { "email": "<addr>" },
  "capabilities": {
    "app": { "mediaTypes": ["banner", "video", "native"] },
    "site": { "mediaTypes": ["banner", "video", "native"] }
  },
  "aliasOf": "<base bidder>"
}
```

`status` is `ACTIVE` or `DISABLED`. `usesHttps` is `true` iff the bidder's
**configured `endpoint` value starts with `https://`** (matches upstream's
reference bidder-detail handling — it checks the endpoint prefix only, not every
`host_allowlist` entry; round-20 finding L7).
`aliasOf` is present only for an alias.

**`/info/bidders/all`** returns a JSON object mapping every configured
bidder name → its detail object above. **Disabled (non-whitelabel) bidders
are included** with `status: "DISABLED"` (matches upstream
reference bidder-detail handling; round-17 finding M8). Active entries may also include
a `dooh` capabilities block when the bidder declares one.

`404` if the name is neither `all`, a bidder, nor an alias. `/info/bidders`
reflects **only host/config-declared aliases** — `ext.prebid.aliases` are
auction-request-local (scoped to a single `/openrtb2/auction` call) and
never appear here. Phase 1 has four base bidders and, unless a bidder-info
TOML declares one, no config aliases.

### 8.2 `POST /cookie_sync`

Behavior-compatible with the baseline (Appendix B.1–B.3). Bounded body (§5.3).
**Explicit ordered pipeline** (round-37 H1 + round-38 M3 + round-42 H1
— now lists every resolution step that earlier prose left implicit):

1. **Body read, bounded** per §5.3 (`MAX_COOKIESYNC_BODY` = 32 KiB);
   oversize → **`400`**.
2. **JSON parse** → typed request struct (Appendix B.1). Malformed JSON
   → **`400`** body `JSON parsing failed: <err>` plus a trailing newline,
   with `Content-Type: text/plain; charset=utf-8` and no `Invalid request:`
   prefix. Appendix F defines the observable error-response contract.
3. **Resolve host config** (round-43 H1 — was implicit before):
   per-request merge + validate per §9.1; malformed host override →
   **`500`** body `Malformed host configuration`. The resolved
   `HostConfig` drives the `host_cookie` lookup in steps 4-5 and the
   `user_sync.*` syncer URL construction (§8.6).
4. **Host opt-out cookie check** (intentional divergence per §15: runs
   _before_ account/privacy resolution) → opt-out → **`401`** with body
   `User has opted out` plus a trailing newline. Set
   `Content-Type: text/plain; charset=utf-8` and
   `X-Content-Type-Options: nosniff`. Do not import the host cookie, decode
   `uids`, or resolve account/privacy policy on this branch.
5. **Resolve account** (round-42 H1 — was implicit before): take
   `request.account` (B.1) → look up via `config_store` (§9.1
   sync-endpoint mapping) → `ResolvedAccount`. Malformed account
   override → **`400`** body `account config is malformed and could
not be read` (Appendix F); disabled account → **`400`** body
   `account is disabled, please reach out to the prebid server host`.
   On clean lookup, embedded default-account fallback is used for an
   absent / `"unknown"` id (§9.1).
6. **Decode `uids` cookie** and apply **host-cookie import** (round-22 M5
   - round-24 L7): if the host cookie's `cookie_name` is present on
     the request _and_ the in-memory `uids` cookie has no entry under
     the host `family` key, copy the host cookie's value into that
     family slot — fill-if-empty, never overwrite (matches upstream
     reference UID-cookie handling).
7. **`resolve_sync_privacy`** (§7.8, with `endpoint:
SyncEndpoint::CookieSync`). Per-field error policy (round-41 M3 +
   round-47 H1 + round-48 H1 + round-49 H1+H2 + round-52 M4 — uses
   §7.8.1 taxonomy):
   - **Empty consent**: 400 when **`parsed_request_signal == Yes`
     OR (`parsed_request_signal == Ambiguous` AND
     `normalized_request_signal == Yes`)** (round-52 H2 — matches
     Upstream reference cookie-sync handling + reference cookie-sync handling fixtures which
     exercises the ambiguous-falls-to-host-default-Yes case).
   - **Malformed non-empty TCF**:
     - **`normalized_request_signal != Yes`** (round-52 H1 —
       normalized, not parsed; the permissions builder normalizes
       Ambiguous via host `default_value` at reference GDPR permission evaluation BEFORE
       deciding to parse): silently ignored: `Ok(_)`,
       `host_cookie_sync_allowed == true`, per-syncer `sync_allowed`
       defaults to `Allowed` (TCF never parsed by reference TCF permission evaluation).
     - **`normalized_request_signal == Yes` AND
       `gdpr_permissions_enabled`** (i.e. `host.gdpr.enabled &&
channel_enabled`): `Ok(_)` but two **separate** gates fail
       (round-49 H2):
       - `host_cookie_sync_allowed` follows
         `host.gdpr.host_vendor_id`: `0` → `true` always (matches
         reference GDPR permission evaluation + reference TCF permission evaluation host-cookie GDPR permission wrapper); non-zero → `false`.
       - Per-syncer `sync_allowed(...)` returns `BlockedByPrivacy`
         regardless of `host_vendor_id` (matches reference bidder-sync permission at reference cookie-sync handling + reference syncer selection).
         Net response: `200` with empty `bidder_status` (per-bidder
         gate blocked all candidates) — round-16 M5.
     - **`!gdpr_permissions_enabled`** (host disabled, regardless of
       signal): permissions builder returns an allow-all permission result (reference GDPR permission evaluation); TCF parsing skipped entirely; same as the "silently
       ignored" branch above.
   - **Malformed GPP / `gpp_sid`** → `Err(_)` → **`400`** (hard
     rejection on both endpoints).
   - **Malformed `us_privacy`** → silently cleared, no error
     (round-41 M3).
   - **Malformed legacy `gdpr`** → deferred GPP-precedence rule
     (round-46 H1).
8. **Host-cookie GDPR gate** uses
   `SyncPrivacyContext::host_cookie_sync_allowed`: blocked → response
   shape per Appendix F (`200`; `bidder_status` empty + JSON `status`
   unchanged, round-16 M5); not blocked → continue.
9. **Resolve `filterSettings` + `limit`** (Appendix B.1): coerce
   `filterSettings.{iframe,image}.filter ∈ {"include","exclude"}`,
   `bidders ∈ "*" | [string]`; cap `limit` at account `max_limit`,
   default to account `default_limit` when absent. Malformed
   `filterSettings` shape → **`400`** with the **raw the parser error message
   from the per-branch parser**, written through reference plain-text error output
   (round-48 M3 + round-49 M3 — matches reference cookie-sync handling / reference cookie-sync handling fixtures). Body is the
   parser's exact error string + trailing newline,
   `Content-Type: text/plain; charset=utf-8`, no "Invalid request:"
   prefix and **no synthesized `error parsing filtersettings:`
   prefix** — the parser already prefixes the body with the offending
   branch. Examples: `error parsing filtersettings.iframe: invalid
bidders type. must either be a string '*' or a string array`;
   `error parsing filtersettings.image: invalid filter value
'whatever'. must be either 'include' or 'exclude'`.
10. **Build candidate list** (Appendix B.3 ordering — GPP overrides
    legacy in the resolved context).
11. **Per-bidder ordered `evaluate`** (Appendix B.3). Steps 1-5
    (registry lookup, dedupe, filters, already-synced) consult the
    build-time bidder registry + syncer table to construct a
    `SyncerLookup` per candidate; steps 6-9 call
    `SyncPrivacyContext::sync_allowed(SyncerLookup{...})`. Unknown
    bidder name → `StatusUnknownBidder` (per-candidate status only,
    not a 4xx — request continues with other candidates).
12. **Response** per Appendix B.2.

**Error precedence** (round-42 H1 + round-43 H1). Earlier checks
short-circuit later ones: body-too-large (1) → parse error (2) →
malformed host (3) → opt-out (4) → account error (5) → privacy error
(7) → `filterSettings` error (9). The per-candidate `evaluate`
(step 11) only ever emits per-candidate `StatusXxx` — it never
produces a 4xx; the response is always `200` once we reach step 10.

### 8.3 `GET /setuid`

Behavior-compatible (Appendix B.4–B.6). **Explicit ordered pipeline**
(round-37 H1 + round-38 M3 + round-42 H1+M2 + round-43 H1+H2+M3+L7 —
opt-out moved before bidder/f validation; us_privacy dropped from the
query parse since B.4 doesn't carry it; error bodies pinned to the
exact reference UID-write handling strings):

1. **Query parse** (Appendix B.4): extract `bidder`, `uid`, `gdpr`,
   `gdpr_consent`, `gpp`, `gpp_sid`, `f`, `account` (NOT
   `us_privacy` — that field is `/cookie_sync`-only per B.4 /
   round-43 L7; `resolve_sync_privacy` receives `us_privacy: None`
   for /setuid). Coerce `gdpr` per Appendix B.1 wire-vs-API rule
   (round-38 L6 + round-46 H1): `0` / `1` -> `Zero` / `One`;
   absent or exactly empty (`gdpr=`) -> `Absent`; other nonempty
   values -> `Invalid(<raw>)`. **No 400 at the wire
   surface for `gdpr` — the decision is deferred to step 9** so
   GPP/SID can override the malformed legacy value (matches upstream
   reference UID-write handling which only rejects malformed legacy `gdpr`
   when GPP did not already provide the signal).
2. **Resolve host config** (round-43 H1 — was implicit before):
   per-request merge + validate per §9.1; malformed host override →
   **`500`** body `Malformed host configuration`.
3. **Host opt-out cookie check** (intentional divergence per §15 —
   round-37 H1 + round-41 H2 + round-43 H2; moved before bidder/`f`
   validation so an opted-out user gets `401` even when their request
   has malformed `bidder`/`f`, matching reference UID-write handling's
   `ReadCookie` / `AllowSyncs()` precedence at line): opt-out →
   **`401` with no body and no `Content-Type` header** (round-46 M2 —
   matches reference UID-write handling, which writes the status only); no `uids` read, no
   host-cookie import, no account/privacy resolution, no bidder/`f`
   validation.
4. **`bidder` parameter validation** (round-42 H1 — was implicit
   before; round-43 M3 — exact upstream wording; round-44 L6 —
   case-insensitive lookup): missing/empty `bidder` → **`400`** body
   `"bidder" query param is required` (matches reference UID-write handling). For the registry lookup, the raw `bidder` value is
   **ASCII-lowercased** before matching (matches reference UID-write handling — `strings.ToLower` before chooser/registry lookup).
   The original-case value is preserved only where upstream preserves
   it (i.e. response cookie attribution / debug logs). A
   case-folded value not in the registry → **`400`** body `The
bidder name provided is not supported by Prebid Server` (matches
   Upstream reference UID-write handling).
5. **`f` parameter validation** (round-42 H1 + round-43 M3 + round-46
   L5 — case-insensitive matching): if `f` is present, **ASCII-case-
   insensitive** match against `"i"` (image) / `"b"` (blank HTML)
   (matches reference UID-write handling Unicode simple-fold equality —
   `f=I` and `f=B` are accepted; the normalized lowercase value
   drives the response format). Other values → **`400`** body `"f"
query param is invalid. must be "b" or "i"` (matches upstream).
6. **Resolve account** (round-42 H1 — was implicit before): id from
   query `account`, typed overlay from the same validated snapshot
   per §9.1. Missing/empty differs from literal named `"unknown"`.
   Whole-blob validation already ran; no separate malformed-account fetch
   occurs in M6. Disabled → **`400`** body
   `account is disabled, please reach out to the prebid server host`
   (Appendix F).
7. **Resolve syncer for `bidder`** (round-42 H1): look up
   `BidderInfo` + `SyncerConfig` for `bidder` from the build-time
   registry to construct `SyncerLookup` (§7.8). The syncer's
   `enabled` flag is consulted in step 11 below via `setuid_allowed`
   (round-46 L6 + round-55 M4 + round-56 M3 — corrects stale
   `sync_allowed`; /setuid uses the narrower `setuid_allowed`
   callback that consults host-cookie GDPR + `enabled` only,
   never the /cookie_sync chooser ladder), not here.
8. **Decode `uids` cookie** and apply **host-cookie import** with the
   fill-if-empty rule (§8.2 step 6 / round-24 L7).
9. **`resolve_sync_privacy`** (§7.8, with `endpoint:
SyncEndpoint::SetUid`; round-47 H1 + round-52 M4 — uses §7.8.1
   taxonomy). Per-field error policy (round-41 M3 + round-43 M3 +
   round-44 M5 + round-45 M2 + round-46 H1+M4 + round-52 H1+H2):
   - **Malformed legacy `gdpr`** (`GdprSignalIn::Invalid(raw)`) AND
     GPP/SID did not resolve the signal → body `the gdpr query
param must be either 0 or 1. You gave <raw>` (round-44 M5 +
     round-46 H1 — exact reference UID-write handling). If GPP/SID
     resolved the signal, `Invalid` is silently ignored.
   - **Empty consent** under **`parsed_request_signal == Yes`**
     (round-52 H2 — keyed off parsed signal only, matches
     reference UID-write handling) → body `GDPR consent is required when gdpr
signal equals 1`. `parsed_request_signal == Ambiguous` +
     host-default Yes does NOT 400 here; it falls into the
     `HostCookiesAllowed` permission path in step 11.
   - **Malformed non-empty TCF consent — handled at the permission
     path, NOT at request extraction** (round-53 H2 — corrects
     earlier "request-extraction 400" claim; reference UID-write handling only _copies_ `gdpr_consent` and checks emptiness at
     extraction, never parses it there — parsing happens inside
     `HostCookiesAllowed`). The body `gdpr_consent was invalid.
<parser error>` (round-46 M4) is therefore emitted from the
     **permission path** in step 11, not from `resolve_sync_privacy`
     directly. Outcomes per `host_vendor_id` (matches reference GDPR permission evaluation + reference TCF permission evaluation):
     - `host_vendor_id == 0` → host-cookie GDPR permission wrapper skips
       TCF parsing entirely; the malformed value is silently
       ignored; request continues to step 12 → **`200`**.
     - `host_vendor_id != 0` AND `normalized_request_signal == Yes`
       AND `gdpr_permissions_enabled` AND **step-10 activity
       already allowed the request** (round-59 M2 — fourth
       precondition matters because this step runs AFTER step 10;
       syncUser-denied wins with 451) → `HostCookiesAllowed`
       parses TCF and returns the parse error; caller emits
       **`400`** body `gdpr_consent was invalid. <parser error>`
       (matches reference UID-write handling the consent-error response).
     - `host_vendor_id != 0` AND `normalized_request_signal != Yes`
       (or `!gdpr_permissions_enabled`) → parse skipped (TCF only
       parsed when normalized signal is Yes per round-52 H1);
       request continues to step 12 → **`200`**.
     - **`syncUser`-denied at step 10** → **`451`** empty body
       (round-59 M2 — wins over the permission-path 400 above;
       matches §15 row + round-54 H3).
       `resolve_sync_privacy` itself does NOT emit a malformed-TCF
       `Err` for /setuid — the decision lives in the permission path.
   - **Malformed `gpp_sid`** → body `invalid gpp_sid encoding, must
be a csv list of integers` (the specified section-ID error response).
   - **Malformed GPP** → body `error parsing GPP header, header must
have type=3` (round-45 M2).
     The /setuid call passes `us_privacy: None` (B.4 has no us_privacy
     field), so the round-41 M3 "USP silently cleared" case is vacuous
     here. **Ordering relative to activity (step 10)** (round-50 L2 +
     round-54 H3 + round-55 H2 — corrected after round-53 H2 moved
     malformed-TCF to permission-path): only **request-extraction**
     errors from this step (bad GPP / bad gpp_sid / unresolved
     Invalid legacy gdpr / empty consent under parsed Yes) fire
     before activity → 400 beats 451 (the §15 intentional divergence
     row). **Malformed non-empty TCF** does NOT — it's a
     permission-path decision in step 11 (round-53 H2), which runs
     AFTER step 10 activity, so `syncUser`-denied + malformed
     non-empty TCF → 451 wins (matches upstream).
10. **`activity_allow(syncUser, Component::bidder(name))`** denied →
    **`451`** with **empty body** (Appendix F; this row matches
    Upstream — only the ordering relative to step 9 is the divergence).
11. **Host-cookie GDPR check + disabled-syncer check** (round-52 H3
    — narrowed; earlier "call sync_allowed which includes
    BidderSyncAllowed + SkipWhen.gpp_sid" was wrong — upstream
    reference UID-write handling only checks the host-cookie GDPR path, not
    chooser-ladder per-bidder decisions). The /setuid handler calls
    a `setuid_allowed(SyncerLookup{...}) -> SyncDecision` helper that
    **only consults** the host-cookie GDPR result (via the
    permission path from step 9) plus the syncer `enabled` flag —
    NOT `BidderSyncAllowed` and NOT `SkipWhen.gpp_sid`:
    - `BlockedByPrivacy` (host-cookie GDPR blocked, i.e.
      `host_cookie_sync_allowed == false` from step 9) → **`451`**
      with the pinned `gdpr_consent string prevents the user sync…`
      body (Appendix F).
    - `BlockedByDisabledUsersync` (syncer `enabled == false` in the
      build-time config) → **`400`** body `The bidder syncer is
disabled` (Appendix F; intentional divergence per §15 row
      "/setuid disabled-syncer rejection" — auctioneer unifies the
      disabled-syncer check across both sync endpoints; upstream
      `/setuid` resolves the syncer at reference UID-write handling without
      the chooser).
    - `Allowed` → continue.
      `BidderSyncAllowed` and `SkipWhen.gpp_sid` apply **only to
      /cookie_sync** (they are chooser-ladder decisions per Appendix
      B.3 steps 6-9); /setuid never invokes them.
12. **Cookie write.** `uid` empty → unsync (remove the family slot);
    else `Sync` with 14-day expiry (`Clock`). Cookie write with
    `PriorityBidderEjector` eviction (Appendix B.6).
13. **Response** per `f` (`i` → 1×1 PNG, `b` → empty HTML, absent →
    syncer default; round-17 L9).

**Error precedence** (round-42 H1 + round-43 H1+H2 + round-47 M2 —
stale "gdpr-coercion (1)" removed; gdpr coercion is no longer a 400
gate at step 1 per round-46 H1, so it can't short-circuit). Earlier
steps short-circuit later ones: query-parse (1, structural-only,
e.g. unparseable query string) → malformed host (2) → opt-out (3) →
bidder validation (4) → `f` validation (5) → account error (6) →
privacy parse error (9, includes the deferred legacy-`gdpr` 400 when
GPP did not override) → activity 451 (10) → per-syncer decision (11). Steps 7-8 have no
failure path (syncer lookup is from a fixed registry; cookie decode
failure → fresh empty cookie per §8.5). **Opt-out (step 3) now beats
bidder/`f` validation** (round-43 H2 — matches reference UID-write handling):
an opted-out request with missing `bidder` returns `401`, not `400`.

### 8.4 `GET /getuids`

Order (round-43 H1 — host config resolution made explicit):

1. **Resolve host config** (per-request merge + validate per §9.1;
   malformed → **`500`** body `Malformed host configuration`).
2. **Host opt-out cookie check** (round-37 H1 / §8.5; matches upstream
   reference UID-read handling). If the host opt-out cookie matches, return **`200`
   with body `{}`** (NOT `{"buyeruids": {}}` and NOT `401`) without
   reading `uids` or running host-cookie import — round-45 M3:
   Upstream's response struct has `Buyeruids` tagged
   `json:"buyeruids,omitempty"`, so an empty map is _omitted_ from
   the serialized output, leaving just `{}`. The `401` opt-out path
   applies only to `/setuid` and `/cookie_sync`.
3. Otherwise, read the `uids` cookie, **run host-cookie import to merge
   the host cookie into the in-memory `uids`** (matches upstream's
   reference UID-read handling / reference UID-cookie handling, round-15 finding M3 —
   observable in `buyeruids` / round-27 finding L5), and return
   `{"buyeruids": {syncerKey: uid}}`. No privacy gating beyond opt-out,
   no writes to the response cookie.

### 8.5 The `uids` cookie module

In `auctioneer-core`. Uses `Clock` (§5.7) for all expiry math.

- **Host opt-out cookie check — runs before `uids` decoding** (round-26
  H2 + round-27 L5 + round-40 M3; matches reference UID-cookie handling):
  `host_cookie.optout_cookie` is **optional** (§9 / round-40 M3 —
  matches reference configuration validation default-empty). When **omitted** or set to
  `{name: "", value: ""}`, the opt-out feature is disabled — no
  endpoint ever short-circuits on a host opt-out cookie. When
  **configured** (both `name` and `value` non-empty after merge), the
  check fires: if the request carries the named cookie with that exact
  value, the user is treated as opted out — `uids` is not consulted.
  Endpoint behavior under host opt-out:
  - `/setuid` → **`401`** (matches activity / cookie-`optout` flow).
  - `/cookie_sync` → **`401`**.
  - `/getuids` → **`200`** with body `{}` (round-45 M3 — upstream
    reference UID-read handling uses `Buyeruids json:"buyeruids,omitempty"` so an
    empty map is omitted from the serialized output, NOT
    `{"buyeruids": {}}`; opted-out cookies yield an empty UID map,
    **not** a 401).
    Tests in §11.1 cover the case where the host opt-out cookie is present
    and `uids` carries a non-opted-out value: the host signal must win.
- **Inner payload:** padded base64-URL of JSON `{ "tempUIDs": { "<syncerKey>": {
"uid": "<id>", "expires": "<RFC3339>" } }, "optout": <bool> }`. Cookie name
  `uids`. Production wire is `v1.<key-id>.<payload>.<mac>` under the
  [M5 integrity addendum](2026-10-13-usersync-cookie-integrity-design.md).
  Authenticate before decoding; invalid/unsigned state is 400, not a fresh
  cookie. Missing cookies remain fresh state, and host opt-out still wins.
- Per-entry TTL **14 days** (independent of the cookie's `Expires`).
- `optout` true → all UIDs discarded.
- **Set-Cookie attributes:** `Path=/`; `Expires = clock.now() +
host_cookie.ttl_days` (default 90d); `Domain` from `host_cookie.domain`
  when set; `Secure` + `SameSite=None` per the UA rule below; no `HttpOnly`
  (the cookie is intentionally JS-readable).
  Empty UID-map writes explicitly delete the same Path/Domain cookie with
  `Max-Age=0` and a past `Expires`; the full signed field is capped at 4 KiB.
- **UA rule (review finding 11):** `Secure` + `SameSite=None` are set only
  when the request `User-Agent` matches `Chrome/<major>` or `CriOS/<major>`
  with `<major> >= 67`. The check scans the UA string for those two tokens,
  parses the integer immediately after the slash, and compares. A missing,
  empty, or unparseable User-Agent → **not a match** → both attributes
  omitted (conservative default; mirrors upstream's `siteCookieCheck`).
- Max-size eviction governed by `host_cookie.max_cookie_size_bytes`
  (`0` = unlimited; if set, ≥ 500). Algorithm in Appendix B.6.

### 8.6 Syncer URL safety & macro escaping

**`/cookie_sync`** emits bidder sync URLs to the client (in
`bidder_status[].usersync.url`); **`/setuid` does NOT emit them** —
it is the _callback_ endpoint that receives a bidder's redirect, parses
the `uid` parameter, and writes the `uids` cookie (round-51 L5 —
corrects earlier "/cookie_sync and /setuid emit bidder sync URLs"
wording). Outbound URL composition / template safety therefore applies
**only to /cookie_sync**; /setuid concerns itself with callback-parse
correctness, cookie writes, and response format. The URL-safety rules
below get the same care as bidder endpoints (§5.4):

- **Two config fields, two macros — exact mapping.**
  - `user_sync.external_url` is a **base URL value** (auctioneer's externally
    reachable origin); it feeds the `{{.ExternalURL}}` macro.
  - `user_sync.redirect_url` is a **template** for the redirect-back URL
    (auctioneer's own `/setuid` callback). Its default is
    `{{.ExternalURL}}/setuid?bidder={{.SyncerKey}}&gdpr={{.GDPR}}&…`; once its
    macros resolve, the result is the value substituted into the bidder
    syncer URL's `{{.RedirectURL}}` macro. Because the default references
    `{{.ExternalURL}}`, `external_url` is effectively always required;
    `redirect_url` may be omitted to take the default.
  - The _bidder syncer endpoint_ itself (scheme + host) is build-time syncer
    config and is never templated.
    All three are **config-derived, not request-derived**; macro values are
    percent-encoded when embedded.
- **`user_sync.external_url` / `user_sync.redirect_url` validation — two
  phases, since `redirect_url` is a template, not a URL.**
  - `external_url` is a **URL value**: must be an absolute `https://` URL
    with a well-formed authority.
  - `redirect_url` is a **template**: first validated to contain only macros
    from the fixed syncer macro set above, then resolved by substituting
    `external_url` into `{{.ExternalURL}}` and dummy values into the
    remaining macros — the resolved URL must be an absolute `https://` URL.
  - Embedded defaults are validated in `build.rs` codegen; invalid defaults
    fail the build. Pushed runtime overrides are validated against the
    effective baked host/account configuration on push and load; an invalid
    stored override fails closed through the existing config error path.
    It is not replaced with a guessed request-derived callback origin.
- **Per-template macro allow-lists** — each template only exposes the macros
  it should reference; no self-reference, no leakage across phases:
  - **`user_sync.redirect_url`** (auctioneer's own setuid-callback template):
    allowed = `{{.ExternalURL}}`, `{{.SyncerKey}}`, `{{.BidderName}}`,
    `{{.SyncType}}`, `{{.UserMacro}}`, and the privacy macros (`{{.GDPR}}`,
    `{{.GDPRConsent}}`, `{{.USPrivacy}}`, `{{.GPP}}`, `{{.GPPSID}}`).
    **`{{.RedirectURL}}` is NOT in this allow-list** — self-reference is
    nonsensical.
  - **Bidder syncer URL template** (per-bidder template from bidder-info):
    allowed = `{{.RedirectURL}}` plus the privacy macros (passed through to
    the bidder for echo if needed). `{{.ExternalURL}}` / `{{.SyncerKey}}` /
    `{{.BidderName}}` / `{{.SyncType}}` / `{{.UserMacro}}` are **not** in
    this allow-list — they belong in `redirect_url`.
  - A macro outside the relevant allow-list causes a build failure for
    embedded data (§6) or `500` at request time for a `config_store` host
    override (§9.1).
  - _When_ each is resolved is in the lifecycle below; `{{.UserMacro}}` is
    never resolved by auctioneer at all.
- **Macro lifecycle — two named phases, in order; mirrors upstream's
  `buildTemplate` / `GetSync`.** Neither phase is `build.rs` codegen — both
  happen at runtime.

  1. **Template construction** (per request, after the host-config
     merge of §9.1 — round-43 M4; earlier "once, when the syncer is
     first assembled" wording cached runtime config in violation of
     the per-request merge rule). In auctioneer's redirect-back URL
     template, resolve the **config macros** from the per-request
     `HostConfig`: `{{.ExternalURL}}` (from
     `user_sync.external_url`), `{{.SyncerKey}}`, `{{.BidderName}}`,
     `{{.SyncType}}`. Leave `{{.UserMacro}}` and the privacy macros
     (`{{.GDPR}}`, `{{.GDPRConsent}}`, `{{.USPrivacy}}`, `{{.GPP}}`,
     `{{.GPPSID}}`) unresolved. **Percent-encode** the resulting
     (still partially-templated) string and substitute it into the
     bidder syncer URL template's `{{.RedirectURL}}` macro. The
     encoding is chosen to preserve unresolved `{{...}}` tags intact
     so they survive the bidder's redirect (mirrors upstream's
     `escapeTemplate`). Per-request reconstruction matches the §9.1
     "no auctioneer-side cache" rule; cost is bounded by the syncer-table
     size (~tens of templates).
  2. **Request resolution** (per `/cookie_sync` invocation only —
     round-61 L4 + round-51 L5; /setuid does NOT emit bidder sync
     URLs and therefore does NOT run template resolution). Resolve
     the **privacy macros** from the request's consent signals,
     percent-encoded before substitution. `{{.GDPR}}` uses
     `parsed_request_signal` after SID precedence: Yes -> `1`,
     No -> `0`, Ambiguous -> empty. Never emit the host-defaulted
     `normalized_request_signal` or the enforcement enable flag.
     A callback `gdpr=` remains ambiguous on `/setuid` (Appendix B.1);
     host defaults apply only to local permission checks, not macro output.
     `{{.GPPSID}}` preserves the decoded raw SID string, including `-1`.
     `{{.UserMacro}}` is
     **never resolved by auctioneer** — the bidder substitutes the
     user's bidder-side UID before calling back into `/setuid`
     (which then parses the cookie callback, not a template).

- **Scheme** — resolved syncer URLs must be `https://`. The
  `insecure-bidders` build feature (§5.4) relaxes this with the
  **exact same host-rule set** as the bidder endpoint validator
  (round-70 M7 — was vague "local Mocktioneer syncer only";
  pinned now so the feature on/off behavior is testable):
  - Allowed insecure host (literal authority): `127.0.0.1`,
    `[::1]`, or `localhost`. Any other private-range IP literal
    (`10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`,
    `169.254.0.0/16`, etc.) is **rejected** even with the feature
    enabled — local-only really means localhost-only.
  - Port: **any** port allowed (Mocktioneer typically binds a
    random high port). No port-allowlist constraint.
  - Public DNS hostnames (e.g. `mocktioneer.local`) → **rejected**:
    the relaxation is for the literal localhost authorities above,
    not for arbitrary `http://` URLs.
  - No runtime config can re-enable insecure syncer URLs. The
    relaxation lives entirely behind `#[cfg(feature =
"insecure-bidders")]`; a stray `allow_insecure` field on a
    `config_store` host override cannot bypass the build-time
    feature gate.
  - **Paired tests in §11.2 — split between codegen (build-time)
    and runtime surfaces** (round-71 M8 — earlier wording conflated
    "fails codegen + validate_endpoint with code 5"; codegen
    failures have no runtime code, and code 5 is for auction
    bidder outbound validation, not sync URL emission):
    - **Codegen tests** (build-time, `build.rs` snapshot — exact
      cargo commands per round-75 M1; auctioneer-config owns the
      build script so the feature must reach its package, not
      auctioneer-bidder's):
      - **Feature OFF** (`cargo build -p auctioneer-config`): a
        vendored bidder-info TOML carrying a syncer URL of
        `http://127.0.0.1:8080/sync` fails the `build.rs`
        template-validation pass with a compile-time error
        message like `syncer URL must be https`.
      - **Feature ON** (`cargo build -p auctioneer-config --features
insecure-bidders`): the same fixture compiles cleanly.
      - **Feature ON + non-localhost** (`cargo build -p
auctioneer-config --features insecure-bidders` with a
        `http://internal.example.com/sync` fixture): fails
        codegen, asserts the localhost-only rule applies even
        with the feature on.
      - **Feature OFF + non-localhost**: also fails codegen
        (same rule, narrower trigger).
        All four assertions are committed as snapshot tests in
        §11.2.
    - **Handler tests** (runtime, `/cookie_sync` end-to-end):
      with the build's feature ON, `/cookie_sync` emits the
      fixture's syncer URL verbatim in
      `bidder_status[].usersync.url`; the response is `200` with
      the URL byte-equivalent to the input. There is **no** code
      5 here — code 5 is `FailedToRequestBids` for auction
      outbound validation per Appendix G; `/cookie_sync` URL
      emission either succeeds with the URL in the response or
      fails at codegen above, never at runtime with a per-bidder
      error code.
- **Authority safety — same rules as §5.4 `validate_endpoint`** (round-33
  finding M6): syncer URLs must satisfy every primary outbound-safety
  check just like bidder endpoints. Specifically: (a) **no authority
  macros** — `{{...}}` may only appear in the path/query of a syncer
  URL, never in the scheme or host. (b) **IP-literal authority rule —
  default build vs `insecure-bidders`** (round-71 M7 — same exception
  as §5.4 / round-71 H1, so the relaxation is uniform across bidder
  endpoints AND syncer URLs): default builds reject any
  private/loopback/link-local IP literal in the authority and reject
  `localhost`; with `#[cfg(feature = "insecure-bidders")]` the
  rejection narrows to a **localhost-only allowlist** —
  `127.0.0.1`, `[::1]`, and `localhost` are permitted, all other
  private / link-local IP literals stay rejected. (c) **Request
  data never
  influences the authority** — request-time macro resolution applies
  only to privacy macros in path/query (§8.6 lifecycle); a syncer
  template whose authority depends on a request value is rejected at
  `build.rs` codegen. (d) For host-varying syncers (rare in phase 1),
  the same build-time `host_allowlist` mechanism applies as for bidder
  endpoints. Tests in §11.2 parallel the `validate_endpoint` test suite.
- Syncer URL templates are build-time data (§6) and are validated in
  `build.rs` codegen with dummy values — an unresolvable template fails the
  build. Edge runtimes have no meaningful long-lived startup phase, so static
  config is validated at build time and runtime overrides at request time.

---

## 9. Configuration Surface

Host config (embedded defaults, optional `config_store` override):

- **`status_response`** — optional host string (round-68 M4 — was
  referenced by §8.1 + Appendix F /status rows without a definition):
  - **Default**: empty (matches reference configuration validation
    `StatusResponse: ""`).
  - **Override behavior**: typed host override resolves per §9.1;
    an absent/null optional overlay inherits the baked value. An explicit
    empty string sets an empty banner. Any string value (including
    whitespace) is treated as the literal response body.
  - **Validation**: round-117 P0-2 (M3b) — capped at **4096 bytes**
    (the M3b memory proof charges every config-derived string, and an
    unbounded operator string inside a whole-isolate envelope is a
    contradiction; validate() rejects longer values at push time —
    M3a shipped uncapped, so the cap lands with M3b-1's validation
    pass). Within the cap, no shape constraint — the bytes
    are emitted verbatim per the §8.1 + Appendix F rows. A
    `status_response` containing CR/LF or non-UTF8 bytes is the
    operator's responsibility; auctioneer does not sanitize it.
  - **✅ RESOLVED — `/status` REPORTS WHAT IS WRONG (D-1, decided by
    this spec's owner 2026-09-19; supersedes the fail-closed rule
    stated below FOR THIS ROUTE ONLY).** This row and the shipped
    handler were in conflict from round 178: the spec required a 500
    on malformed runtime config, while `handlers/status.rs`
    deliberately degraded to baked defaults and
    `corrupt_blob_keeps_status_answering()` pinned that it must NOT 500. The conflict was framed as a choice between availability and
    signal. **It is not a choice: the status code is not the only
    channel.**

    **The contract:** `/status` is a liveness probe. It KEEPS
    ANSWERING when runtime config cannot be loaded — a probe that
    500s through a config-store outage turns a degraded-but-serving
    fleet into a restart loop that cannot fix the store — AND it
    NAMES THE FAULT, because a probe that degrades silently is
    indistinguishable from a healthy one. Concretely: every answer
    carries `x-auctioneer-config-status: ok | degraded`, and a degraded
    answer is 200 with the fault named in its body as
    `config: degraded (<code>)`. **`<code>` is a CLOSED auctioneer-owned
    enum with FOUR variants, whose WIRE FORM is snake_case:
    **`malformed_config`** (blob unparseable, untrusted or invalid),
    **`store_key_rejected`** (the bound key was refused — a distinct
    operator fix from a corrupt blob, which is why two codes were
    not enough), **`store_unavailable`** (transient backend
    failure), **`unfunded_extraction`** (boot funding is absent or the
    App's captured SDK extraction caps exceed the installed normalized caps).
    The latter is the [approved R0 initialization boundary](2026-10-17-m7-r0-wasm-resource-envelope-design.md#42-approval-and-scope-enforcement):
    refuse before any store read, keep bounded liveness, and permit recovery
    through a newly configured funded App. Missing bindings keep their existing
    policy because they perform no extraction.
    The Rust variants are `ConfigStatusCode::
{MalformedConfig, StoreKeyRejected, StoreUnavailable, UnfundedExtraction}`; the wire
    string is fixed per variant and never derived from `Debug`. It
    is NEVER
    `RuntimeConfigError::detail` (round-186 #5).** An earlier
    revision of this row said `detail` was safe to publish because a
    leak guard covered it; that guard covers only upstream error
    text, while `validate_alias_bases` interpolates STORE-CONTROLLED
    alias and base names into the same field. `/status` is
    unauthenticated, so the public channel carries the closed code
    and the full `detail` goes to logs only. A healthy deploy with an
    empty `status_response` still answers 204.

    **Scope of the exception:** it covers `/status` alone, on the
    stated ground that it reaches no privacy or auction decision.
    **The fail-closed 500 rule below is UNCHANGED for every route
    that does** — that rule exists to stop a request being answered
    under config nobody can vouch for, and a liveness probe cannot
    cause that harm. Implemented by **M3b-1c** in the M3b plan;
    `corrupt_blob_keeps_status_answering()` keeps passing unchanged,
    which is the regression guard that this exception did not widen.

  - **Interaction with malformed-host failures**: `/status` does
    NOT bypass the per-request host-config merge — but a malformed
    merged host does **NOT** make it answer `500`. It answers `200`
    `degraded (malformed_config)` like any other load failure, per
    the resolution above. **ROUND-187 #7 — this bullet previously
    stated the `500` rule flatly and left the correction to a
    paragraph above it; a reader who stopped here implemented the
    opposite of the contract.** The `500` + `Malformed host
configuration` body (§9.1, Appendix F) remains the rule for
    every route that reaches a privacy or auction decision. Operators
    relying on `/status` for liveness read
    `x-auctioneer-config-status`, not the status code — a well-formed
    host override is still worth ensuring, but its absence no longer
    breaks the probe.
    **⚠ ROUND-186 #6 — THIS ROW IS CORRECTED: `/status` DOES NOT
    500 HERE EITHER, AND THE ROUND-185 JUSTIFICATION FOR IT WAS
    FACTUALLY WRONG.** Round 185 kept this 500 by calling a malformed
    merged host "per-request, caller-supplied, fixed by correcting
    the request". It is none of those things. The runtime host
    overlay comes from the CONFIG STORE, not from the caller, and
    `resolve::resolved_host(&AuctioneerAppConfig, &HostConfig) ->
HostConfig` is INFALLIBLE — it returns a `HostConfig`, not a
    `Result`, so there is no post-load merge that can fail and no
    request-scoped fault for the 500 to report. Every way the merged
    host can be malformed is detected during LOAD, as
    `RuntimeConfigError::MalformedHost`, which is precisely the case
    D-1 resolved.
    **So `/status` treats it identically to any other load failure:**
    it answers from baked defaults, marks
    `x-auctioneer-config-status: degraded`, and names the closed code
    `malformed_config`. Keeping a 500 here would have reintroduced the
    exact restart loop D-1 removed, through a path an operator could
    not distinguish from the one it fixed. **The 500 remains for
    every OTHER route** (Appendix F): a route that reaches a privacy
    or auction decision must not answer under a host nobody can
    vouch for. `/status` reaches no such decision — that is the whole
    basis of the exception, and it does not depend on where the bad
    bytes came from. Operators relying on `/status` for liveness read
    the header, not the status code.
- `gdpr.enabled`, `gdpr.default_value` (**required** `"0"`/`"1"`),
  `gdpr.tcf2.*` (the EEA country set is **not** a config field —
  round-86 H7: it is baked from `data/eea-countries.txt` into the
  compiled `auctioneer_config::eea_set()` constant, per §6 / the
  `data/eea-countries.txt` row above, and is NOT `config_store`-overridable),
  **`gdpr.host_vendor_id`** (round-48 H2 + round-52 L5 — matches
  Upstream reference configuration validation): integer IAB vendor id for the
  **host's own** syncer. **Range `[0, 65535]`** — values outside
  this range cause a `MalformedHost` failure at config-merge time
  (matches reference configuration validation which rejects out-of-range
  values; round-52 L5). Drives the host-cookie GDPR check in §8.2
  (resolves which vendor's consent the host syncer needs). Special
  value **`0`** skips the host-company GDPR check entirely (matches
  Upstream reference configuration validation + reference GDPR permission evaluation — "if host
  vendor id is zero, skip GDPR check on host cookie"). Default `0`.
  **`gdpr.non_standard_publishers`** (round-48 H2 — matches upstream
  reference TCF permission evaluation): list of publisher account ids for which
  the auction path returns allow-all from `AuctionPermissions`
  regardless of TCF state (the Appendix A.4 "non-standard publisher"
  row references this list). Default empty. Listed account ids
  bypass per-purpose enforcement; they do NOT bypass the activity-
  control framework (§7.6) — that gate runs separately.
- `ccpa.enforce`, `lmt.enforce`.
- `host_cookie.*` — **required vs optional split** (round-40 M3 +
  round-42 M3 — auctioneer hardening; reference configuration validation defaults
  `cookie_name` and `family` to empty strings, auctioneer rejects empty.
  See §15 row "Host-cookie required fields"):
  - **Required (auctioneer hardening).** `cookie_name` identifies a separate
    host identity cookie imported into the fixed `uids` cookie; it cannot
    be empty or equal `uids`. `family` is the imported UID's family key
    (§8.2); an empty key collides with every other family slot.
  - **Optional.** `domain` (omitted → no `Domain` attribute on
    Set-Cookie); `ttl_days` (defaults to 90, must be positive);
    `max_cookie_size_bytes` (0 / unset → no _logical eviction threshold_,
    while the 4 KiB complete signed Set-Cookie hard cap remains); `optout_cookie` (omitted or
    `{name: "", value: ""}` → opt-out feature disabled, matches upstream
    reference configuration validation which defaults both to empty strings). When
    `optout_cookie` is configured, both `name` AND `value` must be
    non-empty strings — `{name: "x", value: ""}` is `MalformedHost`,
    same for the inverse.
- `user_sync.*`: `external_url` is the configured public HTTPS callback
  origin plus optional base path, not a value inferred from `Host`,
  `Origin`, or forwarded headers. The committed
  `https://sync.example.test` is demonstration data to replace before
  deployment. Optional `redirect_url` overrides the default `/setuid`
  callback template. `cooperative.enabled_by_default` and
  `priority_groups` govern selection. `allowed_origins` defaults empty
  and lists exact scheme/host/port origins for credentialed JavaScript
  access; it is not a wildcard, consent grant, or cookie guarantee.
  Baked changes require rebuild; `runtime.user_sync` field overrides
  require a config push. Account `cookie_sync` fields override selection
  defaults; an explicit empty account priority list replaces the host
  list. `runtime.bidders.<name>.usersync_enabled` may disable a compiled
  syncer but cannot create new bidder metadata.
- Resource limits (§5.3); price-granularity presets.

Account config: `disabled` (bool, account-level enablement; matches upstream's
`account.Disabled`), `gdpr.*` overrides, `ccpa.channel_enabled`,
`privacy.allowactivities`, `privacy.ipv4/ipv6.anon_keep_bits`,
`cookie_sync.{default_limit,max_limit,default_coop_sync,priority_groups}`,
`debug_allow`, `bid_rounding`, `targeting_prefix`,
`truncate_target_attribute`.

### 9.1 Config resolution semantics

**Current through M6:** embedded defaults plus one typed runtime
`AuctioneerAppConfig` blob, loaded through EdgeZero's existing extractor.
The [M6 hardening design](2026-10-05-m6-parity-hardening-design.md)
sections 7-8 define residual acceptance and update behavior. The historical
separate `host` / `account/{id}` storage and generic JSON deletion-patch
examples are retired, not compatibility paths. Independent account storage
and its failure/merge policy belong to M8.

- **Runtime source and trust.** The existing push path validates and
  signs/encapsulates typed runtime settings. Each config-consuming request
  resolves one validated snapshot through the existing binding/extractor;
  host and account resolution reuse that snapshot. M6 adds neither a second
  loader nor another runtime signing protocol.
- **Typed overlays.** Absent optional fields inherit baked defaults.
  Serialized JSON `null` on an optional field or optional block also means
  absent, not deletion. Empty objects are no-op overlays. Explicit false,
  zero, empty strings, and empty lists retain their typed meaning wherever
  valid. Present arrays replace whole, including `[]`. Partial nested
  overrides preserve sibling defaults. Invalid effective required values
  fail whole-blob validation; a null overlay does not delete a required
  defaulted field. No generic RFC 7396 rewrite is required.
- **Host and account ownership.** Resolve each typed schema from its own
  baked default plus its runtime overlay. Only declared tri-state account
  privacy settings consult host policy when absent in the effective account
  after baked-plus-overlay resolution, not merely absent/null in its raw
  overlay. Unrelated settings do not cross the host/account boundary.
- **Bidder safety metadata is compiled.** Endpoints, macros, host
  allowlists, syncer templates, and GVL IDs are not runtime-overridable.
  Existing runtime enable/disable, timeout, sampling, and usersync controls
  continue through their declared typed fields; they do not create bidder
  implementations or relax compiled eligibility.
- **Auction account selection.** After bounded JSON syntax/node checks,
  discover identity from raw input before typed preparation, in order:
  `app.publisher.ext.prebid.parentAccount` → `app.publisher.id` →
  `site.publisher.ext.prebid.parentAccount` → `site.publisher.id` →
  `dooh.publisher.ext.prebid.parentAccount` → `dooh.publisher.id`.
  Missing/empty identity selects the baked default without looking up
  `accounts.unknown`. A nonempty named account uses its typed overlay
  when configured; otherwise policy falls back while preserving the
  supplied identity for attribution and allowed endpoint macros.
- **Sync account selection.** `/cookie_sync` and `/setuid` use the
  request's `account` field with the same missing-versus-named distinction.
  Literal named `"unknown"` may select that configured account; an absent
  identity does not. `/getuids` retains its existing host-cookie behavior.
- **Whole-blob failure.** Malformed, invalid, integrity-failing, or
  structurally over-limit runtime data yields the existing
  `RuntimeConfigError::MalformedHost` → 500 with the fixed safe body,
  including malformed data inside an unrelated account. This happens
  before application opt-out/account/body evaluation. A rejected store key
  retains its existing 500 classification. Transient backend, secret-backend,
  or extraction-deadline failures remain 503 with Retry-After. Diagnostic
  details do not reach the public response. Probe routes preserve their
  separately specified degraded response behavior.
- **Missing runtime data.** Baked fallback is allowed only when the compiled
  bootstrap `allow_missing_config` permits absence; otherwise missing
  binding/blob is the existing malformed-config failure. Neither malformed
  data nor outages may invoke missing-data fallback or last-known-good policy.
- **Account disabled.** After a valid snapshot and within-budget account
  discovery, auction account-disabled maps to 503 / code 7;
  `/cookie_sync` and `/setuid` map it to their existing 400 body.
  The separately fetched `MalformedAcct` / code-10 500/400 mappings are
  reserved for the M8 account-store design, not reachable M6 blob outcomes.
- **Snapshots and ceilings.** Permissions, debug gates, aliases, resource
  clamps, and response decoration use the same request snapshot. Later
  requests load again; runtime changes cannot expand boot-funded ceilings
  or reinitialize the admission envelope.
- **Freshness.** Auctioneer adds no cross-request config/policy cache. Local
  controllable-store tests prove update consumption, snapshot consistency,
  and invalid/outage/deletion/recovery behavior, not instantaneous provider
  propagation. Provider cache/refresh bounds require separately recorded
  EdgeZero/provider evidence; an unspecified propagation bound does not
  block unrelated M6 behavioral work.

---

## 10. Error Handling Summary

| Situation                                                                                                                                                                                     | Result                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Auction body parse failure / oversize                                                                                                                                                         | `400` (Appendix F)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Auction validation failure                                                                                                                                                                    | `400` (Appendix F)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Malformed runtime blob, including account overlays                                                                                                                                            | `500` (§9.1; `MalformedHost`, no per-bidder code)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| Auction: account disabled                                                                                                                                                                     | `503` (§9.1; code 7 `AccountDisabled`)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| Sync: account disabled after valid config resolution                                                                                                                                          | `400` (§9.1; endpoint-specific)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| Independently fetched malformed account record                                                                                                                                                | Reserved for M8; not a reachable M6 runtime-blob outcome                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| Malformed host config override                                                                                                                                                                | `500` (§9.1)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| Bidder transport/parse error                                                                                                                                                                  | non-fatal → `ext.errors`, other bidders proceed                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| Auction with no bids / all failed                                                                                                                                                             | `200` — shape per case, Appendix E.5                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| No outbound capability available                                                                                                                                                              | `500`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `cookie_sync`: missing consent under `parsed_request_signal == Yes` OR (`Ambiguous` + `normalized_request_signal == Yes`) (round-52 H2; §7.8.1 taxonomy) / bad `gpp` / bad `gpp_sid`          | `400`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `cookie_sync`: malformed non-empty TCF consent under `normalized_request_signal == Yes` + `gdpr_permissions_enabled` (round-47 H1 + round-48 H1 + round-49 H2 + round-52 H1+M4 + round-59 L3) | **`200`** with empty `bidder_status`. **Host gate** depends on `host.gdpr.host_vendor_id`: `0` → host gate allows (matches host-cookie GDPR permission wrapper); non-zero → host gate fails. **Per-bidder gate** (`sync_allowed`) fails on malformed TCF regardless of `host_vendor_id`, so `bidder_status` is empty in both cases (matches reference cookie-sync handling). **/setuid behavior** on the same input requires the full precondition list (round-59 L3 — earlier "0 → 200; non-zero → 400" shortcut omitted activity): `host_vendor_id == 0` → 200; `host_vendor_id != 0` AND activity allowed → 400 body `gdpr_consent was invalid. <parser error>`; `host_vendor_id != 0` AND activity denied → 451 empty body (matches §15 + round-54 H3). |
| `cookie_sync`: `normalized_request_signal != Yes` OR `!gdpr_permissions_enabled` + malformed TCF consent (round-48 H1 + round-52 H1)                                                          | **`200`** normal response — TCF never parsed when normalized signal is not Yes (or when host disables GDPR); the malformed value is silently ignored on both /cookie_sync and /setuid                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `cookie_sync`: malformed `us_privacy` only (round-41 M3)                                                                                                                                      | **NOT** `400` — silently cleared, request continues (matches reference cookie-sync handling)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| `cookie_sync`: host cookie GDPR-blocked                                                                                                                                                       | `200`; JSON top-level `status` is still `"ok"` or `"no_cookie"` (per request-cookie live-sync state, never `"StatusBlockedByPrivacy"` — that is upstream's _internal_ chooser status, not the JSON value); `bidder_status` is empty (round-16 finding M5)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| `cookie_sync`: user opted out                                                                                                                                                                 | `401`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `setuid`: missing/unknown `bidder`, bad `gpp_sid`, malformed GPP, unresolved `Invalid` legacy `gdpr` value, empty `gdpr_consent` under **`parsed_request_signal == Yes`**                     | `400` (request-extraction errors; round-56 L4 — narrowed to spell out the actual triggers per §8.3 step 9)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| `setuid`: malformed non-empty TCF consent — **permission-path 400**, NOT request-extraction (round-56 L4)                                                                                     | `400` body `gdpr_consent was invalid. <parser error>` **only when** `host_vendor_id != 0` AND `normalized_request_signal == Yes` AND `gdpr_permissions_enabled` AND activity allowed (round-53 H2 + round-54 H3 — runs in §8.3 step 11 after activity, so `syncUser`-denied wins with 451); otherwise 200 (host_vendor_id==0 short-circuit) or 451 (activity-denied wins) per the matrices in §7.8 / §11.1.                                                                                                                                                                                                                                                                                                                                                 |
| `setuid`: empty `gdpr_consent` under `parsed_request_signal == Ambiguous` + `normalized_request_signal == Yes` (round-56 H1)                                                                  | `host_vendor_id == 0` → `200`; `host_vendor_id != 0` + `gdpr_permissions_enabled` → `451` body `gdpr_consent string prevents…`; `host_vendor_id != 0` + `!gdpr_permissions_enabled` → `200`. NOT a request-extraction 400 — empty consent at parsed-Ambiguous flows through the host-cookie permission path.                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| `setuid`: disabled syncer (`SyncDecision::BlockedByDisabledUsersync`, round-39 M2)                                                                                                            | `400` body `The bidder syncer is disabled` (Appendix F)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `setuid`: `syncUser` denied                                                                                                                                                                   | `451` empty body (Appendix F)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| `setuid`: GDPR host cookie blocked (`SyncDecision::BlockedByPrivacy`)                                                                                                                         | `451` body `The gdpr_consent string prevents…` (Appendix F)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| ~~`setuid`: blocked by regulation scope~~ — **NOT APPLICABLE** (round-53 H1)                                                                                                                  | —                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           | /setuid does not invoke the /cookie_sync chooser ladder, so `SkipWhen.gpp_sid` never fires there (§7.8 endpoint mapping / §8.3 step 11 — matches reference UID-write handling which only checks host-cookie GDPR). Earlier rev mistakenly listed this as a /setuid outcome. |
| `setuid`: user opted out                                                                                                                                                                      | `401`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |

Canonical body formats and content types in **Appendix F**.

---

## 11. Testing

Two layers, **both required** (§1.2). TDD: tests are written before the
implementation they cover. A milestone (§14) is "done" only when every
component it ships has green unit tests **and** the corresponding
integration tests are green.

### 11.1 Unit tests (fast, no I/O, per-module/per-function)

- **OpenRTB types** — serde round-trip fidelity; **`prepare()`
  invariants** (§3.2: validate, **rejects requests carrying legacy
  `regs.ext.gdpr` / `regs.ext.us_privacy` / `user.ext.consent`**,
  **rejects `ext.prebid.channel.name == ""`**, **no consent-string
  warnings emitted here** (round-37 H3 + round-38 M2 — consent warnings
  are emitted by `PrivacyContext::take_warnings()`, §7.7, and asserted
  in the GDPR / CCPA / GPP unit tests below); **no alias normalization,
  no `cur` defaulting** — aliases resolve in `run_auction` **step 7**
  (§5.1; round-44 M2 — corrects stale "step 6" after round-43 H1
  renumbering));
  `validate()` cases. Fixtures from reference server
  `endpoints/openrtb2/sample-requests/valid-whole/*.json`.
- **Adapters** — `make_requests` / `make_bids` with fixture pairs (pure;
  the trait is synchronous so no mocking is needed).
- **GDPR** — Appendix A truth tables as a test matrix; TCF v2 string vectors.
- **`PrivacyContext::take_warnings()`** (round-37 H3 + round-38 M2) —
  malformed TCF / US-Privacy / GPP each emit a `NonFatalWarning { code:
10001 InvalidPrivacyConsent, scope }`, returned by `take_warnings()`
  in a single drain; subsequent calls return empty (consume-on-build).
  Scopes use the **native 2.6 paths**: `"user.consent"`,
  `"regs.us_privacy"`, `"regs.gpp"` (round-38 L5 — the legacy 2.5
  `*.ext.*` paths are rejected by §3 / `prepare()` and never appear in
  warning scopes). The auction semantic outcome on malformed/empty
  consent stays Appendix A.4 — these are non-fatal.
- **`resolve_sync_privacy`** (round-38 H1, §7.8) — table-driven test
  matrix:
  - **Malformed TCF — signal-gated, endpoint-split, host-vendor-aware
    matrix** (round-47 H1 + round-48 H1 + round-49 H1+H2 + round-52
    H1 + round-53 H2 + round-54 H2 — rewritten after H2's
    "permission-path-only" correction; `resolve_sync_privacy` does
    NOT emit `Err(MalformedConsent { scope: "gdpr_consent" })` for
    /setuid — the 400 is decided downstream by the host-cookie
    permission path):
    - **`normalized_request_signal != Yes` OR
      `!gdpr_permissions_enabled`** (gdpr=0, host default 0,
      `gdpr.enabled=false`, GPP overrode to No): malformed
      `gdpr_consent` string → `Ok(_)` on **both** endpoints with
      `host_cookie_sync_allowed == true` and `sync_allowed → Allowed`
      / `setuid_allowed → Allowed` regardless of `host_vendor_id`;
      the parser is never invoked (matches reference TCF permission evaluation
      short-circuit).
    - **`normalized_request_signal == Yes` +
      `gdpr_permissions_enabled` + malformed `gdpr_consent`**:
      - `endpoint: SyncEndpoint::CookieSync` + `host_vendor_id == 0`
        → `Ok(_)`, `host_cookie_sync_allowed == true` (matches
        host-cookie GDPR permission wrapper, reference GDPR permission evaluation / reference TCF permission evaluation); `sync_allowed → BlockedByPrivacy` (per-bidder gate
        still fails on parse error, bidder-sync GDPR permission at
        reference cookie-sync handling). Net: 200, `bidder_status` empty;
        top-level `status` derived from the request cookie's
        live-sync count per Appendix B.2 (round-71 M9 — was
        incorrectly "reflects host gate true").
      - `endpoint: SyncEndpoint::CookieSync` + `host_vendor_id != 0`
        → `Ok(_)`, `host_cookie_sync_allowed == false`; `sync_allowed
→ BlockedByPrivacy`. Net: 200, `bidder_status` empty.
      - `endpoint: SyncEndpoint::SetUid` + `host_vendor_id == 0`
        → `Ok(_)`, `host_cookie_sync_allowed == true`; the /setuid
        handler proceeds to step 12 (cookie write). Net: **`200`**
        (round-53 H2 — matches reference GDPR permission evaluation short-circuit).
      - `endpoint: SyncEndpoint::SetUid` + `host_vendor_id != 0`
        → `Ok(_)`, `host_cookie_sync_allowed == false`. The
        observable response from the /setuid HANDLER (not from
        `resolve_sync_privacy` directly) depends on whether the
        §8.3 step-10 activity check already allowed the request
        (round-59 M2 — fourth precondition explicit):
        - **Activity allowed** → step-11 permission path returns
          the parse error → caller emits **`400`** body
          `gdpr_consent was invalid. <parser error>` (round-53 H2
          — matches reference UID-write handling the consent-error response).
        - **Activity denied** (`syncUser`-denied) → step 10
          short-circuits → **`451`** empty body (round-54 H3 +
          §15 ordering row).
          Both `resolve_sync_privacy` return values are identical
          (`Ok(_)` with `host_cookie_sync_allowed == false`); the
          400-vs-451 split is purely in the caller's
          activity-vs-permission ordering. This matches the §11.2
          E2E ordering-regression fixture (round-58 M2).
    - **/setuid empty-consent matrix** (round-54 M4 + round-55 H1 +
      round-55 L5 — taxonomy fix: `parsed_request_signal` enum
      values are Yes / No / Ambiguous, NOT "Absent"; that's
      `GdprSignalIn::Absent` from the wire-coercion layer):
      - `gdpr=1` (parsed Yes) + empty `gdpr_consent` → `Err(_)` →
        **`400`** at request extraction (matches reference UID-write handling).
      - Absent `gdpr` wire (parsed Ambiguous) + `host.gdpr.
default_value="1"` (normalized Yes) + empty `gdpr_consent`
        → does **NOT** 400 at request extraction (matches
        reference UID-write handling keying off the parsed signal). Falls
        into the host-cookie permission path; outcome depends on
        `host_vendor_id` AND `gdpr_permissions_enabled` (round-55
        H1 — corrects round-54 M4's blanket 200):
        - `host_vendor_id == 0` → host-cookie GDPR permission wrapper
          short-circuits to true (reference GDPR permission evaluation) → **`200`** with
          cookie written.
        - `host_vendor_id != 0` AND `gdpr_permissions_enabled` →
          `HostCookiesAllowed` invokes the TCF check with empty
          consent → returns false (matches reference TCF permission evaluation
          which returns false on empty consent under enabled
          GDPR) → /setuid handler maps to **`451`** with the
          pinned `gdpr_consent string prevents…` body (matches
          reference UID-write handling `BlockedByPrivacy` mapping).
        - `host_vendor_id != 0` AND `!gdpr_permissions_enabled`
          (host `gdpr.enabled=false`) → permissions builder
          returns an allow-all permission result (reference GDPR permission evaluation) → **`200`**.
      - `gdpr.enabled=false` + any consent state → **`200`**
        (matches reference GDPR permission evaluation an allow-all permission result) regardless of
        `host_vendor_id`.
    - **/cookie_sync empty-consent matrix** (round-54 M4 —
      round-52 H2 explicitly covers `parsed Ambiguous +
normalized Yes`; the upstream test that pins this is
      reference cookie-sync handling fixtures):
      - `gdpr=1` (parsed Yes) + empty `gdpr_consent` → **`400`**
        (matches reference cookie-sync handling).
      - Absent `gdpr` + `host.gdpr.default_value="1"` (parsed
        Ambiguous, normalized Yes) + empty `gdpr_consent` →
        **`400`** (matches reference cookie-sync handling fixtures fixture).
      - Absent `gdpr` + `host.gdpr.default_value="0"` (parsed
        Ambiguous, normalized No) + empty `gdpr_consent` → 200
        (TCF never required; no permission-path block).
      - `gdpr.enabled=false` + any consent state → 200.
  - Malformed `gpp_sid` / GPP → `Err(_)` on both endpoints (mapped to
    400 by callers).
  - **Malformed `us_privacy` → `Ok(_)` with USP silently cleared**
    (round-41 M3 — matches upstream). Request continues; CCPA enforcement
    derived from the bad string is skipped; no warning emitted on the
    sync path.
  - `sync_allowed` per-syncer covering Appendix B.3 steps 6-9: a
    `BidderSyncAllowed=false` (GDPR vendor-not-allowed) → `BlockedBy
Privacy`; syncer `enabled=false` → `BlockedByDisabledUsersync`;
    `SkipWhen.gpp_sid` exact-string match against a raw `"2,6"` request
    sid (skip set `["2,6"]` matches; `["2"]` does NOT — Appendix B.3 step
    9 / round-15 M5) → `BlockedByRegulationScope`.
  - `host_cookie_sync_allowed` mirrors auction host-syncer GDPR decision.
  - M2 stub: `sync_allowed → Allowed`, **`setuid_allowed → Allowed`**
    (round-57 L3 — was missing; matches the canonical stub text in
    §7.8 + §14 per round-55 M4), `host_cookie_sync_allowed = true`.
- **CCPA / GPP** — 4-char parsing + `ShouldEnforce`; GPP section decode;
  precedence + error table (Appendix D); legacy downgrade fills-empty-only.
- **Activities & scrubbing** — rule order/abstain; Appendix C scrub matrix
  (every row a test case).
- **`PrivacyContext::allow_all_no_scrub()`** (round-35 finding M3) — M2
  stub: returns a context where every `activity_allow` is `true`, every
  bidder's `AuctionPermissions` is `AllowAll`, `ccpa_should_enforce` is
  `false`, and `scrub::apply` is a no-op. Unit tests assert these
  invariants explicitly so M3/M4 implementations cannot accidentally
  weaken the stub.
- **Auction output building blocks** — price-bucket table, every rounding
  mode (incl. seeded `timesplit`), targeting keys + truncation, winner
  selection (Appendix E).
- **Outbound safety** — `validate_endpoint` rejects non-HTTPS, private-IP
  literals, authority macros, off-allowlist hosts; a **default (production)
  build rejects `http://` / loopback endpoints even when bidder data carries
  an `allow_insecure` flag** (the relaxation is compiled out, §5.4); syncer
  URL macro escaping and the two-phase template validation (§8.6);
  **inbound `Content-Encoding` rejected before body read with `400`**
  (round-25 finding H2); **`EndpointBuilder` coverage** (round-30 M3):
  `{{.AccountID}}` resolves to `ResolvedAccount.id` (not raw
  `site.publisher.id`); `{{.PublisherID}}` resolves to the raw publisher
  id; `host_allowlist` membership enforced for host-varying endpoints;
  path/query macro values percent-encoded; path-traversal sequences
  (`..`, `%2e%2e`, `..`, etc.) in macro values rejected.
  **Fixed-host substitution rejection** (round-32 finding M4): a
  fixed-host bidder (`host_allowlist == None`) whose `make_requests`
  returns a `BidderHttpRequest.url` with a _different_ but otherwise
  valid HTTPS host (no macros, no traversal, public IP) is rejected by
  `validate_endpoint` with code 5 — covers the case where the adapter
  bypasses `EndpointBuilder` entirely.
- **Syncer template validation (codegen-time)** — invalid syncer URL
  templates are rejected at build time (non-HTTPS scheme, hard-coded
  private-IP authority, hard-coded `localhost` authority, authority-level
  macros). This is the unit-test layer; the matching end-to-end handler
  coverage lives in §11.2 under _Sync URL safety integration_ (round-33
  M6 + round-34 L7 + round-35 M4 + round-36 M1).
- **Outbound gzip via finalization** — round-110 B3 (REPLACES the
  round-25 M6 transport-side strip-and-rewrite test, retired with that
  behavior): when the bidder's `endpoint_compression` is `gzip` (read
  from `BidderInfo` by the exchange at finalize time — round-150 #6),
  `finalize_for_transport` compresses through the bounded writer and
  appends `Content-Encoding: gzip` into the byte pool reserve BEFORE the
  BidderCall exists (round-147 #2: NO `Content-Length` — framing is
  EdgeZero's, see §5.2); adapter-supplied entity headers never occur
  (reserved-name rejection at push). The test asserts the finalized
  request's headers carry `Content-Encoding: gzip`, carry NO
  `Content-Length`, and that the transport
  conversion forwards them unmodified.
- **Bidder-level debug gate** — compiled base `BidderInfo.debug_allow == false`
  suppresses HTTP-call entries for that bidder in `ext.debug.httpcalls`
  even when account `debug_allow` is true; emits a
  `BidderLevelDebugDisabled` warning (code 10003) in
  `ext.warnings[<bidder>]` (§5.6 / round-25 finding M5).
- **Account-level debug gate** (round-40 L6 + round-42 L5 — M6+ test):
  request asks for debug (`test=1` / `ext.prebid.debug=1` / `?debug=1`)
  AND account `debug_allow == false` → response carries **exactly one
  `AccountLevelDebugDisabled` warning (code 10002) under
  `ext.warnings.prebid[]`** (the reserved upstream key, NOT under
  any per-bidder slot — §5.6); `ext.debug` is omitted from the
  response. Independence of 10002 and 10003 is asserted: a request
  that triggers 10002 (account-level deny) produces no 10003 (no
  bidder reached the debug-capture step). M2-M5 suppression window:
  the warning is **silent** — no 10002 emitted under
  `ext.warnings.prebid[]` even when account `debug_allow == false`
  (round-33 H2 + round-34 M3), so this test runs only from M6.
- **Debug redaction** — `ext.debug` masking; emitted only when requested
  _and_ `debug_allow`. M6 sections 3-6 and 9 define the complete safe
  projection, signal/gate/warning, capture-lifetime, wire, hostile-input,
  resource, deadline, and output-omission tests; a shallow masking test
  alone is not acceptance.
- **Host primitives** — cookie expiry / `auctiontimestamp` /
  `responsetimemillis` under `FakeClock`; shuffling / `timesplit` under a
  seeded `Rng`; `gen_range` unbiased over many draws.
- **`prepare()` implicit-fill — explicit risky-case coverage** (round-33
  finding M5; **note**: §11.1 is the unit-test layer — the same scenarios
  exercised end-to-end against the handler live in §11.2 under outbound
  safety, round-34 finding L7):
  - **Sec-GPC: 1 overrides body**: header `Sec-GPC: 1` + request body
    `regs.ext.gpc: "0"` → final `regs.ext.gpc == "1"` and outbound forward.
  - **JSON-only GPC**: no `Sec-GPC` header + request body
    `regs.ext.gpc: "1"` → outbound forward (the documented §15 divergence).
  - **`at` defaulting**: separate cases for `at` absent vs `at == 0` vs
    `at == 2`; first two default to `1`, third is left alone.
  - **Site materialization without referer**: no `app`, no `dooh`, no
    `Referer` header → an empty `Site{ ext.amp = 0 }` is materialized so
    validation passes.
  - **Client-IP sanitize/fill order**: client-supplied `device.ip` =
    private (e.g. `10.0.0.1`) → cleared; then a header chain
    (`True-Client-IP` → `XFF` hops → `X-Real-IP` → `RemoteAddr`) fills in
    the first valid public IP, in that priority order.
  - **iOS LMT override matrix**: `app` set + `device.os == "iOS"` ×
    `device.ext.atts ∈ {0,1,2,3, missing}` covering the
    Authorized→`lmt=0` and Restricted→`lmt=1` cases plus the
    no-change-when-missing case.
- **`uids` cookie** — encode / decode / expiry / optout; UA-rule cookie
  attributes; max-size eviction + `PriorityBidderEjector` ordering.
- **Config resolution** — §9.1 and M6 sections 7-9: one validated typed
  runtime blob, host/account overlays, and limited tri-state privacy
  fall-through. Disabled account maps to auction 503 / sync 400 after
  structural-budget checks. Malformed data anywhere in the blob maps to
  config 500 before application opt-out/account work; transient store
  faults keep 503/Retry-After. Probe degradation remains separately tested.
- **Typed host/account overlay suite.** Core sibling preservation and
  effective-value validation remain existing regression requirements; M6
  extends deep sibling combinations and all relevant endpoint consumers:
  - A partial nested GDPR override preserves user_sync, host_cookie,
    unrelated privacy settings, and resource-limit defaults.
  - Omitted optional fields, optional-field nulls, whole optional-block
    nulls, and empty objects inherit; no required default is deleted.
  - Explicit false, zero, and valid empty values survive resolution.
    Present arrays replace whole, including an explicit empty list.
  - Invalid effective external_url/cookie fields, partial opt-out settings,
    host vendor ranges, and resource settings fail loader validation.
  - An absent optional cookie domain inherits; removal is not synthesized
    from null. Future deletion policy requires its own owning design.
  - Identical defaults plus typed overrides resolve deterministically.
  - M6 tests update, invalid/integrity-failing blob, unavailable store,
    deletion under both compiled bootstrap policies, and recovery.
  - No per-request lookup of separate host/account store keys is added.
- **Config codegen** — snapshot tests on `build.rs` output.
- **tmax** — every aspect of §5.3's tmax block: effective-budget resolution
  (request / host-default / capped); deadline anchoring at `request_start`;
  the `remaining == 0` short-circuit (before fan-out); the three timeout
  surfaces (code 1 / code 12 / statuscode 101).
- **Adapter-visible TMax vs execution deadline split** (round-28 M5;
  §5.1 step 9): a fixture adapter captures the outbound
  `BidRequest.TMax` it receives and asserts the **defaulted-not-capped**
  rule across four input cases: absent (`0`/None) → `tmax_default`;
  zero → `tmax_default`; normal (under cap) → request value verbatim;
  over-cap → request value verbatim (NOT clamped). Separately verifies
  that the _server-side execution deadline_ is still clamped to
  `tmax_max` (so over-cap requests still get cut off by the fan-out
  deadline even though the bidder saw the larger number).

### 11.2 Integration & end-to-end tests (cross-crate, runtime, snapshot files)

- **Exchange engine** — `auctioneer_exchange::run_auction` driven with a mock
  `BidderFanout`, `FakeClock`, and seeded `Rng`: fan-out, result
  correlation, targeting, winner selection, currency drop, non-fatal
  errors, resource-limit truncation, every Appendix E.5 response case.
- **`run_auction` reuse (§5.8)** — invoke `run_auction` **directly**
  (without going through any EdgeZero handler) to prove the trusted-server
  entry-point contract: same fixtures produce identical `BidResponse`.
- **Sync URL safety integration** (round-33 M6 + round-34 L7 + round-35
  M4 + round-36 M1 + round-51 L5 — emission is /cookie_sync-only per
  §8.6): syncer URLs go through the same rejection matrix as
  `validate_endpoint` — non-HTTPS, private-IP literal authority,
  `localhost` authority, authority-level macros, request-derived
  authority changes — exercised **end-to-end through `/cookie_sync`
  handler tests** (where syncer URLs are actually written into
  `bidder_status[].usersync.url`). **/setuid is NOT in this suite**
  because it does not emit syncer URLs — its tests cover callback
  parsing, cookie writes, and response format (Appendix B.5/B.6 /
  Appendix F /setuid rows) instead. The codegen-time
  template-validation counterpart stays in §11.1.
- **`BidderFanout` conformance suite** — the _native_ EdgeZero-backed
  `BidderFanout` impl must pass before it is production-eligible: the
  `AuctionDeadline` is honored (slow bidders → `Timeout`), per-bidder
  `timeout` is honored, partial failures are isolated, `max_response_bytes`
  rejects oversize responses, results are positionally aligned, and the
  **length invariant** `Ok(results).len() == calls.len()` holds (§5.2).
  Per-call `elapsed` accuracy under a `FakeClock` advanced across calls.
  Batch failure: `Err(FanoutInternalError::NoOutboundCapability)` maps to
  `AuctionError::FanoutUnavailable` → caller emits `500`. The interim
  impl (§5.2) is exempt and explicitly development-only.
- **`FanoutError` → `seatnonbid` mapping** — each variant produces the
  documented `ext.errors[bidder]` code + `seatnonbid` statuscode per §5.2
  and Appendix E.6: `Timeout` → 1 / 101, `Unreachable` → **999** / 103
  (not 5 — code 5 is adapter-side make_requests failure),
  `BadServerResponse` / `OversizeResponse` → 4 / 100, `Other` → 999 / 100.
- **Alias resolution at the HANDLER + request-level bidder-param
  merge at §5.1 step 7** (round-107 H6, matching the round-37 B1
  ownership §5.1/§7.6 record): request-defined aliases resolve via
  `resolve_aliases(&normalized, registry)` at handler entry and ride
  `AuctionPlan.aliases`; `run_auction` consumes `plan.aliases` at
  step 7, where the `ext.prebid.bidderparams` merging also happens
  (round-44 M2 — same step; the old "step 6" reference predated the
  round-43 H1 renumbering). Tests cover **alias validation failures**
  (unknown base, disabled base, no-op alias, unknown `aliasgvlids`
  entry, `aliasgvlids < 1`) producing `AuctionError::BadInput` → `400`
  with the documented `Invalid request: <err>` body (round-25 finding M4),
  and **disabled-bidder removal** producing a `BidderTemporarilyDisabled`
  (code 6) warning per-imp (round-25 finding M3). The observable identity
  contract (§5.9) — `seatbid.seat`, `ext.errors`, `ext.warnings`,
  `ext.responsetimemillis`, targeting suffix, `debug.httpcalls`,
  `seatnonbid.seat` — is all keyed by alias name; the base adapter's code
  runs the I/O; `aliasgvlids` is honored for the GDPR vendor lookup.
- **Determinism** — Appendix E.7: identical input + mocks + `FakeClock` +
  fixed `Rng` seed → byte-identical `BidResponse` across runs and targets
  (snapshot files).
- **User-sync flow** _(M5+ only — round-41 M5)_ — `/cookie_sync`
  candidate selection end-to-end (Appendix B.3 evaluate ladder);
  `/setuid` formats + cookie attributes (Appendix B.5–B.6). M2-M4
  builds do not expose sync routes (§7.8 / §14 H1).
- **Endpoint handlers** _(scope varies by milestone — round-41 M5 +
  round-68 M6)_ — M2 ships and tests `/openrtb2/auction`, `/status`
  (both `204` default + `200` configured branches per Appendix F /
  round-67 M3), `/info/bidders` (200 success + 400 bad-query bodies
  per Appendix F / round-68 M6 — pinned to the reference bidder-list handling exact-byte strings, no trailing newline, no
  `Content-Type` on the 400 path), and `/info/bidders/{name}` (200
  bidder-detail + 404 unknown + the `all` aggregate path) through
  the Axum dev server with realistic requests; assertions on
  status, headers, content type, and body shape (Appendix F).
  **M5+ adds the three sync routes** — `/cookie_sync`, `/setuid`,
  `/getuids` — to this list. M2 acceptance does NOT require the
  sync routes to be wired or to pass any tests (they don't yet
  exist in the build).
  **Intentional scope (round-36 L4):** handler-level integration coverage —
  including the _Sync URL safety integration_ bullet above — runs **only
  against the Axum target**. Cloudflare Workers, Fastly Compute, and Spin
  targets get **build-only verification via `cargo check`** (next bullet):
  they are not exercised end-to-end in CI. Cross-target _behavioral_ parity
  is established by routing the same `run_auction` logic through neutral
  library types (§2.1 / §5.8) — once the auction engine plus its handler
  pre/post-steps pass on Axum, the other targets inherit correctness from
  the shared library; only host-primitive seams (fan-out, clock, entropy)
  vary. M6 prepares the offline provider harness/artifact handoff/refusal
  tests and runbooks. Credentialed live-handler integration for
  Cloudflare/Spin/Fastly executes in M7.6 after M7.5; shared-library/native
  correctness is not provider lifecycle or selected-target pilot proof.
- **Route registration** _(scope varies by milestone — round-42 M1
  recount)_ — M2 registers and dispatches the four auction-shaped
  routes (`/openrtb2/auction`, `/status`, `/info/bidders`,
  `/info/bidders/{name}`); M3a adds `/_auctioneer/health/gvl` (Task
  4a) for five; M5 adds the three sync routes
  (`/cookie_sync`, `/setuid`, `/getuids`) and asserts all eight
  dispatch.
- **Multi-target build** — explicit package/feature matrix
  (round-61 L3 — earlier "cargo check on wasm32-unknown-unknown and
  wasm32-wasip1" was too vague to verify each adapter on its real
  target):
  - **Library crates** (`auctioneer-openrtb`, `auctioneer-config`,
    `auctioneer-runtime`, `auctioneer-privacy`, `auctioneer-bidder`,
    `auctioneer-exchange`): `cargo check -p <crate> --target
wasm32-unknown-unknown` AND `cargo check -p <crate> --target
wasm32-wasip1` — both targets must succeed since these crates
    are target-agnostic.
  - **`auctioneer-adapter-axum`**: `cargo check -p auctioneer-adapter-axum`
    on the host triple (native target; not a wasm build).
  - **`auctioneer-adapter-cloudflare`**: `cargo check -p
auctioneer-adapter-cloudflare --target wasm32-unknown-unknown`
    using the default features (which include `cloudflare`, the
    crate's only feature per its `Cargo.toml`; round-62 M1 —
    earlier "with the `worker` feature enabled" was wrong:
    `worker` is the wasm32-target dependency crate, NOT a Cargo
    feature. Equivalent explicit form: `--features cloudflare`).
    The `worker::*` API is pulled in automatically by the
    `cfg(target_arch = "wasm32")` `[target.'cfg(...)']`
    dependency block.
  - **`auctioneer-adapter-fastly`**: `cargo check -p
auctioneer-adapter-fastly --target wasm32-wasip1` (Compute@Edge
    WASI target).
  - **`auctioneer-adapter-spin`**: `cargo check -p auctioneer-adapter-spin
--target wasm32-wasip2` (Spin WASI target — round-155 #8: the
    pinned EdgeZero builds Spin on `wasm32-wasip2`; the M3b-4 target
    move updates this command, `edgezero.toml`, `spin.toml`, the CI
    matrix and the README together. Fastly stays on `wasm32-wasip1`).
  - **`auctioneer-core`**: `cargo check -p auctioneer-core` on all four
    adapter-relevant targets (host + wasm32-unknown-unknown +
    wasm32-wasip1 + wasm32-wasip2), since it carries the EdgeZero `app!`
    integration that each adapter wires.
    This is the **per-milestone** cross-target gate (round-67 M4 +
    round-68 H1 — milestone-scoped: M1 has no EdgeZero wiring per §14,
    so requiring adapter wrappers to compile in M1 is incompatible with
    the M1 deliverables). Three distinct gates apply:
  - **M1+ library `cargo check` matrix** — the six library crates
    (`auctioneer-openrtb`, `auctioneer-config`, `auctioneer-runtime`,
    `auctioneer-privacy`, `auctioneer-bidder`, `auctioneer-exchange`) on both
    wasm targets. Runs on every PR from M1 onward. The three M1
    crates (openrtb, config, runtime) ship in M1 (round-70 L11 —
    typo corrected, "two" → "three"); the rest are
    skeleton-only until M2 but the deny-list and target-portability
    checks still run.
  - **M1.5+ adapter wrapper + auctioneer-core `cargo check` matrix** —
    `auctioneer-adapter-{axum,cloudflare,fastly,spin}` on their target
    triples PLUS `auctioneer-core` on host + both wasm targets. Runs
    only after **M1.5 wrapper conformance** (round-67 H1) lands —
    the wrappers must align with the `Cargo.lock`-pinned EdgeZero
    `run_app` signatures (round-72 L1 — under `9e661ae` those take NO
    `manifest_src`: Cloudflare `run_app(req, env, ctx)`, Spin
    `run_app(req)`; the manifest is loaded internally), and Spin's
    `#[http_service]` `unsafe_code` lint conflict must be resolved.
    Including this gate in M1 was the earlier inconsistency.
  - **M2-production release-artifact gate** — the per-target
    `cargo build --release` matrix from §5.4 (Axum host triple,
    Cloudflare `wasm32-unknown-unknown`, Fastly `wasm32-wasip1`,
    Spin `wasm32-wasip2`), gated by the **triple-assertion `cargo
expand --release -p auctioneer-bidder` snapshot test** described
    in the next bullet (round-70 M8 + round-71 L10 — earlier
    "asserts no cfg blocks survive" was the weak form that just
    verifies Cargo behavior; the gate uses the strengthened
    function-name + literal absence + feature-on positive
    assertions). Runs on release tags only, becomes mandatory at
    M2 production-complete.
    Handler-level integration tests remain Axum-only by design
    (previous bullet). Each gate is enforced as a CI script — a
    `cargo check` or `cargo build --release` that misses any cell
    counts as a failed cross-target gate at the corresponding
    milestone.
- **Ingress hook contract tests — milestone-scoped** _(round-65
  H1+H2 + round-66 L5 + round-68 H2 — split because /cookie_sync
  doesn't exist as a wired route until M5 per §14)_. Once EdgeZero
  ships the per-route ingress policy hook (open item 14), an
  Axum-only end-to-end suite asserts:
  - **Ingress E2E subset — auction + default routes (owner: M3b Task
    M3b-10; the auction/default-route BEHAVIOR was identified during M2
    design, but these ingress-hook tests land with M3b-10, round-80 M2)**:
    - **Content-Encoding rejection** (round-66 M2 + round-69 L4):
      non-empty `Content-Encoding` on `/openrtb2/auction` → 400
      with the exact body `Invalid request: Content-Encoding of
type <encoding> is not supported\n` (trailing `\n`
      canonical per Appendix F + round-68 M5; matches upstream
      reference auction handling / plain-text error output); no body-read metrics
      recorded.
    - **Content-Length over cap** on `/openrtb2/auction` → 400
      with no body-read metrics (cheap path; round-64 H1).
    - **Missing Content-Length + body > cap** on
      `/openrtb2/auction` → 400 with bounded memory growth
      (bounded-read path).
    - **Chunked + body > cap** on `/openrtb2/auction` → 400 (no
      way to pre-check via header; bounded read trips at cap).
    - **Unmatched path + body > 4 KiB** → 400 with the default
      body `request size exceeded max size of 4096 bytes.\n`
      BEFORE the eventual 404 (round-67 H2 + round-69 H1 —
      `NotFound { cap_bytes: 4096 }` arm); a follow-up assertion
      with `Content-Length` under the cap returns 404 empty body.
    - **Wrong-method on a known path** (e.g. `GET
/openrtb2/auction`, `POST /info/bidders`) → 400 with the
      same default body when body > 4 KiB (round-69 H1 —
      `MethodNotAllowed { cap_bytes: 4096 }` arm); otherwise 405
      empty body. The 256 KiB auction cap does **not** apply to
      a `GET` request against `/openrtb2/auction`.
    - **Content-Encoding precedence on unmatched/wrong-method**
      (round-69 M3): `POST /missing` with `Content-Encoding:
gzip` → 400 with the Content-Encoding body, NOT 404.
      `GET /openrtb2/auction` with `Content-Encoding: gzip` →
      400 with the Content-Encoding body, NOT 405. Asserts the
      hook's "encoding rejection before route-mapping" order.
    - **`POST /cookie_sync` in an M2-M4 build** → 400 (NotFound
      with 4 KiB cap) when body > 4 KiB, OR 404 empty body
      otherwise; **NOT** the 32 KiB `/cookie_sync` cap or 200
      handler response (round-69 H1 — the route isn't wired
      until M5, so `match_route` returns `NotFound`). The 32 KiB
      cap only becomes reachable in M5+ per the M5+ subset
      below.
    - All assertions verify against the values
      `auctioneer-runtime::body_caps::match_route` returns for the
      auction route (`Matched { cap_bytes: 262144 }`), the
      4 KiB default (`MethodNotAllowed` / `NotFound`), and
      milestone-scope arms (round-69 H1).
  - **M5+ subset — `/cookie_sync` ingress**:
    - **Content-Encoding rejection** on `/cookie_sync` → 400 with
      the same exact body as the auction case (round-68 M3 —
      intentional divergence row added to §15 because upstream
      reference cookie-sync handling does NOT reject inbound compression; auctioneer
      unifies the cap+encoding rule across all body routes for
      defense-in-depth).
    - **Content-Length over cap** on `/cookie_sync` → 400 (cap =
      32 KiB per §5.3).
    - **Missing Content-Length + body > 32 KiB** on `/cookie_sync`
      → 400 (bounded read).
    - **Chunked + body > 32 KiB** on `/cookie_sync` → 400.
      These ride in with /cookie_sync handler wiring per §14 M5 —
      M2 acceptance does NOT require them.
  - Cloudflare/Fastly/Spin have build-only evidence in this historical
    acceptance scope. Retain all current offline per-adapter ingress and
    conformance checks in M6; real deployed-handler ingress execution is
    deferred to M7.6, not proved by compilation or offline contracts.
- **`insecure-bidders` feature-gate snapshot test** *(round-65 M4
  - round-66 L5 + round-70 M8 — strengthened from "no cfg blocks
    survive" (weak — that's just Cargo's natural behavior) to a
    triple-assertion form that proves the feature gate actually
    works both ways)*:
  * **(1) Default expansion** — `cargo expand --release -p
auctioneer-bidder` against the committed snapshot asserts: no
    function whose name matches the insecure path
    (`allow_insecure_scheme`, `validate_insecure_endpoint`,
    etc.); no literal string matching `http://127.0.0.1`,
    `http://[::1]`, or `http://localhost` (round-72 M4 — IPv6
    loopback added; matches the runtime allowlist).
  * **(2) `--features insecure-bidders` expansion** — same
    crate, with the feature on, MUST contain those function
    names and at least one allowed-localhost literal (proves
    the gated path is not dead code).
  * **(3) Default test suite** — `cargo test -p auctioneer-bidder`
    (no feature) passes; guards against removing a feature
    breaking non-feature code paths.
    Snapshots for (1) and (2) are committed; any unexpected diff
    fails CI. Paired with the round-65 M4 per-target
    release-build matrix above (Axum host triple, Cloudflare
    wasm32-unknown-unknown, Fastly wasm32-wasip1, Spin wasm32-wasip2) so
    insecure literals never reach a release artifact.
- **Dependency purity** — `cargo metadata`-driven deny-list test asserting
  that no library crate (`auctioneer-openrtb`, `auctioneer-config`,
  `auctioneer-runtime`, `auctioneer-privacy`, `auctioneer-bidder`, `auctioneer-exchange`)
  transitively depends on `edgezero-*`, `tokio`, `reqwest`, `fastly`,
  `worker`, or `spin-sdk`. The only crates allowed those dependencies are
  `auctioneer-core` and the four `auctioneer-adapter-*` crates — making the
  "library crates are pure" principle (§1.2) CI-enforced, not prose-promised.
- **End-to-end against Mocktioneer** — real auctions against the local
  Mocktioneer bidder (deterministic pricing), asserted against snapshot
  `BidResponse` files. Exercised through **both** call sites: (a) the
  EdgeZero `/openrtb2/auction` handler, and (b) a direct
  `run_auction` invocation, proving trusted-server reuse (§5.8). Offline CI
  uses a canned `BidderFanout`; an optional integration variant hits live
  Mocktioneer.
- **Baseline-parity edge cases (round-15 finding L7)** — explicit cases:
  - GPP **SID present, parsed-section absent** (e.g. `gpp_sid=[2]` with no
    sec 2 in `gpp`): GDPR still in scope; only the consent string falls
    back to `user.consent` (Appendix D.2).
  - **Fast-vs-slow bidder timing** under one global batch: a fast
    bidder's `responsetimemillis` reflects its own max-call elapsed + its
    own adapter CPU, **not** the slow neighbour's batch wall time (§5.2).
  - **`/getuids` host-cookie merge**: the response `buyeruids` includes
    the host-cookie value when no entry exists under the host family key.
  - **`/setuid` 451 body** — activity-denied → empty body; GDPR
    host-cookie blocked → the pinned `gdpr_consent string prevents…`
    message (Appendix F).
  - **`/setuid` activity-vs-permission-path TCF ordering** (round-58
    M2): an end-to-end fixture with **`syncUser` denied AND
    `host_vendor_id != 0` AND `normalized_request_signal == Yes` AND
    `gdpr_permissions_enabled` AND malformed non-empty
    `gdpr_consent`** asserts the response is **`451` empty body**,
    NOT the permission-path `400 gdpr_consent was invalid.…`.
    Confirms the activity gate (§8.3 step 10) short-circuits BEFORE
    the host-cookie permission path (§8.3 step 11), matching the
    §15 "/setuid request-extraction-error vs activity ordering" row.
    Pairs with the activity-allowed-400 fixture in §11.1 so the
    ordering regression is covered both ways.
  - **`SkipWhen.gpp_sid` exact-string** — a configured `["2"]` does
    **not** skip a request whose raw `gpp_sid` is `"2,6"`.
  - **Debug signal/gate divergence** — no query/header/body signal bypasses
    account or compiled base-bidder permissions. Pin OR, boolean/integer,
    duplicate/malformed signal, alias, early-return, redaction, resource,
    and exact wire behavior using M6 sections 3-6.
  - **Cookie opt-out precedence** — after a valid runtime snapshot and
    bounded request extraction, opted-out + disabled account, invalid
    filterSettings, or empty required consent returns **401**. An invalid
    runtime blob instead fails **500 before application opt-out**, even if
    the invalid field is inside a different account. This replaces the
    unreachable separately malformed-account fixture.
  - **Auction error precedence** — M6 section 7's current pipeline matrix:
    invalid runtime blob → config 500; transient store failure → 503 with
    Retry-After; valid config + invalid JSON/node overflow → request 400
    before account discovery; within-budget body + disabled account →
    account 503 before typed preparation; clean account + invalid request →
    request 400. Include competing faults and prove later stages do not run.
    Earlier EdgeZero ingress/admission refusals keep their own precedence.
  - **Typed host overlay** — a nested GDPR override preserves sibling
    host settings; omitted/null optional overlays inherit, arrays replace
    whole, and invalid effective values fail whole-blob loader validation.
    No RFC 7396 deletion cases or separate account fetch are implied.
  - **Residual inventory and updates** — the M6 design section 9 identifies
    existing M4/M5 regressions versus new work. Section 8 pins snapshot and
    update/failure/recovery evidence, separately from provider cache freshness.

Observability: lightweight structured logging via EdgeZero middleware; the
full metrics engine is a later phase.

---

## 12. Open Questions & To Revisit Later

| #   | Item                                     | Why deferred / what to revisit                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| --- | ---------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | **EdgeZero fan-out primitive**           | Proposed API in §5.2; shape not final. **M2 production-complete prerequisite** (§14); M2 _development_ uses the interim `ProxyClient` + `join_all` impl. Bind the production `BidderFanout` impl when published.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| 2   | **tmax enforcement**                     | Real whole-auction deadline via the fan-out `.deadline()` (§5.2). Confirm when the EdgeZero API is final; interim impl is advisory only.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| 3   | **Currency**                             | MVP assumes USD; non-USD seatbids dropped. Revisit at the currency-conversion phase.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| 4   | **GPP sections 3, 7–12**                 | Passed through, not decoded (matches upstream v4). Revisit when upstream adds enforcement.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| 5   | **GVL snapshot staleness**               | Embedded at build time. Revisit: runtime refresh via `config_store`/`key_value_store`, or scheduled re-vendoring.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| 6   | **Bidder params JSON-schema validation** | Typed serde structs for MVP. Revisit: embedding `static/bidder-params/*.json`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| 7   | **Account-required gate**                | Phase 1 falls back to the default account. Revisit at the stored-requests phase.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| 8   | **`config_store` caching**               | §9.1 defines resolution; document `config_store`'s own caching/hot-reload once its semantics are finalized.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| 9   | **Reference adapter picks**              | AppNexus/Pubmatic swappable if a better transform exemplar surfaces.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| 10  | **Resource-limit defaults**              | §5.3 numbers advisory; revisit after load testing.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| 11  | **`reportAnalytics` / inert activities** | `reportAnalytics` plumbed but inert until the analytics phase; `enrichUfpd` / `transmitUniqueRequestIds` inert as in upstream v4.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| 12  | **Geo-aware vs host default mismatch**   | upstream uses the geo-aware default only for `EnforceGDPR` and the raw host default inside permissions normalization. auctioneer replicates this.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| 13  | **EdgeZero clock & entropy primitives**  | `Clock` / `Rng` (§5.7) are assumed EdgeZero primitives, like fan-out. **M2 production-complete prerequisite** (§14); M2 _development_ uses the dev `Clock` / `Rng` impls (top-of-spec). Confirm APIs when published.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| 14  | **EdgeZero ingress policy hook**         | **SUPERSEDED (round-143 #7): the `Decision` / `FramingState` / `BoundedReadPolicy` shape in this row is HISTORY — the LANDED contract at `120dd2aa` is EdgeZero's `RouteResolution` / `IngressHead` / `AdmissionDecision` / `IngressGrant` with STRING route classes, wired pre-body in all four adapters (M3b plan Prereq 10 + Task M3b-10 own the live text; only raw-ingress head limits / framing validation remain Unsupported).** Historical row: A per-route ingress policy hook on EdgeZero's `App` that runs **before** `into_core_request` materializes the body (round-65 H2 + round-66 M2 — also covers Content-Encoding rejection, not body caps alone). Required because every current EdgeZero adapter reads the body unconditionally — Axum `to_bytes(_, usize::MAX)`, Cloudflare `req.bytes()`, Fastly `read_to_end`, Spin `req.into_body().await` (round-72 L6 — Spin added to the list; matches §5.1's body-materialization paragraph) — so auctioneer's pre-body-read rejections (non-empty Content-Encoding per §5.1, per-route 400 caps 256 KiB / 32 KiB / 4 KiB) are otherwise enforced AFTER allocation. Shape (round-72 B2 — the round-70 `Continue { bounded_read: Option<usize> }` result was insufficient: the adapter's bounded read must know the route-specific 400 body when it crosses the cap, and a chunked/lengthless request on the wrong-method/unmatched arms must return 405/404 AFTER a within-cap read — neither expressible by a bare `Option<usize>`): the EdgeZero-owned callback type names NO auctioneer type — `Fn(method: &Method, path: &str, headers: &HeaderMap, framing: &FramingState, resolution: &RouteResolution) -> Decision` (round-77 H1 — `RouteTable` lives in `auctioneer-runtime`, so it can NOT appear in the type EdgeZero accepts; round-134 #6 — the fifth parameter is EdgeZero's own `RouteResolution { Matched(RouteId) | MethodNotAllowed { allowed } | NotFound }`: a bare `Option<RouteId>`conflated 404 with 405 and forced a re-match, and`allowed`is what the 405 arm names in the`Allow`header; the four-argument form is RETIRED); auctioneer's implementation`fn decide(method: &Method, path: &str, headers: &HeaderMap, framing: &FramingState, resolution: &RouteResolution, routes: &RegisteredRoutes) -> Decision`resolves`Matched`through the boot-built`RouteId → RouteMatch` map (`RegisteredRoutes`is auctioneer-core-owned, adapter-CONSTRUCTED from the zip of edgezero.toml handler wiring and RouteTable policy rows — round-134 #1) instead of re-matching, and the adapter bridges the two with a closure that CAPTURES the`RegisteredRoutes`and calls`decide`. `Decision = Reject(Rejection) \| Dispatch \| BoundedRead(BoundedReadPolicy)`, `Rejection = { status: u16, body: BoundedRejectionBody, allow: Option<SmallVec<[Method; 4]>> }`(round-130 #7 replacement edit — the 256-byte static-class render buffer, the round-72`String`shape corrected IN PLACE; round-136 #5 —`allow`is the typed 405 carrier: the runtime Allow set comes from`RouteResolution::MethodNotAllowed`and EdgeZero renders the header, since a 'static headers list could never name runtime methods),`BoundedReadPolicy = { max_bytes: usize, on_exceeded: Rejection, on_success: OnSuccess }`, and `OnSuccess = Dispatch \| Reject(Rejection)`. So a bounded read that crosses `max_bytes`yields the exact route-specific 400`on_exceeded`, and a within-cap read yields `on_success`(Dispatch for Matched, or the 405/404 Rejection for MethodNotAllowed/NotFound). The hook consumes the registration-issued`RouteResolution`(round-134 #6 —`body_caps::match_route`survives only as the M3a/dev-path fallback), and owns Content-Encoding rejection, Content-Length pre-check, and the bounded-read policy for missing Content-Length / chunked. Hook contract covers: Content-Encoding rejection AND **malformed/ambiguous framing rejection (round-73 H2)** AND Content-Length pre-check AND bounded-read fallback AND unmatched-path/wrong-method 404/405 with the 4 KiB default cap applied first (round-67 H2 — closes the "POST huge body to /missing" gap that exists in current EdgeZero's`RouterService`ordering). **Framing validation is fail-closed, in TWO layers (round-73 H2 / round-74 H1 / round-75 B2).** The hook receives a NORMALIZED`FramingState { Length(u64) \| Chunked \| None \| Malformed }`from the adapter (which parses`Content-Length`into`u64`— portable on 32-bit`wasm32`). **Layer A — a RAW-boundary guarantee EdgeZero must provide (round-75 B2):** a `FramingState`derived from the PARSED request is INSUFFICIENT for anti-smuggling, because the platform parser resolves rather than rejects the dangerous shapes — the pinned`hyper-1.10.1`accepts`Content-Length`+`Transfer-Encoding`as chunked and SUPPRESSES the`Content-Length`before building the`HeaderMap`, so the CL+TE ambiguity reaches the hook as plain `Chunked`. EdgeZero must therefore guarantee, at the raw parser boundary, STRICT REJECTION of CL+TE / conflicting-or-duplicate `Content-Length` / non-`chunked`or repeated`Transfer-Encoding`/ illegal chunked framing with a fail-closed 400 **before the ingress hook is invoked** (the default accept-as-chunked is unacceptable; round-77 B2/B3 — the earlier "raw framing provenance handed to the hook" alternative is removed, because a normalized`HeaderMap`cannot re-derive the raw framing shapes and auctioneer must not re-implement the parser). Because EdgeZero rejects these before calling the hook, these failures carry the ADAPTER's own body — the exact auctioneer framing body does NOT apply (assert 400 status only), and there is no rejection-body-ownership question. **Layer B (at the hook):**`FramingState::Malformed`→ 400 with the single canonical`Invalid request: malformed or ambiguous request framing\n`body — asserted via a SYNTHETIC contract test, since a strict parser (Hyper) rejects malformed/overflow single`Content-Length`before the app (round-75 M1). **Per-target raw-framing scope (round-78 H1):** the hook API + normalized`FramingState`is a GLOBAL prerequisite, but strict raw-boundary rejection is a PER-TARGET conformance capability (a managed wasm runtime may not permit replacing its raw parser) — Axum's is the M3b-10/Axum production gate, and Cloudflare/Fastly/Spin's is each target's M6 gate. **Milestone ownership (round-78 M2 — resolves the dual-milestone contradiction with §13's "M3b-10 owns it"):** this hook requirement was IDENTIFIED during the M2 design pass, but its current implementation/release owner is **M3b Task M3b-10** (production-completion), NOT M2 — the historical "M2 production-complete prerequisite" phrasing is retired. Behavior-complete (M3b-1..9) does not depend on it; interim development relies on platform-runtime limits + the auctioneer-core embedded read (round-64 H1's three-layer scheme). **Round-128 #7 — M3b consolidation:** the M3b plan's Prereq 10 (the consolidated ingress seam) SUPERSEDES parts of this shape: admission is a SEPARATE pre-decide callback whose grant crosses the boundary in the EdgeZero-owned move-safe`IngressGrant` container (`ctx.take_ingress_grant()`, exactly-once); ingress captures `request_start`before any body byte; parser-level header caps (h1 buffer bound + h2`http2_max_header_list_size`) reject before `decide`can run; the bounded body read runs under the ingress lease and the boot-owned`max_ingress_read_ms`(expiry cancels the read and answers 408`request timeout\n`); (`Rejection.body`'s bounded render is now stated inline in the shape above). `Decision` and its table above are otherwise unchanged — the admission callback precedes it and no Decision variant carries a guard. |
| 15  | **upstream YAML → TOML conversion tool** | auctioneer's baked host/account/bidder config is **TOML** (unified with the runtime `auctioneer.toml` override layer per §9.1 — none of it is OpenRTB, so the wire stays JSON while config follows the Rust-native convention). upstream reference server ships bidder configs as **YAML**, so porting a new adapter's config from upstream currently means a hand-conversion. A one-shot operator-facing `yaml→toml` conversion CLI is **deferred to a future milestone**; auctioneer's own baked configs are already TOML, so nothing in M2/M3 depends on it.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |

---

## 13. Scope Summary

The [product vision](../../../vision.md) records the OpenRTB-first direction
and standalone/in-binary Trusted Server delivery goals. It does not change
this specification's initial protocol scope or imply that the Trusted Server
integration is implemented; §5.8 remains the shared auction-engine contract.

**In scope:** `/openrtb2/auction` (**OpenRTB 2.6 only**, §3); bidder-adapter
framework + 4 reference adapters; `/status`, `/info/bidders`,
`/info/bidders/{name}`; full-fidelity GDPR/TCF v2 + CCPA + GPP (sections 2
& 6) + activity controls + scrubbing + LMT/COPPA; `/cookie_sync` + `/setuid`

- `/getuids` behavior-compatible with the pinned baseline; build-time config
  generation; config resolution semantics; outbound + syncer-URL safety;
  bounded bodies; debug redaction; price granularity & targeting;
  deterministic output; **auction exposed as a reusable library function
  (`auctioneer_exchange::run_auction`, §5.8) over a shared, type-enforced
  `NormalizedBidRequest` (§3.2), usable from EdgeZero handlers and
  trusted-server callers without modification**; the four
  `auctioneer-adapter-*` crates **unchanged once EdgeZero ships the required
  primitives** (fan-out, clock, entropy, **ingress body-cap hook** — round-21 finding M4 + round-65 H2). Round-150 #8:
  the fan-out and clock primitives are landed at `120dd2aa`; Fastly runs
  the production `EdgeZeroFanout` with dispatch-before-harvest and
  BestEffort deadlines (§5.2), not an interim impl.

**Assumes** EdgeZero ships the fan-out, clock, entropy, **and ingress
policy hook** primitives (round-66 M3 — ingress hook added to the
executive summary so the prereq list is complete) — these are
**production-complete prerequisites** (round-35 finding L5 + round-65 H2

- round-66 M2/M3 — ingress hook covers Content-Encoding rejection and
  per-route body caps; see §12 open item 14 for the full hook contract).
  **The ingress policy hook specifically was IDENTIFIED during M2 design,
  but its implementation/release owner is M3b Task M3b-10 (round-79 M2);
  M3b-1..9 remain behavior-complete without it.**
  M2 _development_ on Axum, Cloudflare, Fastly (serialized fan-out per round-72 L5), and Spin proceeds without them via
  the dev-only impls in `auctioneer-core` + `auctioneer-adapter-cloudflare`
  (top-of-spec execution-status note); only behavior-complete release is
  blocked.

**Deferred beyond the initial M1-M6 scope:** stored requests; AMP/video
endpoints and complete native workflows;
price floors; bid adjustments; currency conversion; creative caching;
hooks/modules; analytics; full metrics engine; GPP sections 3 & 7–12
enforcement; the full bidder catalog beyond the reference adapters; **Privacy Sandbox**
(round-21 finding H2): `Sec-Browsing-Topics` enrichment (upstream
reference auction handling reads it under account `PrivacySandbox.TopicsDomain`),
`/cookie_sync` `receive-cookie-deprecation` partitioned cookie (upstream
reference cookie-sync handling under account `privacy.privacysandbox.cookiedeprecation`),
and the `account.privacy.privacysandbox` config surface;
**`ext.prebid.multibid` bid-limiting** (round-23 M5; upstream validates,
rebuilds, and clamps `maxBids` in reference multi-bid limits / reference auction handling); **DSA
mutation/validation and SChain merge/validation** (round-23 M6); **inbound
request gzip** (round-23 L8 / round-24 M6: rejected with `400`);
**request-level FPD subsystem** (round-24 H2: `ext.prebid.data`,
`data.eidpermissions`, `ext.prebid.bidderconfig` — typed pass-through
only; the per-imp `imp.ext.prebid.imp` merge of §5.1 step 7 is the only
FPD path that takes effect). All listed as intentional divergences in
§15 until a later phase opens them.

These deferrals now have explicit milestone ownership in the
[approved roadmap](../../roadmap.md#2-milestone-ownership), as amended by the
[2026-10-26 scope correction](../../roadmap.md#2026-10-26-scope-correction):
M7 delivers foundations and all 14 priority selected flows. The
[AG-approved 2026-10-30 native amendment](2026-10-30-m7-controlled-native-completion-design.md)
now permits controlled-native delivery with enforced schema/input/work limits,
frozen tested reservations, allocation/stress acceptance and actual generated-App
onboarding. Complete worst-case schema resource certification belongs to M7-R0;
guest deployment readiness remains blocked. This supersedes the static
proof-before-activation requirement below, not privacy, source correctness,
workload, deployment ceilings or the additive accounting equation. The earlier
[2026-10-29 qualification amendment](2026-10-29-m7-shipped-schema-qualification-design.md)
scopes serving/resource acceptance to their complete shipped closure, including
existing bindings and shared helpers, rather than speculative resource proofs
for every unshipped source schema. Required shipped-path bounds remain;
measurements are not worst-case proofs. M7.7 owns newly reachable schema/engine
surfaces before the corresponding expansion wave and M20 owns residual universal
catalog closure. Existing source-wide correctness/provenance controls, M7-R0
fit requirements and M7.6 deployment acceptance remain unchanged. The
[2026-10-27 correction](../../roadmap.md#2026-10-27-post-pilot-bidder-expansion)
adds M7.7 bidder expansion after M7.5 direct Trusted Server embedding and
selected M7.6 publisher testing; M20 retains residual migration and full-catalog
closure. M8-M11 prioritize the shared foundations, creative
caching, and end-to-end video support; M12-M19 complete the remaining shared
parity surfaces. Scheduling a feature does not make it implemented or remove
its current intentional divergence.

---

## 14. Implementation Milestones

The implementation plan breaks this spec into ordered milestones so
compliance-sensitive behavior never lands before the auction core.

**Prerequisite for M2 onward (review finding 1 + round-61 M1 + round-66
M3 — full dev-primitive set, not just fan-out; ingress hook listed as a
production-complete prereq even though M2 dev can run without it).**
M2's fan-out step depends on EdgeZero's fan-out primitive; M2+ depend
on the clock and entropy primitives (§5.7); the **EdgeZero ingress
policy hook** (§12 open item 14 / round-65 H2 / round-66 M2 — covers
Content-Encoding rejection AND
per-route body caps before the body is materialized). M1 has no such
dependency and starts immediately. If the native primitives are not
yet published when M2 begins, **M2 development is unblocked only when
ALL four fallback host-primitives are in place** (matches the
top-of-spec execution-status note); the ingress hook is
production-complete-only, not M2 dev-blocking — **it was identified in
M2 design, but its implementation/release owner is M3b Task M3b-10
(round-79 M2), and M3b-1..9 stay behavior-complete without it** —
because the §5.1 three-layer scheme (platform limits + auctioneer-core
embedded read) covers the development window:

1. **Interim `BidderFanout`** — `ProxyClient` + `join_all` (§5.2), with
   the known timeout/deadline gaps: does NOT honor `AuctionDeadline` or
   per-bidder timeouts. Under this interim, **Fastly fan-out runs
   serialized** (no concurrent Compute outbound primitive yet).
2. **Dev native `Clock`** for Axum / wasi targets — wall via
   `std::time::SystemTime::now()`, monotonic via
   `std::time::Instant::now()`. Lives in `auctioneer-core`.
3. **Cloudflare `Clock` shim** — `worker::Date::now()` for wall;
   monotonic derived from a wall-clock delta (NOT strictly monotonic;
   sufficient for M2 dev). Lives in `auctioneer-adapter-cloudflare`
   because it depends on `worker::*` — see §1.2 adapter-shim
   exception.
4. **Dev `Rng`** — **target-specific** (round-61 M1 — earlier "seeded
   from std::time entropy" was Cloudflare-incompatible):
   - Axum / wasi targets: seeded from `std::time::SystemTime::now()`
     nanoseconds (in `auctioneer-core`).
   - Cloudflare Workers: seeded from `worker::Date::now()` milliseconds
     XOR'd with `Math.random()` via `worker::js::Math::random()` (in
     `auctioneer-adapter-cloudflare`; the worker target has no
     `std::time::SystemTime`). Same `xorshift64*` algorithm as the
     test `SeededRng`, just with a runtime-supplied seed.
   - Fastly Compute: seeded from
     `std::time::SystemTime::now().duration_since(UNIX_EPOCH)` (round-73
     L5 — pinned; the `fastly` SDK exposes `std::time::SystemTime` on
     Compute@Edge via its WASI runtime, so the same source as Spin
     works; `fastly::time::Instant::now()` was the round-61 M1 guess
     but is not the actual API — the SDK uses `std::time` directly).
     The seed nanos feed the same `xorshift64*` algorithm as the
     other dev-Rng impls. Fastly is not production-ready under the
     interim regardless (serialized fan-out per §5.2 + round-72 L5).
   - Spin: seeded from `std::time::SystemTime::now()` like the wasi
     case.

With all four in place, **all of Axum, Cloudflare, Fastly (serialized), and Spin actually
run M2 in development**. Fastly is not production-ready, and no target
is behavior-complete, until the native primitives land and the
`BidderFanout` conformance suite (§11) passes — Fastly fan-out remains
serialized, the Cloudflare `Clock` shim remains wall-clock-derived (not
truly monotonic), and the dev `Rng` instances are runtime-seeded
(non-deterministic without test injection) until EdgeZero ships the
native `Rng` primitive.

**Adapter wrapper conformance is a separate M2-dev prerequisite**
(round-67 H1 — top-of-spec execution-status note already added; this
paragraph cross-references for §14 readers). Before any of the four
dev primitives ship, each `auctioneer-adapter-*/src/lib.rs` (or `main.rs`)
must call `edgezero_adapter_<target>::run_app` with the signature the
**`Cargo.lock`-pinned** EdgeZero exports. Under the current pin — `docs/outbound-http-spec` @ `12965da2` (round-138 #6; these
signatures were verified at `9e661ae` and are unchanged there,
where the workspace compiles against them — history: `56796415`,
`c8d6e414`,
`9e661ae`, before that `2277072`) — Cloudflare's
`run_app(req, env, ctx)` and Spin's `run_app(req)` load the manifest
**internally** (via the `#[app]` proc-macro plumbing) and take **no**
`manifest_src` argument — the auctioneer wrappers already call them with
exactly those signatures and `cargo check --target wasm32-...`
succeeds on both crates (**round-65 M2 — the pre-`2277072` API took
`manifest_src: &str` first; that requirement was carried stale into
earlier drafts of this note and would REGRESS the working wrappers if
followed, so it is removed**). The Spin wrapper's only real adaptation
is already in place: a file-scoped `#![cfg_attr(target_arch =
"wasm32", allow(unsafe_code, reason = ...))]` reconciling the
workspace `unsafe_code = "deny"` lint with the **`#[http_service]`**
macro's expanded `unsafe extern "C"` component export (earlier drafts
said `#[http_component]` — also corrected). M1 acceptance does not
require any wrapper fix (M1 has no EdgeZero wiring); M2 acceptance is
already met on all four adapters:

- M2 Axum and M2 Fastly development proceeds once the four dev
  primitives are in place (wrappers build unchanged).
- M2 Cloudflare and M2 Spin wrappers ALSO build unchanged under the
  `9e661ae` pin — the M1.5 "adapter wrapper conformance" transition
  below is, under this pin, a no-op for the `run_app` call shape (the
  Spin `allow(unsafe_code)` scope is the only surviving deliverable,
  and it has landed).

1. **M1 — Foundations.** Deliverables, named explicitly (round-66 M4 —
   added a bulletable enumeration so each item is unmissable in M1
   acceptance):
   - `auctioneer-openrtb` types + `prepare(req, headers)` (the M1 shape;
     M3b-2 adds the third `&PrepareLimits` argument) + `validate()`.
   - `auctioneer-config` + `build.rs` codegen for 4 reference bidders.
   - **`auctioneer-runtime`**: `Clock`, `Rng` traits + `FakeClock` /
     seeded `Rng` for tests, **AND `body_caps::match_route(method,
path, &RouteTable) -> RouteMatch` accessor** (round-65 M5 +
     round-66 M4 + round-71 M4 — return type corrected from stale
     `Option<usize>` to the round-69 H1 enum; `RouteMatch = Matched
{ route_id, cap_bytes } | MethodNotAllowed { cap_bytes } |
NotFound { cap_bytes }`). Made an explicit M1 bullet so the
     round-65 H2 ingress hook can wire to it on day one.
   - No EdgeZero wiring.

   Detailed scope: `auctioneer-openrtb` types + **`prepare(req,
headers)` + `validate()`** (round-32 finding M2 — `prepare()` is
   the sole construction gate for `NormalizedBidRequest` and is
   required by every M2 call site, so it ships in M1 alongside the
   types it produces: implicit-field fill, 2.6 privacy rejection,
   channel validation, `at == 0` defaulting, Sec-GPC reconciliation
   — **no consent-syntax parsing here**, round-37 H3 / §3.2 step 6
   — those warnings ship in M3/M4 inside `PrivacyContext::resolve`
   per §7.7); `auctioneer-config` + `build.rs` codegen for the 4
   reference bidders; `auctioneer-runtime` (`Clock`, `Rng` traits +
   `FakeClock` / seeded `Rng` for tests; **`body_caps::match_route(
method, path, &RouteTable) -> RouteMatch`** method + route-match
   - milestone-scoped cap accessor (round-65 M5 + round-69 H1 +
     round-70 M2 — corrects stale `for_path` reference) — exported in
     M1 so the EdgeZero ingress hook (open item 14) can read it once
     published; the cap table encodes §5.3's `MAX_AUCTION_BODY` =
     256 KiB, `MAX_COOKIESYNC_BODY` = 32 KiB, default 4 KiB; the
     `RouteTable` parameter lets callers scope to the milestone's
     active route set per round-69 H1). No EdgeZero wiring.
     **M1.5 — Adapter wrapper conformance.** A small mechanical milestone
     (round-67 H1 + round-69 M2 — formalized so the M1→M2 transition has
     a named place to live; earlier rounds defined the work as a "prereq
     note" but §14 still jumped straight from M1 to M2). Deliverables,
     each a no-behavior-change wrapper fix:

- **`auctioneer-adapter-cloudflare`**: **round-65 M2 — NO `manifest_src`
  change.** Under the `Cargo.lock`-pinned EdgeZero SHA `9e661ae`
  (authoritative pin at §2; earlier drafts said `2277072`, superseded
  round-19), `edgezero_adapter_cloudflare::run_app::<auctioneer_core::App>
(req, env, ctx)` loads the manifest internally and takes NO
  `manifest_src` argument. The wrapper already calls it with that
  three-argument shape and `cargo check -p auctioneer-adapter-cloudflare
--target wasm32-unknown-unknown` succeeds as-is — do NOT add a
  `manifest_src` first argument or an `include_str!(".../edgezero.toml")`
  (both belonged to the pre-`2277072` API and would fail to compile
  against the current pin).
- **`auctioneer-adapter-spin`**: **round-65 M2 — NO `manifest_src`
  change.** `edgezero_adapter_spin::run_app::<auctioneer_core::App>(req)`
  likewise takes only the request. The one real deliverable — already
  landed — is the file-scoped `#![cfg_attr(target_arch = "wasm32",
allow(unsafe_code, reason = ...))]` that reconciles the workspace
  `unsafe_code = "deny"` lint with the **`#[http_service]`** macro's
  expanded `unsafe extern "C"` export (earlier drafts said
  `#[http_component]`). Acceptance: `cargo check -p auctioneer-adapter-spin
--target wasm32-wasip2` succeeds.
- **No-op for Axum and Fastly**: their wrappers already build today
  per round-67 verification; no work needed.

Gate: M1.5 ships before M2 begins. Once M1.5 is green, the §11.2
M1.5+ adapter-wrapper + `auctioneer-core` `cargo check` matrix
activates per round-68 H1. M1.5 has no EdgeZero primitive
dependency (the wrappers just need to call the correct EdgeZero
signature); the four dev primitives remain M2 work, and the ingress
hook — identified in M2 design — has its implementation/release owner
in **M3b Task M3b-10** (round-79 M2; M3b-1..9 behavior-complete without
it).

2. **M2 — Auction core, no privacy logic (privacy stub in place).** A
   `PrivacyContext::allow_all_no_scrub()` constructor lives in
   `auctioneer-privacy` from M2 onward (round-34 finding H1 — `run_auction`
   always executes §5.1 steps 6 and 8, so milestone-gating the _steps_
   would diverge from the engine contract; instead, the _context_ is a
   permissive stub that grants all activities, allows every bidder
   through, and performs no scrubbing). M2-M4's **historical sync
   privacy stub** existed only to keep dependent crates compiling;
   no sync routes or `uids` cookie lifecycle were exposed. M5 replaces
   that stub with typed normalization and endpoint-specific permissions
   (§7.8), not compatibility-preserving allow-all callbacks. M3 swaps in real **GDPR** (and starts emitting TCF v2
   syntax warnings via `take_warnings()` per §7.7); M4 swaps in **CCPA,
   GPP, activity controls, scrubbing, and the corresponding US-Privacy
   and GPP syntax warnings**. **M5 supplies the full
   `resolve_sync_privacy`** — real GDPR vendor decisions, exact raw-SID
   `SkipWhen` matching, and per-syncer privacy decisions. M2 stub-mode assertions are narrow
   (round-37 L7 — earlier "no warnings emitted" was too broad now that
   consent warnings live in `PrivacyContext`, not `prepare`): under the
   stub, `PrivacyContext::take_warnings()` returns empty (TCF/USP/GPP
   parsers are inert in M2); `NormalizedBidRequest.warnings()` may be
   non-empty if a future prepare-time warning is added, but is empty in
   M1; no bidder is dropped by privacy; all FPD is preserved. M3
   narrows the GDPR slice; M4 finishes the auction-path rest; M5
   ships separate host, candidate, and narrower `/setuid` permission
   methods; `/setuid` consults host-cookie GDPR and disabled-syncer
   policy without the `/cookie_sync` chooser ladder. M5
   finishes the sync-path rest.
   `Bidder` trait + 4 adapters;
   `auctioneer-exchange` with `BidderFanout` and the reusable `run_auction`
   entry point (§5.8); **outbound SSRF gates ship with the fan-out — not
   later** (round-32 finding H1): `validate_endpoint` (the full §5.4
   ruleset — fixed-host equality, `host_allowlist` membership, HTTPS
   enforcement, IP-literal rejection), the `auctioneer-bidder`
   `EndpointBuilder` helper (macro percent-encoding, path-traversal
   rejection, `host_allowlist` lookup), and the build-time
   host-set invariant must all be in place before any adapter exercises
   real outbound calls; **resource limits ship with the fan-out — not
   later** (round-33 finding H1): the full §5.3 enforcement — body caps
   (already embedded constants), `max_bidder_response_bytes`,
   `max_bidders_per_auction`, `max_outbound_requests`, `bidder_timeout`,
   tmax effective-budget resolution + deadline anchoring + `remaining
== 0` short-circuit — must be in place when handlers and adapters
   land; **debug surface is disabled in M2** (round-33 finding H2 +
   round-34 finding M3): the `ext.debug` path is suppressed until M6
   ships the §5.6 redaction module; until then `run_auction` never
   populates `response.ext.debug` even if the request asks for it, and
   **no warning is emitted** during the M2–M5 suppression window
   (reusing code 10002 `AccountLevelDebugDisabled` would falsely
   implicate the account's `debug_allow`; silent omission is honest
   until M6 wires the real gate). Targeting / price
   granularity / winner selection (Appendix E); **deterministic
   serialization (Appendix E.7) ships with the response builder in M2,
   NOT M6** (round-40 M4 — snapshot Mocktioneer E2E is part of M2 and
   requires byte-stable output; deterministic sort over §5.9 registry
   order, sorted `ext.*` maps, sorted `seatbid` by seat). `/openrtb2/
auction`, `/status`, `/info/bidders` wired in `auctioneer-core` as a
   thin shim over `run_auction`. E2E against Mocktioneer, exercising
   both the handler path and a direct `run_auction` call (proving
   trusted-server reuse) with **snapshot `BidResponse` assertions**
   enabled by the M2-resident deterministic output.
3. **M3 — GDPR auction gate.** Split into **M3a + M3b** (round-20
   scope re-decision — the reviewer's two successive "not ready to
   implement" verdicts on rounds 18/19 identified issues that
   cannot be closed inside auctioneer alone; the split lets M3a ship
   the executable slice while M3b tracks upstream EdgeZero
   prerequisites).
   - **M3a — GDPR auction gate (privacy + Axum production +
     DevFanout).** `auctioneer-privacy` GDPR/TCF v2 (Appendix A);
     wire `AuctionPermissions` into the auction split/drop logic.
     Config overlays for host + per-account GDPR via single-TOML
     `AuctioneerAppConfig`. Real IAB GVL v3 snapshot bundled at build
     time with schema validation. `BidderFanout` conformance
     against `DevFanout` on all four adapters. `EdgeZeroFanout`
     ships fully green on Axum only. `PrivacyContext::resolve`
     reads `user.consent` **only when GPP section 2 is NOT in scope**
     (round-76 B1): when `regs.gpp_sid` contains section 2, the
     section-2 (TCFEU2) consent is authoritative and M3 has no decoder,
     so M3 marks consent UNUSABLE (`consent_usable = false` →
     `default_permissions()`, `PassID: false`) rather than reading the
     possibly-overridden `user.consent` (GPP TCFEU2 decoding stays M4
     per D.1). The M3a-committed config path
     silently degrades on config-store parse errors (this matches
     M2's `.unwrap_or_default()` shape at `auction.rs:261`) —
     fail-loud config semantics move to M3b once EdgeZero exposes
     typed error variants. **Round-25 H8 (round-29 M3 name
     refresh):** the full mapping table (`MissingKey` /
     `EnvelopeParse` / `EnvelopeSha` / `TypedDeserialize` /
     `Validation` / `MissingStoreBinding` / `MissingSecret` /
     `InvalidKey` / `BackendUnavailable` / `Internal` /
     `UnknownVersion` / `MissingSecretBinding` /
     `SecretValidation` → `RuntimeConfigError::MalformedHost` /
     `RuntimeConfigError::ConfigStoreUnavailable` /
     silent-default) lives in M3b Prereq 1 and Task M3b-1.
     Round-32 M4: the canonical route error is
     `RuntimeConfigError` (core-owned, per M3b Task M3b-1). The
     obsolete `AuctionError::MalformedHost` /
     `AuctionError::ConfigStoreUnavailable` variants are removed
     — config loading happens BEFORE `run_auction`, so
     auction-level variants were unreachable in practice. The
     obsolete round-25 names `Parse` / `CorruptEnvelope` /
     `AuthFailure` are dropped — the pinned upstream never
     exposed an auth signal, and `Parse`/`CorruptEnvelope` split
     into the more precise
     `EnvelopeParse`/`EnvelopeSha`/`TypedDeserialize`.
   - **M3b — Multi-adapter production fanout + fail-loud config +
     bounded buffered fan-out (round-143 #7: the outbound client,
     typed reasons, store-extraction taxonomy, string-class ingress
     and application clock are LANDED at `120dd2aa` — upstream-gated
     ONLY on raw-ingress head limits / framing validation
     (Unsupported on every adapter) and response-egress completion
     (BestEffort on all four at head `4790a140` — implemented but not
     certifiable, so still gating; self-review 2026-09-18 corrects the
     blanket "Unsupported" here); the
     M3b plan's compatibility matrix is the live status, and the
     prerequisite items (i)-(iv) further below are HISTORICAL).** Round-43
     B1: the round-42 B4 `M3b-partial` label was withdrawn on the
     claim that "with the WBITS-20 ceiling every adapter admits
     bounded concurrent fanout (6 streams on CF/Fastly/Spin, 32 on
     Axum)". Round-47 B4 RETIRES that claim: the canonical window
     is **WBITS-22** (round-44 B2 — WBITS-20 would reject a
     default-configured encoder). **Round-96: the admission + memory
     proof is now DESIGN-READY** — the M3b plan's round-98 design
     (portable try-acquire `AdmissionGate` before body
     materialization, isolate-wide `StreamGate` bounding live
     response streams, one validated `ResolvedIsolateLimits` with per-stream
     decoder charging and a pinned parsed-GVL expansion factor)
     replaces the unsound round-38→45 drafts. §5.2's
     concurrent-fanout contract remains **PROVISIONAL only on the
     EdgeZero outbound prerequisite — the BUFFERED batch
     (round-131 #8: the streaming phrasing here is replaced; upstream
     fan-out is buffered-only)**; the spec certifies bounded
     concurrent fanout once M3b-2 implements that design against the
     landed primitive. Ships (once unblocked):
     `EdgeZeroFanout` wave-lease dispatch with composed absolute
     deadlines / per-call timeout on Cloudflare, Fastly, and Spin;
     `RuntimeConfigError::MalformedHost` variant + the fail-loud
     handler match that distinguishes `MissingKey` from
     `EnvelopeParse`/`TypedDeserialize`/`Validation`; and the
     dual-cap ownership rows (`max_bidder_response_bytes` split into
     transport + decoded caps) enforced by upstream
     `ResponseMode::Buffered { max_bytes }` per row — the interim
     single-cap `Buffered.max_bytes = max(transport, decoded)` is
     replaced under the G5 facade swap before the behavior-complete
     transport-cap item closes.
     **EdgeZero prerequisites (each gates only its consuming M3b
     tasks — round-116 P3, matching the M3b plan's per-task rule)**:
     (i) typed, categorized store-extraction errors — LANDED at
     the candidate head as `EdgeError::StoreExtraction { reason:
StoreExtractionReason }` with a distinguishable missing-blob
     cause (round-141 #2; the `StoreAppConfigError` name this item
     used historically survives only as auctioneer's mapping-table
     label); (ii) per-adapter
     BOUNDED BUFFERING (round-132 #6 — replaces the round-27
     streaming requirement; the adopted upstream outbound is
     buffered-only): the adapter collect paths — Axum's
     `response.bytes().await` at
     [`edgezero-adapter-axum/src/proxy.rs:61`](https://github.com/IABTechLab/edgezero/blob/9e661ae/crates/edgezero-adapter-axum/src/proxy.rs#L61)
     and Spin's buffered-then-decompressed path at
     [`edgezero-adapter-spin/src/proxy.rs:48`](https://github.com/IABTechLab/edgezero/blob/9e661ae/crates/edgezero-adapter-spin/src/proxy.rs#L48)
     (round-27 M1 correction — the pinned code path is
     `response.bytes()`, not `body.collect()`) — must enforce
     `ResponseMode::Buffered { max_bytes }` DURING the read per
     the M3b dual-cap ownership rows, never an unbounded collect
     followed by a size check; the decoded-cap row carries
     round-27 H5's requirement that gzip/brotli compression
     bombs cannot amplify a small transport read; (iii)
     cancellable proxy futures on Cloudflare (`AbortController`
     wired through `worker::Fetch`) and Spin (drop-cancels the
     `send(...).await` future); (iv) OR a native fanout
     primitive that carries per-call timeout + auction deadline
     - positional results + bounded response reads. Fastly's
       `pending_request.wait()` remains blocking until the native
       primitive lands — Fastly production-fanout coverage
       definitionally waits on (iv).
   - **Production-complete boundary (round-57 B-m; round-70 H1 — the
     ingress hook now has a named owner).** Neither M3a alone NOR "M3
     as currently scoped through behavior-complete" is
     production-complete under the pinned EdgeZero
     (`docs/outbound-http-spec` @ `12965da2` — round-138: the gaps
     below were established at `9e661ae`/`fcefdde3` and remain
     UNIMPLEMENTED in code, though several are now design-agreed
     per the M3b plan's compatibility matrix): M3a
     fails OPEN on a malformed GDPR config and enforces
     `max_bidder_response_bytes` only AFTER the adapter buffers the
     whole body, and M3b's fanout/fail-loud/bounded-buffering work plus the
     §12-item-14 per-route **ingress hook** are all
     EdgeZero-upstream-gated. **⚠ ROUND-182 #9 — THAT IS NO LONGER
     TRUE OF BEHAVIOUR, ONLY OF CERTIFICATION.** At the pinned
     revision the outbound prerequisites are LANDED (see the M3b
     plan's prerequisite checklist); M3b's fanout IMPLEMENTATION
     waits on auctioneer-side work — the batch rebase, the pin move and
     the roster reconciliation — not on upstream. What remains
     upstream-gated is PRODUCTION CERTIFICATION: raw-ingress
     guarantees are Unsupported and the response-egress rows are
     BestEffort. Any reference below to `12965da2` is a superseded
     pin (the current one is named at the top of this document). **Behavior-complete = M3a + M3b
     tasks M3b-1..9 **PLUS M3b-0 and M3b-11** (round-180 #4 — those
     two were added after this formula was written and are NOT
     optional: M3b-0 rebases the outbound contract off an upstream
     method that no longer exists, and M3b-11 moves the pin that
     makes the batch API present. Without them the final gate could
     certify code targeting a removed API and an old pin. M3b-10
     remains excluded, as every "M3b-1..9 behavior-complete without
     it" statement in this spec says. **ROUND-184 #8 — THIS FORMULA
     IS NOW COMPLETE AND AGREES WITH THE PLAN'S DAG AND GATE.** The
     round-183 plan briefly added a SECOND pin task (M3b-12) that
     adopted upstream's G18–G22 lifecycle contracts, which this
     formula did not name — so spec and plan disagreed on what
     behavior-complete required. Upstream then landed those contracts
     on the same revision as the batch API (`351698c9`), the two pin
     tasks COLLAPSED back into M3b-11, and the formula above is
     correct as written: M3b-0 + M3b-1..9 + M3b-11, with M3b-11
     carrying both the batch migration and the lifecycle adoption in
     one atomic commit. There is no M3b-12; the promotion-barrier
     unit that briefly shared that number is M3b-13, and it is a
     PRODUCTION-gate unit, excluded from behavior-complete like
     M3b-10) (gate: M3b-8, EXECUTED LAST —
     after M3b-9 lands;
     round-95 H2: task numbers are identifiers, not execution order,
     and the gate certifies all of M3b-1..9 including M3b-9's runtime
     alias validation). Production-complete additionally
     requires the ingress hook, owned by M3b Task M3b-10 — **blocked
     on EdgeZero Prereqs 5 AND 10 for IMPLEMENTATION, plus Prereq 9
     for PRODUCTION CERTIFICATION (round-171 #3: this line named only
     Prereq 5, which would let a reader claim the production
     milestone with the egress-completion and ingress-seam
     prerequisites still open; the M3b plan's ledger carries the same
     three-way split)** — a distinct PRODUCTION-release gate.** M3
     becomes production-deployable ONLY once those prerequisites land
     and M3b-10 is green. **Round-75 H2 — the production label is
     per-target: for AXUM, M3b-10's raw-wire E2E suite + synthetic
     `decide` contract tests green; for Cloudflare / Fastly / Spin, a
     passing BUILD is NOT enough** — compilation establishes nothing
     about the managed runtime's parser behaviour, pre-allocation
     capping, cancellation, or framing normalization, so a production
     label on those targets requires EdgeZero adapter conformance +
     deployed runtime probes (execution now owned by M7.6). Until then the
     production GDPR-gate label applies to Axum only; the wasm adapters are build-verified,
     not production-claimed. Until any of this lands M3a is explicitly
     the **dev-complete** slice, not a production GDPR gate.
     **Current release rule (2026-10-03 integration):** compiled
     milestone stamps and Auctioneer-owned promotion commands have been
     removed. `/status` reports `x-auctioneer-config-status`, not an
     artifact certification label. A PRODUCTION deploy goes through
     EdgeZero's production-deploy tooling
     (https://github.com/IABTechLab/edgezero/pull/381); auctioneer ships no
     promotion tooling of its own, and the repo's raw deploy commands
     are dev/staging-only. Deployment qualification follows the
     manifest's provider capability policy and runtime evidence.
     Earlier milestone-label descriptions in this document are
     historical acceptance categories, not current runtime APIs or
     an additional signing scheme.
     **Round-75 B1 / round-76 B2 — the production-complete label states
     that M3 treats GDPR-via-GPP-section-2 CONSENT as UNUSABLE** (round-72
     H2's "GPP-only exclusion" was wrong — it missed the fail-OPEN
     mixed/conflicting case). Because GPP section 2 (TCFEU2) is the
     authoritative GDPR consent source that WINS over `user.consent`
     (§7.4 / Appendix D.1) and M3 has no GPP decoder, M3 must NOT read
     `user.consent` when `regs.gpp_sid` contains section 2 — otherwise a
     stale legacy `user.consent` that grants could override an
     authoritative GPP denial (a fail-OPEN privacy violation). So
     whenever section 2 is in scope M3 sets `consent_usable = false` and
     returns the `default_permissions()` row (**`PassID: false`
     unconditionally**; `AllowBidRequest`/`PassGeo` denied where the
     operator enforces P2/SF1) — this is the SAME unusable-consent
     handling as empty/malformed/GVL-unavailable, NOT a new "deny all"
     result, and it applies on the enforcement path only (an out-of-
     scope / `enforcement_active == false` request — host kill-switch,
     non-standard publisher, disabled channel, `Signal::No` — is
     allow-all and never reaches this row). The guarantee is no ID leaks
     from a stale legacy consent. Full GPP section-2 EXTRACTION + the
     `user.consent`-vs-GPP conflict warning that would let M3 HONOUR
     (not just nullify) section-2 consent lands in **M4** (GPP decoding).
     A production-complete M3 release must document that it nullifies
     section-2 GDPR consent (not that it "denies all section-2
     traffic"); it is not a scope reduction (GPP was always M4).
4. **M4 — CCPA, GPP, activities, the NON-GDPR scrub rows.**
   Appendix C + D; full `PrivacyContext`; LMT/COPPA. **Round-49 H4:
   the two GDPR scrub rows (`scrub_gdpr_id` on `PassID=false`,
   `scrub_geo_and_device_ip` on `PassGeo=false`) ship in M3, not
   M4** — M3 is the GDPR gate and must enforce, not merely decide.
   M4 owns the remainder of the Appendix C.2 matrix
   (`scrub_user_fpd`, the CCPA row, LMT/COPPA, `scrub_tid`) plus the
   activity model that drives them.
   **2026-09-29 status: implemented and locally verified, not yet merged.** Strict USP,
   bounded GPP selection, Disclosed Vendors retention, independent policies,
   configuration, fetch gates, scrubs, and legacy projection are implemented.
   Thirty-five focused exchange tests verify outbound bytes and both adapter
   hooks; real-handler tests cover all three shipped adapters. Fresh native
   default/alternate-feature tests, emitted-config/runtime parity, fanout
   conformance, strict Clippy selections, and all three WASM check/conformance
   builds passed. The live handler prepays measured phase reservations,
   bounds outbound construction, and rejects excess projection/response
   amplification before allocation. Allocation and cancellation regressions
   cover the corrected retained-node and overlapping-state charges. See the
   [verification record](../plans/2026-09-28-m4-verification.md). The
   [M4 design](2026-09-26-m4-privacy-design.md) and
   [implementation plan](../plans/2026-09-28-m4-privacy.md) define the reviewed
   contract and acceptance evidence. This closes M4's local integration,
   not overall provider production qualification or the broader design's
   unimplemented response-cap overlays. Logical accounting is not a physical-OOM
   guarantee; over-ceiling boot still warns. Merge M3b before M4 into `main`.
5. **M5 — User-sync.** **First milestone where sync endpoints are
   reachable** (round-40 H1). Ships: `uids` cookie module (§8.5);
   host-cookie import merge (§8.2 / round-22 M5); the
   `/cookie_sync` / `/setuid` / `/getuids` route wiring + handler
   pipelines (§8.2-§8.4); candidate-selection ladder (Appendix B.3);
   cookie-write + eviction (`PriorityBidderEjector`, Appendix B.6);
   syncer-URL safety integration (§8.6); and the **real
   `resolve_sync_privacy`** (§7.8) — TCF v2 / US-Privacy / GPP parsing,
   GDPR vendor decisions, `SkipWhen.gpp_sid` exact-string matching,
   `BidderSyncAllowed`, per-syncer decisions — replacing M2's permissive
   stub with no API-surface change.
6. **M6 — Parity hardening.** The [M6 hardening design](2026-10-05-m6-parity-hardening-design.md) is the executable contract for:
   bounded safe debug projections and enablement; typed-overlay sibling
   cases and current whole-blob/structural-budget error precedence; one
   snapshot per request with update/invalid/outage/deletion/recovery tests;
   an explicit existing-versus-new parity inventory; offline shared-handler
   harness/artifact handoff/refusal tests and runbooks for Cloudflare/Spin/Fastly;
   and explicit Q2 deployment-resource qualification under M6 design section
   10.1. Q2 requires repeatable release
   load tooling, cold/warm and maximum-admitted memory/latency evidence, tuned
   funded profiles, overload recovery and a rollout/rollback runbook. Native
   acceptance (Q2-A/B/C) and reusable Q2-E are M6 implementation deliverables.
   Credentialed real provider execution, authorized corrupt-config
   injection/recovery and selected actual deployment Q2-D execute in M7.6
   after M7.5 plus the selected bidder migration wave/host setup; that
   deployment must pass Q2-D before its live limited-bidder publisher pilot.
   M6 local/staging acceptance alone does not establish live-pilot readiness.
   Q1/Q3/Q4 SDK controls remain EdgeZero-owned, and Q2 does not block M3b's
   implementation merge.
   Separate account storage/deletion policy remains M8.
   Provider propagation/production capability claims
   remain per-target evidence, not a reason to block unrelated behavioral
   implementation. No new routing or signing path is added.
   Outbound/SSRF/resource invariants and deterministic snapshot tests remain
   existing regression requirements, not scope postponed to M6. All current
   multi-target compile checks remain per-milestone; passing a build does
   not prove live handler or provider conformance behavior.
   The [M6 implementation plan](../plans/2026-10-07-m6-parity-hardening.md)
   sequences the final foundation inheritance, debug/config acceptance and
   Q2 tooling/evidence without adding bidder-catalog scope.

### 14.1. Roadmap Beyond M6

M1-M6 retain their initial-scope requirements except the explicitly
user-approved 2026-10-15 transfer of credentialed live execution/pilot
acceptance to M7.6. The
[approved roadmap's 2026-10-26 scope correction](../../roadmap.md#2026-10-26-scope-correction)
extends milestone ownership through M20: bidder foundations and full
14-priority selected-flow acceptance (M7), direct Trusted Server embedding (M7.5),
limited-bidder publisher pilot and hosted acceptance (M7.6), additional
demand-led bidder waves (M7.7) under the
[2026-10-27 correction](../../roadmap.md#2026-10-27-post-pilot-bidder-expansion), then
stored requests/accounts (M8), currency, multibid, auction UID projection and
the [global strict/reference-compatible protocol mode](../../roadmap.md#m9-global-protocol-mode)
(M9), creative caching (M10), and full video support (M11).
M7's [controlled-native amendment](2026-10-30-m7-controlled-native-completion-design.md)
uses tested resource reservations and enforced admission, not complete
worst-case resource proofs. M7-R0 retains those proofs and absolute WASM fit;
neither native acceptance nor a compiled guest artifact waives them.
The planned mode defaults to strict; compatibility cannot weaken privacy,
security or resource/deadline protections. It does not change current behavior.
Remaining FPD, floors/adjustments, AMP/native, privacy
and transparency, compression, hooks, telemetry, and browser compatibility
are assigned to M12-M19. M20 owns residual full-catalog migration and closure,
not an M7 completion prerequisite. New implementation needs a separately reviewed
specification and plan; M6 is not a catch-all for these new features. M7.6
executes retained harness/runbook contracts under the roadmap's actionable
prerequisites, cases, evidence and invalidation rules, without needing a new
implementation specification merely for deferred execution. Its selected
bidder migration wave and M7.5 integration/host setup are prerequisites;
M20 full-catalog closure and video parity are not. All selected-target
safety/privacy/credential/creative-rendering checks remain before real traffic.

Catalog migration and shared feature parity are separate completion claims.
The roadmap requires reviewed adapter fixtures and an explicit capability
matrix, not metadata-only migration. Runtime enable/disable of a compiled,
baked-eligible bidder does not require a rebuild; new code or baked metadata
does. Full video requires creative delivery and ad-pod integration, not only
acceptance of `imp.video`.

Each milestone is independently testable and leaves the workspace green.
**A milestone is "done" only when both unit tests (§11.1) and integration
tests (§11.2) for everything it ships are green — neither layer optional.**

---

## 15. Intentional Divergences from reference server

This spec targets behavior-compatibility with the pinned baseline, but a few
departures are deliberate. They are listed here so "behavior-compatible" is
never read as "byte-identical to upstream."

The [M4 design, section 12](2026-09-26-m4-privacy-design.md#12-explicit-compatibility-decisions)
records the complete M4 decisions against upstream commit
`d7f8515b86258688304b0d9b6668c6a0e258bc9e`. In particular, M4 requires active
SID membership for section selection/backfill, leaves unsupported payloads
opaque, rejects hard GPP bounds instead of falling back, independently
validates no-sale, removes both EID representations, and does not let USP
conflict warnings disable enforcement. It retains M3's floor-based negative
coordinate rounding instead of the reference's truncation-toward-zero drift.
Unicode simple folding, the account-wide CCPA fallback, and empty section-6
fallback are compatibility corrections, not new divergences. DV retention
does not introduce any new permission-granting rule.

| Divergence                                                                                                 | auctioneer                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | reference server                                                                                                                                                                                                                                                                                                                                              | Rationale                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| ---------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| OpenRTB version                                                                                            | 2.6 only; **no 2.5→2.6 upconverter** (§3)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               | both, with upconverter                                                                                                                                                                                                                                                                                                                                        | Explicit OpenRTB 2.6 contract reduces ambiguity at edge                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Response ordering                                                                                          | `seatbid` sorted by seat; `ext` maps sorted by key; bidders processed in deterministic registry order (Appendix E.7)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    | unspecified map iteration order; live adapter list randomized                                                                                                                                                                                                                                                                                                 | Stable snapshot tests; reproducibility                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| Currency                                                                                                   | single-currency USD; non-USD seatbids dropped with a `DisabledCurrencyConversion` warning                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               | full currency conversion                                                                                                                                                                                                                                                                                                                                      | Conversion is a later phase (open item 3)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `allow_insecure`                                                                                           | build-time cargo feature (`insecure-bidders`), never in production                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      | n/a                                                                                                                                                                                                                                                                                                                                                           | Prevents a runtime config mistake enabling insecure outbound targets                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| Sync URL scheme                                                                                            | HTTPS only in default builds (the `insecure-bidders` feature relaxes this for local testing)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            | accepts HTTP                                                                                                                                                                                                                                                                                                                                                  | Edge-deployed defensive default                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| Debug override                                                                                             | Conservative: must be both **requested** _and_ account-permitted (`debug_allow`); no header/query override path that bypasses the account gate                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | A debug-override path can force-enable response debug + bidder HTTP calls beyond account gates (reference auction policy, reference bidder request/response processing)                                                                                                                                                                                       | Avoid a side channel that leaks consent / IP / tokens past operator policy (round-15 finding M6)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `responsetimemillis[bidder]`                                                                               | Post-batch synchronous approximation: `make_requests` + `max(call elapsed)` + `make_bids` (§5.2)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        | Streaming per-call: bid response conversion runs as each response arrives, overlapping with in-flight calls (reference bidder request/response processing)                                                                                                                                                                                                    | Simpler batch model; over-counts overlap by ~milliseconds (round-16 finding H2)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| Outbound `Sec-GPC` source                                                                                  | Forwarded iff **`regs.ext.gpc == "1"`**; `regs.ext.gpc` is populated by `prepare()` from a `"1"`-valued inbound `Sec-GPC` header _or_ a client-supplied JSON value (single source of truth — round-29 finding M4). Any other value is ignored.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | Forwarded from the _original_ HTTP `Sec-GPC` header only; JSON `regs.ext.gpc` does not trigger header propagation (reference auction handling, reference bidder request/response processing)                                                                                                                                                                  | Simpler — one signal channel. Observable on the JSON-only edge case (client sets `regs.ext.gpc=1` without sending the header): auctioneer forwards, upstream doesn't (round-28 finding M3 / round-29 M4)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| Privacy Sandbox                                                                                            | **Not implemented in phase 1**: no `Sec-Browsing-Topics` enrichment, no `/cookie_sync` `receive-cookie-deprecation` partitioned cookie, no `account.privacy.privacysandbox` config surface (round-21 finding H2)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        | Both implemented (reference auction handling, reference cookie-sync handling)                                                                                                                                                                                                                                                                                 | Deferred to a dedicated Privacy-Sandbox phase to avoid shipping a partial implementation                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| CMP authenticity (round-99 B1/B2/B3)                                                                       | `TcfString.cmp_id` is validated against the bundled Global CMP List: unknown CMP, CMP with `deleted_date <= now`, or a GCL past its freshness window (`now >= fetched_at + cache_max_age_secs` — the upstream RFC 9111 lifetime EXACTLY, no floor; round-100 B2) ⇒ consent UNUSABLE (fail-closed to the Appendix A.4 default row; codes 10001/10019). The GCL ships in the GVL archive (manifest `gcl` object: path/sha256/last_updated/fetched_at/cache_max_age_secs), refreshed by the same weekly transaction, validated by `validate_gcl_json`, baked via `auctioneer_config::baked_gcl_json()`, lowered once into a boot-initialized `Gcl` static, and consumed by `PrivacyContext::resolve(req, account, host, gvl_store, gcl, now)`                                                                                                                                                                                                                                                                                  | reference consent parser never consults the GCL; any `cmp_id` is accepted                                                                                                                                                                                                                                                                                     | The TCF requires TC strings to originate from registered CMPs, and the current spec invalidates strings from deleted CMPs once `deletedDate` passes; accepting unauthentic strings would over-grant processing. Stricter-than-upstream, fail-closed                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| Account lookup ordering                                                                                    | After earlier framework ingress/admission succeeds: one validated config snapshot and host clamp → bounded body read → syntactic JSON/node check → raw account identity and typed overlay → typed parse/prepare → auction. Config failure wins over application account/body faults; JSON syntax or node-budget 400 wins over account-disabled; a within-budget body naming a disabled account gets 503 before typed preparation. Separate MalformedAcct fetch failures are M8, not current blob behavior.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              | Reference account discovery precedes full request preparation                                                                                                                                                                                                                                                                                                 | Preserve account-before-typed-validation behavior without spending unbounded structural work on account discovery; M6 section 7 pins the competing-fault matrix.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `multibid` bid limiting                                                                                    | **Parsed/typed only** in phase 1 (round-22 M8 / round-23 M5); no validation, no warning code 10006, no rebuild, no per-bidder bid limit                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 | upstream validates + rebuilds (reference auction handling), clamps `maxBids` (reference multi-bid limits), applies limiting (reference auction handling)                                                                                                                                                                                                      | Bid-limiting is targeting-adjacent and deferred together with the other targeting deferrals                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| DSA / SChain                                                                                               | **Typed pass-through only** in phase 1 (round-23 finding M6): `regs.ext.dsa` and `source.ext.schain` round-trip losslessly, but no account-DSA writer, response-DSA validation, schain merge, or schain validation                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      | upstream mutates request DSA from account privacy config (reference auction processing), validates response DSA, applies bidder schains (reference auction policy)                                                                                                                                                                                            | Both are large privacy-adjacent surfaces deferred to a dedicated phase; passing through preserves client-supplied values without claiming enforcement                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| Request-level FPD (`ext.prebid.data`, `data.eidpermissions`, `ext.prebid.bidderconfig`)                    | **Typed pass-through only** in phase 1 (round-24 finding H2); the per-imp `imp.ext.prebid.imp` merge (§5.1 step 7) is the only FPD path that takes effect. Privacy scope's "full fidelity" applies to GDPR/TCF/CCPA/GPP/activities/scrubbing — **not** to request-level FPD resolution                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | upstream extracts/resolves bidder-level FPD before splitting (reference auction processing), resolves request-level first-party-data extensions, and applies bidder FPD later (reference auction policy)                                                                                                                                                      | Request-level FPD is a substantial sub-system deferred to a dedicated FPD phase                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| Inbound request compression                                                                                | **Not supported in phase 1 on ANY body route — auction AND `/cookie_sync`** (round-23 L8 / round-24 M6 / round-28 L6 / round-29 L6 / round-68 M3 — scope broadened to match the round-68 H2 ingress hook: a non-empty `Content-Encoding` header on either `/openrtb2/auction` or `/cookie_sync` is rejected with `400` body `Invalid request: Content-Encoding of type <encoding> is not supported\n` (round-69 L4 — body string includes the trailing `\n` canonically per Appendix F); exact upstream plain-text error output wording on auction, auctioneer extends the same rule to /cookie_sync).                                                                                                                                                                                                                                                                                                                                                                                                                      | upstream handles request gzip with size caps and explicit unsupported-encoding errors on auction (reference auction handling); upstream does NOT reject `Content-Encoding` on `/cookie_sync` — it would accept a gzip body there                                                                                                                              | Edge body limits already bound size; deferring inbound decompression on auction matches upstream, and **extending the same rejection to /cookie_sync** is a deliberate defense-in-depth hardening — the ingress hook applies one uniform Content-Encoding rule to every body route, so a future compressed-body decoder lands once for all routes (round-68 M3 — observable on a /cookie_sync request that carries `Content-Encoding: gzip`: auctioneer → 400, upstream → tries to decompress).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| Cookie-sync priority groups                                                                                | auctioneer consults host _and_ account `cookie_sync.priority_groups` unconditionally when cooperative is enabled (Appendix B.3)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | upstream uses account `priority_groups` only under a specific account cookie-sync condition (reference cookie-sync handling)                                                                                                                                                                                                                                  | Simpler precedence; if strict parity is needed later, gate behind an account flag (round-23 finding L9)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| /setuid eviction priority source                                                                           | auctioneer uses the **same composed list** as /cookie_sync — i.e. account `cookie_sync.priority_groups` when `Some(_)`, else host `user_sync.priority_groups` (Appendix B.6)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                | upstream /setuid eviction uses only host `UserSync.PriorityGroups` (reference UID-write handling), independent of any account override                                                                                                                                                                                                                        | Symmetric with /cookie_sync — same composed list drives candidate ordering AND eviction (round-31 finding M4); avoids a confusing split where two adjacent flows consult different sources                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| Outbound HTTP                                                                                              | EdgeZero fan-out primitive                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              | Concurrent HTTP requests                                                                                                                                                                                                                                                                                                                                      | Platform; not observable in responses                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| `/cookie_sync` opt-out vs application policy ordering                                                      | After a valid runtime snapshot and bounded request extraction, host opt-out returns 401 before account state, consent evaluation, and filterSettings interpretation. Invalid data anywhere in the runtime blob instead fails config 500 before application opt-out.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     | Reference request/account/privacy/filter interpretation and opt-out ordering differs                                                                                                                                                                                                                                                                          | Current fixtures cover opted-out + disabled account, bad filter, or missing consent; malformed whole-blob precedence is tested separately. No independently malformed-account record is implied before M8.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `/setuid` request-extraction-error vs activity ordering                                                    | **Request-extraction errors caught by `resolve_sync_privacy` (step 9) → 400 run _before_ `syncUser` activity → 451 (step 10)** (§8.3, round-37 H1 + round-41 H2 + round-45 L7 + round-50 L2 + round-54 H3 — narrowed again after round-53 H2 moved malformed-TCF off the privacy-parse path). The 400-eligible **request-extraction** errors are: bad GPP envelope/section, bad `gpp_sid` encoding, an `Invalid(_)` legacy `gdpr` value that GPP/SID didn't override (round-46 H1), and empty `gdpr_consent` under **`parsed_request_signal == Yes`** (round-52 H2; matches reference UID-write handling). **Malformed non-empty TCF is NOT a request-extraction error** (round-53 H2) — it's decided downstream in the **permission path** (step 11) by `HostCookiesAllowed`, which runs AFTER activity (step 10). So `syncUser`-denied + malformed non-empty TCF → 451 (activity wins). Malformed TCF when `normalized_request_signal != Yes` is silently ignored on /setuid (round-52 H1) and does NOT block at all. | upstream checks the `syncUser` activity first (reference UID-write handling), then calls reference GDPR signal resolution to parse consent                                                                                                                                                                                                                    | Structural-error-first principle: a 400 indicates a malformed request that the operator can fix; a 451 indicates a policy outcome on a well-formed request. Surfacing the 400 first (when it comes from request extraction) makes operator debugging easier. Observable on requests that are both `syncUser`-denied AND would 400 by the request-extraction rules above (e.g. `syncUser`-denied + empty consent under `parsed_request_signal == Yes`: auctioneer → 400, upstream → 451). For `syncUser`-denied + malformed non-empty TCF: auctioneer → 451 (matches upstream; permission-path decision runs after activity). (round-41 H2 + round-54 H3)                                                                                                                                                                                                                                                                                                                                 |
| `/setuid` disabled-syncer rejection                                                                        | **auctioneer emits `SyncDecision::BlockedByDisabledUsersync` → `400 The bidder syncer is disabled`** even on `/setuid` (§8.3 pipeline step 11, Appendix F, round-39 M2 + round-41 M4 + round-45 L7 + round-55 M4 — uses `setuid_allowed` per round-54 H1, NOT `sync_allowed`)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               | reference UID-write handling fetches the syncer directly without going through the `chooser` path that emits `StatusBlockedByDisabledUsersync`; a disabled syncer on `/setuid` produces a different upstream response shape                                                                                                                                   | auctioneer intentionally unifies the disabled-syncer check across `/cookie_sync` and `/setuid` — /cookie_sync via `sync_allowed(SyncerLookup{...})` (chooser ladder), /setuid via the narrower `setuid_allowed(SyncerLookup{...})` (host-cookie GDPR + `enabled` only). Both return `SyncDecision::BlockedByDisabledUsersync` for `enabled == false`, so the same operator-visible rejection applies on both endpoints — symmetric with the /setuid eviction-priority unification (above row). Tests cover both endpoints with shared fixtures (round-41 M4)                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `/setuid` chooser-ladder scope                                                                             | **auctioneer /setuid does NOT invoke `BidderSyncAllowed` or `SkipWhen.gpp_sid`** (§8.3 step 11, round-52 H3 — narrowed; earlier wording incorrectly applied the chooser ladder). /setuid step 11 only consults host-cookie GDPR (from step 9) + disabled-syncer                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | reference UID-write handling only invokes the host-cookie GDPR check, not the chooser ladder (reference syncer selection is /cookie_sync-only)                                                                                                                                                                                                                | This is parity, not a divergence — both auctioneer and upstream restrict the chooser ladder to /cookie_sync. The row exists to dispel the earlier "regulation-scope matches upstream /setuid" claim which was wrong. /setuid's `BidderSyncAllowed`-equivalent decision happens implicitly through `HostCookiesAllowed` instead. Tests cover: `SkipWhen.gpp_sid` matching request → /cookie_sync blocks, /setuid proceeds; `BidderSyncAllowed=false` → /cookie_sync blocks per-syncer, /setuid blocks only through the host-cookie path (round-52 H3)                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| Host-cookie required fields                                                                                | `host_cookie.cookie_name` and `host_cookie.family` are required non-empty strings; the name also cannot be `uids` (§9)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | Pinned upstream defaults both to empty strings                                                                                                                                                                                                                                                                                                                | Operator hardening: an empty name cannot identify a host identity cookie to import, and an empty family cannot identify its UID slot. Auctioneer's writable cookie remains the separately fixed `uids` cookie. Invalid baked config fails generation; invalid runtime config fails push/load rather than silently disabling host identity import.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| TCF core-segment strict validation (round-89 H3 / round-90 M6)                                             | auctioneer REJECTS: (a) `tcfPolicyVersion < 4` with core `Created` on/after 2023-10-01 (`PolicyVersionTooOldForCreated` — round-90 M6: `< 4`, all pre-v4 versions, not just `== 3`); (b) core `Created != LastUpdated` or either not day-truncated (`CreatedLastUpdatedMismatch` / `TimestampNotDayTruncated`); (c) `IsServiceSpecific == 0` (`NotServiceSpecific`); (d) a CORE segment over 64 KiB / over the decoded/range caps (the M3a plan's Task 3 caps table — round-90 M6: NOT spec Appendix D, which is GPP). §7.2.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                | reference consent parser `Parse` performs NONE of these — it accepts stale-policy, unequal-timestamp, non-service-specific, and larger strings                                                                                                                                                                                                                | Standards-defensible fail-closed hardening on the privacy input boundary; each is an observable rejection where upstream would accept. Real-world CMP strings satisfy all of them.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| TCF flexible-purpose legal basis (round-89 B1 / round-90 LOW)                                              | A flexible purpose is authorized on its **DEFAULT basis only** absent a publisher restriction (`purposes` ⇒ consent, `legIntPurposes` ⇒ LI); a restriction switches it (Appendix A.2). The GVL validator rejects a purpose with no/dual default and Purpose 1 in `flexiblePurposes`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    | **reference server at pinned commit `d7f8515` (reference TCF legal-basis evaluation) evaluates BOTH consent and legitimate-interest and ORs them when there is NO publisher restriction** — so a flex purpose (which reference consent parser's consent authorization/legitimate-interest authorization both report established) is allowed via EITHER basis. | **This is a DELIBERATE enforcement divergence from upstream (round-90 LOW — reference TCF legal-basis evaluation ORs both bases; auctioneer does not).** Fail-closed on the authorization boundary: a consent-default flex purpose is NOT LI-authorized (and vice versa) without an explicit publisher restriction, avoiding over-granting under the wrong legal basis per the IAB vendor-list contract. Standards-defensible and observably stricter than upstream on a flex purpose whose one declared TCF basis is absent but the alternate is present.                                                                                                                                                                                                                                                                                                                                                                                                                           |
| GVL policy-version cross-check is ASYMMETRIC (execution-ratified)                                          | When the CMP-declared `(spec, list)` GVL is present, auctioneer fails closed only when the TC string's `tcfPolicyVersion` is **greater than** the bundled blob's (`policy > gvl.tcf_policy_version()`), not on any inequality. An older string against a newer list stays usable.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           | upstream performs no policy-version cross-check at all, so this is stricter than upstream in the newer-string direction and equal in the older-string direction                                                                                                                                                                                               | **The plan's original round-86 B3 rule was strict equality; execution proved it unshippable.** The live bundled spec-3 archive declares policy 5 while TCF v2.2 CMPs emit policy-4 strings, so `!=` marks EVERY v2.2 request unusable — a production-wide fail-closed on legitimate traffic (it also failed eight landed Task 8 tests). The asymmetric form still fails closed in the genuinely unsafe direction (a string claiming a policy the list PREDATES cannot be interpreted against it) while preserving normal forward compatibility. Emits warning 10018 and routes to the A.4 default row exactly like the missing-blob branch.                                                                                                                                                                                                                                                                                                                                      |
| Disclosed Vendors segment: presence REQUIRED, contents deferred (round-88 B1 / round-90 H5 / round-108 H5) | auctioneer semantically interprets and RETAINS only CORE contents (round-115 L4 wording — trailing-segment STRUCTURAL contents are decoded for framing validation), and enforces the STRICT trailing set (round-110 B2): exactly `{DV}` or `{DV, PublisherTC}`, both framing-validated (bounded decode; DV bitfield/range completeness — round-109 B1; PubTC 57+2N-bit layout); empty/duplicate/unknown/obsolete-type-2/extra/incomplete segments → the §7.2 rejection taxonomy → consent unusable, fail-closed; validated trailing contents stay unretained.                                                                                                                                                                                                                                                                                                                                                                                                                                                               | reference consent parser core-segment parsing truncates at the first `.` and accepts a core-only string — the current official TCF specification (May 2026) makes the Disclosed Vendors segment MANDATORY, and disclosure is load-bearing for special-purpose-only vendors                                                                                    | **Round-108 H5 corrects the round-90 "M3 blast radius is nil" claim, which was too strong: a structurally invalid (core-only) string's core was still treated as usable authorization input.** Now: presence is validated in M3 (a structurally invalid string fails closed — observably STRICTER than reference consent parser, recorded as a deliberate divergence); the segment's structural FRAMING is decoded and validated in M3 while its semantic vendor membership is neither retained nor consulted until M4 (round-114 M3 — the earlier "remain unread" wording contradicted the framing validation, which necessarily reads structural contents), so special-purpose disclosure-membership enforcement stays M4-scoped (§7.3) — when M4 lands it MUST parse the segment (or re-derive membership from the raw string) before authorizing an undisclosed vendor for special purposes. Deleted vendors remain denied (round-90 B1 / round-108 M8 timestamp semantics). |

Everything else aims for observable parity with the baseline; remaining gaps
are tracked as out-of-scope phases (§13) or open items (§12).

---

# Appendix A — GDPR / TCF v2 Truth Tables

The pinned baseline (`gdpr/`) supplies the comparison tables. Explicit
Auctioneer enforcement differences are documented below and in §15, including
the owner-approved Basic-policy tightening in A.3.

## A.1 Is GDPR enforced for an auction request?

| `gdpr.enabled` | signal      | resolves to                     | channel enabled | `gdpr_enforced`              |
| -------------- | ----------- | ------------------------------- | --------------- | ---------------------------- |
| false          | any         | —                               | any             | no (permissions = allow-all) |
| true           | `Yes`       | —                               | true            | **yes**                      |
| true           | `Yes`       | —                               | false           | no                           |
| true           | `No`        | —                               | any             | no                           |
| true           | `Ambiguous` | `Yes` (host default or EEA geo) | true            | **yes**                      |
| true           | `Ambiguous` | `No`                            | —               | no                           |

Signal: `gpp_sid` with section 2 → `Yes`; non-empty `gpp_sid` without it →
`No`; else `regs.gdpr`; else `Ambiguous`.

## A.2 Full-enforcement legal basis (per purpose, per vendor)

First matching row wins:

| Condition                                                                                                                  | Result                                                                                                           |
| -------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| Publisher restriction `NotAllowed` for (purpose, vendor)                                                                   | **deny** (vendor exception cannot override)                                                                      |
| Both `RequireConsent` and `RequireLegitInterest` match (purpose, vendor), including overlapping ranges in separate records | **deny** (Auctioneer fail-closed ambiguity policy; neither disabled enforcement nor a vendor exception overrides it) |
| `!enforce_purpose && !enforce_vendors`                                                                                     | **allow**                                                                                                        |
| Bidder ∈ `vendor_exceptions` (and not blocked)                                                                             | **allow**                                                                                                        |
| Publisher restriction `RequireConsent`                                                                                     | allow iff _consent established_ (flex SWITCHED to consent)                                                       |
| Publisher restriction `RequireLegitInterest`                                                                               | allow iff _LI established_ (flex SWITCHED to LI)                                                                 |
| otherwise (no restriction)                                                                                                 | allow iff the vendor's **DEFAULT** basis for the purpose is established                                          |

**Flex legal basis (round-89 B1 — IAB vendor-list contract).** Every
purpose a vendor declares has exactly ONE default basis: `purposes` ⇒
consent, `legIntPurposes` ⇒ LI (the GVL validator rejects a purpose in
BOTH, a `flexiblePurposes` entry with no default, and Purpose 1 in
`flexiblePurposes` — §7.2). `flexiblePurposes` means the publisher MAY
switch that default via a restriction; **absent a restriction ONLY the
default basis applies** — a consent-default flex purpose is NOT
LI-authorized (and vice versa) unless a `RequireConsent` /
`RequireLegitInterest` restriction flips it.

- In the "otherwise (no restriction)" row: _consent established_ uses the
  **consent-default** set (`purposes` only, NOT flex); _LI established_
  uses the **LI-default** set (`legIntPurposes` only, NOT flex).
- In the `RequireConsent` row: _consent established_ treats flex as
  consent (`purposes` ∪ `flexiblePurposes`). In the `RequireLegitInterest`
  row: _LI established_ treats flex as LI (`legIntPurposes` ∪
  `flexiblePurposes`).

In every case _consent established_ additionally requires
(`!enforce_purpose` OR `PurposeAllowed`) AND (`!enforce_vendors` OR
`VendorConsent`); _LI established_ requires (`!enforce_purpose` OR
`PurposeLITransparency`) AND (`!enforce_vendors` OR `VendorLegitInterest`).
Vendor id 0 / absent → both fail.

## A.3 Basic-enforcement legal basis

No GVL declarations; publisher restriction denial checks precede Basic
bypasses, without single-restriction legal-basis remapping. In order:

| Condition                                                                                                                  | Result                                                                       |
| -------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| Publisher restriction `NotAllowed` for (purpose, vendor)                                                                   | **deny** (no Basic bypass overrides it)                                      |
| Both `RequireConsent` and `RequireLegitInterest` match (purpose, vendor), including overlapping ranges in separate records | **deny** (Auctioneer fail-closed ambiguity policy; no Basic bypass overrides it) |
| `!enforce_purpose && !enforce_vendors`                                                                                     | allow                                                                        |
| Bidder ∈ `vendor_exceptions`                                                                                               | allow                                                                        |
| `!enforce_purpose && bidder ∈ basic_enforcement_vendors`                                                                   | allow                                                                        |
| `enforce_purpose && PurposeAllowed && bidder ∈ basic_enforcement_vendors`                                                  | allow                                                                        |
| `enforce_purpose && PurposeLITransparency && allowLITransparency override && vendor_id != 0`                               | allow                                                                        |
| `enforce_purpose && !PurposeAllowed`                                                                                       | deny                                                                         |
| `!enforce_vendors`                                                                                                         | allow                                                                        |
| otherwise                                                                                                                  | `VendorConsent`                                                              |

After the restriction-deny rows, the Basic LI-transparency override grants
iff `enforce_purpose`, `PurposeLITransparency`, the `allowLITransparency`
override, and a nonzero resolved `vendor_id` all hold. Its nonzero-ID condition
is specific to that override: disabled-enforcement, vendor-exception,
basic-list, and `!enforce_vendors` allow rows retain their existing behavior,
including at vendor ID 0 when reached. Duplicate same-type restriction matches
and restrictions on a different purpose/vendor do not create a conflict.
Basic does not consult GVL declarations or remap the basis for a single
`RequireConsent` / `RequireLegitInterest`; only Full does.

This replaces the old Basic table/reference behavior under the owner's
2026-10-12 approval. It is an explicit Auctioneer policy tightening, not a
normative IAB mandate; no legacy-Basic compatibility path is retained.

A full-enforced purpose downgrades to basic for any bidder in
`basic_enforcement_vendors`.

## A.4 `AuctionPermissions` assembly

| Input state                                                                                                     | `AllowBidRequest` | `PassGeo`       | `PassID`                 |
| --------------------------------------------------------------------------------------------------------------- | ----------------- | --------------- | ------------------------ |
| GDPR not enforced / non-standard publisher (account id ∈ `host.gdpr.non_standard_publishers`, §9 / round-48 H2) | true              | true            | true                     |
| Enforced, empty consent                                                                                         | `!P2_enforced`    | `!SF1_enforced` | **false**                |
| Enforced, malformed consent                                                                                     | `!P2_enforced`    | `!SF1_enforced` | **false**                |
| Enforced, GVL fetch fails                                                                                       | `!P2_enforced`    | `!SF1_enforced` | **false**                |
| Enforced, valid consent                                                                                         | legal basis P2    | SF1 check       | OR of legal basis P2–P10 |

- `PassID` always enforces both purpose and vendor regardless of per-purpose
  config; OR of legal basis over Purposes 2–10. `PassGeo`: geo passes iff
  `SpecialFeatureOptIn(1)` AND (vendor declares SF1 OR bidder is a
  basic-enforcement vendor), unless SF1 unenforced or the bidder is an SF1
  exception.
- Auction consequences: `AllowBidRequest=false` → bidder dropped;
  `PassID=false` → `scrub_gdpr_id`; `PassGeo=false` →
  `scrub_geo_and_device_ip`.
- Sync endpoints (round-48 H1 + round-52 H1+H2+M4 — uses §7.8.1
  signal taxonomy):
  - **`normalized_request_signal != Yes`** (or
    `!gdpr_permissions_enabled`): TCF never parsed; malformed
    `gdpr_consent` silently ignored on both endpoints.
  - **`normalized_request_signal == Yes` + `gdpr_permissions_enabled`**:
    - **Empty consent**:
      - /cookie_sync → `400` when `parsed_request_signal == Yes` OR
        (`Ambiguous` + normalized Yes) — matches reference cookie-sync handling + reference cookie-sync handling fixtures.
      - /setuid → `400` only when `parsed_request_signal == Yes`
        (matches reference UID-write handling); `Ambiguous` + normalized Yes
        falls into the host-cookie permission path.
    - **Malformed (non-empty) TCF** (round-58 M1 — full precondition
      list, not the earlier "host_vendor_id != 0" shorthand):
      - /setuid → `400` body `gdpr_consent was invalid. <parser
error>` **only when ALL of** host_vendor_id != 0 AND
        normalized_request_signal == Yes AND gdpr_permissions_
        enabled AND activity already allowed (parse runs via
        `HostCookiesAllowed`, returns 400 per reference UID-write handling;
        the activity-allowed precondition matters because the
        permission path runs after activity, so syncUser-denied
        wins with 451 — round-54 H3); otherwise 200
        (host_vendor_id == 0, or GDPR permissions disabled, or
        normalized != Yes) or 451 (activity denied).
      - /cookie_sync → **`200` + empty `bidder_status`** via
        per-syncer privacy denial (§7.8 / round-47 H1 / round-49
        H2).
  - Neither path uses `default_permissions()` on sync (sync has no
    auction-style permissions surface).

---

# Appendix B — `/cookie_sync` & `/setuid` Schemas

**M5 effective bounds and scope.** `/cookie_sync` accepts at most 32 KiB of
uncompressed body data and the current handler caps its JSON parse at 4,096
nodes. `/setuid` accepts at most 32 KiB of encoded query and 32 parameters,
counting duplicates. Current adapter request-target profiles allow 33 KiB
for path plus query, but a provider may impose a smaller effective limit.
The cookie codec separately caps 64 header fields, 32 KiB header/encoded
value, 24 KiB decoded value, and 4,096 JSON nodes; the installed admitted
header envelope can be narrower. Selection/config limits are 256 names per
list, 128 UTF-8 bytes per name, 32 priority groups, and 256 priority
entries. URLs are capped at 32 KiB each, response bodies at 256 KiB, and
Set-Cookie field values at 4 KiB, including the signed envelope and attributes.
A logical unlimited candidate or cookie
setting does not disable those hard caps. M5 does not migrate the full
bidder catalog or inject stored `buyeruid` into auction bidder requests.
Credentialed CORS is exact-origin and begins only after valid runtime config
resolution; it does not override endpoint privacy decisions or guarantee
third-party cookie acceptance. Auctioneer-owned sync responses are `no-store`.

**Issued-cookie integrity is M5 scope.** The approved
[integrity addendum](2026-10-13-usersync-cookie-integrity-design.md) supersedes
the unsigned production wire contract: authenticate `v1.key-id.payload.mac`
before UID decoding, load one bounded atomic keyring from the existing
EdgeZero secret store, sign transactional writes, and explicitly expire
deletions. Inner JSON/reference fixtures still qualify payload semantics.
Signing does not prove ownership of a callback UID, grant consent, authenticate
accounts, encrypt identifiers or prevent replay. M9 identity projection must
continue treating those identifiers as unverified hints.

## B.1 `POST /cookie_sync` request

```json
{
  "bidders": ["bidderA", "bidderB"],
  "gdpr": 0,
  "gdpr_consent": "<TCF string>",
  "us_privacy": "<4-char string>",
  "gpp": "<GPP string>",
  "gpp_sid": "2,6",
  "coopSync": true,
  "limit": 5,
  "filterSettings": {
    "iframe": { "bidders": "*", "filter": "include" },
    "image": { "bidders": ["x"], "filter": "exclude" }
  },
  "account": "<account id>",
  "debug": false
}
```

The redirect filter JSON key is `image`. `bidders` is `"*"` or a string
array. `limit`: request → account `default_limit` → unlimited, capped by
account `max_limit`.

**Sync `gpp_sid` applicability grammar.** Both `/cookie_sync` and `/setuid`
accept a comma-separated list of ASCII positive IDs `1..=65535`, or the
standalone literal `-1` meaning no section applies. Missing/empty means
unspecified applicability and is not equivalent to `-1`. Reject zero, other
negative values, `-01`, mixed/repeated sentinel lists (`-1,2`, `-1,-1`), empty
components, plus signs, and whitespace. Positive IDs may have leading zeroes
or duplicates; count all entries toward the 64-entry limit before allocation.
Preserve the decoded raw string for macros and exact `SkipWhen` comparison.
These are applicability signals, not envelope IDs: the GPP envelope parser
continues to accept positive section IDs only. Standalone `-1` supplies wire
GDPR No and activates no GPP section; like other valid nonempty SID inputs it
overrides legacy GDPR, even an invalid legacy value. It does not bypass bad
GPP-envelope/resource checks, activity controls, or other independent gates.
The [IAB applicability contract](https://github.com/InteractiveAdvertisingBureau/Global-Privacy-Platform/blob/main/implementation.md#34-applicable-sections)
defines the sentinel; the wider positive range and rejection of mixed sentinel
lists are Auctioneer rules rather than claims of literal signed-int8 parser parity.

**`gdpr` wire-vs-API coercion** (round-38 L6 + round-46 H1). The wire
format here is a **JSON number `0 | 1`**; the §7.8 API surface
(`SyncPrivacyInput.gdpr`) takes `GdprSignalIn { Absent, Zero, One,
Invalid(String) }`. The handler (`auctioneer-core`) coerces wire bytes
into this enum without rejecting at the wire surface:

- JSON number `0` → `GdprSignalIn::Zero`.
- JSON number `1` → `GdprSignalIn::One`.
- Field absent / JSON `null` → `GdprSignalIn::Absent`.
- Any other JSON shape (string, other number `2`, object, etc.) →
  `GdprSignalIn::Invalid(<raw wire string>)` (round-46 H1 — defer the
  400 decision to `resolve_sync_privacy`, which checks GPP/SID
  precedence first). The handler does NOT 400 at the wire surface.
  If `resolve_sync_privacy` ultimately returns
  `Err(MalformedConsent { scope: "gdpr" })` — i.e. GPP did not
  override — the caller emits **`400`** body `GDPR signal should be
integer 0 or 1` (cookie_sync) or `the gdpr query param must be
either 0 or 1. You gave <value>` (/setuid;). Matches
  Upstream reference cookie-sync handling + reference UID-write handling where
  legacy `gdpr` parse only matters if GPP did not already resolve
  the signal.

`/setuid` (B.4) sends `gdpr` as a **query-string** value, conventionally
the string `"0"` or `"1"`. Coerce `"0"`/`"1"` -> `Zero`/`One`;
absent or exactly empty (`gdpr=`) -> `Absent`; any other nonempty value,
including whitespace, -> `GdprSignalIn::Invalid(<raw>)`. Empty acceptance
is specific to the query surface: a JSON string `""` in `/cookie_sync`
remains invalid. The handler does **NOT** emit `400`
at the query-parse surface (round-46 H1 + round-47 M2 — corrects
earlier "/setuid 400s on the wire" wording); the decision is deferred
to `resolve_sync_privacy`, which only rejects malformed legacy `gdpr`
when GPP/SID did not already resolve the signal (matches upstream
reference UID-write handling). If `resolve_sync_privacy` returns
`Err(MalformedConsent { scope: "gdpr" })`, the caller emits **`400`**
with body `the gdpr query param must be either 0 or 1. You gave
<raw>` (round-44 M5).

## B.2 `/cookie_sync` response

```json
{
  "status": "ok",
  "bidder_status": [
    {
      "bidder": "bidderA",
      "no_cookie": true,
      "usersync": { "url": "<sync url>", "type": "redirect" }
    }
  ],
  "debug": [{ "bidder": "bidderA", "error": "<message>" }]
}
```

`status` = `ok` iff the request cookie has ≥1 live sync, else `no_cookie`.
`debug[]` only when request `debug=true`. Content type
`application/json; charset=utf-8`.

**Byte-exact body contract** (round-51 M3 — matches upstream
reference cookie-sync handling JSON response encoding,
pinned by reference cookie-sync handling fixtures): the body is the JSON
above **plus a single trailing `\n`** appended by the reference's
`json.Encoder.Encode`. Auctioneer emits the same trailing newline for
byte-equivalence. **HTML escaping is disabled** (upstream uses
`SetEscapeHTML(false)`); raw `<`, `>`, and `&` characters in
`bidder_status[].usersync.url` are emitted unescaped — auctioneer must
match.

## B.3 `/cookie_sync` per-bidder `evaluate` (ordered, short-circuit)

**Bidder-name normalization** (round-44 L6): each requested bidder name
is **ASCII-lowercased** before registry / syncer-table lookup and
before `filterSettings.bidders` membership testing — matches upstream
reference syncer selection (`strings.ToLower` in the chooser before
registry consult). The original-case value is preserved only in
`bidder_status[].bidder` of the response (so clients see what they
requested). The same case-folding applies to `filterSettings.bidders`
allow/block lists when matching.

1. Unknown bidder name → `StatusUnknownBidder`.
2. Syncer lookup miss → `StatusUnknownBidder` / `StatusUnconfiguredBidder`.
3. Syncer-key already used this request → `StatusDuplicate`.
4. `filterSettings` rejects the available sync type → `StatusRejectedByFilter`.
5. Bidder already has a live sync → `StatusAlreadySynced`.
6. `syncUser` activity denied → `StatusBlockedByPrivacy`.
7. `BidderSyncAllowed` (GDPR) false → `StatusBlockedByPrivacy`.
8. Syncer `enabled == false` → `StatusBlockedByDisabledUsersync`.
9. `SkipWhen` (GDPR in scope OR an **exact-string match** of the raw request `gpp_sid` against any configured skip-string — _not_ parsed-section membership) → `StatusBlockedByRegulationScope`. Configured skip values are stored as strings (matches reference bidder metadata syncer config + reference syncer selection exact comparison; round-15 finding M5).
10. else → `StatusOK`.

Candidate ordering (round-22 finding M4 — matches upstream
reference bidder syncer selection):

- **Requested non-empty, cooperative OFF:** `shuffle(requested)` **only**
  — `available` is _not_ appended; auctioneer does not emit syncs for
  unrequested bidders.
- **Requested non-empty, cooperative ON:** `shuffle(requested)` ++
  `shuffle(priority_group[0])` ++ `shuffle(priority_group[1])`… ++
  `shuffle(available)`.
- **Requested empty, cooperative ON:** `shuffle(requested = ∅)` ++
  `shuffle(priority_group[0])` ++ `shuffle(priority_group[1])`… ++
  `shuffle(available)` — **priority groups always precede `available`**
  (round-30 finding M4; matches reference bidder syncer selection).
- **Requested empty, cooperative OFF:** `shuffle(available)` only.

Shuffles use the injected `Rng` (§5.7); dedup by bidder name. **`limit`
counts chosen (StatusOK) syncers only**: rejected/disqualified candidates
(unknown, already synced, blocked by privacy, etc.) do not consume the
quota — the loop continues until `chosen.len() == limit` or candidates
are exhausted (matches upstream's `Limit` semantics; round-13 finding L8).

**Priority-group composition** (round-26 M7 + round-27 finding M4 —
explicit precedence and presence semantics):

- The field type that resolves into the composed list is `Option<Vec<...>>`,
  **not** `Vec<...>`. This matters because the merge-patch account
  resolution (§9.1) merges into the embedded default-account config; if
  that default materializes a sentinel `priority_groups: []`, a
  presence-by-non-emptiness rule would never fall back to host.
- **Resolution rule:** account `cookie_sync.priority_groups` is used
  **iff `Some(_)`** (the account explicitly set the key, possibly to
  an empty list — which means "no priority groups, override host");
  else host `user_sync.priority_groups`. There is no merge of the two
  lists — account wholesale-replaces host when present.
- The composed list is the single source for both /cookie_sync candidate
  ordering and /setuid `PriorityBidderEjector` (Appendix B.6).
- This is **simpler than upstream**, which gates account `priority_groups`
  on a specific account cookie-sync condition (§15 already records the
  divergence). The composed-list rule above is the auctioneer contract;
  callers and tests assert it.

## B.4 `GET /setuid` parameters

| Param                  | Required | Purpose                                                                    |
| ---------------------- | -------- | -------------------------------------------------------------------------- |
| `bidder`               | yes      | bidder → syncer-key; unknown → `400`                                       |
| `uid`                  | no       | UID to store; empty → unsync                                               |
| `gdpr`, `gdpr_consent` | no       | legacy GDPR signal/consent; missing/empty `gdpr` is ambiguous, not invalid |
| `gpp`, `gpp_sid`       | no       | GPP (precedence over legacy); SID grammar including standalone `-1` in B.1 |
| `f`                    | no       | `i` (1×1 PNG) / `b` (empty HTML); absent → syncer default                  |
| `account`              | no       | account config; empty → `unknown`                                          |

## B.5 `uids` cookie wire format

```
Cookie: uids=v1.<key-id>.<base64url( JSON )>.<mac>
JSON = { "tempUIDs": { "<syncerKey>": { "uid": "<id>",
                                        "expires": "<RFC3339>" } },
         "optout": false }
```

Per-entry TTL 14 days. Verify the version/key-ID/payload MAC before parsing
JSON; invalid MAC, unknown key or corrupt authenticated payload is 400, not
fresh state. The missing cookie is fresh state. `optout=true` →
all UIDs dropped. Attributes: `Path=/`, `Expires = now + host_cookie.ttl_days`
(default 90d), `Domain` when configured, `Secure` + `SameSite=None` per the
UA rule (§8.5), no `HttpOnly`. Empty writes use `Max-Age=0` and a past
expiry. Complete fields have a 4-KiB ceiling. The M5 integrity addendum
defines key provisioning/rotation and the unverified-UID trust boundary.

## B.6 Max-size eviction (`PriorityBidderEjector`)

When the **full serialized `Set-Cookie` header line** — `uids=<value>` plus
`Expires=…`, `Path=/`, `Domain=…`, `Secure`, `SameSite=None` attributes —
exceeds `host_cookie.max_cookie_size_bytes` (> 0) (round-24 finding L8;
matches reference UID-cookie handling's measurement of the full serialized cookie,
including name, value, `Expires`, and `Path`). Priority groups come from
**the composed list defined in §B.3** (`Option<Vec<...>>`: account
`cookie_sync.priority_groups` when `Some(_)`, else host
`user_sync.priority_groups`), with bidder-name → syncer-key resolution
via the bidder registry — matches reference UID eviction (round-22 L11):

1. Drop the **oldest non-priority** UID (priority = a syncer key whose
   owning bidder name appears in the **composed priority-group list
   from §B.3** — `Option<Vec<...>>` with account `Some(_)`
   wholesale-replacing host; round-28 finding M4 corrects an earlier
   stale reference to `user_sync.priority_groups`).
2. If only priority UIDs remain, drop from the **lowest** priority group
   (oldest within the group if it has several).
3. If exactly one non-priority UID remains and the syncer being set is not a
   priority → respond `200` "cookie not updated", do not write.
4. Single UID still too large → `400`.

---

# Appendix C — Activity Controls & Scrubbing Matrix

## C.1 Activity evaluation

`Component = { type ∈ {bidder, analytics, rtd, general}, name }`. Per
activity: `default` (allow when unset); ordered `rules`; first non-abstaining
rule wins. A rule abstains unless every clause matches (`componentName` /
`componentType`; empty = match-all; case-insensitive). No deny-override.
Upstream v4 activity conditions expose **only** `componentName` /
`componentType` — there is no GPP-section condition (see §7.4).
Case comparison uses Unicode simple folding consistent with the pinned
behavioral reference, not ASCII-only lowercasing or full folding. Activity keys
and field names remain case-sensitive; component type values canonicalize.
Missing `allow` or unknown activity/condition/type is a config error. Runtime
rules replace the inherited array (including `[]` clearing); absent fields
inherit. M4 limits are 128 rules per activity, 128 combined condition entries
per rule, and 128 UTF-8 bytes per name, plus existing global config limits.

## C.2 Auction scrub matrix (per bidder-scoped request)

| Signal / decision                                                  | Scrub applied                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| ------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `fetchBids` denied                                                 | bidder dropped entirely (no request built)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| `transmitUfpd` denied                                              | `scrub_user_fpd` (device ids, user ids/demographics, `user.data`, `user.keywords`, `user.kwarray`, `user.ext.data`, and both native/legacy EID locations)                                                                                                                                                                                                                                                                                                                                                                                                 |
| `transmitUfpd` allowed, GDPR `PassID=false`                        | `scrub_gdpr_id` (device ids — `device.ifa` + the legacy `dpidsha1`/`dpidmd5`/`didsha1`/`didmd5`/`macsha1`/`macmd5` keys; user demographics `yob`/`gender`; user identifiers `user.id` + `user.buyeruid`; and eids in **both** locations — native 2.6 `user.eids` AND legacy `user.ext.eids`). Round-49 H1: the earlier summary named only `user.ext.eids`, which predates native 2.6 `user.eids` and would leave the scrub bypassable via the 2.6 location; and `PassID=false` must not leave a direct user id (`buyeruid`) in the bidder-scoped request. |
| CCPA `ShouldEnforce`, independently of activity decisions          | Identifier/demographic/EID removal plus geo/IP scrub; includes both EID locations, but not broader user FPD unless separately required                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `transmitPreciseGeo` denied                                        | `scrub_geo_and_device_ip` (round geo 2dp, mask IP)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `transmitPreciseGeo` allowed, GDPR `PassGeo=false`                 | `scrub_geo_and_device_ip`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| LMT (`host.lmt.enforce && device.lmt=1`) or COPPA (`regs.coppa=1`) | Identifier/demographic/EID removal plus IP/geo scrub; COPPA also removes all four user-FPD branches and empties existing user/device Geo objects as `{}`, while LMT-only rounds and retains permitted user FPD                                                                                                                                                                                                                                                                                                                                            |
| `transmitTid` denied                                               | `scrub_tid` (`source.tid` + `imp.ext.tid`)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |

The M4 [classified disclosure design](2026-10-09-gdpr-scrub-classification-design.md)
defines one compiled per-object rule inventory, shared by typed slots and
flattened occurrences. Either ID denial or Geo restriction closes unknown
fields and opaque extensions throughout user/device, Geo, EID/UID, supported
device extensions and structured user-agent descendants. Reviewed context and
control fields survive only with their declared shapes; objects cannot hide in
string/number/array context slots. Unknown subtrees are removed as units, not
recursively searched for guessed identifier names. Standard device context:
`ua`, `sua`, `devicetype`, `make`, `model`, `os`, `osv`, `hwv`, `h`, `w`,
`ppi`, `pxratio`, `js`, `geofetch`, `flashver`, `language`, `langb`, `carrier`,
`mccmnc`, and `connectiontype` survive ID denial alongside `atts`/`dnt`/`lmt`.
The typed `device.ext.prebid.interstitial.minwidthperc` and `minheightperc`
rendering constraints also survive. Remove unknown fields in `prebid` and
`interstitial` independently so those objects cannot carry alternate IDs.
Structured user-agent browser/platform brands and versions survive too; clear
their unknown fields and opaque extensions, and those of `sua` itself, so an
identifier cannot bypass the gate through a newly typed object. Field shapes
follow [OpenRTB 2.6 Device/UserAgent/BrandVersion](https://github.com/InteractiveAdvertisingBureau/openrtb2.x/blob/main/2.6.md#objectdevice).
Newly classified context distinguishes missing values from explicit JSON nulls
and preserves empty arrays. `pxratio` retains its JSON number form (integer or
floating point); `h`, `w`, and `ppi` use signed 64-bit integers. Wrong JSON
types are rejected at request parsing with HTTP 400: no string-to-number,
float-to-integer, or number-to-string coercion. No publisher-log evidence has
established a need for lenient conversion. Round-trip fidelity means JSON-value
equality, not preservation of whitespace, property order, or numeric lexemes.
Consent and opt-out controls survive. Independently permitted IP/Geo remain
unchanged except for classified shape/unknown closure under ID denial.
FPD explicitly owns `user.data`, `user.keywords`, `user.kwarray` and
`user.ext.data`; valid outer keyword shapes and opaque owned data survive
ID/Geo restrictions unless `transmitUfpd` is denied or COPPA applies. `customdata` is
not an FPD exception. When both ID and Geo pass, unknowns remain untouched
unless another composed restriction denies them. TID-only denial visits exactly
its owned leaves and does not normalize unrelated source/user/device data.
FPD, CCPA/LMT and COPPA use the same compiled composition; no branch can restore
a value another restriction denied.

For GDPR `PassGeo=false`, coarsen both `device.geo` and `user.geo`.
Preserve reviewed `country`, `region`, `regionfips104`, `city`, `metro`,
`utcoffset`, `type`, `ipservice` and `lastfix` with valid shapes. Coarsen
coordinates and remove `zip`, original `accuracy`, unknown fields and opaque
`Geo.ext`. This supersedes the conservative M3b blanket `Geo.other.clear()`
behavior. COPPA still replaces existing Geo objects with `{}` and does not
fabricate absent objects. Geo-only denial retains typed-Geo precedence over a
same-field flattened shadow; ID-only denial does not remove valid shadow Geo
merely because typed Geo exists.

Geo rounding = lat/lon to 2 decimals (round-half-up, toward +∞;
round-51 M2 — applied over the already-parsed `f64`, so at an
unrepresentable exact half the binary value decides, e.g. `1.005`
scales to `100.4999…` and rounds to `1.00`). IP masking keeps
`account.privacy.ipv4/ipv6.anon_keep_bits` bits via CIDR.
The coordinate transform returns an optional result: non-finite input,
intermediate overflow and invalid/non-finite output are removed, never passed
through or re-emitted as JSON null. This is an intentional change from M3b.
Policies compose as the union of removals; an activity branch cannot skip
CCPA/LMT IP/geo scrubbing, and COPPA full removal dominates rounding. Preserve
ordinary request/imp IDs and unrelated context/extensions. See M4 section 12
for the pinned reference's different negative-coordinate rounding.
This rounding rule is Auctioneer's privacy policy, not an OpenRTB requirement:
[OpenRTB Geo](https://github.com/InteractiveAdvertisingBureau/openrtb2.x/blob/main/2.6.md#objectgeo)
specifies coordinate ranges and accuracy but no privacy-rounding algorithm.
Reference-server truncation after adding `0.5` is not the compatibility target:
Auctioneer retains `-1.238 -> -1.24` and the explicit tie rule `-1.125 -> -1.12`.

---

# Appendix D — GPP Precedence, Errors & Legacy Downgrade

## D.1 Precedence

For M4 value selection, a section is eligible only when present in a valid
envelope and active in `gpp_sid`. An inactive payload cannot select or backfill
consent. Empty/invalid active TCF remains authoritative; empty USPV1 differs
as specified below. Missing/empty SID retains legacy GDPR scope resolution.

| Signal                   | Rule                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| ------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| GDPR applies             | non-empty `gpp_sid` → section 2 present ⇒ `Yes`, absent ⇒ `No`; **`regs.gdpr` ignored**. Else `regs.gdpr`. Else `Ambiguous`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| GDPR consent             | GPP section 2 value if present; else `user.consent`. **This row is the FULL (M4) precedence.** **M3 behaviour (round-15 H4 / round-76 B1 — corrected):** M3 has no GPP decoder, and GPP section 2 WINS over `user.consent` (§7.4), so when `regs.gpp_sid` contains **section 2** M3 does NOT fall back to `user.consent` — it marks consent UNUSABLE (`consent_usable = false` → `default_permissions()`, `PassID: false`), so a possibly-overridden legacy `user.consent` can never grant. M3 reads `user.consent` ONLY when section 2 is absent. The wire-signal path already consults `regs.gpp_sid`, so "GDPR applies" is correct in M3. Full GPP section-2 extraction + the legacy-vs-GPP conflict warning that would let M3 HONOUR section-2 consent ship in M4. (This is NOT "misclassify as no-consent"; it is deliberate fail-closed nullification of an undecodable authoritative signal.) |
| US-Privacy               | Nonempty GPP USPV1 value **only if section 6 ∈ `gpp_sid`**; else `regs.us_privacy`. Empty/absent active section 6 falls back without a value-conflict warning or backfill; nonempty invalid/whitespace USP does not fall back. Both nonempty and different → GPP wins + non-fatal warning, without disabling CCPA. **M4**, not M3.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Conflict (legacy vs GPP) | non-fatal validation warning; GPP value used. **This row is M4 (round-77 H2).** It requires decoding the GPP section value to compare it against the legacy field, which M3 has no decoder for. **M3 emits NO legacy-vs-GPP _value_ conflict warning**; the only conflict M3 raises is the structural `regs.gpp_sid`-vs-`regs.gdpr` routing conflict (Appendix D.1 "GDPR applies" row / §7.6), which is section-id routing, not string decode.                                                                                                                                                                                                                                                                                                                                                                                                                                                       |

## D.2 GPP error behavior (review finding 6)

For M4 auctions, recoverable envelope syntax/unsupported-version errors and
terminal limits are distinct. The malformed-envelope fallback below applies
only within bounds. Exceeding 256 KiB raw GPP, a 4 KiB encoded header, 64
section entries, section ID 65535, or 524288 charged header operations, or
checked decode overflow, is `PrivacyError::GppLimitExceeded` mapped to `400`
before adapter construction, even if enforcement is disabled or legacy consent
would grant. Fixed diagnostic: `regs.gpp exceeds privacy parsing limits`.
Reservation exhaustion separately follows M3b's terminal resource error path.
An active section-2 value that is empty or invalid remains authoritative and
cannot reactivate a legacy grant; this differs from an absent section or a
recoverably malformed envelope. M4 design sections 5 and 9 specify provenance,
warning scopes/order, and all exact boundary cases. Sync semantics remain M5.

| Input                                                                        | Auction (`/openrtb2/auction`)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | Sync (`/cookie_sync`, `/setuid`) |
| ---------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------- |
| Malformed `gpp` string                                                       | **M4 auction behaviour (round-77 H2):** non-fatal `regs.gpp` well-formedness warning (code 10001); **only section-decoded values** (the TCFEU2 consent string, the USPV1 value) fall back to legacy fields. **`gpp_sid` itself remains trusted** — it is a separate `regs.gpp_sid` field, parsed independently. So malformed `gpp` with `gpp_sid=[2]` still puts GDPR in scope (§7.1). **M3 auction behaviour:** M3 has no GPP decoder, so it emits **NO malformed-`gpp` well-formedness warning** (`regs.gpp` is passed through un-decoded) — and because `gpp_sid=[2]` puts section 2 in scope, M3 marks consent UNUSABLE (`consent_usable = false`) rather than falling back to `user.consent` (see the D.2 M3 note below). | `400`                            |
| Malformed / non-numeric `gpp_sid`                                            | **`400`** (round-22 finding M6 — `regs.gpp_sid` is a typed integer array; non-numeric JSON fails _parse_, not validation, so this is a hard parse error in auctioneer's typed path, matching upstream's pre-unmarshal failure)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     | `400`                            |
| Unsupported section id in `gpp_sid` (3, 7–12)                                | passed through; not decoded; usable for `SkipWhen.gpp_sid` matching                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            | same                             |
| `gpp_sid` names a section absent from the (successfully) parsed `gpp` string | **SID still drives the signal** — e.g. `gpp_sid=[2]` with no section 2 in `gpp` still puts GDPR in scope (§7.1, matches upstream's `GetGDPR`). Only the per-section _value_ falls back: TCFEU2 consent → `user.consent`; USPV1 → `regs.us_privacy`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                            | same                             |
| Section present in `gpp` but its id absent from `gpp_sid`                    | section ignored (not "active")                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 | same                             |

**M3 behaviour note (round-76 B1).** The "TCFEU2 consent string falls
back to `user.consent`" outcomes above are the FULL (M4) behaviour. In
M3 there is no GPP decoder and section 2 is authoritative (§7.4), so
whenever `gpp_sid` contains **section 2** — including the malformed-`gpp`
and section-absent-from-`gpp` rows — M3 does NOT fall back to
`user.consent`; it sets `consent_usable = false` (→ `default_permissions()`,
`PassID: false`). The fallback-to-`user.consent` for TCFEU2 lands with
the M4 GPP decoder. (`gpp_sid` parse errors → `400` are unchanged; USPV1
/ section 6 is separate and M4.)

These outcomes are pinned to the behavioral baseline: `/cookie_sync`
rejects invalid GPP envelopes or section-ID encoding with `400`;
`/setuid` rejects invalid section-ID encoding with `400` and applies the
specified GPP-versus-legacy signal precedence. M5 verifies absent `gpp`,
`gpp_sid` without `gpp`, and fallback behavior using pinned reference
fixtures, not source-function correspondence.

## D.3 Legacy downgrade — fills empty fields only

For bidders without `openrtb.gpp_supported`, before sending: set `regs.gdpr`
**only if absent and `gpp_sid` is present**, to `1` if SID contains 2 and `0`
otherwise (including a present-empty list). Set empty/missing `user.consent`
from a present active section-2 value, and empty/missing `regs.us_privacy`
only from a valid nonempty active section-6 value. Validate/clear invalid
legacy USP first; otherwise populated legacy fields are never overwritten.
Preserve original GPP/SID and derive permissions before projection, never
from backfilled fields. Raw consent copying is not an additional authorization
parse or a permission grant.

Decoded/enforced sections: **2 (TCFEU2)**, **6 (USPV1)**. Sections 3, 7–12 are
passed through, not decoded or enforced (open item 4).

---

# Appendix E — Auction Response & Targeting

Behavior-compatible with the pinned baseline (`exchange/`).

## E.1 Price granularity presets

All presets use precision 2.

| Preset                     | Ranges (min, max, increment)              |
| -------------------------- | ----------------------------------------- |
| `low`                      | (0, 5, 0.5)                               |
| `med` / `medium` (default) | (0, 20, 0.1)                              |
| `high`                     | (0, 20, 0.01)                             |
| `auto`                     | (0, 5, 0.05), (5, 10, 0.1), (10, 20, 0.5) |
| `dense`                    | (0, 3, 0.01), (3, 8, 0.05), (8, 20, 0.5)  |

`ext.prebid.targeting.pricegranularity` accepts a preset string or a custom
object `{ precision, ranges: [{min, max, increment}] }`. Custom:
**`precision` is optional — defaults to `2`** (matches reference configuration defaults;
round-19 finding M6), allowed range `0..=15`; ranges ordered by strictly
increasing `max`; every `increment > 0`. Range `min` defaulting per the
previous-range rule (§E.3 / round-18 M8). Optional
`mediatypepricegranularity` overrides per banner/video/native.

Retained custom ranges must have finite nonnegative bounds and positive normal
increments, with finite division and upward-rounded bucket arithmetic across
the full range. Successive ranges meet exactly at the previous maximum; an
overlap, interior gap, or unsafe arithmetic makes the configuration fall back
to `med`. This also applies to directly constructed custom configurations.
Precision is capped at 15. The first minimum may be above zero; prices below
that explicitly chosen minimum have no bucket. Individual malformed/reversed
rows are dropped without advancing the implicit minimum.

An invalid global or media-specific setting that falls back to `med` emits
`InvalidPriceGranularity` (code 10021) in the auction's global warning group.
The diagnostic identifies only the fixed setting path, never rejected input.
Each of the five supported settings (global, banner, video, native, audio) is
resolved once during targeting, so warnings are bounded independently of bid count.
Safe row drops that still produce a valid custom configuration do not warn.
Warnings use the existing capped response writer and do not enlarge its budget.
Pre-fanout deadline short-circuits do not resolve targeting or use a granularity
fallback and therefore do not emit this warning; an empty bidder response on
the normal targeting path still does.

## E.2 Bucket computation

For a bid CPM, with the (possibly media-type-overridden) granularity config:

- `bucket_max` = the largest `max` across ranges.
- `cpm > bucket_max` → bucket string = `bucket_max` at `precision` decimals.
- Else find the range with `min <= cpm <= max` (bounds inclusive; on an exact
  boundary the later range wins). `increments = (cpm - range.min) /
range.increment`, rounded per the account `bid_rounding` mode; `bucket =
round(increments) * range.increment + range.min`, formatted at `precision`
  decimals.
- No matching range (e.g. negative CPM) → empty string.

`bid_rounding` modes: `down` / **default** → floor; `up` → ceil; `true` →
nearest; `timesplit` → random floor/ceil per bid, drawn from the injected
`Rng` (§5.7) — seeded under test for determinism.

## E.3 Targeting keys

Prefix from `ext.prebid.targeting.prefix` → account `targeting_prefix` →
`"hb"`. Key suffixes: `_pb`, `_bidder`, `_size`, `_deal`, `_format`, `_env`,
`_cache_id`, `_uuid`, `_cache_host`, `_cache_path`, `_pb_cat_dur`.

- **Winner key** = `prefix + suffix` — emitted only when `includewinners` and
  the bid is the overall imp winner.
- **Per-bidder key** = `prefix + suffix + "_" + bidder` — emitted when
  `includebidderkeys`, or when `alwaysincludedeals` and the bid has a deal.
- **Defaults (round-17 M7 + round-18 M8).** When `ext.prebid.targeting` is
  present but `includewinners` / `includebidderkeys` are unset, both
  default to `true` (matches reference configuration defaults). Price granularity
  defaults to `med` (§E.1) with `precision` defaulted to `2`. For custom
  ranges, **each range's `min` defaults to the previous range's `max`**
  (the first range's `min` defaults to `0`) — matches upstream
  reference configuration defaults; auctioneer previously defaulted all mins to `0`, which was
  wrong for multi-range custom granularity.
- **Truncation:** keys cut to `account.truncate_target_attribute` (if set and
  ≥ `MinKeyLength` 12), else `MaxKeyLength` 20. A prefix that cannot fit
  alongside `MinKeyLength` falls back to `"hb"` with a warning. Truncation
  respects **UTF-8 char boundaries**: bidder / alias names carry through
  as arbitrary UTF-8 (`BidderName` has no ASCII invariant; §5.9 sorts
  UTF-8 names), so the byte cutoff drops back to the nearest char
  boundary before slicing. Naive `String::truncate` panics mid-multibyte
  — that would take the auction handler down for any non-ASCII alias
  that lands the cutoff inside a multibyte codepoint.
- Per-bid key set: `hb_pb` (rounded price, **only when nonempty**), `hb_bidder`, `hb_size`
  (`WxH`, only when both non-zero), `hb_deal` (when `DealID` non-empty),
  `hb_format` (when `includeformat`), `hb_env` (`mobile-app` for app
  requests, `amp` for AMP), cache keys when cached. Stored on
  `bid.ext.prebid.targeting`. `setTargeting` runs only when
  `includewinners || includebidderkeys || includeformat`.
  When no price range matches, omit both winner and per-bidder price keys;
  retain the bid, winner identity, and all other applicable targeting keys.

## E.4 Winner selection

Per imp, the winner is chosen by `is_new_winning_bid`:

- With `preferdeals`: a bid **with** a deal beats a bid **without** one
  regardless of price.
- Otherwise (or tie on deal status): strictly higher `bid.price` wins; ties
  keep the incumbent. Because auctioneer processes bidders in deterministic
  registry order (Appendix E.7), the incumbent on a tie is well-defined.

## E.5 `BidResponse` shape & empty/failed cases (review finding 10)

`cur` rule: `"USD"` iff `seatbid` is non-empty, else `""`.

| Case                                                                      | Status | `seatbid`                       | `nbr`                 | `cur`   | `ext`                             |
| ------------------------------------------------------------------------- | ------ | ------------------------------- | --------------------- | ------- | --------------------------------- |
| Invalid request (parse / validation)                                      | `400`  | — (plain-text body)             | —                     | —       | —                                 |
| Malformed account config                                                  | `500`  | — (plain-text body)             | —                     | —       | —                                 |
| Account disabled                                                          | `503`  | — (plain-text body)             | —                     | —       | —                                 |
| No live bidders (all dropped by split / privacy / limits before dispatch) | `200`  | `[]`                            | `NoBidInvalidRequest` | `""`    | `ext.warnings` per dropped bidder |
| Live bidders, all transport-failed                                        | `200`  | `[]`                            | unset                 | `""`    | `ext.errors` per bidder           |
| Live bidders, responded with no bids                                      | `200`  | `[]`                            | unset                 | `""`    | —                                 |
| Bids returned but all currency-dropped (non-USD)                          | `200`  | `[]`                            | unset                 | `""`    | `ext.warnings` per dropped seat   |
| Normal                                                                    | `200`  | one seat per bidder with ≥1 bid | unset                 | `"USD"` | `errors` / `warnings` if any      |

**`NoBidInvalidRequest` for the no-live-bidders row** mirrors upstream's
behavior verbatim: reference auction processing sets the invalid-request no-bid reason
whenever `len(liveAdapters) == 0` — regardless of _why_ (invalid input,
privacy drop, or resource cap). This is verified against the pinned baseline,
not a auctioneer choice.

- `seatbid` — one `SeatBid` per bidder that produced ≥1 bid; `seatbid.group`
  = `0` always. **Losing (non-winning) bids remain** in the seatbid; only the
  winner gets the un-suffixed winner keys. Bids within a seatbid are **not**
  price-sorted (adapter return order preserved).
- `bid.ext.prebid` = `{ type, targeting, meta (always present), bidid,
dealpriority, dealtiersatisfied, targetbiddercode }`; plus top-level
  `bid.ext.origbidcpm` / `origbidcur` when known.
- `ext` (`ExtBidResponse`): `debug` (when permitted+requested, §5.6),
  `errors` / `warnings` (`map<bidder, [{code,message}]>`),
  `responsetimemillis` (`map<bidder, int>`), `tmaxrequest` (= `tmax`),
  `prebid` (`{ auctiontimestamp, seatnonbid, ... }` when non-empty).

## E.6 `seatnonbid` & non-bid reason codes

`ext.prebid.seatnonbid` = `[{ seat, nonbid: [{ impid, statuscode, ext }] }]`.
**Emitted only when `req.ext.prebid.returnallbidstatus == true`** on the
request, matching reference auction processing behavior (round-12 finding H5).
When the flag is set and the builder is non-empty, the array is attached;
otherwise the field is omitted entirely.

Reason codes: 100 Error-General, 101 Error-Timeout, 103 Error-Bidder-
Unreachable, 300 Rejected-General, 301 Below-Floor, 303 Category-Mapping-
Invalid, 304 Below-Deal-Floor, 351 Creative-Size-Not-Allowed, 352 Creative-
Not-Secure. Phase 1 emits 100/101/103 (transport) and 300 (general
rejection); floor and creative codes arrive with their later phases.
**Mapping from `ext.errors` codes:** only **code 1** (`Timeout`, per-bidder)
maps to `statuscode 101`; **code 12** (`TmaxTimeout`, auction-wide) is
auction-level only and does not produce per-imp `seatnonbid` entries
(matches reference no-bid reason mapping).

## E.7 Deterministic serialization (review finding 12)

Unlike upstream (unspecified map iteration order), auctioneer is fully deterministic:

- Bidders are processed in **registry order** as defined in §5.9
  (single lexicographic sort over base names ∪ request-defined alias
  names ∪ config alias names participating in this auction; round-40
  M2). Fan-out `calls`, winner tie-breaking, and per-bidder iteration
  all use this ordering.
- `seatbid` is serialized **sorted by `seat`**.
- The `ext.errors`, `ext.warnings`, and `ext.responsetimemillis` maps are
  serialized **sorted by key**.
- Given identical input, mocks, `FakeClock`, and a fixed `Rng` seed, the
  `BidResponse` is byte-identical — snapshot E2E tests are stable.

---

# Appendix F — Error Response Formats

Hard HTTP failures return **plain text**; structured per-bidder errors live
inside the `200` JSON body's `ext`.

| Case                                                                                                                                                                                                                        | Status                               | Content-Type                                                                | Body                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------ | --------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Auction inbound `Content-Encoding` non-empty (gzip/br/etc.)                                                                                                                                                                 | `400`                                | `text/plain; charset=utf-8`                                                 | `Invalid request: Content-Encoding of type <encoding> is not supported\n` (round-68 M5 — trailing `\n` appended by reference auction handling writer). Exact reference auction handling / plain-text error output wording (round-29 finding L6, corrects round-28's approximation). Checked _before_ body read (round-25 finding H2).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `/cookie_sync` inbound `Content-Encoding` non-empty (gzip/br/etc.)                                                                                                                                                          | `400`                                | `text/plain; charset=utf-8`                                                 | `Invalid request: Content-Encoding of type <encoding> is not supported\n` (intentional divergence per §15, same body as the auction row above). The admitted handler checks all encoding values _before_ the declared-length check and body read; M5's runtime config extraction may precede these application checks. This is not supplied by route registration or the aux admission hook.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| Unmatched path OR wrong-method with non-empty `Content-Encoding`                                                                                                                                                            | `400`                                | `text/plain; charset=utf-8`                                                 | `Invalid request: Content-Encoding of type <encoding> is not supported\n` (round-69 M3 — Content-Encoding rejection short-circuits the route-mapping in the ingress hook, so `POST /missing` with `Content-Encoding: gzip` returns 400, NOT 404; `GET /openrtb2/auction` with `Content-Encoding: gzip` returns 400, NOT 405).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| Auction body unreadable                                                                                                                                                                                                     | `400`                                | `text/plain; charset=utf-8`                                                 | `Invalid request: <err>\n` (round-68 M5 — trailing `\n` from reference auction handling writer)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| Auction body oversize (Content-Length over 256 KiB OR bounded stream exceeds cap)                                                                                                                                           | `400`                                | `text/plain; charset=utf-8`                                                 | `Invalid request: request size exceeded max size of 262144 bytes.\n` (round-68 M5 — exact bytes; trailing `\n`)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `/cookie_sync` body oversize (Content-Length over 32 KiB OR bounded stream exceeds cap)                                                                                                                                     | `400`                                | `text/plain; charset=utf-8`                                                 | `cookie_sync request size exceeded max size of 32768 bytes.\n` (round-68 M5)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| Default 4 KiB cap exceeded (unmatched path, `/status` with body, etc.)                                                                                                                                                      | `400`                                | `text/plain; charset=utf-8`                                                 | `request size exceeded max size of 4096 bytes.\n` (round-68 M5)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| Auction JSON parse failure                                                                                                                                                                                                  | `400`                                | `text/plain; charset=utf-8`                                                 | `Invalid request: <err>\n` (round-70 L9 — trailing `\n` from reference auction handling writer, matches the body-oversize row)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| Auction validation failure                                                                                                                                                                                                  | `400`                                | `text/plain; charset=utf-8`                                                 | `Invalid request: <err>\n` (one line per error, each line ends with `\n`; round-70 L9)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| Auction: invalid runtime blob, including account data                                                                                                                                                                       | `500`                                | `text/plain; charset=utf-8`                                                 | `Malformed host configuration`; whole-blob extraction/validation precedes application account/body work (§9.1). Separate account-record failure mappings are M8 design work.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| Auction: account disabled                                                                                                                                                                                                   | `503`                                | `text/plain; charset=utf-8`                                                 | `Invalid request: <err>\n` (round-70 L9 — trailing `\n`)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| Auction: admission full (M3b — the Prereq-10 INGRESS lease try-acquire refuses, pre-body, pre-routing-dispatch; round-126 #8 retires the "first handler action" phrasing — handler-first survives only as the dev fallback) | `503`                                | `text/plain; charset=utf-8`                                                 | `overloaded\n` (round-98 M9 — the exact overload wire shape; auctioneer-owned, no upstream analogue since upstream has no admission gate — §15 divergence note)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `/status`, `/info/bidders`, `/info/bidders/{name}`: aux gate full (M3b — `aux_gate` try-acquire `None`; the FIRST action on every config-consuming non-auction route, so overload PRECEDES every other rejection)           | `503`                                | `text/plain; charset=utf-8`                                                 | `overloaded\n` (round-100 B5 — same shape as the auction row)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| Auction: egress-wait deadline expired (M3b, round-107 B2 — the auction COMPLETED but no egress permit became available within the response-write deadline; the typed response is dropped un-serialized)                     | `503`                                | `text/plain; charset=utf-8`                                                 | `overloaded\n` (same wire shape as admission-full — both are isolate-capacity overload; auctioneer-owned, §15 divergence note)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| Sync: invalid runtime blob, including account data                                                                                                                                                                          | `500`                                | `text/plain; charset=utf-8`                                                 | `Malformed host configuration`; even an opted-out request cannot bypass current blob validation (§9.1). Separate account-record failure mappings are M8 design work.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| Sync: account disabled                                                                                                                                                                                                      | `400`                                | `text/plain; charset=utf-8`                                                 | `account is disabled, please reach out to the prebid server host` (upstream wording)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| Auction success (incl. all-bidders-failed)                                                                                                                                                                                  | `200`                                | `application/json` (no charset)                                             | `BidResponse`; per-bidder issues in `ext.errors` / `ext.warnings`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| any ingress route — admitted header caps exceeded (M3b)                                                                                                                                                                     | `431`                                | `text/plain; charset=utf-8`                                                 | `too many header bytes\n` (round-125 #10; HTTP/1.x wire shape — an HTTP/2 field-section breach is a protocol-level stream reset per RFC 9113 §4.3, no body is sent — round-128 #3)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| any ingress route — request-target over `max_request_target_bytes` (M3b)                                                                                                                                                    | `414`                                | `text/plain; charset=utf-8`                                                 | `uri too long\n` (round-130 #6; HTTP/1.x wire shape via the static class — on HTTP/2 the oversized :path/:authority is rejected at header decode as a stream reset, no body)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| any ingress route — ingress-read deadline expired (M3b)                                                                                                                                                                     | `408`                                | `text/plain; charset=utf-8`                                                 | `request timeout\n` (round-125 #10)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `GET /status` — default (`host.status_response` empty)                                                                                                                                                                      | `204`                                | _(none)_                                                                    | empty body                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| `GET /status` — configured (`host.status_response` non-empty)                                                                                                                                                               | `200`                                | _(none)_                                                                    | the exact bytes of `host.status_response`; no synthesized `Content-Type`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `GET /status` — **DEGRADED: runtime config unloadable, malformed or unfunded**                                                                                                                                              | `200` — **never `204`, never `500`** | _(none)_                                                                    | The baked `host.status_response` bytes (empty if unset), followed by `config: degraded (<code>)\n`, where `<code>` is one of `malformed_config`, `store_key_rejected`, `store_unavailable`, `unfunded_extraction`. Refused store contents are unavailable and never echoed. A single `\n` separator precedes the diagnostic only when the baked banner is non-empty and does not already end in `\n`. `RuntimeConfigError::detail` is never published. See §6.1 and the approved R0 initialization boundary.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| `GET /status` — **every answer, all three rows above**                                                                                                                                                                      | _(as above)_                         | _(as above)_                                                                | header `x-auctioneer-config-status: ok \| degraded`, plus the milestone stamp header. The header is the machine-readable channel and is the ONLY degraded signal on the `204` shape                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `GET /info/bidders` success                                                                                                                                                                                                 | `200`                                | `application/json`                                                          | JSON array of bidder names per section 8.1; no trailing newline                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `GET /info/bidders` bad query (`enabledonly` / `baseadaptersonly` not `true`/`false`)                                                                                                                                       | `400`                                | _(none)_                                                                    | `Invalid value for 'enabledonly' query param, must be of boolean type` or `Invalid value for 'baseadaptersonly' query param, must be of boolean type`; no trailing newline                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| `GET /info/bidders/{name}` success                                                                                                                                                                                          | `200`                                | `application/json`                                                          | bidder-detail JSON per section 8.1; no trailing newline                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `GET /info/bidders/{name}` unknown bidder                                                                                                                                                                                   | `404`                                | _(none)_                                                                    | empty body (round-68 M6 — matches upstream; the special `all` name returns the aggregate, never 404)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `cookie_sync` request error                                                                                                                                                                                                 | `400`                                | `text/plain; charset=utf-8`                                                 | Raw parser error message plus a trailing newline; no `Invalid request:` prefix. Examples: `JSON parsing failed: <err>`; `error parsing filtersettings.iframe: invalid bidders type. must either be a string '*' or a string array`; `error parsing filtersettings.image: invalid filter value '…'. must be either 'include' or 'exclude'`; `GDPR signal should be integer 0 or 1`. Empty consent under `parsed_request_signal == Yes` or (`Ambiguous` + `normalized_request_signal == Yes`), and invalid `gpp`/`gpp_sid`, use their documented parser error messages (section 7.8.1).                                                                                                                                                                                                                                                                                                                                                                                                       |
| `cookie_sync` user opted out                                                                                                                                                                                                | `401`                                | `text/plain; charset=utf-8`                                                 | `User has opted out\n`; also sets `X-Content-Type-Options: nosniff`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `cookie_sync` success                                                                                                                                                                                                       | `200`                                | `application/json; charset=utf-8`                                           | Appendix B.2 shape, JSON plus trailing `\n`; raw `<`/`>`/`&` in syncer URLs are emitted unescaped                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `setuid` parameter/consent error                                                                                                                                                                                            | `400`                                | _(none)_                                                                    | Exact bodies without a trailing newline or `Content-Type`: `"bidder" query param is required` (missing/empty bidder); `The bidder name provided is not supported by Prebid Server` (unknown bidder); `"f" query param is invalid. must be "b" or "i"` (invalid format); `the gdpr query param must be either 0 or 1. You gave <value>`; `GDPR consent is required when gdpr signal equals 1` (empty consent under `parsed_request_signal == Yes` only); `gdpr_consent was invalid. <parser error>` (permission-path rejection only when `host_vendor_id != 0`, `normalized_request_signal == Yes`, `gdpr_permissions_enabled`, and activity allowed); `invalid gpp_sid encoding, must be a csv list of integers`; `error parsing GPP header, header must have type=3`. Permission-path errors follow the activity check, so a `syncUser` denial wins with `451`. Empty consent under `Ambiguous` + normalized Yes is a permission-path outcome, not a request-extraction error.             |
| `setuid` disabled syncer (`SyncDecision::BlockedByDisabledUsersync` returned by `setuid_allowed`)                                                                                                                           | `400`                                | _(none)_                                                                    | `The bidder syncer is disabled`; no trailing newline or `Content-Type`. Intentional difference (section 15): both sync endpoints enforce disabled-syncer policy, with `/cookie_sync` using `sync_allowed` and `/setuid` using the narrower `setuid_allowed` host-cookie GDPR and `enabled` checks.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| `setuid` opted out                                                                                                                                                                                                          | `401`                                | _(none)_                                                                    | empty body; no `Content-Type`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| `setuid` activity `syncUser` denied                                                                                                                                                                                         | `451`                                | _(none)_                                                                    | empty body                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| `setuid` GDPR host-cookie blocked (`SyncDecision::BlockedByPrivacy`)                                                                                                                                                        | `451`                                | _(none)_                                                                    | `The gdpr_consent string prevents cookies from being saved`; no trailing newline or `Content-Type`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| ~~`setuid` blocked by regulation scope~~ — **REMOVED** in round-52 H3                                                                                                                                                       | —                                    | —                                                                           | The earlier row claimed `SkipWhen.gpp_sid` produced a 451 on /setuid "matching upstream." That was wrong: reference UID-write handling does NOT invoke the chooser ladder, so `SkipWhen.gpp_sid` never fires on /setuid. The regulation-scope path remains a /cookie_sync-only chooser decision (Appendix B.3 step 9). On /setuid, a request that would have matched `SkipWhen.gpp_sid` on /cookie_sync simply proceeds (subject to host-cookie GDPR + disabled-syncer checks per §8.3 step 11).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| `setuid` success                                                                                                                                                                                                            | `200`                                | `image/png` for `f=i` / `text/html` for `f=b` / _(none)_ for syncer-default | **`f=i` body**: the original **86-byte** transparent 1x1 PNG from `util/httputil/pixel.png` at pinned upstream commit `d7f8515b86258688304b0d9b6668c6a0e258bc9e`, with **`Content-Length: 86`**. Hex `89504E470D0A1A0A0000000D49484452000000010000000108060000001F15C4890000000473424954080808087C0864880000000D494441540899636060606000000005000187A14ED40000000049454E44AE426082`; SHA-256 `c2ecff291918a3caf0b7e470323e89f2a1f05b92e12a10649e598cacebe62acf`. Preserve the signature, IHDR, sBIT, IDAT, and IEND chunks; commit the source asset as an implementation fixture and verify byte equality, chunk CRCs, successful image decoding, and header length. **`f=b` body**: empty (zero bytes; `Content-Type: text/html` per upstream explicit status output+direct body output). **Default-response branch**: when the requested syncer has no declared default format, `200` with **no body and no `Content-Type`** (matches reference UID-write handling; round-17 finding L9). |
| `setuid` max-size eviction with sole non-priority UID                                                                                                                                                                       | `200`                                | _(none)_                                                                    | `Warning: ..., cookie not updated`; no trailing newline or `Content-Type`; Appendix B.6 step 3                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| `setuid` single UID still too large after eviction (Appendix B.6 step 4)                                                                                                                                                    | `400`                                | _(none)_                                                                    | empty body; abandon the cookie write after eviction cannot fit the new UID within `host_cookie.max_cookie_size_bytes`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `getuids` success                                                                                                                                                                                                           | `200`                                | `text/plain; charset=utf-8`                                                 | `{"buyeruids":{<key>:<uid>,…}}` for a non-empty map; `{}` when empty, with `buyeruids` omitted. JSON is followed by a trailing newline; Auctioneer sets the documented content type explicitly.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| Malformed host config override                                                                                                                                                                                              | `500`                                | `text/plain; charset=utf-8`                                                 | `Malformed host configuration` (round-44 M3 — fixed body; **no Appendix G numeric code** — host failure is a fail-fast plain-text response, not a structured per-bidder error).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| Resolver refuses config extraction without installed funding or with oversized captured caps                                                                                                                                | `500`                                | `text/plain; charset=utf-8`                                                 | `Unfunded configuration extraction`; `Cache-Control: no-store`, no `Retry-After`, and no store read. `/status` alone retains the degraded liveness disposition above. Earlier admission/static refusals retain their own statuses. This is initialization failure, not a transient backend failure.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| Internal fault                                                                                                                                                                                                              | `500`                                | `text/plain; charset=utf-8`                                                 | generic message (no internal detail)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |

Structured error objects inside `ext.errors` / `ext.warnings` are
`{ "code": <int>, "message": <string> }`, keyed by bidder name (upstream-level
issues under a reserved `prebid` key).

---

# Appendix G — Error & Warning Codes

Auctioneer's `BidderError` / `AuctionError` taxonomy uses the numeric diagnostic
codes defined below and verified against the pinned behavioral baseline. Errors are fatal
(suppress a bid / fail a request); warnings are non-fatal (surfaced in
`ext.warnings`). Phase-1-relevant codes:

**Errors (fatal)**

| Code | Name                      | Raised when                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| ---- | ------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 999  | Unknown                   | uncategorized failure                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| 1    | Timeout                   | a bidder call exceeded its timeout                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| 2    | BadInput                  | malformed request, params, or imp                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| 4    | BadServerResponse         | bidder returned an unparseable response                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| 5    | FailedToRequestBids       | Two failure points share this code (round-32 finding M3): (a) the adapter's `make_requests` failed to generate any outbound HTTP request; (b) the exchange's `validate_endpoint` (§5.4) rejected an adapter-generated outbound request as invalid or out of policy (off-allowlist host, non-HTTPS, private-IP literal, fixed-host mismatch). (c) round-100 H6 / round-102 H13 — `OversizeRequest`: `stage: Raw` (construction-time via `CappedWriter`) is an adapter-side PRE-FANOUT per-bidder entry; `stage: Encoded` (final encoded body in `finalize_for_transport` — round-108 B2; the M3a-era in-drive `prepare_outbound_request` is retired at M3b-2) is a positional per-call result. All are _adapter-side_ failures — the bidder never reaches the fan-out. **Not** raw fan-out transport failure; DNS / refused / no-such-host surface as code 999 (§5.2 `FanoutError::Unreachable`) with `seatnonbid` 103. |
| 6    | BidderTemporarilyDisabled | bidder disabled by config. **Numeric code is in the error block, but severity is _warning_** — appears in `ext.warnings[<bidder>]`, not `ext.errors` (round-25 finding M3; matches reference diagnostic severity warning severity).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| 7    | AccountDisabled           | account resolution succeeded but the resolved account carries `disabled = true`. Endpoint-specific HTTP mapping (round-15 M1 — earlier drafts omitted this row): auction → `503`; `/cookie_sync` and `/setuid` → `400`. `AuctionError::AccountDisabled(String)` at `auctioneer-exchange/src/error.rs` (M3 Task 14).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| 10   | MalformedAcct             | Reserved for independently fetched account-record failures under M8; invalid data in the current runtime blob is MalformedHost, not code 10 (§9.1).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| 12   | TmaxTimeout               | the whole-auction `tmax` deadline elapsed                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| 14   | FailedToUnmarshal         | bidder response decode failure                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| 15   | InvalidImpFirstPartyData  | bidder-scoped FPD merge changes/deletes the original impression ID or produces an invalid `Imp` (§5.1 step 7; round-20 finding H1). Fatal — auction maps to `500` via the non-`BadInput` exchange-error rule in reference auction handling.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |

**Warnings (non-fatal)**

| Code  | Name                           | Raised when                                                                                                                                                                                                                                                                                                                                                                             |
| ----- | ------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 10999 | Unknown                        | uncategorized warning                                                                                                                                                                                                                                                                                                                                                                   |
| 10001 | InvalidPrivacyConsent          | malformed TCF / GPP / US-Privacy string ignored (§7)                                                                                                                                                                                                                                                                                                                                    |
| 10002 | AccountLevelDebugDisabled      | debug requested but account `debug_allow` is false (§5.6)                                                                                                                                                                                                                                                                                                                               |
| 10003 | BidderLevelDebugDisabled       | Debug requested and account-permitted, but the finalized call's compiled base has debug_allow false; suppress alias-keyed HTTP entries and emit once per observable bidder name (§5.6; M6 section 3).                                                                                                                                                                                   |
| 10004 | DisabledCurrencyConversion     | a non-USD seatbid was dropped (currency conversion deferred)                                                                                                                                                                                                                                                                                                                            |
| 10013 | InvalidUserEIDs                | invalid `user.eids` entry dropped                                                                                                                                                                                                                                                                                                                                                       |
| 10014 | InvalidUserUIDs                | invalid user uid dropped                                                                                                                                                                                                                                                                                                                                                                |
| 10015 | TooLongTargetingPrefix         | targeting prefix too long → fell back to `hb` (Appendix E.3)                                                                                                                                                                                                                                                                                                                            |
| 10016 | TooShortTargetingPrefix        | `truncate_target_attribute` below the 12-char minimum                                                                                                                                                                                                                                                                                                                                   |
| 10017 | BidderBlockedByPrivacySettings | bidder dropped by a `fetchBids` / privacy gate (§5.1 step 8 — privacy-per-bidder)                                                                                                                                                                                                                                                                                                       |
| 10018 | GvlVersionUnavailable          | the CMP-declared `(spec, vendor_list_version)` GVL revision is not in the bundled archive (or its policy version disagrees with the TC string), so consent is unusable and the request fails closed to the Appendix A.4 default row (§7 — round-86 M10; also drives the `/_auctioneer/health/gvl` counter)                                                                                  |
| 10019 | GclStale                       | the bundled Global CMP List is past its freshness window (`now >= gcl.expires_at`, derived from the manifest's `fetched_at` + upstream cache lifetime), so CMP authenticity cannot be established — consent is unusable and the request fails closed to the Appendix A.4 default row (§7 CMP-authenticity contract, round-99 B3; the deployment must be rebuilt via the weekly refresh) |

M3b additionally allocates code 10020 (`OutboundBudgetClamp`) for funded-envelope
response-cap clamping. It must not reuse privacy deployment codes or feed the
stale-GCL health counter. Health observes only auction-owned privacy warnings.
Code 10021 (`InvalidPriceGranularity`) reports the nonfatal targeting fallback
defined in Appendix E.1 and likewise must not feed privacy-health counters.

Codes appear in `ext.errors` / `ext.warnings` objects as `{ "code": <int>,
"message": <string> }` (Appendix F). The mapping is pinned to the baseline and
re-verified when the upstream pin is bumped (§1.1).

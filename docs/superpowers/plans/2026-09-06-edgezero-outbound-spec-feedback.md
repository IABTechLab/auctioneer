# Feedback on the EdgeZero outbound-HTTP spec branch (from auctioneer bounded fanout)

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

**To:** EdgeZero maintainers, `docs/outbound-http-spec` branch (pinned @ `120dd2aa` since round-142; previously `12965da2`, originally `fcefdde3`. Branch HEAD @ **`325134e4`, verified by `ls-remote` 2026-09-24** (`46f85958` superseded) (`1e7e0562`, `03cdba98`, `351698c9` superseded) — auctioneer has not moved to it; see the current-head summary below. Earlier heads cited in this file — `3d87d5bc`, `543b782d`, `5eccff8e`, `fc8d7e9f`, `4790a140`, `351698c9` — are all superseded.)

> **⚠ RESOLVED-UPSTREAM SUMMARY — verified 2026-09-19 against the
> tree at `351698c9`, NOT inferred from commit subjects (round-184
> #10). Four gaps below are CLOSED and their entries are HISTORY:**
>
> | Gap                                                     | Status @`351698c9`                             | Landed as                                                                                                                                                                                                                                                                                                  |
> | ------------------------------------------------------- | ---------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
> | **G18** response-scoped completion                      | **CLOSED**                                     | `ResponseEgressCompletion::new(FnOnce(&ResponseEgressReport))` — response-scoped, fired once at terminal egress, panic-guarded (`response_egress.rs`)                                                                                                                                                      |
> | **G19** fallible configure                              | **CLOSED**                                     | `configure` → `Result<(), EdgeError>`, `build_app()` → `Result<App, EdgeError>` (`edgezero-macros/src/app.rs`). BREAKING for every adapter entry point                                                                                                                                                     |
> | **G21** refusal/detached completion                     | **CLOSED for refusals, PARTIAL overall**       | `AdmissionDecision::Refuse` carries `completion` (stronger than the ask). **Does NOT cover pre-admission normalized-head failures** — see G23                                                                                                                                                              |
> | **G22** abort/reset disposition                         | **CLOSED**                                     | `AdmissionDecision::Abort` — closes or resets the native request with no response (`ingress.rs`)                                                                                                                                                                                                           |
> | **G20** never-ready provider read                       | **CLOSED @`03cdba98`**                         | `ConfigStore::get_bounded` is REQUIRED with no compatibility default — "awaiting an unbounded provider call can hide a never-ready future". auctioneer's outer race is deleted on every target                                                                                                                 |
> | **G23** detached pre-admission completion               | **CLOSED @`1e7e0562`** (PARTIAL at `03cdba98`) | `03cdba98` added a completion FACTORY, which could supply a completion but could NOT refuse — so pool exhaustion still sent an ungated response, and round-186's "closed" was premature. `ac69b359` replaced it with `DetachedResponseEgressDecision { Abort, Send(completion) }`, and the invariant holds |
> | **G26** egress-start-relative deadline                  | **CLOSED @`46f85958`**, ONE ROUND after filing | `ResponseEgressDeadline::after(Duration)`, resolved against `egress_started_at` inside `begin()`. Retires auctioneer's marker-header workaround, which could not have worked (the policy callback's headers are immutable)                                                                                     |
> | **G24** post-handler abort                              | **OPEN**                                       | filed round 186; auctioneer does not need it (reserve-at-admission)                                                                                                                                                                                                                                            |
> | **G25** detached responses ignore the app egress policy | **CLOSED @`0d84247e`**                         | `ce61f92a` made the deadline a REQUIRED field of `DetachedResponseEgressDecision::Send { completion, deadline }`. EdgeZero constructs the response and installs auctioneer's deadline, so the extension workaround is retired                                                                                  |
>
> **✅ ROUND-188: EVERY GAP AUCTIONEER FILED SINCE ROUND 182 IS NOW
> CLOSED EXCEPT G24, WHICH AUCTIONEER DOES NOT NEED.** `38f61a19` also
> added `ResponseEgressCompletion::join`, which auctioneer had NOT filed
> — it is exactly the composition primitive the round-187 composite
> `T` was built to work around, and auctioneer now uses it instead.
> **Deadline anchoring (the one open contract question the upstream
> review raised): auctioneer ACCEPTS `write_deadline_after`'s
> request-start anchor.** It is strictly earlier than auctioneer's
> acquisition-time wording, so the static-pool drainage bound holds a
> fortiori. No upstream change requested.
>
> **Round-186 #1: head moved again to `03cdba98`, closing G20 and
> G23 as well — SIX of the seven gaps filed since round 182 are now
> fixed upstream.** It also shipped
> `ResponseEgressCompletion::late_bound<T>()`, which is the typed,
> nine-test version of the permit-binding mechanism auctioneer had
> specified by hand; auctioneer deletes its own and uses upstream's.
> auctioneer adopts all of it in Fanout-11, ONE atomic pin move to
> `03cdba98` carrying the batch migration and the lifecycle work
> together (round-184 #1).
> **From:** auctioneer bounded fanout prerequisite alignment (round-120), 2026-09-06
> **Status:** DELIVERED to the outbound-spec author, 2026-09-06. Upstream
> responses land as revisions on `docs/outbound-http-spec`; auctioneer picks them
> up through the branch pin (`cargo update` moves it deliberately).

> **⚠ CURRENT-HEAD SUMMARY (round-157 #6, refreshed 2026-09-17 —
> read this before the per-gap entries below, which are HISTORICAL
> as of `120dd2aa`).**
>
> **THE BATCH API CHANGED TWICE SINCE THE PIN** (rounds 159-163), so
> several entries below describe a surface that no longer exists at
> head. At `3d87d5bc`:
> `OutboundHttpClient::send_all` is REMOVED, replaced by
> `start_batch_until(..) -> OutboundBatch` with `next() ->
Result<OutboundBatchNext, EdgeError>` (`Item` |
> `Finished(Completed | Cutoff)`), `cancel()`, and `collect() ->
Result<OutboundBatchResults, OutboundBatchFailure>` — whose
> failure arm PRESERVES the slots that already completed. Outbound
> requests also default to `OutboundCachePolicy::PlatformDefault`,
> which auctioneer must override to `Bypass` on every bidder call.
> auctioneer still compiles against the PINNED `120dd2aa`; the move is
> task Fanout-11, gated on the Fanout-0 documentation rebase.
> Most of this file was written against `fcefdde3`/`12965da2`, when
> the outbound surface was a design rather than code, and its
> signatures reflect that draft — e.g. entries that describe
> a one-shot vector call returning `Vec<Result<..>>` are stale twice
> over: that CLIENT METHOD IS REMOVED, and the shipped API is the
> batch handle
> `start_batch_until(Vec<OutboundRequest>, Deadline) -> OutboundBatch`,
> polled with `next()` until `Finished`. Each `OutboundBatchItem`
> carries `{ index, result }`, and `OutboundSlotResult { elapsed,
outcome }` still carries per-slot elapsed on BOTH outcomes. STATUS AT THE PINNED HEAD:
>
> | Gap                                      | Status @120dd2aa                                                                                                                                                                                                                                                                              |
> | ---------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
> | G5 dual encoded/decoded caps             | CLOSED — `max_encoded_response_bytes` + `max_decoded_response_bytes`                                                                                                                                                                                                                          |
> | G7 response header byte/count caps       | CLOSED — `ResponseLimitReason::{HeaderBytes, HeaderCount}`                                                                                                                                                                                                                                    |
> | G10 unreachable / decode discriminants   | CLOSED — `BadGatewayReason::{Unreachable, Decode(..)}`                                                                                                                                                                                                                                        |
> | G11 multi-member gzip / EOF              | CLOSED — drained to native EOF                                                                                                                                                                                                                                                                |
> | G15 per-slot elapsed                     | CLOSED — `OutboundSlotResult.elapsed`                                                                                                                                                                                                                                                         |
> | G1/G16 ingress route surfaces            | PARTIAL — string route classes landed; adapter wiring absent (blocks Fanout-10)                                                                                                                                                                                                                  |
> | G6 bounded source-read allocation        | OPEN — bounds the certificate SCOPE to `managed`, blocks no task                                                                                                                                                                                                                              |
> | G17 batch handle with partial collection | **CLOSED at head (2026-09-17)** — `collect()`'s `OutboundBatchFailure { error, slots }` delivers exactly the partial collection G17 asked for; auctioneer's own policy still discards those slots deliberately (see the bounded fanout plan), so the ask is satisfied even though the behaviour is unchanged |
>
> The per-gap entries below are kept for the derivations and for the
> asks that are still open; where one contradicts this table, this
> table wins.
> **Round-137 reclassification, CORRECTED round-138 #2 (stable gap
> ids — the round-137 paragraph mislabeled G6 and G11; every id below
> matches its ORIGINAL entry, and partially-resolved gaps are split
> into subitems). All "closed" verdicts are DESIGN-closed at
> `12965da2` — the implementing code is NOT yet landed (round-138
> #1):**
> DESIGN-CLOSED (code pending) — G2 (raw framing policy), **G7a**
> (final header caps; split from G7), G8's window-preallocation gate,
> G9 (CF manual encoding + abort), **G11 (multi-member gzip / EOF —
> its original meaning)**, G12 (frozen-clock fairness), plus the
> cross-cutting encoded-byte caps and absolute-deadline provenance.
> WITHDRAWN — G13 (auctioneer passes ONE precomposed absolute deadline;
> the second-slot ask is moot).
> DOWNGRADED — G14 (the designed header contract retains non-identity
> `Content-Encoding` on passthrough, so the actionable state is
> header-observable and overflow origin arrives via
> `ResponseLimitReason`; decoded-by-whom provenance is analytics-only).
> STILL OPEN — **G6 (its original meaning: the SOURCE-CHUNK
> read-allocation bound — round-138 #3: the proposed `max_chunk_bytes`
> rechunks OUTPUT only and does not constrain provider-controlled
> source-chunk allocation; either upstream guarantees a bounded
> source read or auctioneer's certificate excludes it, which auctioneer now
> documents)**, **G7b** (parser/interim/trailer header bounds; split
> from G7), G1 residue (route-aware classification, grant transfer,
> context access, parser caps), G3 (egress lifetime), G4 (config
> classification/deadline — upstream's generic `StoreExtractionReason`
> categories are the accepted direction, round-138 #5), G5
> (encoding-sensitive dual-cap), G10 residue (Unreachable /
> decode-corruption discriminants), G15 (per-slot elapsed; on Fastly
> auctioneer accepts a NAMED observation-time metric — round-138 #4), G16
> (registration surfaces).
> **Round-139:** G5 (independent encoded + decoded caps) and G15
> (per-slot elapsed) are IMPLEMENTED at the candidate EdgeZero head
> `120dd2aa`; they close for auctioneer when the ATOMIC pin move lands
> (the move also deletes `edgezero_core::proxy`, so it ships in one
> commit with the facade migration — bounded fanout plan, round-139 #1).
> **Inputs reviewed (all four, in full):**
> `docs/.../2026-05-21-outbound-http-design.md` (the master outbound design),
> `2026-08-22-inbound-body-design.md`, `2026-07-10-outbound-http-implementation.md`,
> `2026-07-10-outbound-http-phase1a-error-time.md`.

auctioneer's bounded fanout plan predates these documents and had invented its own API shapes for the
same surface (`ProxyClient::send_bounded`, `ResponseBodyPolicy`, `TypedBodyError`,
`TypedProxyError`, `BodySource`, …). Round-120 re-expresses auctioneer's prerequisites in the
branch's canonical vocabulary wherever the branch defines the capability, and records a
**gap** wherever auctioneer needs something the branch does not provide (or contradicts).
This file is the gap list, written to be citable: each entry says what auctioneer needs,
where the upstream design is silent or conflicting, one concrete failure that results,
and the smallest upstream change that closes it. Nothing here asks upstream to adopt a
auctioneer enum or auctioneer naming — every ask is expressed against upstream's own types.

## Summary — verdicts per auctioneer prerequisite

| auctioneer prerequisite                                | adopt | match |    gap | Gap ids                  |
| -------------------------------------------------- | ----: | ----: | -----: | ------------------------ |
| Prereq 1 — typed store-config errors               |     0 |     0 |      1 | G4                       |
| Prereq 2 — bounded response streaming              |     4 |     3 |      7 | G5 G6 G7 G10 G11 G14 G15 |
| Prereq 3 — cancellable CF/Spin sends               |     1 |     1 |      2 | G9 G12                   |
| Prereq 4 — Fastly nonblocking fan-out              |     1 |     1 |      0 | —                        |
| Prereq 5 — ingress hook + framing                  |     0 |     0 |      2 | G2 G16                   |
| Prereq 8 — Brotli window enforcement               |     0 |     1 |      1 | G8                       |
| Prereq 9 — egress completion/abort                 |     0 |     0 |      1 | G3                       |
| Prereq 10 — ingress admission lease                |     0 |     0 |      1 | G1                       |
| Cross-cutting (redirects, deadlines, memory model) |     2 |     2 |      1 | G13                      |
| **Total**                                          | **8** | **8** | **16** |                          |

Gap G2 is **P0-class** for auctioneer (framing rejection has no home in the design).
G1 was delivered as P0-class and is CORRECTED to P1 by round-121 — see its entry:
the ownership-conflict premise was wrong; the policy-point/timestamp/deadline/
cancellation asks stand.

## What auctioneer adopts (no upstream action needed — recorded for context)

- `OutboundHttpClient::send` / the BATCH HANDLE + `ResponseMode::Buffered
{ max_bytes }` replace auctioneer's invented `ProxyClient::send_bounded` /
  `BoundedProxyResponse` (outbound design §3.1.1, §3.1.3). The batch's
  index-carrying per-slot results, single shared `now` snapshot, and
  buffered-only preflight are exactly the fan-out shape auctioneer's Prereq 4 asked for
  (§3.2, §4.3 dispatch-all-then-harvest).
- `EdgeError::{BadGateway, GatewayTimeout{cause: BudgetSource}, ResponseTooLarge}` plus
  `Body::Stream(LocalBoxStream<'static, Result<Bytes, EdgeError>>)` replace auctioneer's
  invented `TypedProxyError` / `TypedBodyError` stream-item error surface (§3.4.3, §6
  migration bullet on the `Body::Stream` error-type change, phase1a Task 1).
- `Deadline` with the equality-expires rule (`deadline <= now` ⇒ expired; phase1a
  Task 2) matches auctioneer's tie rule verbatim, and `dispatch_budget`'s equality
  attribution to `PerCallTimeout` (§3.3.2) maps cleanly onto auctioneer's
  "equality ⇒ bidder-timeout" cause rule.
- Redirects: §3.1.4 makes no-auto-follow the **baseline** on all four adapters and
  names the exact per-adapter mechanics (`redirect::Policy::none()`,
  `worker::RequestRedirect::Manual` — the enum, not the string; Spin/Fastly need no
  opt-out). A 3xx arrives as `Ok(OutboundResponse)`, so auctioneer's code-4 classification
  of received 3xx stays app-side, consistent with the "non-2xx is not an error" rule.
- Header multi-value preservation (`append`, never `insert`/`set`) and the CF
  `set-cookie` fix are mandated upstream (§3.1.4, §4.2) — auctioneer's round-32 foundations adapter
  precondition is satisfied by design.
- The §3.4.1 content-encoding policy (decode only bare single `gzip`/`br`; everything
  else passes through **untouched with `content-encoding`/`content-length` intact**)
  eliminates the pinned-adapter hazard auctioneer's round-43 H2 flagged (deflate passed
  through still-compressed but **labelled identity**). Under the branch's table, bytes
  the adapter did not decode are never relabelled, so auctioneer can run its own decoder
  pipeline on the preserved raw form.
- Memory-model scoping honesty: §3.4.4's explicit "core-owned payload bound, not
  process RSS" matches auctioneer's round-117 P1-7 / round-118 F13 `managed` vs
  `whole_process` certificate split.

## Gaps

### G1 — Ingress policy point, canonical timestamp, drain deadline + cancellation

**(CORRECTED after delivery — round-121.** The original entry, sent 2026-09-06,
characterized lazy ingress as an admission-OWNERSHIP conflict with an eager-buffering
failure ("N × cap bytes of not-yet-admitted buffering"). That premise was wrong and is
withdrawn: the inbound-body design's `BodyState::Initial` holds the platform body
UNREAD until an extractor pulls, so an application can acquire admission before any
byte buffers. What remains — and it is still the substantive ask — is that the design
provides no STANDARD place to do so: no all-route policy point (per-handler discipline
is a convention, not a contract), no canonical `request_start` captured before
routing, no finite deadline on the drain (bytes-only bounds: a 1 B/s under-cap
trickle drains forever), and no cancellation of the underlying platform read. Severity
accordingly P0 → P1: a design gap to close, not a conflict to resolve. The "smallest
upstream change" below survives unchanged — it was always about the seam, the
timestamp, and the time axis.)**

- **auctioneer needs (Prereq 10, round-117 P0-1 / round-118 F04):** for routes declaring an
  ingress policy, the adapter captures `request_start` from the monotonic clock and
  try-acquires an app-owned admission lease **before any inbound body byte is
  buffered**; gate-full ⇒ a constant 503 with zero body bytes read; the ingress read
  runs under an absolute deadline whose expiry **cancels the platform read**, answers
  408, and releases the lease exactly once across every terminal (complete, overflow,
  deadline, disconnect).
- **Upstream:** the inbound-body design (`2026-08-22-inbound-body-design.md` §§2–3)
  deliberately moves body reading to **extractor-pull** (`body_bytes(max)` /
  `json_within(max)` on `RequestContext`, lazy `Body::Stream` from the adapter). There
  is no admission concept, no pre-handler hook, no start-instant capture, and — the
  sharper conflict — **no time bound anywhere on the inbound drain**: `body_bytes` is
  bounded in bytes only. The two designs disagree about who owns admission (auctioneer:
  adapter ingress, before routing; upstream: whichever extractor first pulls) and when
  reads become bounded (auctioneer: before the first buffered byte; upstream: at first
  helper call, with size-only bounds).
- **Concrete failure:** on Cloudflare (one concurrent auction permit per isolate under
  auctioneer's model), a client trickling an under-cap body at 1 B/s never violates
  `body_bytes(max)` and never finishes; the handler holds its slot indefinitely with
  zero recourse. (Round-121 correction: the earlier claim here that such clients also
  pin "not-yet-admitted buffering" was wrong — the lazy body buffers nothing until
  pulled. The trickle-forever hole and the missing pre-read start capture stand.)
  Slow-upload time also escapes the request's own
  tmax because nothing captured a start instant before the read.
- **Smallest upstream change:** an adapter-level ingress seam, before routing/handler
  dispatch, that (a) stamps a monotonic `request_start` into `RequestContext`, (b)
  consults one app-supplied admission callback (`fn(&Parts) -> Admit | Refuse(Response)`)
  before the `BodyCell` can leave `Initial`, and (c) accepts an optional absolute
  read-deadline on the bounded drain whose expiry cancels the underlying platform read
  and poisons the cell with a **typed timeout** `StoredError` (the existing poison
  machinery already gives exactly-once semantics for later accessors). The `BodyCell`
  state machine is a good foundation; it needs a time axis and a pre-drain gate point.

### G2 (P0) — Per-route ingress hook, normalized framing state, strict raw-boundary rejection

- **auctioneer needs (Prereq 5):** a per-adapter ingress hook that runs before the body is
  read, receiving a normalized `FramingState { Length(u64) | Chunked | None | Malformed }`;
  and, per target, strict raw-parser rejection (fail-closed 400, pre-normalization) of
  CL+TE together, duplicate/conflicting `Content-Length`, non-`chunked`/repeated
  `Transfer-Encoding`, and illegal chunked framing — because hyper resolves these
  shapes (accept-as-chunked, suppress CL) before any `HeaderMap` consumer can see them.
- **Upstream:** neither the outbound design (out of scope by charter) nor the
  inbound-body design says anything about request framing, smuggling shapes, or a
  pre-body hook; the inbound design's adapters hand the platform body to `BodyCell`
  as-is.
- **Concrete failure:** a CL+TE request reaches auctioneer's handler as clean `Chunked`
  with the `Content-Length` already suppressed by hyper — the ambiguity that request
  smuggling exploits is unobservable at every layer upstream exposes, so auctioneer cannot
  even detect-and-400 it.
- **Smallest upstream change:** on the same ingress seam as G1: pass a normalized
  framing summary to the hook, and (per-target, starting with Axum where the raw
  parser is configurable) reject the four ambiguous framing shapes before
  normalization. The wasm targets can document platform-parser behaviour instead of
  claiming rejection — auctioneer gates those per target — but the hook + framing summary
  needs to be a portable core contract.

### G3 — Egress-lifetime completion signal, response-write deadline, abort handle

- **auctioneer needs (Prereq 9, round-117 P1-6, round-119 F4):** for the **response-out**
  direction (app → client): a per-response signal that fires exactly once with a
  terminal reason from {completed, client_disconnected, deadline_aborted,
  transport_error} after real transmission ends; an absolute write deadline the
  adapter enforces by aborting the connection mid-write; and body ownership retained
  until that terminal, so permits/buffers can be released on truth rather than on
  hand-off.
- **Upstream:** the branch covers outbound requests and inbound request bodies.
  Response-out appears only inside response converters: Axum/Fastly/Spin **buffer**
  streamed bodies into a 16 MiB adapter constant and hand bytes to the platform
  (§3.5.2 footnotes 3/6/7); Cloudflare streams lazily. No completion callback, no
  write deadline, no abort handle, no disconnect signal reaches the app. The §3.1.1
  rustdoc's "abort the downstream body" applies only to mid-stream upstream errors,
  and is adapter-internal.
- **Concrete failure:** a slow client that reads one byte per second holds the
  adapter-owned copy of a 2 MiB serialized response for ~24 days; auctioneer released its
  egress permit at hand-off (it has no other signal), so a population of such clients
  accumulates unbounded adapter-owned buffers outside every gate — the exact term that
  makes auctioneer's whole-isolate memory certification unobtainable.
- **Smallest upstream change:** a response-out contract on the adapter response
  converter: accept an optional absolute write deadline (abort on expiry once the
  first byte is committed), and expose a completion future/callback resolving exactly
  once with a small terminal-reason enum. Even a first cut that only distinguishes
  {completed, aborted-or-disconnected} would let permit release attach to
  transmission truth.

### G4 — Typed store-config error taxonomy (and a deadline on the store fetch)

- **auctioneer needs (Prereq 1):** `AppConfig<C>::from_store` returning a dedicated typed
  error distinguishing missing-key / envelope-parse / envelope-SHA / typed-deserialize
  / validation / missing-binding / missing-secret / backend-unavailable (… — the full
  variant table is in auctioneer's bounded fanout Prereq 1), instead of today's collapse into
  `EdgeError::ConfigOutOfDate` / `EdgeError::internal`. Additionally (round-119 F1)
  the complete extraction must be runnable under a caller deadline whose expiry
  cancels the pending store/secret lookup.
- **Upstream:** all four branch docs scope store/config lifecycle **out** ("non-outbound
  store/config lifecycle changes remain outside this design", outbound design header;
  §6/§7 "store/config lifecycle behavior remain owned by their dedicated designs").
  Meanwhile the inbound-body design's `StoredError` enumerates the **end-state**
  `EdgeError` surface as exactly eleven variants — i.e. the end-state it pins still
  funnels every store-config failure through `ConfigOutOfDate`/`Internal`.
- **Concrete failure:** one mistyped value in a pushed config blob surfaces as the
  same `ConfigOutOfDate` as "no blob yet"; auctioneer's runtime must degrade to defaults
  (fresh-deploy ergonomics) and thereby silently disables GDPR enforcement on a
  malformed production push — auctioneer's recorded dev-complete privacy fail-open, which
  only an upstream typed split can close.
- **Smallest upstream change:** none of the outbound branch's shapes need to change —
  the ask is that the owning store/config design (a) adopt a typed store-extraction
  error before the `EdgeError` end-state list hardens further (the `StoredError`
  match in the inbound design would gain arms or wrap it), and (b) make the store
  fetch cancellable/deadline-bounded. auctioneer's Prereq 1 carries a ready-made variant
  list to react to.

### G5 — Transport-side byte cap and overflow origin

- **auctioneer needs (round-118 F10, round-119 F8):** two independent caps — compressed
  wire/transport bytes AND decoded bytes — and an overflow error that says **which
  side** overflowed (auctioneer's diagnostics and admission arithmetic distinguish them
  even when the public wire result coincides).
- **Upstream:** `max_response_bytes` counts **decoded** bytes only (§3.4.1); the early
  `Content-Length` reject applies to identity responses only; compressed wire bytes
  are otherwise uncounted, and `ResponseTooLarge` carries only a message.
- **Concrete failure:** an origin returns 512 MiB of incompressible "gzip" — decoded
  output stays under the decoded cap for a long time while the transport bytes and
  the decoder's input-side buffering transit unbounded by any configured number; and
  when a cap does fire, the operator cannot tell a decompression bomb (decoded-side)
  from a plainly-huge response (transport-side) without string-parsing the message,
  which the branch's own `BudgetSource` design rightly forbids for causes.
- **Smallest upstream change:** count raw bytes pulled from the platform stream
  against a second (defaultable to `max_response_bytes`) transport cap in the same
  incremental drain, and give `ResponseTooLarge` a typed
  `origin: {Transport, Decoded}` field (non_exhaustive), mirroring how
  `GatewayTimeout` carries `cause`.

### G6 — Per-chunk yield bound

- **auctioneer needs (Prereq 2 item 4):** a bounded per-chunk ALLOCATION, not merely a
  bounded yield length. (Round-121 correction to this entry's earlier ask: a
  consumer-side rechunker slicing a received 4 MiB allocation into 8 KiB yields
  bounds nothing — the 4 MiB backing store was already allocated and stays alive
  behind the slices. The ask below is amended accordingly.) auctioneer charges
  `cap + MAX_CHUNK_SIZE` per stream only if the PRODUCER-side read is bounded;
  otherwise the charge must be the platform's actual receive-allocation ceiling,
  named and cited, or the certificate scope stays managed-memory with this exclusion
  documented.
- **Upstream:** §3.4.1 states "the spec does not rechunk"; `sizeof(current_chunk)` is
  explicitly source-controlled, and §8 risk 11 defers the cap with three sketched
  options.
- **Concrete failure:** a peer yields one 4 MiB `Bytes` against a 256 KiB cap: the
  pre-append check correctly refuses the append, but the 4 MiB allocation already
  happened; multiplied across fan-out slots this breaks any per-stream memory proof.
- **Smallest upstream change (corrected round-123 — the earlier ask here
  recommended the consumer-side rechunker this entry's own analysis rejects;
  slicing an existing allocation bounds nothing):** a PRODUCER-side bounded read —
  the adapter's receive loop reads at most `max_chunk_bytes` per platform read
  before yielding — or, where the platform hands pre-sized buffers the adapter
  cannot influence, a documented per-target receive-allocation ceiling auctioneer can
  charge. The consumer-side rechunker remains useful only as a yield-length
  convenience and is not claimed as an allocation bound.

### G7 — Response header/trailer size + count caps, with an oversize discriminator

- **auctioneer needs (round-111 B3, round-115 B1):** caps on summed response-header bytes
  (8 KiB) and count (64), same for trailers, enforced where the cause is knowable, and
  an error that distinguishes "headers oversize" from generic transport failure
  (auctioneer wires it to a dedicated wire outcome).
- **Upstream:** no response-header limit exists on `OutboundRequest` or in the drains.
  Where a platform enforces one, the signal is flattened: Fastly's
  `HttpResponseHeaderSectionTooLarge` → generic `bad_gateway` (stage-2 table, §4.3);
  Spin's `HTTP-response-*` size codes → the 502 bucket (§4.4); Axum/CF have no ceiling
  configured at all, so an arbitrarily large header block is materialized in guest
  memory before the app sees the response.
- **Concrete failure:** a misbehaving bidder returns 1 MiB of headers; on Axum this is
  buffered wholesale (outside every configured cap), and on Fastly the refusal is
  indistinguishable from DNS failure — auctioneer cannot classify it, and its
  header-memory admission term is uncharged.
- **Smallest upstream change:** `max_response_header_bytes` / `max_response_header_count`
  on `OutboundRequest` (enforced in each adapter's conversion layer where the raw
  fields are visible), plus a distinct error kind — or a typed detail on `BadGateway`
  — for the oversize case, produced only where the cause is actually knowable (never
  inferred from a generic parse error).

### G8 — Brotli window ceiling enforced before decoder allocation

- **auctioneer needs (Prereq 8):** parse each brotli layer's RFC 7932 window header and
  refuse (typed error) **before constructing the decompressor** when declared WBITS
  exceeds a policy ceiling; plus a way to learn the decoder-state bound an adapter
  actually enforces, so admission can charge it. (auctioneer's layers-cap concern is moot
  under the branch: only a single bare `br` layer is ever decoded — stacked lists pass
  through — so `max_brotli_layers = 1` holds by construction. Recorded as a match.)
- **Upstream:** §3.4.1's decode path allocates the brotli decoder with no window
  inspection and no policy field; nothing reports an enforced bound.
- **Concrete failure:** a bidder sends `br` with WBITS 24; the shared decoder allocates
  ~16 MiB of window state inside a wasm isolate auctioneer budgeted at 4.5 MiB for that
  stream — the admission weight is unsound exactly where memory is scarcest.
- **Smallest upstream change:** an optional `max_brotli_window_bits` on
  `OutboundRequest` (default: current behaviour); when set, the shared decoder reads
  the stream header and fails the stream with a typed error before allocation. A
  constant documenting decoder state as f(window_bits) covers the reporting half.

### G9 — Cloudflare outbound fetch: raw (manual) response-body encoding unstated

- **auctioneer needs (round-118 F06):** the CF adapter's outbound fetch must explicitly
  request raw response delivery (workerd `encodeResponseBody: "manual"`-equivalent on
  the constructed request), because workerd defaults response decoding to automatic —
  otherwise the guest-side gzip/br decode path and the §3.4.1 passthrough table are
  operating on bytes workerd may have already transformed.
- **Upstream:** §4.2 specifies `EncodeBody::Manual` for the **response-out** (to
  client) direction only, and says "the existing gzip/br decompression path is kept"
  for the outbound fetch — with no statement about requesting manual encoding on the
  **outbound request**, and no deployed-Workers verification row for
  compressed-bytes-observed-in-guest.
- **Concrete failure:** on deployed Workers with automatic decoding, the upstream's
  in-guest gzip decoder receives already-decoded bytes: either it errors (spurious
  502 on every compressed bidder response) or, for the passthrough rows, the
  "untouched raw bytes + preserved content-encoding" promise is false — the body was
  decoded while the header says compressed, which is exactly the mislabelling class
  §3.4.1 works to prevent. Local Wrangler behaves differently, so only a deployed
  probe can tell.
- **Smallest upstream change:** one sentence in §4.2 requiring manual response-body
  encoding on the outbound fetch (the request-side mirror of the already-mandated
  `EncodeBody::Manual`), plus a Tier 3 deployed-Workers row asserting compressed
  bytes are observed in-guest with encoding metadata intact and exactly one decode.

### G10 — "Unreachable" is not distinguishable from other transport failures

- **auctioneer needs (Prereq 2 error surface):** a typed discriminator for
  DNS-resolution/refused/TCP-connect/TLS-handshake failures **before any response**
  versus other pre-body failures — auctioneer's wire contract maps unreachable to a
  distinct public outcome (code 999 / seatnonbid 103) and everything else to 100.
- **Upstream:** every transport failure is one `BadGateway { message: String }`
  (§3.4.3). The Fastly adapter internally distinguishes (`SendFailure::Transport`)
  and Spin's classifier sees per-variant `ErrorCode`s, but the distinction is erased
  at the `EdgeError` boundary; recovering it would mean parsing message strings —
  the very thing the branch's `BudgetSource` design was added to avoid for timeouts.
- **Concrete failure:** a bidder whose DNS is down and a bidder returning malformed
  `connection` headers produce byte-identical auctioneer analytics, so operators cannot
  separate "endpoint is dead" (routing/infra action) from "endpoint is broken"
  (bidder-integration action).
- **Smallest upstream change:** the `GatewayTimeout { cause }` pattern applied to
  `BadGateway`: a `#[non_exhaustive]` reason enum with at least
  {Unreachable, UpstreamProtocol, CodecCorruption { coding, layer }, Other}
  (round-123: `CodecCorruption` added here to match what auctioneer's classifier
  requires — round-121 #6 widened the ask in auctioneer's plan but this entry, the
  one upstream actually reads, had not been updated), populated where adapters already know the
  answer (reqwest `is_connect`, Fastly `SendErrorCause`, wasi `ErrorCode`), defaulting
  to Other.

### G11 — Multi-member gzip and drain-to-EOF semantics unspecified

- **auctioneer needs (round-119 F7):** RFC 1952 gzip is a sequence of members; a JSON body
  split across members is valid input. The decoder must decode ALL members under the
  cumulative decoded cap, then drain the transport to its own EOF; corrupt/truncated
  members and non-member trailing junk fail; a transport error after valid JSON still
  fails the call.
- **Upstream:** §3.4.1's shared gzip decoder does not state member policy, trailing-junk
  policy, or whether decoder EOF is distinguished from transport EOF. (Common Rust
  decoder defaults stop at the first member.)
- **Concrete failure:** a bidder flushes a large bid response as two gzip members; a
  first-member-only decoder yields a truncated-but-well-formed JSON prefix — a silent
  misparse (wrong bids), not an error.
- **Smallest upstream change:** one paragraph in §3.4.1 pinning: gzip decodes all
  members under the cumulative cap; trailing non-member bytes are a `bad_gateway`;
  after decoder EOF the drain still reads transport EOF under the same budget; and a
  test row with a two-member split document.

### G13 — Two absolute deadline inputs per request (round-124)

- **auctioneer needs:** each outbound request races TWO independent absolute instants —
  the bidder's per-call deadline and the whole-auction deadline — with the cause
  attributed by which expired. Upstream exposes one absolute `.deadline()` plus a
  relative `.timeout()` re-anchored at each `send_all`, so auctioneer composes
  `min(bidder, auction)` into the single slot and re-derives the cause from its own
  clock at completion.
- **Upstream:** one absolute slot (outbound design §3.3).
- **Concrete cost:** none for correctness (the min() composition is exact); the cost
  is diagnostic — upstream's own timeout reporting can never name which budget
  expired, so every consumer must re-derive it.
- **Smallest upstream change:** accept an optional second absolute deadline (or a
  small list of named absolute deadlines) and report which fired in
  `GatewayTimeout { cause }`. Purely additive; the single-slot form remains valid.

### G12 — Deadline checks on deployed Workers with a frozen clock

- **auctioneer needs (round-118 F12):** awareness that on deployed Workers,
  `Date.now`/`performance.now` are frozen across CPU-only work — two bracketing
  deadline reads around a long synchronous phase can return the same instant, so a
  deadline crossed during CPU work is invisible until the next awaited host I/O.
- **Upstream:** the CF deadline design (§4.2, §3.3.4) races `worker::Delay` and
  re-checks `is_expired()` at yield boundaries; nothing notes that between awaited
  host points the clock does not advance on deployed Workers (local Wrangler
  differs), so cooperative `is_expired()` checks inside CPU-bound loops (e.g. the
  decoder wrapper's post-ready checks against an always-ready buffered stream) can
  never observe expiry.
- **Concrete failure:** a decode of an already-buffered large body runs entirely
  between host yields; every `is_expired()` check reads the same frozen instant, and
  the 504 that the design promises "at the next yield boundary" arrives only after
  the whole decode completes.
- **Smallest upstream change:** a documented caveat plus a cooperative yield budget
  in the CF decoded-output wrapper (yield to the event loop every N chunks so the
  clock refreshes), and one deployed-Workers Tier 3 timing probe.

### G14 — Buffered responses need explicit decode-disposition and overflow-origin metadata (round-132; renumbered round-133 — G13 was already the two-deadline ask)

- **auctioneer needs (round-132 #3):** for every buffered response, an explicit
  statement of what produced the delivered bytes — `decode_disposition:
Identity | DecodedByUpstream { original: ContentCoding } | Passthrough
{ chain }` — and, on `ResponseTooLarge`, an explicit origin field naming
  which budget overflowed (transport read vs upstream decoded output).
- **Upstream:** `ResponseMode::Buffered { max_bytes }` returns bytes with the
  usual header set; successful upstream decoding STRIPS `Content-Encoding`,
  and a size failure may carry no response at all — so neither the
  disposition nor the overflow origin is reconstructible from headers after
  the fact.
- **Concrete failure:** auctioneer's cap-ownership rows (dual-cap accounting) and
  its `OversizeResponse { source: Transport | Decoded }` classification both
  require knowing which row a response took; guessing from a stripped header
  set misclassifies bare-gzip-decoded bodies as identity and cannot attribute
  an overflow at all.
- **Smallest upstream change:** two additive fields — a disposition enum on
  the buffered response, and an origin discriminant on the size-limit error.
  Both are metadata the adapter already knows at the point of decision.

### G15 — Per-slot elapsed time on batch results (round-133)

- **auctioneer needs (round-133 #5):** per-slot completion metadata — an elapsed
  duration or completion instant — on EVERY `send_all` slot outcome, `Ok` and
  `Err` alike. The auctioneer spec makes per-call elapsed a hard requirement
  (`responsetimemillis` per bidder): timing the batch await attributes the
  slowest slot's latency to every fast bidder in the wave.
- **Upstream:** the `send_all` result vector and `OutboundResponse` carry
  status, headers, and body only — no timestamp, no duration, on either
  outcome (outbound design §3.1.3 result shape).
- **Concrete failure:** a 40 ms bidder sharing a wave with a 900 ms timeout
  is reported at wave granularity; its `responsetimemillis` inflates ~22x,
  poisoning latency-based bidder selection and SLA reporting.
- **Smallest upstream change:** one additive field per slot outcome (elapsed
  `Duration` from that slot's dispatch to its completion or failure), or a
  completion `Instant` auctioneer can difference against its own dispatch
  capture. Purely additive; no behavioral change.

### G16 — Route registration must issue ids, accept metadata, and expose the table (round-134)

- **auctioneer needs (round-134 #1):** three additive registration surfaces on
  the `app!` / `RouterBuilder` path: (i) a stable `RouteId` issued per
  registered route (declaration order suffices); (ii) per-route attached
  metadata — an opaque `ingress_class = <u16>` key on the manifest route
  entry, passed through by the `app!` macro (round-135 #7 pins the source:
  the manifest, so the macro never needs a separate policy resolver);
  (iii) the completed registration table, `&[RegisteredRoute { id, method,
template, class }]`, exposed to the app after build. The ingress hook's
  fifth parameter should be a `RouteResolution { Matched(RouteId) |
MethodNotAllowed { allowed } | NotFound }` rather than a bare id, so 404
  and 405 are distinguishable without the app re-matching.
- **Upstream:** at the pinned rev, `app!` generates handler registrations
  directly from edgezero.toml (edgezero-macros app.rs:140) and
  `RouterBuilder::route` returns only the builder (router.rs:186) — no id,
  no metadata slot, no table access.
- **Concrete failure:** auctioneer's one-classification-source contract
  (admission class, body cap, and 404/405 all derived from one matched
  entry) is unconstructible: the adapter cannot zip auctioneer's policy table
  against registrations it cannot see, and the ingress callback cannot
  learn WHICH route matched.
- **Smallest upstream change:** all three surfaces are additive to the
  builder and macro; no existing route's behavior changes.

### G17 — `send_all` needs a batch handle that cancels pending slots while collecting completed ones (round-156)

- **Ask:** alongside `send_all(Vec<OutboundRequest>) -> Vec<OutboundSlotResult>`,
  a handle (or a stream of `(index, OutboundSlotResult)` items) that lets the
  caller stop dispatching/awaiting the slots still in flight while KEEPING the
  outcomes of the slots that already completed.
- **Upstream:** at `120dd2aa` the batch is one opaque future; its `Vec` is
  produced only on completion, so dropping the future to cancel discards every
  result the batch had already computed.
- **Concrete failure:** auctioneer cannot cut a wave short on a BestEffort host
  (one that does not honor the per-request `Deadline`) without destroying the
  successful slots, the facade's recorded preflight failures, and each slot's
  own `elapsed` and error class — turning one slow bidder into a whole-wave
  loss. auctioneer therefore awaits every wave to completion and documents the
  overrun in the target's `outbound-deadlines: BestEffort` row instead.
- **Smallest upstream change:** additive — the existing `send_all` keeps its
  signature; the handle/stream is a second entry point for callers that need
  partial collection.

### G18 — response egress needs a per-response completion guard (round-181)

> **STATUS: UPSTREAM WILL FIX (confirmed 2026-09-18).** auctioneer should
> therefore NOT build a workaround. 2a.7 implements checkpoints (1)-(3)
> now; the permit's release-on-terminal-report binding waits for this
> guard rather than settling for an early release. Re-verify the shape
> when it lands — Fanout-11's pin move is the natural checkpoint.

- **Ask:** a per-response opaque token or application-owned extension that
  `ResponseEgressAttempt` retains until its exactly-once terminal transition,
  and that appears on the matching `ResponseEgressReport`.
- **Upstream:** at head the observer installed by
  `App::set_response_egress_observer` is GLOBAL, and `ResponseEgressReport`
  carries `{ body_kind, bytes_written, elapsed, fallback_disposition,
outcome, request_start, route }` — no unique identity. Two concurrent
  responses on the same route, started in the same instant, are
  indistinguishable in the report.
- **Concrete failure:** auctioneer's egress permit must release on the terminal
  report for ITS OWN response (2a.7's `PermitBody`, exactly-once). With a
  global observer and no token, auctioneer cannot bind one permit to one report,
  so it must either release early — losing the transmission-leg bound the
  permit exists to enforce — or risk releasing another response's permit.
- **Smallest upstream change:** additive. An opaque `u64`/newtype carried
  from attempt to report, or a typed extension slot the application sets and
  reads back.
- **Consequence while open:** auctioneer's response-write checkpoints (1)-(3) are
  implementable now (2a.7); checkpoint (4), release on write-complete, stays
  BestEffort and uncertifiable.

### G19 — `configure` cannot report a failure (round-181)

> **STATUS: UPSTREAM WILL FIX (confirmed 2026-09-18).** Until it lands,
> 2a.7's installer may `.expect(..)` on an invalid envelope, but that is
> a TEMPORARY shape with a known replacement — when `configure` becomes
> fallible, the installer returns its error and the panic is deleted.
> Do not design around the panic.

- **Ask:** a fallible configure/build path — `Hooks::configure(&mut App) ->
Result<(), EdgeError>`, with the macro propagating it.
- **Upstream:** `Hooks::configure` returns `()` and the `app!` expansion
  discards the callback's value, while the setters it is meant to call
  (`set_config_extraction_limits`) are themselves `Result`-returning and
  validate their input.
- **Concrete failure:** auctioneer installs its extraction limits in that
  callback (2a.7). An invalid envelope — a `max_total_bytes` below
  `max_backend_bytes`, say — can only `.expect(..)` there, so a
  configuration error becomes a panic at boot instead of a typed startup
  failure. That is a worse operator experience than the fail-loud startup
  error auctioneer gives every other invalid limit.
- **Smallest upstream change:** additive if the default impl stays
  `Ok(())`; every existing callback keeps compiling.

### G20 — a provider read that never resolves escapes the extraction deadline (round-182)

- **Ask:** `get_bounded`'s default should RACE the provider future against the
  deadline, not bracket it — or the trait should require providers to be
  cancellable before a deadline is advertised as enforced.
- **Upstream:** the default is documented as "a cooperative compatibility
  implementation: it checks before and after the unbounded provider call",
  with a note that providers "must override it before claiming native
  allocation or cancellation guarantees".
- **Concrete failure:** a provider future that never resolves never reaches
  the second check, so `max_config_fetch_ms` never fires and the request
  hangs. auctioneer's own never-ready regression cannot pass against the default.
- **auctioneer's interim:** an OUTER race it owns, duplicating a bound upstream
  also checks. Removed once this lands.
- **Smallest upstream change:** additive — race inside the default body; no
  signature change.

### G21 — completion hook must cover detached and refusal responses (round-182)

- **Ask:** a resource/completion hook spanning EVERY converted response,
  including admission refusals and buffered fallbacks.
- **Upstream:** refusals use detached egress, and `BufferedIngressResponse`
  carries no extension slot, so nothing can be attached to them.
- **Concrete failure:** auctioneer requires every response — refusals included —
  to hold a static-egress permit released on terminal completion. Refusals
  are exactly the responses a saturated system emits most, so the class the
  accounting most needs is the one it cannot observe.

### G22 — admission needs an abort/reset disposition (round-182)

- **Ask:** an explicit portable `Abort`/`Reset` outcome on `AdmissionDecision`.
- **Upstream:** it offers admit, buffered fallback, or respond — all of which
  require sending a response.
- **Concrete failure:** when auctioneer's static-egress pool is exhausted there is
  no buffer to send a 503 from; the correct action is to reset the transport.
  Today auctioneer must either allocate the very buffer it has run out of, or
  degrade to a 503 it cannot guarantee it can emit.

### G23 — detached pre-admission responses carry no completion (round-184)

- **Ask:** a completion hook for responses emitted BEFORE application
  admission, or a documented statement that this class is outside any
  application-observable accounting.
- **Upstream @`351698c9`:** `App::detached_ingress_error_egress` renders
  400/414/431 for a head that fails normalization and hardcodes
  `ResponseEgressCompletion::empty()`. Its own doc: detached responses "do
  not invoke the application's response-egress policy or observer".
- **Why this is the residue of G21, not a new class:** G21 asked for a hook
  spanning EVERY converted response, and upstream delivered it for everything
  the application admits — including refusals, which was the hard part. What
  remains is the responses the application never sees.
- **Concrete failure:** auctioneer's static-egress pool cannot bound these
  responses, so its "every response holds a permit" invariant is false for
  them. auctioneer has WEAKENED the guarantee explicitly rather than claim it:
  the pool bounds every response auctioneer emits, and pre-admission rejections
  are bounded by adapter connection limits instead.
- **Severity:** LOW. The bodies are fixed-size upstream constants and the
  class is not attacker-amplifiable beyond connection count. auctioneer is not
  blocked; it is only prevented from stating a universal invariant.
- **Smallest upstream change:** additive — let the detached path accept a
  `ResponseEgressCompletion` instead of hardcoding `empty()`.

### G24 — no application-selectable abort after the handler returns (round-186)

- **Ask:** a way for the application to select abort/reset for a response
  decided AFTER admission — a post-handler egress decision, or an abort
  disposition the application can set on the response before transmission.
- **Upstream @`03cdba98`:** `AdmissionDecision::Abort` is returned
  synchronously before the body read and before the handler runs.
  `ResponseEgressOutcome` has no application-selectable abort, and
  `ResponseEgressFallbackDisposition::Aborted` is a REPORTED disposition,
  not a decision the application makes.
- **Concrete failure:** auctioneer's static-egress class wants "reset the
  transport when the pool is exhausted". For a response decided post-admission
  — a config-fetch 503, a serializer-overflow 500, an egress-queue expiry —
  there is no admission decision left to return, so the reset is
  inexpressible.
- **auctioneer's resolution, which does NOT need this:** reserve one static
  permit per admitted request at admission, so exhaustion is always detected
  at admission, where `Abort` already works. No post-admission response can
  find the class exhausted.
- **Severity:** LOW. This is an efficiency ask, not a correctness one — it
  would let auctioneer reserve less eagerly. auctioneer is not blocked.
- **Smallest upstream change:** additive — an abort disposition settable on
  the response before transmission begins.

### G25 — detached responses ignore the application egress policy (round-187)

- **Ask:** let detached pre-admission responses consult the application's
  installed `ResponseEgressPolicy`, or document that they never will.
- **Upstream @`1e7e0562`:** `ResponseEgressEnvelope::detached` hardcodes
  `Arc::new(default_response_egress_policy)`, applying
  `DEFAULT_RESPONSE_WRITE_BUDGET = 30s` regardless of what the application
  installed.
- **Concrete failure:** auctioneer sizes its static-egress pool against a 2s
  refusal budget. A slow client on a detached 400 holds a permit 15x longer
  than the bound the pool is sized for.
- **auctioneer's resolution, which does NOT need this:** insert
  `ResponseEgressDeadline` into the detached response's extensions. `begin()`
  enforces the earlier of that and the policy deadline, so the 2s value wins
  the `min`. Filed anyway because relying on an extension to override a
  hardcoded policy is a sharp edge for the next consumer.
- **Severity:** LOW. A documented workaround exists and auctioneer uses it.
- **Smallest upstream change:** pass the app's policy into the detached path.

### G26 — no egress-start-relative per-response deadline (round-193)

- **Ask:** `ResponseEgressDeadline::after(Duration)`, resolved inside
  `ResponseEgressEnvelope::begin` against `egress_started_at`.
- **Upstream @`0d84247e`:** `ResponseEgressDeadline::new(Deadline)` takes an
  ABSOLUTE instant, and the application annotates the response BEFORE
  `begin()` runs, so it cannot name the egress-start instant. The policy
  callback does receive `egress_started_at`, but `ResponseEgressHead` carries
  no application response class — status and route do not distinguish a
  static refusal from an auction or aux response.
- **Concrete failure:** auctioneer's static-class budget is
  `egress_started_at + refusal_write_deadline_ms`. Expressing it today means
  either a construction-time deadline (strictly tighter, silently shortening
  the budget by however long the response waits to be picked up) or leaking a
  auctioneer response class into a global callback.
- **auctioneer's resolution, which does NOT need this:** auctioneer sets a private
  `x-auctioneer-egress-class: static` header on the responses it constructs and
  reads it back via `head.headers()` in its own policy, stripping it before
  transmission. Self-contained and needs no upstream change.
- **Severity:** LOW. A relative deadline would be cleaner than a marker
  header, but auctioneer is not blocked.
- **Smallest upstream change:** additive — an `after(Duration)` constructor
  resolved in `begin()`.

### G27 — env overlay cannot populate absent typed `Option` fields (round-196)

- **Ask:** let the env overlay INSERT a value for a typed `Option<_>` field
  the TOML omits, or offer a typed projection API
  (`run_config_push_typed<Source, Payload>`) so an application can push a
  subtree of a larger source document.
- **Upstream @`46f85958`:** "The overlay only overrides keys that already
  exist in the parsed tree" (`app_config.rs`), with a regression test pinning
  it. `run_config_*_typed::<C>(args)` takes only command args and
  deserializes the file root itself — there is no typed-value or projection
  parameter.
- **Concrete failure:** auctioneer's runtime knobs are `Option<_>` overrides that
  are absent by default, so `AUCTIONEER__AUCTION__DEFAULT_TMAX_MS` reaches
  nothing. Making them present to fix that PINS the value and stops the blob
  inheriting future baked defaults — the two requirements are mutually
  exclusive today.
- **auctioneer's resolution, which does NOT need this:** document the limitation
  honestly (an absent key is not env-overridable) and do the projection in a
  auctioneer wrapper type whose `Serialize` emits only the runtime subtree. That
  works because the pushed payload is `serde_json::to_value(typed)`.
- **Severity:** LOW for the projection half (the wrapper works, at the cost
  of hand-written `Serialize`/`Validate`/`AppConfigMeta`); MEDIUM for the
  overlay half, since the documented env-override workflow is unusable for
  any optional field.
- **Smallest upstream change:** additive on both halves — typed insertion for
  absent `Option` leaves, and a two-parameter push/diff.

### G28 — config extraction accepts only a relative timeout (round-201)

- **Ask:** let `ConfigExtractionLimits` carry an OPTIONAL absolute
  `Deadline` alongside its relative `timeout`, shared across the root read
  and every secret read **without resetting per read**. `timeout` keeps its
  current meaning and stays the default, so this is additive.
- **Upstream @`46f85958`:** `ExtractionBudget::start` computes
  `started_at = clock.now()` at extraction entry and adds
  `validated_limits.timeout` (`extractor.rs`). The struct exposes
  `timeout: Duration` only, so an application cannot name an instant the
  extraction should not outlive.
- **Why this is GENERAL, not a auctioneer special case:** EdgeZero already
  treats absolute `Deadline` as its universal primitive —
  `ConfigStore::get_bounded` takes one, `DispatchBudget` carries one, and
  `ResponseEgressDeadline` shipped `after(Duration)` / `at(Deadline)` last
  week for exactly this reason (the application could not name the right
  instant from outside). Config extraction is the one bounded path that
  still re-anchors internally. **Relative timeouts do not compose:** any
  application bounding a whole request rather than one operation finds that
  N bounded operations of `t` each may consume `N×t`.
- **Concrete failure:** auctioneer wants `request_start + max_config_fetch_ms`
  so the bound covers the request, not just the extraction. Today upload and
  pre-extraction work fall outside it.
- **auctioneer's resolution, which does NOT need this:** adopt extraction-start
  semantics and say so in the contract. auctioneer does not block on G28.
- **Severity:** LOW for auctioneer, MEDIUM as an API-consistency gap — this is
  the only bounded operation in EdgeZero without an absolute form.
- **⚠ CORRECTED (round-202, upstream review): a boot-configured FIELD IS THE
  WRONG SHAPE.** `ConfigExtractionLimits` is boot policy copied into every
  request, so one absolute deadline stored there would expire permanently
  and fail every later extraction. The ask is a **REQUEST-SCOPED** input:
  an extractor argument or a request-context override supplying the
  absolute deadline per extraction, with the no-reset rule across the root
  read and every secret read. The relative `timeout` stays boot-configured
  and unchanged.
- **Smallest upstream change:** a per-extraction optional absolute deadline
  argument, plus the no-reset rule.

### G29 — framework error bodies interpolate unbounded caller input (round-205) — **CLOSED @`325134e4`**

**Closed one round after filing.** `App::set_error_response_renderer(Fn(EdgeError)
-> Response)` hands the application the whole error body, so auctioneer renders the
error KIND only — measured at 9 bytes against ~840 for the default renderer, and
against the 256 auctioneer charges per permit. No truncation API was needed and no
exemption survives: the per-permit byte charge is a true ceiling again. The
original filing follows.

- **Ask:** bound the caller-controlled text EdgeZero interpolates into its own
  error bodies — a cap with a truncation marker on the path in
  `EdgeError::NotFound` and on the allowed-set in
  `EdgeError::MethodNotAllowed`. A short constant cap (say 64 bytes plus an
  ellipsis) keeps the diagnostic useful while making the body's size a
  property of the framework rather than of the request.
- **Upstream @`46f85958`:** `EdgeError::NotFound { path }` renders
  `format!("no route matched path: {path}")` (`edgezero-core/src/error.rs`),
  embedding the full unmatched path; `MethodNotAllowed` embeds the allowed
  set the same way.
- **Concrete failure:** auctioneer charges one fixed 256-byte render buffer per
  terminal-response permit. A 513-byte path produces a 536-byte body —
  more than double the charge — and the size scales with whatever URI limit
  the application configures. Nothing on the application side bounds it:
  completion ownership governs WHEN the permit is released, the egress
  policy selects a deadline and cannot rewrite a body, and these responses
  never reach the application's bounded serializer.
- **Why it is general, not auctioneer-shaped:** any application that pre-charges
  memory per in-flight response has the same problem, and the input is
  attacker-chosen. EdgeZero already bounds ingress heads and response
  egress; its own error bodies are the remaining unbounded path.
- **auctioneer's resolution, which does NOT need this:** declare framework
  errors outside the per-permit byte charge and bounded instead by auctioneer's
  configured URI limit, with a long-path regression test. The pool still
  bounds concurrency; only the per-response byte figure is affected.
- **Severity:** LOW-MEDIUM. Not a leak and not unbounded growth — one
  response, one body — but it defeats a per-response byte budget.
- **Smallest upstream change:** truncate at render time; no API change.

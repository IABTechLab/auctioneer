# bounded fanout historical appendix — superseded prerequisite text

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

> **Status: HISTORICAL, NON-NORMATIVE.** Every block below was
> written while the corresponding EdgeZero surface was still an
> open ask. The authoritative status of each surface is the
> COMPATIBILITY MATRIX at the top of
> `2026-07-10-multi-adapter-production-fanout.md` (pinned
> `120dd2aa`), and the consuming auctioneer code where it exists. This
> file exists so the plan's executable body no longer contradicts
> its own matrix (round-148 #5); nothing here gates any task.

## 1. Round-112 H4 prerequisite audit (fcefdde3-era)

**Round-112 H4 — prerequisite audit at the `docs/outbound-http-spec`
pin (now `12965da2` — round-138 #6; the notes here record the
`fcefdde3`-era audit; re-verified at this
alignment):** every prerequisite below
was re-checked against the release and remains OPEN — Prereq 1:
`AppConfig::<C>::from_store` still returns `Result<C, EdgeError>`
(now at `extractor.rs:780`; the 975-line extractor delta is the
`State<T>` work, not typed store errors — `AppConfigError` remains
file-load-only); Prereq 2: no `send_bounded` /
`BoundedProxyResponse` / `ResponseBodyPolicy` in
`edgezero-core/src/proxy.rs` (still the one-method `ProxyClient`) —
and round-115 B1 ADDS to Prereq 2's deliverable: a PARSER-STAGE
header-oversize discriminator for BOTH protocols (hyper's public
surface cannot provide the h2 one — `is_parse_too_large()` is
http1+server-gated and h2 overflow arrives as an opaque
`h2::Error` — so the bounded client must integrate where the cause
is knowable: its own field-section accounting or an h2-level
integration; generic errors are never classified oversize);
Prereq 5: no ingress hook API; the remaining prereqs' surfaces are
likewise absent. Historical line anchors in this plan stay dated
`9e661ae`; where a cited symbol MOVED at v0.0.6 (from_store
717→780) the contract was verified unchanged, and `from_store` is
STILL at `extractor.rs:780` at v0.0.8. Plan snippets that
quote FUTURE upstream APIs stay prereq-gated by construction —
nothing in the v0.0.6..v0.0.8 delta pre-empts or contradicts them
(v0.0.8's extractor delta is confined to the secret-path walk —
an `OptionalField` segment variant — which auctioneer never reaches:
`AuctioneerAppConfig` declares no secret paths).

## 2. Round-121 #6 — G10 discriminator ask (pre-120dd2aa)

ROUND-121 #6: one requirement upstream's current variants CANNOT
carry — guest-decoder corruption (malformed gzip/brotli/deflate)
folds into `BadGateway` indistinguishably from transport failure,
losing auctioneer's decoded-side category and its sanitized
category-only message. The G10 reason-enum ask is widened: the
discriminator must separate {Unreachable, UpstreamProtocol,
CodecCorruption { layer identity }, Other}, or auctioneer's mapper
cannot meet its own taxonomy.

## 3. G15 per-slot elapsed ask (pre-120dd2aa)

- Per-slot elapsed time → NO upstream surface (**GAP G15,
  round-133 #5**): spec §5.7 requires per-call
  `responsetimemillis`, and timing the batch await inflates every
  fast bidder to the slowest slot. Upstream `send_all` results
  carry no completion timestamp on either `Ok` or `Err` outcomes.
  EdgeZero must add per-slot elapsed (or a completion instant)
  metadata on BOTH outcomes — and G15 is a BEHAVIOR-COMPLETE
  prerequisite for Fanout-2's fanout metrics (round-134 #3):
  `responsetimemillis` is a bare integer on the wire with nowhere
  to disclose degradation, so there is NO interim wave-quantized
  fallback — the metric ships only with per-slot elapsed, carried
  by the facade's `BoundedSlotResult.elapsed`.

## 4. Prereq 1 — store-config error taxonomy (pre-120dd2aa)

- [ ] **Prereq 1 — TYPED, CATEGORIZED store-config errors from
      `extractor::AppConfig<C>::from_store` (round-138 #5 re-bases the
      ask onto upstream's latest design: EdgeZero now proposes GENERIC
      `StoreExtractionReason` categories — auctioneer REQUIRES the
      behavioral categories, NOT the upstream adoption of auctioneer
      naming. The ask: distinguishable categories for at least
      missing-key / envelope-parse / envelope-integrity /
      typed-deserialize / validation, plus a store-deadline outcome;
      auctioneer maps them onto its own `RuntimeConfigError` taxonomy —
      `StoreAppConfigError` below survives only as auctioneer's
      requirements name for that mapping table). LOG HYGIENE is part
      of this prerequisite's consumer contract (round-138 #5): the
      current `degrade_to_default` logs `err.to_string()`
      (runtime.rs) — deserialization/validation/secret errors can
      embed stored values or secret identifiers, so Fanout-1a REQUIRES
      allowlisted STRUCTURED fields only (category, key id, byte
      lengths) and PROHIBITS arbitrary upstream error text in
      production logs; pinned by
      `config_error_logs_structured_allowlist_no_raw_text()` in
      Fanout-1a's own test list.** Round-27 H3+H4
      rewrite — earlier drafts had two factual errors against the
      pinned upstream at `9e661ae` (citations below keep that snapshot's
      line anchors; contracts re-verified unchanged at `fcefdde3`;
      round-138 — the re-based behavioral-categories ask ABOVE is the
      live contract, this audit is historical):
  1. The store-backed `AppConfig::from_store` is defined in
     [`edgezero-core/src/extractor.rs:717`](https://github.com/IABTechLab/edgezero/blob/9e661ae/crates/edgezero-core/src/extractor.rs#L717),
     NOT in `edgezero_core::app_config` — the bounded fanout path
     `app_config::AppConfig::from_store` does not exist.
  2. `edgezero_core::app_config::AppConfigError` already
     exists at
     [`app_config.rs:105`](https://github.com/IABTechLab/edgezero/blob/9e661ae/crates/edgezero-core/src/app_config.rs#L105)
     but is scoped to the **filesystem loader** (variants:
     `Deserialize`, `EnvOverlay`, `InvalidValue`, `Io`, `Parse`,
     `Validation`). Reusing that name for the store-backed
     path would conflate two distinct sources of failure.

  What the pinned upstream actually distinguishes today, verified
  against `extract_from_handle` at
  [`extractor.rs:766-820`](https://github.com/IABTechLab/edgezero/blob/9e661ae/crates/edgezero-core/src/extractor.rs#L766-L820)
  and its collaborators:
  - `ConfigStoreError` (`config_store.rs:165`) — three variants:
    `Internal { source: AnyError }`, `InvalidKey { message }`,
    `Unavailable { message }`. That is ALL. There is NO variant
    for envelope parse, SHA mismatch, typed deserialization,
    validation, missing binding, missing secret, or auth
    failure — every one of those is currently either lost
    inside `Internal` or collapsed downstream into
    `EdgeError::ConfigOutOfDate` / `EdgeError::Internal`.
  - `SecretError` (`secret_store.rs:113`) — four variants:
    `Internal`, `NotFound { name }`, `Unavailable`, `Validation`.
    No `AuthFailure` variant.
  - `AppConfigError` (`app_config.rs:105`) — six variants, all
    scoped to filesystem load, none of which the store path
    can produce.
  - `extract_from_handle` collapses envelope parse and SHA
    mismatch and secret walk failures into
    `EdgeError::internal(...)`; missing key, serde deserialize,
    and validation failures into `EdgeError::config_out_of_date`.

  What auctioneer needs upstream:
  - A NEW dedicated return type
    `pub enum StoreAppConfigError { ... }` (name chosen to
    avoid the existing `AppConfigError` in `app_config.rs`;
    the store-backed extractor is in `extractor.rs`, so this
    can live in `extractor.rs` next to `AppConfig<C>` or in a
    new sibling module `extractor_store_error.rs`). The type
    replaces `EdgeError` as the return of `AppConfig::from_store`
    (and of `AppConfig::named` for symmetry). Callers convert
    into `EdgeError` via a keep-as-is `From` impl for the
    existing status-code mapping.
  - Variants for EVERY auctioneer mapping row below. **Only the
    two already-typed classes** (`InvalidKey` from
    `ConfigStoreError::InvalidKey`, `BackendUnavailable` from
    `ConfigStoreError::Unavailable` and
    `SecretError::Unavailable`) survive intact; every other
    variant is NEW work in EdgeZero (splitting apart today's
    `EdgeError::internal` / `EdgeError::config_out_of_date`
    catch-alls). Precise variant list:
    - `MissingKey { key: String }` — key absent (today: silent
      `ConfigOutOfDate` inside `extract_from_handle`).
    - `EnvelopeParse { key: String, source: serde_json::Error }`
      — envelope JSON unparseable (today: `EdgeError::internal`).
    - `EnvelopeSha { key: String, sha_expected: String,
sha_actual: String }` — SHA-256 mismatch on envelope
      verify (today: `EdgeError::internal` with message
      "envelope verification failed").
    - `TypedDeserialize { key: String, field_path: String,
source: serde_path_to_error::Error<serde_json::Error> }`
      — typed `C` deserialize failed (today: `ConfigOutOfDate`).
    - `Validation { key: String, source: ValidationErrors }`
      — `cfg.validate()` rejected (today: `ConfigOutOfDate`).
    - `MissingStoreBinding { store_id: String }` — no
      `[stores.config]` (today: `EdgeError::internal` from
      `extract_from_handle`'s ok_or_else).
    - `InvalidKey { key: String, message: String }` —
      preserves `ConfigStoreError::InvalidKey`.
    - `MissingSecret { field: String, secret_name: String }`
      — preserves `SecretError::NotFound` at a specific field
      (today collapsed via `impl From<SecretError> for
EdgeError` into `EdgeError::internal`).
    - `BackendUnavailable { source: BackendKind, retry_after:
Option<Duration> }` where `BackendKind = ConfigStore |
SecretStore` — preserves `ConfigStoreError::Unavailable`
      AND `SecretError::Unavailable` with a source
      discriminant. Retry hint is optional because the pinned
      backends do not surface one today; auctioneer consumes
      `None` as the default 5 s.
    - `Internal { source: anyhow::Error }` — catchall for
      backend-provider failures that don't fit the above; maps
      to `EdgeError::internal` and auctioneer treats as
      `MalformedHost`.
    - `UnknownVersion { got: u32 }` (round-28 H5 addition) —
      envelope's declared schema version is a value auctioneer does
      not recognise. Today this collapses into the
      `envelope.verify()` catchall inside
      `extract_from_handle`; split out because auctioneer's
      MalformedHost taxonomy needs to distinguish "operator
      pushed a config from a future auctioneer release" from
      "envelope was corrupted."
    - `MissingSecretBinding { store_id: String }` (round-28 H5
      addition) — the deployment references a secret store id
      that is not in `[stores.secret.<id>]`. Distinct from
      `MissingStoreBinding` (which is the CONFIG store). Today
      this surfaces as `SecretError::Internal` because the
      lookup fails inside the secret-store handle acquisition
      path.
    - `SecretValidation { field: String, secret_name: String,
source: ValidationErrors }` (round-28 H5 addition) —
      preserves `SecretError::Validation`. Today collapsed into
      `EdgeError::internal` by `impl From<SecretError> for
EdgeError`. Distinct from `MissingSecret` because the
      secret was found but its value violates auctioneer's own
      validator (length / regex / custom).

  **`AuthFailure` intentionally omitted (round-27 H4).** The
  pinned EdgeZero has no auth-failure signal (`SecretError`
  lacks it; `ConfigStoreError` lacks it). Inventing an
  `AuthFailure` variant would require a corresponding new
  backend signal that auctioneer cannot fabricate. Deployments
  whose secrets are permission-denied surface as
  `SecretError::Internal` (wrapping the provider's auth error
  in `anyhow::Error`) → `StoreAppConfigError::Internal` →
  `MalformedHost`. If EdgeZero later grows a typed `AuthFailure`
  channel across both stores, auctioneer can add the variant then
  under `#[non_exhaustive]`.

  - Response body: auctioneer maps every `MalformedHost` variant to
    a **plain-text 500** with body `Malformed host
configuration` (matches Appendix F). `BackendUnavailable`
    maps to a **plain-text 503** with body `Config store
unavailable, retry after {n}s`; the `Retry-After` header
    value is the `retry_after` duration when present, else the
    default 5 seconds.
  - Forward-compat: `StoreAppConfigError` is `#[non_exhaustive]`.
    auctioneer pattern-matches with a catchall `_ =>
RuntimeConfigError::MalformedHost(...)` so a future EdgeZero
    variant does not silently fail-open — new failure modes
    default to 500 until auctioneer's mapping is updated.

  **Full set of variants auctioneer needs** — the mapping table
  Task Fanout-1 references verbatim:

  | `StoreAppConfigError` variant                                                                     | auctioneer mapping                                                                                                                                                                                                                                                                                                                                                            | HTTP                                                                       |
  | ------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
  | `MissingKey { key }`                                                                              | silent default IFF the baked `[runtime] allow_missing_config = true` (round-118 F14 — boolean-only, immutable, no env/store override); else `RuntimeConfigError::MalformedHost`                                                                                                                                                                                           | (handler continues on allow_missing_config; 500 plain text otherwise)      |
  | `EnvelopeParse { key, source }`                                                                   | `RuntimeConfigError::MalformedHost`                                                                                                                                                                                                                                                                                                                                       | 500 plain text                                                             |
  | `EnvelopeSha { key, sha_expected, sha_actual }`                                                   | `RuntimeConfigError::MalformedHost`                                                                                                                                                                                                                                                                                                                                       | 500 plain text                                                             |
  | `TypedDeserialize { key, field_path, source }`                                                    | `RuntimeConfigError::MalformedHost`                                                                                                                                                                                                                                                                                                                                       | 500 plain text                                                             |
  | `Validation { key, source }`                                                                      | `RuntimeConfigError::MalformedHost`                                                                                                                                                                                                                                                                                                                                       | 500 plain text                                                             |
  | `MissingStoreBinding { store_id }` (deployment misconfigured, no store attached)                  | `RuntimeConfigError::MalformedHost`                                                                                                                                                                                                                                                                                                                                       | 500 plain text (**not** silent — this is a deploy bug, not a fresh deploy) |
  | `InvalidKey { key, message }` (key name violated the store's naming rules)                        | `RuntimeConfigError::MalformedHost` (500 plain text). Round-25 foundations: production key names are static auctioneer-controlled string literals, so an `InvalidKey` reply means a auctioneer bug / deploy misconfiguration — silent fall-through would mask the defect and silently disable every runtime override. Regression test: `invalid_key_returns_malformed_host_not_default()`. |
  | `MissingSecret { field, secret_name }` (deployment expects a secret that's absent)                | `RuntimeConfigError::MalformedHost`                                                                                                                                                                                                                                                                                                                                       | 500 plain text                                                             |
  | `BackendUnavailable { source: BackendKind, retry_after }` / transient store or secret-store error | HTTP 503 with `Retry-After`; do NOT fall back to the baked default (transient failure ≠ fresh deploy). New `RuntimeConfigError::ConfigStoreUnavailable`.                                                                                                                                                                                                                  |
  | `Internal { source }`                                                                             | `RuntimeConfigError::MalformedHost` (catchall, deploy bug)                                                                                                                                                                                                                                                                                                                | 500 plain text                                                             |
  | `UnknownVersion { got }` (round-28 H5)                                                            | `RuntimeConfigError::MalformedHost`                                                                                                                                                                                                                                                                                                                                       | 500 plain text                                                             |
  | `MissingSecretBinding { store_id }` (round-28 H5)                                                 | `RuntimeConfigError::MalformedHost`                                                                                                                                                                                                                                                                                                                                       | 500 plain text                                                             |
  | `SecretValidation { field, secret_name, source }` (round-28 H5)                                   | `RuntimeConfigError::MalformedHost`                                                                                                                                                                                                                                                                                                                                       | 500 plain text                                                             |
  | (`#[non_exhaustive]` catchall)                                                                    | `RuntimeConfigError::MalformedHost` — fail-loud default for any variant added upstream in a later release                                                                                                                                                                                                                                                                 | 500 plain text                                                             |

  This taxonomy MUST be settled BEFORE the EdgeZero API is
  frozen. Task Fanout-1 spec references this table verbatim.

  **GAP vs upstream outbound spec (round-120) — feedback G4.** The
  branch's four docs scope store/config lifecycle OUT ("store/config
  lifecycle behavior remain owned by their dedicated designs", outbound
  design header + §6/§7), so no upstream design yet provides this
  taxonomy — and the inbound-body design's `StoredError` (§3) pins an
  eleven-variant END-STATE `EdgeError` surface that still funnels every
  store-config failure through `ConfigOutOfDate`/`Internal`. The
  round-119 F1 config-fetch deadline additionally needs the store lookup
  to be cancellable, which upstream is equally silent on (folded into
  G4). Prerequisite stands unchanged and OPEN.
  _Blocks:_ Task Fanout-1.

## 5. Pre-G5 single-slot interim facade ("scaffolding" / "swap" wording, retired round-149 #6)

### Former plan text — G5 'swap' wording (facade architecture paragraph)

```text
  semantically enforced) CLOSES with the atomic pin move that brings
  the independent caps. Round-126
  #10 (supersedes round-125 #7's waiver option, which quietly
  permitted reducing scope): Fanout-8's behavior-complete acceptance
  REQUIRES the G5-swapped facade for the transport-cap line item.
  There is no exclusion path; the interim facade is development
  scaffolding, and behavior-complete WAITS on the swap for this
  item. The
  round-123 ownership table's OWNER columns remain the target state
  under G5; its tests run against the facade both before and after
  the swap.
```

### Former plan text — pre-G5 interim single slot (send_bounded doc comment)

```text
      // #2: the builder shape is upstream's, not a auctioneer-invented
      // dual-field literal. The pre-G5 interim single slot is
      // max(transport, decoded), round-124 #5 — SCAFFOLDING only:
      // with transport > decoded it cannot enforce the decoded
      // limit during the read, so behavior-complete waits on G5).
```

### Former plan text — Prereq 2 pre-G5 interim

```text
  carrying BOTH caps once G5 lands (round-137 #2: the exact builder
  shape is UPSTREAM'S choice — auctioneer must not implement a
  hypothetical dual-field literal; the requirement is
  encoding-sensitive decoded-cap enforcement selected after
  response headers. Pre-G5 interim: the single slot is
  max(transport, decoded), round-124 #5 — DEVELOPMENT SCAFFOLDING
  ONLY: when transport > decoded it does NOT satisfy the
  during-read decoded limit, so behavior-complete gates on G5);
```

## 7. Struck admission drafts (rounds 37–46: `AdmissionSemaphore` / `AdmissionBootConfig` / `install_admission`, per-adapter stream defaults, proof table, Fastly sandbox lifecycle) — moved round-150 #8

> Superseded in full by the plan's "Admission + whole-isolate budget — the round-98 design" section. Do not implement anything below.

    round-37 B6 admissibility gate at fanout construction —
    the per-adapter concurrency envelope. Round-36 H4's
    formula multiplied per-call peak by
    `max_bidders_per_auction`, but that treats every
    auction's fanout as an isolated memory byte pool and
    ignores concurrent auctions in the same
    process/isolate. The reviewer's counterexample: 50
    bidders × three 24 MiB decoder states = 3.6 GiB, above
    the Axum 75% × 4 GiB threshold. Round-37 B6 fixes both
    parts: platform request cap (adapter-wide
    concurrency) AND checked u64 arithmetic so overflow
    can't silently drop the gate.

    > **⚠ STRUCK — NON-NORMATIVE HISTORY (struck round-96/97; visible
    > marker added round-107 stored requests and accounts). The admission/budget shapes in this
    > block (`AdmissionSemaphore` / `AdmissionBootConfig` /
    > `install_admission`, per-adapter stream-default constants) are
    > RETIRED. The "Admission + whole-isolate budget — the round-98
    > design" section + the Fanout-2 deliverable list own every one of
    > these decisions; in particular the Cloudflare default is
    > `max_active_body_streams = 2` from the §8 envelope (round-105
    > B8) — NOT the 6 below. Do not implement anything from this
    > block.**

    Round-39 B5+B6+B8 rewrite (retires round-38 B3+B4's
    admission shape). Round-38 shipped three latent
    contradictions the round-39 review flagged:
    (a) B5 — Fastly/Spin default 8 streams × 72.5 MiB per
        RawCompressed stream = 580 MiB, blowing past the
        128 MB heap; Axum 512 streams × 72.5 MiB = 36 GiB.
        The proof text said these adapters ship
        RuntimeDecoded, but `supported_body_sources()`
        includes RawCompressed for all three and strategy
        negotiation picks the strongest → RawCompressed
        gets selected and the admission gate fires at boot.
    (b) B6 — `AdmissionSemaphore` was defined but never
        assigned an owner. `ProductionFanout` is
        constructed per-auction in
        [auction.rs:210](../../../crates/auctioneer-core/src/handlers/auction.rs#L210);
        a semaphore on that struct limits only that one
        auction, not the isolate. Nothing said where the
        semaphore lives across requests, and per-request
        configuration would drift from a long-lived
        instance.
    (c) B8 — `RuntimeDecoded` was modelled as costing zero
        decoder memory, but the pinned EdgeZero adapters
        actually run the decoder INSIDE THE RUST GUEST:
        Cloudflare's `transform_stream` at
        `edgezero-adapter-cloudflare/src/proxy.rs:105`
        drives the guest-side decompressor; Fastly's
        gzip decoder likewise; Spin's is even more
        aggressive (buffer-then-decompress full body).
        Zero decoder overhead was wishful.

    Round-39 rebuilds all three axes:
    - **BodySource** splits into three variants — where the
      decoder actually runs decides the guest memory bound.
    - **AdmissionSemaphore** is adapter-owned (one instance
      per adapter process/isolate), byte-weighted (permit
      cost = per-stream footprint), constructed at
      `install_host_primitives` time with the deployer's
      resolved config.
    - **Defaults are proven passing** by an at-boot
      admission check that runs against the actual
      shipped `host-defaults.toml`, with round-39
      regression tests locking each adapter's defaults in
      place.

    ```rust
    // Round-42 B6 canonical placement (retires the
    // round-39 `#[cfg(feature = "cloudflare")]` +
    // `#[cfg(feature = "fastly")]` guards inside
    // `auctioneer-core`, which never had those features
    // declared — the constants disappeared at compile
    // time). Under round-42 B6:
    //
    // - Universal constants (`LAYERS_MAX`, `MAX_CHUNK_SIZE`
    //   , the brotli/flate2 state helpers) live in
    //   `crates/auctioneer-core/src/host_primitives/
    //   proxy_bounds.rs`. NO cfg guards.
    //
    // - Per-adapter constants (isolate/heap ceilings,
    //   connection/subrequest limits) live in EACH
    //   adapter crate at
    //   `crates/auctioneer-adapter-<name>/src/limits.rs`.
    //   The adapter reads its own constant when
    //   populating `AdmissionBootConfig` in its
    //   fetch/main.rs bootstrap. `auctioneer-core` never
    //   sees adapter-specific numbers.
    //
    // Round-42 B3 note on isolate accounting: the
    // adapter-supplied `adapter_max_memory_bytes` must
    // subtract the GDPR fanout GVL cost (embedded bytes + parse
    // cache) BEFORE handing it to `install_admission`.
    // See "Whole-isolate memory budget" below for the
    // canonical decomposition.

    // crates/auctioneer-core/src/host_primitives/proxy_bounds.rs
    pub const LAYERS_MAX: usize = 3;
    pub const MAX_CHUNK_SIZE: usize = 8 * 1024;         // 8 KiB

    // ─── Round-44 B2 + B4: THE canonical decoder-memory
    // model. This block is the ONLY place decoder state is
    // computed; `MAX_DECODER_STATE` is deleted from every
    // plan + spec section (round-44 B4 sweep). ───
    //
    // History, so nobody re-derives the wrong number:
    //  - Rounds 32-42 pinned `MAX_DECODER_STATE = 24 MiB`
    //    (RFC 7932's full 16 MiB window + overhead) and
    //    treated it as an immovable cost → per_stream
    //    72.5 MiB → 1-stream admission → the bounded fanout-partial
    //    downgrade. Round-43 B1 showed that config could
    //    not even boot.
    //  - Round-43 B1 lowered the ACCEPTED WBITS to 20 on
    //    the premise that "every conformant encoder
    //    targeting a sub-1-MiB payload emits WBITS ≤ 20".
    //    **Round-44 B2: that premise is FALSE.** Verified
    //    against the pinned encoder: `brotli-8.0.4`
    //    `enc/encode.rs:332` sets `lgwin: 22` as the
    //    DEFAULT, and `SanitizeParams` (encode.rs:546)
    //    only clamps it to [10, 24] — there is no
    //    payload-size-driven shrink anywhere in the
    //    encoder. RFC 7932 §9.1 likewise permits WBITS
    //    10-24 independent of payload size. A WBITS-20
    //    ceiling would reject a default-configured
    //    encoder's output — i.e. most real brotli traffic.
    //
    // Round-44's model keeps the round-43 concurrency win
    // WITHOUT the false compat premise, by moving the
    // lever from the WINDOW to the LAYER COUNT:
    //
    //  1. Accept WBITS up to 22 by default — exactly the
    //     reference encoder's default, so conformant
    //     bidders are never rejected.
    //  2. Cap BROTLI layers at 1. `Content-Encoding: br,
    //     br` is not something any real encoder emits;
    //     LAYERS_MAX still allows 3 total layers, but at
    //     most one may be brotli. The other two can only
    //     be gzip/deflate, whose flate2 state is ~128 KiB
    //     — three orders of magnitude smaller.
    //
    // Net: per_stream ≈ 20.77 MiB at WBITS 22 (round-140 #1) — the same
    // ballpark round-43 got at WBITS 20, but without the
    // false compatibility claim. Same admission math, no
    // rejected traffic.

    /// Round-44 B2: default accepted Brotli WBITS.
    /// **22 = the pinned reference encoder's default**
    /// (`brotli-8.0.4` enc/encode.rs:332 `lgwin: 22`) →
    /// 4 MiB window. Accepting less would reject
    /// default-configured encoders regardless of payload
    /// size, because the encoder does NOT shrink lgwin
    /// for small inputs (verified: `SanitizeParams` only
    /// clamps to [10, 24]).
    pub const DEFAULT_MAX_BROTLI_WBITS: u8 = 22;

    /// RFC 7932 §9.1 hard ceiling for the STANDARD
    /// (non-large-window) format. A deployer may raise
    /// `max_brotli_wbits` to this; above it the stream is
    /// a large-window extension auctioneer never accepts.
    /// Raising it costs concurrency — see the proof table.
    pub const RFC7932_MAX_BROTLI_WBITS: u8 = 24;

    /// Round-44 B2: at most ONE brotli layer per stream.
    /// Chains declaring more (`br, br`) are rejected with
    /// `FanoutError::UnsupportedContentEncoding` before
    /// any decoder is allocated — no real encoder stacks
    /// brotli on brotli, and allowing it would multiply
    /// the dominant term by LAYERS_MAX.
    pub const MAX_BROTLI_LAYERS: usize = 1;

    // ⚠ Round-44 B3 — WHO ENFORCES THIS. On the
    // `RawCompressed` path auctioneer owns the decoder, so
    // auctioneer's own `parse_brotli_wbits` pre-check
    // (round-33 foundations) enforces the ceiling before it
    // allocates — the constants above are self-enforcing.
    //
    // On the `GuestDecoded` path they are NOT. The
    // adapter allocates its decoder inside EdgeZero
    // BEFORE auctioneer ever sees the body (pinned CF:
    // `transform_stream` at proxy.rs:137). Passing
    // `max_brotli_wbits` into auctioneer's admission
    // installer constrains auctioneer's arithmetic, not
    // EdgeZero's allocator. **This is a BLOCKING
    // EDGEZERO PREREQ, tracked as Prereq 8 below** —
    // until it lands, the GuestDecoded admission weight
    // is an assumption, not a bound.

    /// Worst-case flate2 (gzip/zlib) decoder state, from
    /// the pinned `flate2 = "=1.1.9"` rust_backend
    /// (round-39 H2 pin). Measured, not estimated — the
    /// codec-pin CI check re-measures on any bump.
    pub const FLATE2_STATE_BYTES: u64 = 128 * 1024;

    /// Round-44 B4 / round-99 creative caching: THE canonical brotli decoder-state
    /// formula. Every caller — the round-98 design's
    /// `validate_isolate_limits` (per_stream_peak derivation), the
    /// proof table, the allocator harness — goes through
    /// here. `MAX_DECODER_STATE` is DELETED. (`install_admission` is
    /// struck; the envelope derivation replaced it.)
    ///
    ///   window(wbits) = 1 << wbits
    ///   + 512 KiB ring buffer / Huffman tables / static
    ///     dictionary overhead (measured on
    ///     `brotli-decompressor = "=5.0.3"`, round-42 H2 pin)
    ///
    /// Round-45 H2: WBITS validation lives here, so no
    /// caller can shift by an out-of-range amount. Returns
    /// the RFC 7932 range error rather than panicking on
    /// `1 << wbits`.
    #[must_use]
    pub fn validate_wbits(wbits: u8) -> Result<u8, AuctioneerBootError> {
        if (10..=RFC7932_MAX_BROTLI_WBITS).contains(&wbits) {
            Ok(wbits)
        } else {
            Err(AuctioneerBootError::WbitsOutOfRange { wbits })
        }
    }

    /// THE canonical Brotli decoder-state size. Round-45 H2:
    /// checked. Shifts by a VALIDATED wbits (caller must have
    /// passed it through `validate_wbits`, which every
    /// in-tree caller does), so `1 << wbits` cannot overflow;
    /// the fixed-charge add is `checked_add` for defence.
    ///
    /// ROUND-140 #1 — the model MATCHES upstream's audited charge
    /// (edgezero-core compression.rs:
    /// `BROTLI_DECODER_FIXED_CHARGE_BYTES = 16_777_216`, covering
    /// the boxed decoder state, context maps, Huffman tree groups
    /// and bridge buffers of the pinned brotli-decompressor 5.0.1;
    /// the ring buffer is charged as `2^WBITS` on top). The old
    /// `window + 512 KiB` model UNDERCHARGED every decoder by
    /// ~16 MiB and is retired; the upstream constant re-audits on
    /// any decoder pin bump.
    ///
    /// Worked values (the ONLY numbers any section may quote):
    ///   wbits 20 → 16 MiB + 1 MiB  = 17 MiB
    ///   wbits 22 → 16 MiB + 4 MiB  = 20 MiB  ← default (20_971_520)
    ///   wbits 24 → 16 MiB + 16 MiB = 32 MiB
    #[must_use]
    pub fn brotli_state_bytes(max_brotli_wbits: u8) -> u64 {
        debug_assert!((10..=RFC7932_MAX_BROTLI_WBITS).contains(&max_brotli_wbits));
        (1_u64 << max_brotli_wbits)
            .checked_add(BROTLI_DECODER_FIXED_CHARGE_BYTES)
            .expect("2^wbits + the 16 MiB fixed charge fits u64 for wbits <= 24")
    }

    /// Round-45 H2: THE canonical per-stream footprint, now
    /// with CHECKED arithmetic throughout (round-44's `const
    /// fn` used unchecked `<<`, `*`, `+`). Returns
    /// `Err(AuctioneerBootError::BudgetArithmeticOverflow)` on
    /// any overflow and `WbitsOutOfRange` on a bad window.
    /// Still the ONE source of truth — `validate_isolate_limits`
    /// (round-99 creative caching — replaces the struck `install_admission`), the
    /// proof table, and the allocator harness all call this.
    #[must_use]
    pub fn per_stream_bytes(
        max_transport_bytes: u64,
        max_decoded_bytes: u64,
        max_brotli_wbits: u8,
        max_brotli_layers: u8,
        guest_decoder_bound_bytes: u64,
                                 // round-134 #4: FIVE parameters —
                                 // the mode parameter and the Option
                                 // are RETIRED. The helper computes
                                 // BOTH decoder-cost arms and takes
                                 // their checked MAX; adapters with
                                 // no in-guest decode pass 0.
    ) -> Result<u64, AuctioneerBootError> {
        let wbits = validate_wbits(max_brotli_wbits)?;
        let ovf = || AuctioneerBootError::BudgetArithmeticOverflow;
        if max_brotli_layers > LAYERS_MAX {
            return Err(AuctioneerBootError::LayerCountExceedsMax {
                requested: max_brotli_layers, max: LAYERS_MAX,
            });
        }
        let layers = u64::from(max_brotli_layers);
        // auctioneer's own passthrough chain — ALWAYS a candidate:
        // deflate/stacked/unknown responses reach auctioneer's decoder
        // on every adapter (round-133 #6), so no mode can exclude
        // it. The explicit layer argument drives the brotli count
        // (round-125 #1).
        let chunk_staging = (LAYERS_MAX as u64)
            .checked_mul(MAX_CHUNK_SIZE as u64).ok_or_else(ovf)?;
        let brotli = layers
            .checked_mul(brotli_state_bytes(wbits)).ok_or_else(ovf)?;
        let flate = (u64::from(LAYERS_MAX)
                .checked_sub(layers).ok_or_else(ovf)?)
            .checked_mul(FLATE2_STATE_BYTES).ok_or_else(ovf)?;
        let auctioneer_chain = chunk_staging
            .checked_add(brotli).ok_or_else(ovf)?
            .checked_add(flate).ok_or_else(ovf)?;
        // round-134 #4: one stream takes ONE disposition, but
        // EITHER can occur — the budget charges the maximum of the
        // two arms (the adapter-reported in-guest state vs auctioneer's
        // own chain).
        let decoder = auctioneer_chain.max(guest_decoder_bound_bytes);
        max_transport_bytes
            .checked_add(max_decoded_bytes).ok_or_else(ovf)?
            .checked_add(decoder).ok_or_else(ovf)
    }

    // Round-44 B4 worked example — the ONLY per-stream
    // numbers any section may quote, all derived from the
    // helper above with the shipped 256 KiB budgets:
    //
    //   wbits 22 (default; round-140 #1 — upstream's 16 MiB
    //   fixed decoder charge + the ring):
    //     transport      0.25 MiB
    //     decoded        0.25 MiB
    //     chunk staging  3 × 8 KiB      = 0.023 MiB
    //     brotli × 1     16 MiB + 4 MiB = 20.00 MiB
    //     flate2 × 2     2 × 128 KiB    = 0.25  MiB
    //     ────────────────────────────────────────
    //     per_stream                    ≈ 20.77 MiB (21_782_528)
    //
    //   wbits 24 (deployer opt-in):
    //     brotli × 1     16 MiB + 16 MiB = 32.00 MiB
    //     per_stream                     ≈ 32.77 MiB

    // crates/auctioneer-adapter-cloudflare/src/limits.rs
    pub const ISOLATE_MAX_BYTES: u64 = 128 * 1024 * 1024;
        // 128 MB per isolate — SHARED across all
        // in-flight requests, code, GVL data, fanout
        // buffers, AND the JS-side heap. Round-43 H4:
        // Cloudflare labels this "128 MB" in decimal;
        // auctioneer's binary-MiB constant (134_217_728) is
        // LARGER than the decimal limit (128_000_000).
        // Use the decimal value to stay inside the
        // documented ceiling.
    pub const ISOLATE_MAX_BYTES_DECIMAL: u64 = 128_000_000;
    // ─── Round-44 foundations: these are TWO DIFFERENT limits.
    // Rounds 38-43 conflated them under the name
    // "subrequest cap", which made the 6 read like a
    // budget on how many bidders CF lets you call. It
    // isn't. ───

    /// Simultaneous outgoing CONNECTIONS that may be
    /// waiting for response HEADERS at once. A connection
    /// leaves this set as soon as headers arrive, even
    /// while its body is still streaming. This is the
    /// limit auctioneer's 6-stream admission default sits
    /// beside.
    pub const MAX_SIMULTANEOUS_CONNECTIONS: u32 = 6;

    /// TOTAL subrequests permitted per invocation —
    /// 50 free / 10,000 paid (round-45 foundations: the paid ceiling
    /// is 10,000, not the 1,000 rounds 38-44 carried).
    /// Nothing to do with the 6 above. auctioneer counts ALL
    /// outbound calls against this, not just bidder calls:
    /// a GVL fetch, an analytics beacon, and each bidder
    /// request are separate subrequests. So the boot WARN
    /// compares the ADAPTER's resolved `max_outbound_requests`
    /// (bidders + non-bidder subrequests per auction) — NOT
    /// `max_bidders_per_auction`, which undercounts — against
    /// the free-plan ceiling. Round-45 foundations: keying the check
    /// on bidder count alone let a 40-bidder + GVL-fetch +
    /// beacon config (42 subrequests) pass silently while
    /// still overrunning the 50 free ceiling under load.
    pub const SUBREQUESTS_FREE_PLAN: u32 = 50;
    pub const SUBREQUESTS_PAID_PLAN: u32 = 10_000;
    pub const WORKER_UPLOAD_MAX_COMPRESSED_BYTES: u64 = 10_000_000;
        // Round-43 H4: decimal, per Cloudflare docs
        // ("10 MB after compression"; 3 MB on free plan).
    pub const WORKER_UNCOMPRESSED_SCRIPT_MAX_BYTES: u64 = 64_000_000;
        // Round-43 H4: decimal ("64 MB before
        // compression"), includes WASM module + data
        // segments where the GVL blobs live.
    /// Round-43 B1: CF accepts the default 1 MiB Brotli
    /// window. Per-stream drops from 72.5 MiB to ~6.3 MiB,
    /// which restores concurrent fanout (see the proof
    /// table below).
    pub const MAX_BROTLI_WBITS: u8 = DEFAULT_MAX_BROTLI_WBITS;

    // crates/auctioneer-adapter-fastly/src/limits.rs
    pub const HEAP_MAX_BYTES: u64 = 128_000_000;   // decimal, round-43 H4
    pub const MAX_CONCURRENT_REQUESTS: u32 = 8;
    pub const PACKAGE_MAX_COMPRESSED_BYTES: u64 = 100_000_000;
        // Round-42 H4 correction: Fastly's documented
        // compiled-package ceiling is 100 MB (was falsely
        // stated as 50 MB in round-41 H4). See docs.
        // fastly.com/products/compute-resource-limits.
    pub const MAX_BROTLI_WBITS: u8 = DEFAULT_MAX_BROTLI_WBITS;

    // crates/auctioneer-adapter-spin/src/limits.rs
    pub const HEAP_MAX_BYTES: u64 = 128_000_000;
    pub const MAX_CONCURRENT_REQUESTS: u32 = 8;
    pub const MAX_BROTLI_WBITS: u8 = DEFAULT_MAX_BROTLI_WBITS;

    // crates/auctioneer-adapter-axum/src/limits.rs
    //
    // Round-44 H5: a REAL implementation, and an honest
    // fallback. Round-43 H5 left `detect_process_max_bytes
    // () -> u64 { … }` as a placeholder and called 512 MiB
    // "conservative" — but a 256 MiB non-cgroup host would
    // be overcommitted 2×, which is the same class of bug
    // as round-42's fictional 4 GiB, just smaller.
    //
    // Detection order (first hit wins; the winning source
    // is logged at boot so operators can see it):
    //   1. `AUCTIONEER__RUNTIME__MAX_MEMORY_BYTES` — explicit
    //      deployer override.
    //   2. This process's cgroup, resolved properly (NOT a
    //      hardcoded path — a process is rarely in the root
    //      cgroup):
    //        - read `/proc/self/cgroup` to get the relative
    //          path,
    //        - v2: `/sys/fs/cgroup/<path>/memory.max`,
    //          walking up to parent cgroups since a limit
    //          may be set on an ancestor; literal `"max"`
    //          means unlimited → keep walking, then fall
    //          through,
    //        - v1: `/sys/fs/cgroup/memory/<path>/
    //          memory.limit_in_bytes`; reject the
    //          "unlimited" sentinel (`u64::MAX` rounded to
    //          the page size — commonly
    //          `0x7FFF_FFFF_FFFF_F000`) rather than
    //          believing it.
    //   3. `getrlimit(RLIMIT_AS)` — a deployer may bound
    //      address space without a cgroup. Ignore
    //      `RLIM_INFINITY`.
    //   4. Total system RAM via `sysinfo`, × 0.5 — a
    //      bare-metal / VM deploy with no limits at all.
    //
    // If NONE of 1-4 yields a value, auctioneer **refuses to
    // boot** with `AuctioneerBootError::MemoryCeilingUnknown`,
    // whose message names the override env var. Round-44
    // H5: this replaces the invented fallback. Guessing a
    // ceiling we cannot observe is exactly what produced
    // the 4 GiB and 512 MiB numbers; requiring one line of
    // deployer input is better than silently overcommitting
    // a small host.
    //
    // Lives in the Axum adapter because `/proc`, `/sys`,
    // and `getrlimit` are native-only — portable core must
    // not read them (round-41 B4).
    pub fn detect_process_max_bytes() -> Result<u64, AuctioneerBootError>;

    pub const MAX_BROTLI_WBITS: u8 = DEFAULT_MAX_BROTLI_WBITS;
    ```
    Round-44 H5 regressions (Axum adapter,
    `tests/limits.rs`):
    - `detect_prefers_env_override()`.
    - `detect_reads_cgroup_v2_from_proc_self_cgroup()` —
      fixture `/proc/self/cgroup` + `memory.max` under a
      tempdir root, injected via a test-only path prefix.
    - `detect_walks_to_ancestor_when_leaf_is_max()`.
    - `detect_rejects_v1_unlimited_sentinel()` — asserts
      `0x7FFF_FFFF_FFFF_F000` is NOT treated as a real
      ceiling.
    - `detect_falls_back_to_rlimit_as()`.
    - `detect_errors_when_nothing_detectable()` — asserts
      `MemoryCeilingUnknown`, and that its message names
      `AUCTIONEER__RUNTIME__MAX_MEMORY_BYTES`.

    **Whole-isolate memory budget (round-43 B1 canonical,
    supersedes round-42 B3's decomposition which left CF
    with a 51 MiB budget against a 72.5 MiB per-stream
    weight — i.e. the shipped default could not boot).**
    The 128 MB CF isolate limit is SHARED across the WASM
    module, WASM linear memory, GVL data segments, parse
    caches, the JS-side heap, and every in-flight body
    stream. Canonical decomposition (Cloudflare, worst
    case, decimal MB per round-43 H4):

    **PROVISIONAL (round-46 B4/B5).** This decomposition is NOT
    a proof yet — it double-books nothing only once the admission
    track is redesigned (see the round-46 banner). Two defects the
    round-46 review established are reflected below: the parsed-GVL
    cache is COUNT-bounded (no honest byte figure — round-46 B3),
    and retained `Arc<GvlInner>`s held by in-flight
    `PrivacyContext`s are a SEPARATE line the earlier table omitted
    (round-46 B4). The table now also leaves explicit headroom
    instead of summing to exactly the ceiling.

    | Component                                 | Bytes           | Notes                                                    |
    |-------------------------------------------|----------------:|----------------------------------------------------------|
    | WASM code + module tables                 | ~8 MB           | auctioneer + edgezero + std                                  |
    | JS shim + V8 baseline heap                | ~8 MB           | round-43 H4: CF's 128 MB includes JS, not just Wasm       |
    | GVL `include_bytes!` data segments (GDPR fanout)  | ≤ 24 MB         | round-43 B1: 24 MB default on ALL wasm adapters           |
    | GVL parsed cache (GDPR fanout)                    | envelope, unproven | count-bounded by `GVL_PARSE_CACHE_MAX_ENTRIES` (round-46 B3); no byte const |
    | Retained GVL Arcs (in-flight contexts)    | unbounded today | round-46 B4: bounded only after the admission redesign (admit-before-resolve, or drop the Arc pre-fanout) |
    | Runtime scratch (headers, stack)          | ~4 MB           |                                                          |
    | **Fanout body streams (bounded fanout)** *[STRUCK — round-107 stored requests and accounts: superseded by the §7/§8 envelope]* | **≤ 60 MB**     | Passed as `AdmissionBootConfig.adapter_max_memory_bytes`; the GVL cache + retention reservation must be SUBTRACTED from this once B5's single boot budget owns them |
    | **Headroom**                              | **> 0**         | round-46 B4: the sum must stay strictly under 128 MB, not equal it |
    | **Total**                                 | **< 128 MB**    | CF isolate ceiling — with margin, not at the edge         |

    **Round-44 B2+B4 resolution (round-140 #1: the per-stream
    numbers below predate the 16 MiB fixed brotli charge and are
    INVALID with the rest of this table — the round-118 F03
    re-derivation uses the corrected `brotli_state_bytes` model,
    under which wbits-22 per_stream is ≈ 20.77 MiB and these
    stream counts do not survive).** Per the canonical
    `per_stream_bytes()` helper above, at the WBITS-22
    default (= the reference encoder's default) with at
    most one brotli layer:

    ```
    per_stream = max_transport(256 KiB)
               + max_decoded(256 KiB)
               + chunk staging  3 × 8 KiB     = 0.023 MiB
               + brotli × 1     4 MiB + 512K  = 4.50  MiB
               + flate2 × 2     2 × 128 KiB   = 0.25  MiB
             ≈ 5.27 MiB
    ```

    CF: fanout headroom 60 MB → byte budget 75% = 45 MB →
    `45 MB / 5.27 MiB ≈ 8` streams. auctioneer defaults CF
    to **6** (33.2 MB charged) — a MEMORY cap. It happens to
    equal the platform's 6-simultaneous-connection number, but
    that is coincidence, not a shared mechanism (round-46 auction core:
    the connection limit counts only header-wait, the
    subrequest cap is 50 free / 10,000 paid — both distinct
    from this memory cap; see the `limits.rs` note above).
    **The default boots, concurrent fanout holds, and
    no conformant encoder is rejected** — the round-41 B6
    / round-42 B4 `bounded fanout-partial` downgrade stays REVERTED
    (round-43 B1's outcome survives round-44's corrected
    premise).

    `GVL_BUNDLE_MAX_RAW_BYTES` is **24 MB on every wasm
    adapter** (CF, Fastly, Spin) — round-42 lowered CF
    only, leaving Fastly/Spin strictly worse. Axum keeps
    the 60 MB default (its ceiling is detected, not
    fixed — round-44 H5). Round-44 H1 sweeps the GDPR fanout
    sections that still said 60 MB for the wasm targets.

    `WORKER_UNCOMPRESSED_SCRIPT_MAX_BYTES = 64_000_000`
    is checked at build time by the CI gate (round-43 H4)
    that parses wrangler's reported Total Upload figures
    rather than measuring the output directory.

    **BodySource split (round-39 B8)** — decides where the
    decoder runs and therefore where its memory lives:
    ```rust
    // edgezero-core::proxy addition.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum BodySource {
        /// Raw compressed bytes handed to auctioneer. auctioneer runs
        /// its 3-layer decoder pipeline inside its own
        /// process/isolate. Guest memory bound includes
        /// `per_stream_bytes(.., RawCompressed, wbits)`.
        RawCompressed,
        /// Adapter decompressed the body IN THE GUEST (Rust
        /// side of the adapter runs the decoder). auctioneer gets
        /// identity bytes but the memory has already been
        /// charged to the guest heap. Round-39 B8: the
        /// admission gate MUST include the decoder-memory
        /// bound reported by the adapter. Adapters report
        /// their measured guest decoder state via
        /// `ProxyClient::guest_decoder_memory_bound()`.
        GuestDecoded,
        /// Runtime/host decompressed the body OUTSIDE the
        /// guest (JavaScript-side Cloudflare fetch decoder,
        /// Fastly host runtime). auctioneer gets identity bytes
        /// and the decoder memory is charged to the host
        /// process, not the guest. Zero decoder allocation
        /// on the guest side.
        HostDecoded,
    }
    ```
    Round-40 B2 adapter capability mapping (retires
    round-39 B8's "CF is HostDecoded" misread — the
    pinned CF adapter runs a RUST-SIDE decoder in the
    guest, so its actual body source is `GuestDecoded`):
    - **Cloudflare** (Workers, guest-side Rust decoder):
      `&[RawCompressed, GuestDecoded]` — ROUND-144 #3 corrects the
      round-40 B2 `&[GuestDecoded]`-only reading: at `120dd2aa`
      EdgeZero's outbound contract passes unknown, parameterized,
      repeated and STACKED encodings through RAW on every adapter,
      and the Cloudflare implementation preserves those encoded
      bytes and headers rather than decoding them (outbound.rs) —
      so auctioneer's own chain decodes deflate/stacked there, and the
      profile MUST carry `RawCompressed` or the max-rule charge
      under-accounts that path. Historic reading: `&[GuestDecoded]`
      because the pinned adapter imports
      `flate2::read::GzDecoder` +
      `brotli_decompressor::DecompressorReader` at
      [proxy.rs:4](https://github.com/IABTechLab/edgezero/blob/9e661ae/crates/edgezero-adapter-cloudflare/src/proxy.rs#L4)
      and runs them at
      [proxy.rs:137](https://github.com/IABTechLab/edgezero/blob/9e661ae/crates/edgezero-adapter-cloudflare/src/proxy.rs#L137).
      That is IN-GUEST decompression: bytes reach auctioneer
      already decoded, but the decoder charged
      its decoder state to the isolate heap before
      auctioneer ever saw them. Round-39 B8's HostDecoded
      classification was wrong — the JS-side
      `transform_stream` invocation is unrelated to
      gzip/brotli.
    - **Fastly** (Compute@Edge, guest-side decoder):
      `&[RawCompressed, GuestDecoded]`. GuestDecoded
      charges the same decoder state per stream.
    - **Spin** (guest-side, buffer-then-decode):
      `&[RawCompressed, GuestDecoded]`. Same as Fastly.
    - **Axum** (reqwest + tokio, guest-side):
      `&[RawCompressed, GuestDecoded]`. Round-40 B2:
      dropped `HostDecoded` — no pinned adapter honours
      it. If a future host-decoded platform lands, add
      the variant back to that adapter's mapping.

    Strategy negotiation (round-40 B2): under the
    current adapter fleet, `HostDecoded` is never
    available, so the negotiation reduces to
    `RawCompressed` (auctioneer controls the pipeline) if
    available, else `GuestDecoded`. The
    HostDecoded branch is retained in the type / policy
    for forward-compatibility only.

    > **⚠ STRUCK — NON-NORMATIVE HISTORY (struck round-96/97; visible
    > marker added round-107 stored requests and accounts). The admission/budget shapes in this
    > block (`AdmissionSemaphore` / `AdmissionBootConfig` /
    > `install_admission`, per-adapter stream-default constants) are
    > RETIRED. The "Admission + whole-isolate budget — the round-98
    > design" section + the Fanout-2 deliverable list own every one of
    > these decisions; in particular the Cloudflare default is
    > `max_active_body_streams = 2` from the §8 envelope (round-105
    > B8) — NOT the 6 below. Do not implement anything from this
    > block.**

    **Runtime concurrency model** (round-40 B3 canonical
    ownership; retires round-39 B6's "resolved runtime
    config" story which conflated the per-request async
    config-store load
    [auction.rs:261](../../../crates/auctioneer-core/src/handlers/auction.rs#L261)
    with the boot-time singleton). The `AdmissionSemaphore`
    is an ADAPTER-owned process singleton, constructed
    once at `install_host_primitives` time from
    **build-baked** configuration:
    - `HostConfig.runtime_defaults.max_active_body_streams`
      — new field on `crates/auctioneer-config/data/host-
      defaults.toml`, baked into the binary at compile
      time.
    - `AUCTIONEER__RUNTIME__MAX_ACTIVE_BODY_STREAMS` env var —
      deploy-time override read by `install_host_primitives`;
      wins over the baked default if present. Env vars are
      part of the deployment manifest (Cloudflare
      `wrangler.toml`, Fastly `fastly.toml`, Spin
      `spin.toml`), so a change requires a redeploy — the
      correct semantics for admission caps.

    **Config-update semantics (round-40 B3):** the
    async per-request store overlay
    (`AuctioneerAppConfig.runtime.max_active_body_streams`
    from `AppConfig::from_store`) has NO effect on the
    admission cap; it is preserved on the struct for
    forward-compatibility only. Live traffic sees the
    at-boot value. Deployers who need to change the cap
    push a new baked build or update the env var and
    redeploy; the health endpoint (Task 4a) reports the
    effective at-boot cap so operators can confirm what's
    live vs what's configured in the async store.

    ```rust
    // crates/auctioneer-core/src/host_primitives/registry.rs
    // Round-41 B2 + B4 canonical (retires round-40 B3's
    // <C,R> installer + std::env reads):
    //  - Preserves GDPR fanout Task 11's <C, R, T, S> generic
    //    signature (Task 11 wires timer + strategy;
    //    dropping either would overwrite that capability).
    //  - The admission bootstrap is a SEPARATE installer
    //    called BEFORE `install_host_primitives`. Adapters
    //    call it once per process from their fetch handler
    //    (CF calls it inside `#[event(fetch)]` on every
    //    invocation — the installer is idempotent).
    //  - The adapter passes typed `AdmissionBootConfig` values
    //    it resolved from ITS host API (worker::Env for CF,
    //    fastly::ConfigStore for Fastly, spin::Variables
    //    for Spin, std::env for Axum). Portable core NEVER
    //    reads std::env — that's Axum-specific.

    // ─── Round-43 B2 + B3 canonical ───
    //
    // B3: `BootEnvelope` had no owner — round-42 said it
    // "lives on admission_handle()" but that handle
    // returned a bare `Arc<AdmissionSemaphore>`, so
    // split.rs's overlay clamp had nothing to read.
    // B2: the installer accepted `per_stream_weight_bytes`
    // from the adapter and never cross-checked it against
    // `boot_body_source` / `guest_decoder_bound_bytes`, so
    // an adapter reporting a larger real decoder footprint
    // stayed undercharged.
    //
    // Round-43 fixes both by having CORE derive the weight
    // from independently-supplied components and storing
    // the whole resolved bundle behind one handle.

    pub(crate) static ADMISSION: OnceLock<Arc<InstalledAdmission>> = OnceLock::new();

    /// Round-43 B3: the single immutable admission bundle.
    /// `install_admission` computes every field; nothing
    /// downstream recomputes.
    pub struct InstalledAdmission {
        pub semaphore: AdmissionSemaphore,
        /// Round-43 B3: per-field caps split.rs reads for
        /// the overlay clamp (round-42 B7).
        pub envelope: BootEnvelope,
        pub body_source: BodySource,
        pub decoder_bound_bytes: u64,
        /// The weight `semaphore.acquire()` charges. Derived
        /// by core, not supplied by the adapter.
        pub per_stream_weight_bytes: u64,
        /// Round-45 H2: the exact config this bundle was
        /// installed from, so a second `install_admission`
        /// call can be checked for identity (idempotent) vs
        /// conflict (fail loud).
        pub boot_config: AdmissionBootConfig,
    }

    pub fn admission_handle() -> Arc<InstalledAdmission> {
        ADMISSION
            .get()
            .expect("install_admission called before request handling")
            .clone()
    }

    /// Round-43 B2 canonical boot config: COMPONENTS only.
    /// The adapter reports what it measured / read from its
    /// platform; core does the arithmetic. Round-42's
    /// `per_stream_weight_bytes` field is REMOVED — letting
    /// the adapter pre-compute the weight is exactly how the
    /// decoder bound got discarded.
    // Round-45 H2: `PartialEq, Eq` so `install_admission`
    // can reject a non-identical reinstall.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct AdmissionBootConfig {
        /// Cap on concurrent body streams, resolved from the
        /// adapter's platform config (worker::Env on CF,
        /// fastly::ConfigStore, spin::Variables, cgroup +
        /// env on Axum). Falls back to the baked default.
        pub max_active_body_streams: u32,
        /// Guest memory available to FANOUT — the adapter has
        /// already subtracted WASM + GVL + scratch per the
        /// whole-isolate decomposition above.
        pub adapter_max_memory_bytes: u64,
        /// The adapter's chosen source from its
        /// `supported_body_sources()` (round-41 B1: required,
        /// no default).
        pub boot_body_source: BodySource,
        /// Round-45 B6: the bound the adapter ENFORCES for
        /// the boot policy — the adapter computes it by
        /// calling its own `guest_decoder_memory_bound(&
        /// boot_policy)`, where `boot_policy.max_brotli_wbits
        /// = max_brotli_wbits` below. Core uses THIS value
        /// (not a hardcoded constant) when `boot_body_source
        /// == GuestDecoded`. An adapter that can't enforce
        /// Prereq 8 passes `u64::MAX`, failing boot loudly.
        pub guest_decoder_bound_bytes: u64,
        /// Round-45 B6: the accepted Brotli window ceiling
        /// the adapter will enforce (Prereq 8). Core passes
        /// the SAME value into `ResponseBodyPolicy.
        /// max_brotli_wbits` on every request, so the enforced
        /// window and the charged weight agree. On the
        /// RawCompressed path core derives auctioneer's own
        /// decoder state from it.
        pub max_brotli_wbits: u8,
        /// Resolved per-bidder budget ceilings. Core clamps
        /// overlays to these (round-42 B7) and folds them
        /// into the derived weight.
        pub max_transport_bytes: u64,
        pub max_decoded_bytes: u64,
    }

    /// Round-43 B1 + B2 + B3 canonical admission installer.
    /// Idempotent (round-41 B2) so CF's per-fetch call at
    /// `crates/auctioneer-adapter-cloudflare/src/lib.rs:16`
    /// stays valid.
    pub fn install_admission(cfg: AdmissionBootConfig) -> Result<(), AuctioneerBootError> {
        // Round-43 B2: CORE derives the weight from the
        // components. There is exactly one arithmetic site,
        // so the validated weight IS the charged weight
        // (round-42 B1's requirement) by construction
        // rather than by an assert the adapter could
        // contradict.
        // Round-45 H2: all arithmetic is checked (the
        // per_stream_bytes helper returns Result now).
        // Round-127 #7: ONE canonical arithmetic path for every
        // mode — the match only maps the adapter-capability
        // BodySource onto (DecodeMode, guest bound); all charging
        // lives in the 6-arg helper.
        let (mode, guest_bound) = match cfg.boot_body_source {
            BodySource::RawCompressed => (DecodeMode::RawCompressed, None),
            BodySource::GuestDecoded => (DecodeMode::GuestDecoded,
                Some(cfg.guest_decoder_bound_bytes)),
            BodySource::HostDecoded => (DecodeMode::HostDecoded, None),
        };
        let per_stream_weight_bytes = per_stream_bytes(
            cfg.max_transport_bytes,
            cfg.max_decoded_bytes,
            mode,
            cfg.max_brotli_wbits,
            cfg.max_brotli_layers,
            guest_bound,
        )?;

        // Round-44 B4 floor, round-45 H2 checked: a
        // GuestDecoded adapter that under-reports cannot slip
        // through. `validate_wbits` runs inside per_stream_
        // bytes, so a bad window fails here too.
        if matches!(cfg.boot_body_source, BodySource::GuestDecoded) {
            let auctioneer_equivalent = per_stream_bytes(
                0, 0, DecodeMode::RawCompressed,
                cfg.max_brotli_wbits, cfg.max_brotli_layers, None,
            )?;
            if cfg.guest_decoder_bound_bytes < auctioneer_equivalent {
                return Err(AuctioneerBootError::GuestDecoderBoundTooSmall {
                    reported: cfg.guest_decoder_bound_bytes,
                    minimum: auctioneer_equivalent,
                });
            }
        }

        let byte_budget = cfg.adapter_max_memory_bytes
            .checked_mul(3).ok_or(AuctioneerBootError::BudgetArithmeticOverflow)? / 4;
        let peak = per_stream_weight_bytes
            .checked_mul(u64::from(cfg.max_active_body_streams))
            .ok_or(AuctioneerBootError::BudgetArithmeticOverflow)?;
        if peak > byte_budget {
            return Err(AuctioneerBootError::BudgetsExceedAdapterMemory {
                per_stream: per_stream_weight_bytes,
                peak,
                adapter_max: cfg.adapter_max_memory_bytes,
                streams_cap: cfg.max_active_body_streams,
                body_source: cfg.boot_body_source,
            });
        }

        // Round-45 H2: the byte budget is a u64 but the
        // tokio Semaphore takes usize permits. On 32-bit
        // wasm targets that cast can TRUNCATE — check it.
        let byte_budget_usize = usize::try_from(byte_budget)
            .map_err(|_| AuctioneerBootError::ByteBudgetExceedsUsize { byte_budget })?;
        let per_stream_usize = usize::try_from(per_stream_weight_bytes)
            .map_err(|_| AuctioneerBootError::ByteBudgetExceedsUsize {
                byte_budget: per_stream_weight_bytes,
            })?;

        let installed = InstalledAdmission {
            semaphore: AdmissionSemaphore::new(
                byte_budget_usize,
                cfg.max_active_body_streams,
                per_stream_usize,
            ),
            envelope: BootEnvelope {
                max_transport_bytes: usize::try_from(cfg.max_transport_bytes)
                    .map_err(|_| AuctioneerBootError::ByteBudgetExceedsUsize {
                        byte_budget: cfg.max_transport_bytes })?,
                max_decoded_bytes: usize::try_from(cfg.max_decoded_bytes)
                    .map_err(|_| AuctioneerBootError::ByteBudgetExceedsUsize {
                        byte_budget: cfg.max_decoded_bytes })?,
            },
            body_source: cfg.boot_body_source,
            decoder_bound_bytes: cfg.guest_decoder_bound_bytes,
            per_stream_weight_bytes,
            // Round-45 H2: store the full boot config so a
            // second install can be checked for identity.
            boot_config: cfg,
        };

        // Round-45 H2: idempotent for the CF per-fetch call
        // ONLY when the config is IDENTICAL. A DIFFERENT
        // second config is a bug (some code path installed
        // conflicting admission) and must fail loudly, not
        // silently keep the first.
        match ADMISSION.get() {
            None => {
                ADMISSION.set(Arc::new(installed))
                    .map_err(|_| AuctioneerBootError::AdmissionInstallRace)?;
                Ok(())
            }
            Some(existing) if existing.boot_config == cfg => Ok(()), // idempotent
            Some(existing) => Err(AuctioneerBootError::AdmissionReinstallConflict {
                existing: existing.boot_config,
                attempted: cfg,
            }),
        }
    }
    ```
    `AdmissionBootConfig` derives `PartialEq, Eq` so the
    identity check compiles (round-45 H2). New
    `AuctioneerBootError` variants: `WbitsOutOfRange { wbits }`,
    `ByteBudgetExceedsUsize { byte_budget }`,
    `AdmissionReinstallConflict { existing, attempted }`,
    `AdmissionInstallRace`.
    `assert_budgets_admissible` as a standalone function is
    REMOVED (round-43 B2) — its logic is inlined above so
    there is no second implementation to drift. The
    round-42 helper that "checks the reported bound exceeds
    72 MiB then calculates with the fixed 72 MiB constant"
    is gone.

    Round-43 B3 call-site update: `ProductionFanout::new`
    reads `admission_handle().semaphore`; split.rs's
    `resolve_budget` reads `admission_handle().envelope`.

    `install_host_primitives<C, R, T, S>(clock_fn, rng_fn,
    timer_fn, strategy_fn)` is UNCHANGED from GDPR fanout Task 11
    (round-41 B2: bounded fanout does NOT touch its signature).
    Admission bootstrap is the separate `install_admission`
    installer above, called by the adapter BEFORE
    `install_host_primitives`.

    New `AuctioneerBootError` variants (round-43 B2 +
    round-45 H2):
    ```rust
    #[error("adapter reported guest decoder bound {reported} < required minimum {minimum}")]
    GuestDecoderBoundTooSmall { reported: u64, minimum: u64 },
    #[error("brotli window bits {wbits} outside [10, {}]", RFC7932_MAX_BROTLI_WBITS)]
    WbitsOutOfRange { wbits: u8 },
    #[error("admission byte budget {byte_budget} exceeds usize on this target")]
    ByteBudgetExceedsUsize { byte_budget: u64 },
    #[error("admission arithmetic overflowed u64")]
    BudgetArithmeticOverflow,
    #[error("admission already installed with a different configuration \
             (existing: {existing:?}, attempted: {attempted:?})")]
    AdmissionReinstallConflict {
        existing: AdmissionBootConfig,
        attempted: AdmissionBootConfig,
    },
    #[error("admission install lost a boot race (OnceLock::set failed)")]
    AdmissionInstallRace,
    ```
    Round-45 H2 requires `AdmissionBootConfig` to derive
    `Clone, Debug, PartialEq, Eq` — the conflict variant
    stores both configs and the identity check compares
    them. All its fields (`u64`/`u32`/`u8`/`BodySource`)
    are already `Eq`, so the derive is free.
    `ProductionFanout::new(proxy, clock, ...)` calls
    `admission_handle()` to get its shared `Arc<...>`.

    Config field on `AuctioneerAppConfig.runtime`:
    ```rust
    // crates/auctioneer-core/src/config.rs additions.
    #[derive(Debug, Deserialize, Validate, Default)]
    pub struct RuntimeConfig {
        // Round-39 B6: cap on simultaneous in-flight
        // bidder body streams per ADAPTER PROCESS
        // (isolate on CF/Fastly/Spin; OS process on Axum).
        // Read once at boot; runtime overlay changes are
        // ignored (see config-update semantics above).
        #[serde(default)]
        #[validate(range(min = 1, max = 4096))]
        pub max_active_body_streams: Option<u32>,
    }
    ```

    > **⚠ STRUCK — NON-NORMATIVE HISTORY (struck round-96/97; visible
    > marker added round-107 stored requests and accounts). The admission/budget shapes in this
    > block (`AdmissionSemaphore` / `AdmissionBootConfig` /
    > `install_admission`, per-adapter stream-default constants) are
    > RETIRED. The "Admission + whole-isolate budget — the round-98
    > design" section + the Fanout-2 deliverable list own every one of
    > these decisions; in particular the Cloudflare default is
    > `max_active_body_streams = 2` from the §8 envelope (round-105
    > B8) — NOT the 6 below. Do not implement anything from this
    > block.**

    **Default streams per adapter** (round-44 B2
    canonical, supersedes round-40/41/42's 1-stream
    defaults — forced by the fixed 24 MiB decoder
    constant, and shown by round-43 B1 not to boot at
    all — and correcting round-43's WBITS-20 derivation,
    whose compat premise round-44 B2 disproved). With
    `brotli_state_bytes(22) = 4.50 MiB` and
    `MAX_BROTLI_LAYERS = 1`, `per_stream_bytes(..)`
    returns ~5.27 MiB and every adapter admits real
    concurrency at the reference encoder's own default
    window. Constants live in each adapter's `limits.rs`
    (round-42 B6 — no cfg-guards on auctioneer-core):
    ```rust
    // crates/auctioneer-adapter-cloudflare/src/limits.rs
    pub const DEFAULT_MAX_ACTIVE_STREAMS: u32 = 6;
        // Round-44 B2: 6 × 5.27 MiB = 31.6 MiB against a
        // 45 MB (42.9 MiB) byte budget — 75% of the 60 MB
        // fanout headroom. This is a MEMORY cap. Round-46
        // auction core: it is NOT the same thing as the platform's
        // 6-simultaneous-connection limit, and the two do not
        // "agree" by construction — that limit counts
        // connections only WHILE they wait for response
        // headers (a connection leaves the set once headers
        // arrive, even while its body streams), whereas an
        // admission permit is held through body EOF. The
        // subrequest cap is a third, separate limit (50 free
        // / 10,000 paid — round-45 foundations). The 6-stream default
        // is deliberately conservative and sits at/under all
        // three, but for the memory reason above, not because
        // it mirrors the connection number.

    // crates/auctioneer-adapter-fastly/src/limits.rs
    pub const DEFAULT_MAX_ACTIVE_STREAMS: u32 = 6;
        // Round-43 B1: Fastly's GVL default also drops to
        // 24 MB (round-42 lowered CF only, leaving
        // Fastly/Spin strictly worse — the reviewer's
        // finding). Same 60 MB headroom → same 6 streams.

    // crates/auctioneer-adapter-spin/src/limits.rs
    pub const DEFAULT_MAX_ACTIVE_STREAMS: u32 = 6;

    // crates/auctioneer-adapter-axum/src/limits.rs
    pub const DEFAULT_MAX_ACTIVE_STREAMS: u32 = 32;
        // Round-44 H5: sized against the DETECTED ceiling
        // from `detect_process_max_bytes()` — never a
        // guess. Worked example on a 512 MiB container:
        // headroom ≈ 512 - 8 (code) - 60 (GVL raw) - 24
        // (GVL cache) - 8 (scratch) = 412 MiB → budget
        // 309 MiB → 309 / 5.27 ≈ 58 streams, so the 32
        // default has ~2× headroom. On a 256 MiB
        // container the same arithmetic gives headroom
        // ≈ 156 MiB → budget 117 MiB → 22 streams, and
        // the boot check REJECTS the 32 default rather
        // than overcommitting; the deployer lowers
        // AUCTIONEER__RUNTIME__MAX_ACTIVE_BODY_STREAMS and
        // gets an honest config. That rejection is
        // precisely what a guessed 512 MiB fallback would
        // have skipped.
    ```

    **Proof table (round-44 B2).** Every row uses the
    adapter's REAL fanout headroom (round-42's table gave
    every adapter the full 96 MiB, which is what hid the
    boot failure) and the per-stream figure that
    `per_stream_bytes(256 KiB, 256 KiB, .., wbits=22)`
    returns — no hand-computed values (round-44 B4).
    Headroom/budget in decimal MB for the wasm adapters
    (matching the platform docs, round-43 H4); MiB for
    Axum, whose ceiling is detected.

    | Adapter | body_source | fanout headroom | budget (75%) | streams | per_stream | peak | admits? |
    |---|---|---:|---:|---:|---:|---:|---|
    | CF     | GuestDecoded  | 60 MB    | 45 MB   | 6  | 5.27 MiB | 31.6 MiB (33.2 MB) | **YES** |
    | Fastly | GuestDecoded  | 60 MB    | 45 MB   | 6  | 5.27 MiB | 31.6 MiB (33.2 MB) | **YES** |
    | Spin   | GuestDecoded  | 60 MB    | 45 MB   | 6  | 5.27 MiB | 31.6 MiB (33.2 MB) | **YES** |
    | Axum   | RawCompressed | 412 MiB (on a 512 MiB detected ceiling) | 309 MiB | 32 | 5.27 MiB | 169 MiB | **YES** |

    Axum's row is an EXAMPLE at one detected ceiling, not
    a constant — `detect_process_max_bytes()` supplies the
    real number at boot and the gate re-runs this
    arithmetic against it (round-44 H5).

    Regression tests (round-43 B1 — replaces round-42 B5's
    `admission_concurrency_gated_by_decoder_state`, which
    asserted a limitation that no longer exists):
    - `admission_default_cloudflare_boots_with_6_streams()`.
    - `admission_default_fastly_boots_with_6_streams()`.
    - `admission_default_spin_boots_with_6_streams()`.
    - `admission_axum_injected_512mib_ceiling_admits_default_streams()`
      — round-89 diagnostic qualification (replaces the retired
      `admission_default_axum_boots_under_conservative_fallback`): there
      is NO 512 MiB fallback (round-44 H5 made an undetectable ceiling a
      fail-closed boot error). INJECT a 512 MiB ceiling via the override
      env var and assert it admits the 32-stream default.
    - `admission_axum_no_detectable_ceiling_refuses_boot()`
      — round-89 diagnostic qualification: with none of the ceiling sources 1–4 available and
      no override, boot FAILS with
      `AuctioneerBootError::MemoryCeilingUnknown` (naming the override var).
      Proves the fail-closed rule at §Prereq, NOT a silent 512 MiB guess.
    - `admission_wbits_24_ceiling_rejects_default_streams()`
      — asserts an adapter that raises `max_brotli_wbits`
      to 24 (16.50 MiB brotli state → 17.27 MiB
      per-stream) FAILS the boot check at 6 streams
      (6 × 17.27 = 103.6 MiB vs a 42.9 MiB budget), so
      the wbits knob and the stream count can't silently
      disagree.
    - `admission_wbits_24_admits_at_two_streams()`
      — round-44 B2 companion: the same adapter DOES boot
      at `max_active_body_streams = 2` (34.5 MiB ≤ 42.9
      MiB). Documents the real trade a deployer makes
      when they need large-window brotli: 2 concurrent
      streams instead of 6.
    - `admission_default_wbits_accepts_reference_encoder_output()`
      — round-44 B2 regression: encodes a fixture with
      the pinned `brotli-8.0.4` encoder at its DEFAULT
      params (which emit `lgwin: 22`, per
      `enc/encode.rs:332`) and asserts auctioneer's WBITS
      gate ACCEPTS it. This is the test round-43's
      WBITS-20 ceiling would have failed, and it is the
      reason the default is 22.
    - `admission_guest_decoder_under_report_rejected()`
      — round-43 B2: a GuestDecoded adapter reporting a
      bound below `per_stream_bytes(0, 0, wbits, layers, 0)` (the
      auctioneer-chain floor — round-134 #4) gets
      `AuctioneerBootError::GuestDecoderBoundTooSmall`.

    **Admission semaphore (byte-weighted + count-capped).**
    Round-40 B4 canonical: the round-39 B6 sketch showed
    only a byte semaphore, mentioned
    `max_active_body_streams` in prose, and used
    `acquire_many_owned` in a way that would DEADLOCK
    when a single call's weight exceeds the total (the
    caller waits forever). Round-40 B4 wires TWO
    orthogonal admission gates and rejects impossible
    calls up-front.

    **Cargo.toml addition** (round-40 B4): the pinned
    workspace at
    [Cargo.toml:74](../../../Cargo.toml#L74) declares
    `tokio` without the `sync` feature. `tokio::sync::Semaphore`
    requires that feature — this task's
    Cargo.toml delta adds it to the `auctioneer-core`
    dependency block:
    ```toml
    tokio = { workspace = true, features = ["sync"] }
    ```

    ```rust
    // crates/auctioneer-core/src/host_primitives/admission.rs
    use std::sync::Arc;
    use tokio::sync::{OwnedSemaphorePermit, Semaphore};

    /// Round-40 B4: TWO independent admission gates.
    /// `bytes` bounds concurrent memory footprint;
    /// `streams` bounds concurrent connection count.
    /// The weight-per-call is fixed at boot from the
    /// resolved per-stream budget, so a single acquire
    /// can never exceed the semaphore capacity —
    /// weight-vs-capacity is verified up-front at
    /// construction.
    pub struct AdmissionSemaphore {
        bytes: Arc<Semaphore>,
        streams: Arc<Semaphore>,
        per_stream_weight: u32,
    }

    impl AdmissionSemaphore {
        /// `total_bytes` = adapter_max_memory × 3 / 4.
        /// `total_streams` = configured concurrent-stream cap.
        /// `per_stream_weight` = per-call memory footprint,
        /// derived from resolved budgets + body source.
        pub fn new(
            total_bytes: u64,
            total_streams: u32,
            per_stream_weight: u64,
        ) -> Self {
            // Round-40 B4: reject weight > total_bytes at
            // construction — an acquire that could never
            // complete would deadlock the first call.
            let weight_u32 = u32::try_from(per_stream_weight)
                .expect("per-stream weight fits u32 (admission gate caps this at boot)");
            let byte_cap = usize::try_from(total_bytes)
                .unwrap_or(usize::MAX / 2);
            assert!(
                (weight_u32 as usize) <= byte_cap,
                "AdmissionSemaphore: per-stream weight {weight_u32} > byte cap {byte_cap}; \
                 the boot-time admission gate should have caught this",
            );
            let stream_cap = usize::try_from(total_streams)
                .expect("stream cap fits usize");
            Self {
                bytes: Arc::new(Semaphore::new(byte_cap)),
                streams: Arc::new(Semaphore::new(stream_cap)),
                per_stream_weight: weight_u32,
            }
        }

        /// Acquire ONE stream slot + N bytes (N =
        /// per_stream_weight). Awaits when either gate is
        /// saturated. Returns a guard holding BOTH
        /// permits; dropping releases both.
        pub async fn acquire(&self) -> AdmissionPermit {
            let stream_permit = Arc::clone(&self.streams)
                .acquire_owned()
                .await
                .expect("admission stream semaphore never closed");
            let byte_permit = Arc::clone(&self.bytes)
                .acquire_many_owned(self.per_stream_weight)
                .await
                .expect("admission byte semaphore never closed");
            AdmissionPermit { _stream: stream_permit, _bytes: byte_permit }
        }
    }

    /// Round-40 B4: paired-permit guard. Held for the
    /// lifetime of the bounded body stream; dropping
    /// releases the stream slot AND the byte weight.
    pub struct AdmissionPermit {
        _stream: OwnedSemaphorePermit,
        _bytes: OwnedSemaphorePermit,
    }
    ```
    Under this model:
    - `max_active_body_streams` is an actual runtime
      ceiling (round-39 said "prose only"; round-40 B4
      implements it).
    - `bytes` cap × `per_stream_weight` acquisition can
      never deadlock because the constructor asserts
      weight ≤ byte cap.
    - A permit release drops both semaphores atomically —
      no ordering-dependent deadlock between them.

    **Round-41 B3 — overlay handling.** The semaphore's
    `per_stream_weight` is a WORST-CASE figure core DERIVES
    at boot from the components the adapter supplied
    (round-43 B2 — `AdmissionBootConfig` carries components
    only; the pre-computed field was removed). Per-request
    runtime / account / bidder overlays via
    `AuctionConfig.max_transport_bytes` / `max_decoded_bytes`
    (round-32 H3 overlay path) can request LARGER budgets
    than the baked default. Two canonical options for the
    split code path:
    - **(a) Cap overlays at boot envelope (preferred for
      bounded fanout landing):** `resolve_budget()` in split.rs (which
      builds `BidderCall` from the resolved overlays) reads
      `admission_handle().envelope` (round-43 B3) and caps
      its resolved `max_transport_bytes` / `max_decoded_bytes`
      at those boot `BootEnvelope` values. Overlays
      requesting more are silently clamped down; deployers
      who need larger bidder budgets redeploy with a larger
      boot envelope. Simplest, matches admission's "fixed
      per-call weight" model.
    - **(b) Per-call weight acquisition:** the caller
      computes the actual per-stream footprint from its
      resolved BidderCall (`max_transport + max_decoded +
      guest_decoder_bytes`) and passes it into a
      `AdmissionSemaphore::acquire_weighted(weight)`
      helper that runs an on-the-fly weight-vs-capacity
      check. Rejects with `TypedProxyError::Other` when
      the resolved call can never fit. More flexible but
      loses the compile-time simplicity of "constructor
      asserts weight ≤ byte cap".
    bounded fanout Task Fanout-2 lands (a). The regression `overlay_budget_larger_than_boot_envelope_is_clamped()`
    asserts split.rs downgrades a
    `[bidders.foo] max_transport_bytes = 80 MiB` overlay
    to the boot envelope's `max_transport_bytes`
    (the shipped 256 KiB default — round-44 B4) and
    emits a per-bidder ext.warnings entry
    `BidderBudgetClampedToBootEnvelope`.

    Permit rides inside the bounded body stream — round-37
    B8 already put the abort guard there; the round-40 B4
    `AdmissionPermit` (paired stream + byte permits) joins
    it:
    ```rust
    struct AdmittedBody {
        inner: BoundedBodyStream,
        _permit: AdmissionPermit,   // round-40 B4: paired guard
        _abort: AbortGuard,
    }
    ```
    Dropping the response drops the permit, releasing
    both the stream slot AND the byte weight for the
    next queued call.

    **Admission formula (memory).** Round-44 B4: the
    standalone `assert_budgets_admissible` function is
    DELETED — round-43 B2 said so but left its body here,
    which is exactly the "no canonical formula" problem
    the round-44 review flagged (three different decoder
    figures live in one document). The single
    implementation is inlined in `install_admission`
    above and derives everything from
    `per_stream_bytes(...)`. There is no second site.

    **Default sizing** — see the round-44 B2 proof table
    in the "Default streams per adapter" block above.
    Per shipped `host-defaults.toml` at
    [`host-defaults.toml`](../../../crates/auctioneer-config/data/host-defaults.toml):
    `max_bidder_response_bytes = 262_144` (256 KiB) feeds
    both `max_transport_bytes` and `max_decoded_bytes`;
    `max_bidders_per_auction = 50`. All four adapters
    admit their defaults at the WBITS-22 ceiling with
    `MAX_BROTLI_LAYERS = 1`.

    **Concurrency restored — the round-41 B6 / round-42 B4
    `bounded fanout-partial` downgrade stays REVERTED (round-43 B1's
    outcome, round-44 B2's corrected mechanism).**
    Rounds 40-42 concluded that CF/Fastly/Spin must
    serialize because a fixed 24 MiB decoder constant made
    per-stream 72.5 MiB. Round-43 B1 showed that constant
    was a self-imposed policy choice, but chose the wrong
    lever (a WBITS-20 window that would reject a
    default-configured encoder). Round-44 B2 keeps the
    default window at the reference encoder's own 22 and
    instead caps brotli LAYERS at 1, which yields the same
    ~5.27 MiB per-stream. Every adapter admits 6+
    concurrent streams, so:
    - Spec §5.2's concurrent-fanout contract is SATISFIED
      on every adapter under bounded fanout defaults. The §5.2 +
      §14 `bounded fanout-partial` amendments from round-42 B4 are
      reverted in the same commit.
    - Follow-on task Admission-1 (two-stage admission) is
      DROPPED — it existed only to work around the
      oversized decoder reservation.
    - A deployer who explicitly raises
      `max_brotli_wbits` toward 24 trades concurrency for
      large-window Brotli support; the boot check makes
      that trade visible rather than silent
      (`admission_wbits_24_ceiling_rejects_default_streams`).

    **Fastly sandbox lifecycle** (round-42 H4 correction
    to round-40 H1's incorrect `always_on_state` claim):
    Fastly's Compute@Edge fresh-sandbox default means the
    admission singleton is BOOTED ANEW for each request —
    the semaphore state does NOT persist across requests.
    Since the semaphore now bounds INTRA-request fanout
    concurrency (6 streams within one auction), the
    fresh-sandbox model is not a problem for admission;
    it does mean process-local caches (GVL parse cache,
    DNS cache) start cold each request. **Deployers who
    need reusable sandboxes on Fastly use the Rust Serve
    API (`fastly::main::serve` entrypoint) rather than the
    `#[fastly::main]` fresh handler.** Round-40 H1's
    `always_on_state = true` in `fastly.toml` was
    fabricated — no such flag exists. See
    docs.fastly.com/products/compute/rust-serve-api;
    the deployer picks it at the entrypoint, not via
    fastly.toml.

    Round-38 B3 note preserved: **NO
    `assert_request_concurrency_admissible`** for
    Cloudflare. The 6-simultaneous-connection limit
    (round-44 foundations: not a subrequest cap) is a runtime
    queue, not a boot-time reject.
    (round-32 stated "1..=100 MiB" in prose but the literal

## 8. Round-36 B4 streaming "canonical shape" for Prereq 2 (struck round-130 #2 / round-134 #5) — moved round-150 #8

     **The Prereq 2 canonical shape (round-36 B4 rewrite —
     supersedes round-35 B8. `send_bounded` returned typed
     errors as a method-level `Result`, but body-cap and
     chunk-cap breaches happen DURING polling AFTER the
     method returns, so they cannot reach the method-level
     classifier. Round-36 B4 replaces the response `Body`
     with a dedicated bounded body type whose stream items
     carry a typed error).**

     **⚠ STRUCK — NON-NORMATIVE HISTORY (round-130 #2 /
     round-134 #5).** The shapes below predate the buffered
     adoption and reuse the name `BoundedProxyResponse` for a
     STREAMING type that no longer exists — read them as
     upstream's `OutboundResponse` / `Body::Stream` vocabulary
     (the alignment section is the mapping). The ONLY
     `BoundedProxyResponse` in auctioneer is the facade's buffered
     struct; `TypedProxyError`/`TypedBodyError` are
     requirements-list names; the request-time `body_source`
     field below is retired (round-133 #6). Do not implement
     anything from this block.
     (Historic text:) EdgeZero adds an OBJECT-SAFE request-time
     primitive that returns a `BoundedProxyResponse` — a distinct
     type whose body is a typed stream:
     ```rust
     // crates/edgezero-core/src/proxy.rs additions.
     pub struct ResponseBodyPolicy {
         /// Hard per-chunk yield cap. The adapter reads the
         /// transport in chunks no larger than this before
         /// allocation.
         pub max_chunk_size: usize,
         /// Hard total-body cap. The stream ends with
         /// `Err(TypedBodyError::TooLarge)` after this many
         /// bytes.
         pub max_body_bytes: u64,
         /// Round-118 F10: the INDEPENDENT decoded-side cap. On
         /// GuestDecoded the adapter owns every intermediate stream,
         /// so the outer reader can never observe an oversized
         /// intermediate that decodes small — each decoding layer
         /// enforces this cap itself, and overflow classification
         /// (decoded- vs transport-side) is preserved in the error.
         /// Callers set it from `max_decoded_bytes`; fixtures include
         /// unequal caps and a stacked stream whose INTERMEDIATE
         /// output exceeds it while the final output does not.
         /// Round-119 F8 — the ORIGIN crosses the typed boundary:
         /// `TypedBodyError::TooLarge` gains an EdgeZero-owned
         /// discriminator `origin: OverflowOrigin { Transport,
         /// Decoded }` (upstream stays independent of auctioneer enums),
         /// auctioneer's classifier maps Decoded-origin to the
         /// decoded-side error, and the policy CONSTRUCTOR gains the
         /// eighth field so `max_decoded_bytes` cannot be silently
         /// unset. Tests pin the exact internal variant for transport
         /// vs decoded overflow under equal AND unequal limits —
         /// public code/seatnonbid may coincide; the diagnostics must
         /// not.
         pub max_decoded_bytes: u64,
         /// Adapter compression contract. Round-40 B1:
         /// see the canonical three-way `BodySource`
         /// definition in the "BodySource split" block near
         /// the top of Task Fanout-2.
         pub body_source: BodySource,
         /// Round-45 B6: the accepted Brotli window ceiling
         /// (RFC 7932 WBITS, 10-24). REQUIRED by Prereq 8.
         /// On the `GuestDecoded` path the adapter MUST
         /// parse each brotli layer's header and fail the
         /// stream with `TypedBodyError::WbitsTooLarge`
         /// BEFORE allocating a decoder whose window exceeds
         /// this. Round-44 B3 established that auctioneer's
         /// admission weight is only a bound if EdgeZero
         /// enforces this — the field is the enforcement
         /// contract, not just a hint. auctioneer always passes
         /// the same value the round-98 boot envelope's
         /// `ResolvedIsolateLimits.max_brotli_wbits` carries
         /// (round-98 shared auction foundations — `install_admission` is struck), so
         /// the charged weight and the enforced window agree
         /// by construction.
         pub max_brotli_wbits: u8,
         /// Round-45 B6: at most this many brotli layers per
         /// stream (auctioneer passes `MAX_BROTLI_LAYERS = 1`).
         /// A chain declaring more is rejected before
         /// decoder allocation.
         pub max_brotli_layers: u8,
         /// Round-111 B3 (the round-110 H5 prose claimed these
         /// travelled on this policy while the struct carried
         /// neither): summed name+value byte cap on response
         /// headers (auctioneer passes MAX_RESPONSE_HEADER_BYTES =
         /// 8_192). Enforced per the round-111 H4 per-target
         /// client contract; over → the call fails
         /// OversizeResponse.
         pub max_header_bytes: u64,
         /// Round-111 B3: response header COUNT cap (auctioneer
         /// passes MAX_RESPONSE_HEADER_COUNT = 64).
         pub max_header_count: u32,
     }

     // BodySource is defined once in the "BodySource split"
     // block near the top of Task Fanout-2. The round-40 B1
     // sweep DELETED the duplicate two-way enum that used
     // to sit here.

     /// Round-36 H2 canonical typed error. Preserves the
     /// spec's Unreachable → code 999 / statuscode 103
     /// wire mapping by keeping a dedicated variant. This
     /// requires the adapter to classify: reqwest exposes
     /// `is_connect` / `is_request` / DNS-error kinds;
     /// Fastly / CF / Spin ship equivalent classification
     /// helpers.
     /// Round-115 B1: WHICH header limit was breached — the
     /// byte cap and the count cap are different contracts
     /// (a 65-tiny-header response breaches Count, not
     /// Bytes; `limit_bytes` alone could not say so).
     #[derive(Debug, Clone, Copy)]
     pub enum HeaderLimit {
         Bytes { limit: u64 },
         Count { limit: u32 },
     }

     #[derive(Debug)]
     pub enum TypedProxyError {
         /// DNS / refused / TCP-connect / TLS-handshake
         /// failure BEFORE any response arrived.
         Unreachable(anyhow::Error),
         /// Non-unreachable connect error (e.g. proxy
         /// authentication failed at CONNECT).
         Connect(anyhow::Error),
         /// Timeout during initial send.
         Timeout,
         /// Round-114 B1 / round-115 B1: an INITIAL or
         /// INFORMATIONAL (1xx) response field block breached
         /// a header limit — either the Layer-1 parser
         /// ceiling (h1 `max_headers`/`max_buf_size`, h2
         /// field-section knob) or the Layer-2 logical check
         /// (sum(name+value) / count). Pre-body by
         /// definition. PRODUCER contract (round-115 B1 —
         /// hyper's public surface CANNOT provide the h2
         /// discriminator: `is_parse_too_large()` is
         /// http1+server-feature-gated and checks hyper's
         /// HTTP/1 `Parse::TooLarge`, while an h2 header-list
         /// overflow is handled inside the `h2` crate and
         /// reaches hyper as an `h2::Error` with NO stable
         /// public "header list too large" reason):
         /// producing this variant is an EDGEZERO PREREQ-2
         /// DELIVERABLE — the bounded client must integrate
         /// at a layer where the cause is still knowable
         /// (h1: hyper's parse-too-large where the feature
         /// set exposes it, else its own header accounting;
         /// h2: its own field-section accounting or an
         /// h2-level integration that surfaces the reason
         /// BEFORE `h2::Error` erases it). A generic
         /// `h2::Error`/parse error is NEVER classified as
         /// oversize. Layer-2 failures are produced directly
         /// by the client's own post-parse check. Classified
         /// → `FanoutError::OversizeResponse { source:
         /// OversizeSource::Headers }` → code 4 /
         /// seatnonbid 100.
         ResponseHeadersOversize { limit: HeaderLimit },
         /// Anything else that occurred before body polling.
         Other(anyhow::Error),
     }

     /// Round-36 B4: the typed body stream. Every yielded
     /// chunk is bounded by `policy.max_chunk_size`; the
     /// stream terminates with `Err(TypedBodyError::...)`
     /// on cap breach.
     pub type BoundedBodyStream =
         futures_util::stream::LocalBoxStream<'static, Result<Bytes, TypedBodyError>>;

     #[derive(Debug)]
     pub enum TypedBodyError {
         /// max_body_bytes exceeded mid-stream.
         TooLarge { observed: u64, cap: u64 },
         /// Adapter yielded a chunk larger than
         /// max_chunk_size (should be unreachable — adapter
         /// bug or upstream misbehaviour).
         ChunkTooLarge { observed: usize, cap: usize },
         /// Round-45 B6 / Prereq 8: a brotli layer declared
         /// a window larger than `policy.max_brotli_wbits`,
         /// or the chain had more than
         /// `policy.max_brotli_layers` brotli layers. The
         /// adapter emits this BEFORE allocating the
         /// oversized decoder, so the isolate never allocates
         /// past the admitted weight.
         WbitsTooLarge { declared: u8, max: u8 },
         BrotliLayersExceeded { declared: u8, max: u8 },
         /// Round-114 B1 / round-115 B1: a TRAILER field
         /// block breached a header limit mid-stream (the
         /// same Layer-1 ceilings bound every non-initial
         /// field block — round-113 H1). Same PRODUCER
         /// contract as `ResponseHeadersOversize`: the h2
         /// discriminator is an EdgeZero Prereq-2
         /// deliverable (hyper's mid-body error surface
         /// cannot distinguish a trailer-list overflow);
         /// h1 chunked-trailer overflow maps from the
         /// parse layer where exposed, else the client's
         /// own accounting. Never mapped from a generic
         /// error. Classified →
         /// `FanoutError::OversizeResponse { source:
         /// OversizeSource::Headers }` → code 4 /
         /// seatnonbid 100.
         TrailersOversize { limit: HeaderLimit },
         /// Transport error mid-stream (broken connection,
         /// TLS shutdown, etc.).
         Transport(anyhow::Error),
     }

     pub struct BoundedProxyResponse {
         status: StatusCode,
         headers: HeaderMap,
         body: BoundedBodyStream,
     }

     pub trait ProxyClient: Send + Sync {
         // Preserved existing signature.
         async fn send(&self, request: ProxyRequest) -> Result<ProxyResponse, EdgeError>;
         // Round-36 B4: object-safe primitive that returns
         // a typed bounded response. Method-level errors
         // are all pre-body-polling; body-stream errors
         // live on the stream item type.
         async fn send_bounded(
             &self,
             request: ProxyRequest,
             policy: ResponseBodyPolicy,
         ) -> Result<BoundedProxyResponse, TypedProxyError>;
         // Round-36 H3: capabilities declaration.
         fn supported_body_sources(&self) -> &'static [BodySource];
         /// Round-45 B6 / Prereq 8: the guest decoder-state
         /// bound this adapter ENFORCES for the given policy.
         /// Because the bound depends on the policy's
         /// `max_brotli_wbits`, it cannot be a constant — it
         /// is a function of the policy. `install_admission`
         /// calls this with the boot policy and uses the
         /// returned value as the GuestDecoded weight, so the
         /// charged weight is whatever the adapter can
         /// actually enforce. An adapter that cannot enforce
         /// the ceiling returns `u64::MAX`, which fails the
         /// boot admission check loudly (round-44 B3) rather
         /// than under-charging.
         fn guest_decoder_memory_bound(&self, policy: &ResponseBodyPolicy) -> u64;
     }
     ```
     auctioneer calls `send_bounded` with `max_chunk_size = MAX_
     CHUNK_SIZE` (8 KiB) and `max_body_bytes = max_transport_
     bytes`. Body-cap and chunk-cap breaches arrive as
     `Err(TypedBodyError::...)` stream items auctioneer can
     match on inside its decoder pipeline; timeout /
     unreachable / connect failures arrive as method-level
     `TypedProxyError`. The two error surfaces are
     complementary and non-overlapping.

     Round-36 B4 also RETIRES the earlier `send_one_bounded`
     shape that wrapped `ProxyClient::send`. There is only
     ONE bounded API: the trait's `send_bounded`. Every
     reference in Task Fanout-2 pointing at `send_one_bounded`
     as a auctioneer-owned helper is deleted — auctioneer consumes
     `send_bounded` directly through the pinned trait.

     Until upstream ships these primitives, auctioneer cannot
     make the `observed <= cap + MAX_CHUNK_SIZE` guarantee.

## 9. Master-spec §5.2 interim fan-out text ("known semantic gaps", auction core/GDPR fanout era) — moved round-150 #8

**Interim impl — development-only, with known semantic gaps.** If the native
primitive is not yet published (see §14 prerequisites), a fallback
`BidderFanout` in `auctioneer-core` is backed by `ProxyClient` +
`futures::future::join_all`. It correctly issues requests and isolates
partial failures, but has the following **known semantic gaps**
(round-69 user sync — list now covers response-size / preallocation
behavior, not only deadlines/Fastly):

1. **No `AuctionDeadline` enforcement** — the interim does not honor
   the whole-auction deadline; outstanding calls keep running past
   the budget. Observable via `responsetimemillis` and lack of
   `TmaxTimeout` errors. Production path passes the per-call
   absolute `Deadline::at_instant(..)` on the application clock per
   §5.2 (round-135 #5 / round-147 #1).
2. **No per-bidder `timeout`** — the interim has no per-call timer.
   A slow bidder runs until the underlying `ProxyClient::send`
   completes or the runtime kills the request.
3. **Serialized on Fastly** — `fastly::http::Request::send_async` /
   the Compute@Edge `ProxyClient` blocks on `wait()`, so multiple
   bidders run sequentially even though `join_all` polls them
   concurrently. Axum / Cloudflare / Spin are truly parallel under
   the interim.
4. **No pre-allocation `max_bidder_response_bytes` enforcement**
   (round-69 user sync — current EdgeZero `ProxyClient` exposes only
   `send()` with no size-limit knob, and Axum's response handling
   does `response.bytes().await` which buffers the full response
   before auctioneer can inspect it). The interim **enforces the
   response cap post-buffer** by checking `body.len() >
max_bidder_response_bytes` after the read and dropping the
   bidder with code 4 `BadServerResponse` + `seatnonbid` 100 if
   exceeded; an adversarial response can still force memory
   allocation up to whatever the platform runtime allows
   (Cloudflare 100 MiB per response, Fastly/Spin platform default)
   before auctioneer trips the cap. Pre-allocation enforcement
   (true streaming cap that aborts the read at
   `max_bidder_response_bytes`) is **production-only** and
   requires the native `BidderFanout` to expose a
   `.max_response_bytes()` knob on `OutboundRequest` (§5.2). auction core
   development relies on the post-buffer check + platform-level
   response limits; production-complete requires the native cap.

## 10. Retired streaming / negotiation material (TypedProxyError / TypedBodyError classifier, Prereq 2 inventory, source negotiation) — moved round-151 #3

> Superseded by the buffered facade (`ProxyClientApi::send_bounded_all`, `BoundedSlotError`, the round-130/147 mapping table). Do not implement anything below.

### Typed error surface — TypedProxyError/TypedBodyError classifier

Round-34 B4 crate-layering note: `auctioneer-exchange` stays
neutral of `EdgeError` and `anyhow::Error` — those types
never appear in the `FanoutError` variant list. The
classifier lives in `auctioneer-core`
(`crates/auctioneer-core/src/host_primitives/fanout.rs`) and
translates BEFORE crossing into exchange:

```rust
// auctioneer-core::host_primitives::fanout
//
// Round-35 B9: classification is driven by the EdgeZero-
// owned `TypedProxyError` variants returned from
// `send_bounded` (round-35 B8). Round-33/34 called into
// hypothetical `is_network_unreachable(&err)` /
// `is_timeout(&err)` helpers that don't exist — pinned
// EdgeError wraps adapter errors in generic
// `Internal(anyhow::Error)` so there is nothing to match
// on. Under the round-35 B8 typed API, no downcasting or
// debug-repr matching is needed.
// Round-123 #10: the reason model this classifier consumes MUST be
// able to represent guest-decoder corruption distinctly — the G10
// ask (as widened by round-121 #6) is {Unreachable, UpstreamProtocol,
// CodecCorruption { coding, layer }, Other}; the arm below maps
// CodecCorruption to the decoded-side malformed category with the
// sanitized category-only message, NEVER to TransportError.
fn classify_send_error(err: TypedProxyError) -> FanoutError {
    // Round-39 B7 canonical: raw source is logged HERE
    // (auctioneer-core owns anyhow). Only the neutral
    // TransportError with its `kind` crosses into
    // auctioneer-exchange. No shared struct field carries the
    // source string, so exchange serialisers cannot leak
    // it even via Debug.
    // Round-52 H1: log the typed CATEGORY only. The `source`
    // anyhow chain is untrusted (may carry a bidder URL's
    // userinfo/query or an auth header) and is NEVER logged —
    // an allowlist (fixed category string), not a blocklist.
    match err {
        TypedProxyError::Timeout =>
            FanoutError::Timeout(TimeoutCause::Bidder),
        TypedProxyError::Unreachable(_source) => {
            log::warn!(target: "auctioneer::fanout", "bidder unreachable");
            FanoutError::Transport(TransportError {
                kind: TransportErrorKind::Unreachable,
            })
        }
        TypedProxyError::Connect(_source) => {
            log::warn!(target: "auctioneer::fanout", "bidder connect failed");
            FanoutError::Transport(TransportError {
                kind: TransportErrorKind::Initial,
            })
        }
        TypedProxyError::ResponseHeadersOversize { limit } => {
            // Round-114 B1: header-limit breach at the response
            // head (initial or 1xx block) — the OVERSIZE wire
            // shape, never a generic transport error.
            log::warn!(target: "auctioneer::fanout",
                "response headers over limit {limit:?}");
            FanoutError::OversizeResponse { source: OversizeSource::Headers }
        }
        TypedProxyError::Other(_source) => {
            log::warn!(target: "auctioneer::fanout", "bidder pre-body transport error");
            FanoutError::Transport(TransportError {
                kind: TransportErrorKind::Initial,
            })
        }
    }
}

/// ROUND-133 #1 — REQUIREMENTS-RECORD: under the adopted
/// buffered model the fan-out classification site is the
/// per-slot `Err(EdgeError)` via the round-130 mapping table;
/// Fanout-2 implements `classify_slot_error(err: EdgeError, ..)
/// -> FanoutError` targeting THESE SAME FanoutError variants,
/// and this stream-item classifier survives only for non-fanout
/// raw-body paths.
/// (Round-36 B4: stream-item errors arrive AFTER `send_bounded`
/// returned; round-39 B7: raw source stays in auctioneer-core.)
fn classify_body_stream_error(err: TypedBodyError, decoded_bytes: u64) -> FanoutError {
    match err {
        TypedBodyError::TooLarge { .. } =>
            FanoutError::OversizeResponse { source: OversizeSource::Transport },
        TypedBodyError::ChunkTooLarge { observed, cap } => {
            log::warn!(target: "auctioneer::fanout",
                "oversize chunk observed={observed} cap={cap}");
            FanoutError::Transport(TransportError {
                kind: TransportErrorKind::ChunkTooLarge {
                    observed: observed as u64,
                    limit: cap as u64,
                },
            })
        }
        // Round-45 B6: the adapter enforced Prereq 8's
        // window/layer ceiling and refused the stream
        // before allocating an oversized decoder. Maps to
        // UnsupportedContentEncoding (bidder code 4 +
        // seatnonbid 100) — the bidder drops out rather
        // than blowing the isolate.
        TypedBodyError::WbitsTooLarge { declared, max } => {
            log::warn!(target: "auctioneer::fanout",
                "brotli window {declared} exceeds enforced max {max}");
            FanoutError::UnsupportedContentEncoding {
                actual: format!("br;wbits={declared};max={max}"),
            }
        }
        TypedBodyError::BrotliLayersExceeded { declared, max } => {
            log::warn!(target: "auctioneer::fanout",
                "brotli layers {declared} exceeds enforced max {max}");
            FanoutError::TooManyEncodingLayers { actual: declared as usize }
        }
        TypedBodyError::TrailersOversize { limit } => {
            // Round-114 B1: trailer-block header-limit breach
            // mid-stream — the OVERSIZE wire shape (code 4 /
            // seatnonbid 100), never a midstream transport error.
            log::warn!(target: "auctioneer::fanout",
                "response trailers over limit {limit:?}");
            FanoutError::OversizeResponse { source: OversizeSource::Headers }
        }
        TypedBodyError::Transport(_source) => {
            // Round-52 H1: byte count is safe metadata; the
            // `source` chain is NOT logged.
            log::warn!(target: "auctioneer::fanout",
                "mid-stream transport error after {decoded_bytes} decoded bytes");
            FanoutError::Transport(TransportError {
                kind: TransportErrorKind::MidStream { decoded_bytes },
            })
        }
    }
}

fn classify_stream_error(
    _err: anyhow::Error,   // round-52 H1: NOT logged (untrusted)
    decoded_bytes: u64,
) -> FanoutError {
    log::warn!(target: "auctioneer::fanout",
        "stream error after {decoded_bytes} decoded bytes");
    FanoutError::Transport(TransportError {
        kind: TransportErrorKind::MidStream { decoded_bytes },
    })
}
```

### Prereq 2 — current-state inventory at 9e661ae (round-33 B2 + H2)

**Current-state inventory (round-33 B2 + H2 correction —
round-32 misread three adapters).** Pinned EdgeZero at
`9e661ae` (re-verified unchanged at the `docs/outbound-http-spec`
pin — `fcefdde3`-era audit, historical (round-138 #6) —
the fastly proxy's chunked-config/dispatch work did not change
these buffering behaviors, and no adapter `proxy.rs` is touched
in the v0.0.6..v0.0.8 delta or by the branch, whose `crates/`
tree is identical to the tag's):

- Axum: `edgezero-adapter-axum/src/proxy.rs::send` buffers
  via `response.bytes().await` and returns `Body::Once`.
  Uses **`reqwest`**, not raw hyper. Any per-frame bound
  must come through `reqwest::ClientBuilder`, not through a
  hyper HTTP/1 connection knob.
- Cloudflare: already emits `Body::Stream` at
  [`proxy.rs:70`](https://github.com/IABTechLab/edgezero/blob/9e661ae/crates/edgezero-adapter-cloudflare/src/proxy.rs#L70).
  Round-41 B1 canonical (retires round-39 B8's
  HostDecoded classification which was factually wrong;
  see round-40 B2 upstream in this task): CF's
  `supported_body_sources()` returns
  `&[BodySource::GuestDecoded]` — the pinned adapter
  imports `flate2::read::GzDecoder` +
  `brotli_decompressor::DecompressorReader` at
  [proxy.rs:4](https://github.com/IABTechLab/edgezero/blob/9e661ae/crates/edgezero-adapter-cloudflare/src/proxy.rs#L4)
  and drives them at
  [proxy.rs:137](https://github.com/IABTechLab/edgezero/blob/9e661ae/crates/edgezero-adapter-cloudflare/src/proxy.rs#L137).
  That is IN-GUEST Rust decompression — the decoder
  charged its state to the isolate heap before auctioneer
  saw a single byte. The `transform_stream` reference
  at
  [proxy.rs:105](https://github.com/IABTechLab/edgezero/blob/9e661ae/crates/edgezero-adapter-cloudflare/src/proxy.rs#L105)
  re-frames post-decoder bytes into chunks; the
  JS-side is not running gzip/brotli/deflate. The
  round-33 B2 "opt out of implicit decompression"
  plan stays retired — **for the RESPONSE-BODY-POLICY seam only;
  round-118 F06 reopens the FETCH WIRING half it overreached on.**
  Guest decoding, compressed-byte counting, and pre-allocation WBITS
  inspection all require the ORIGINAL COMPRESSED stream, and workerd
  defaults fetch response decoding to AUTOMATIC. The CF adapter MUST
  therefore explicitly request raw delivery (workerd RequestInit
  `encodeResponseBody: "manual"`, or the pinned Rust SDK's
  equivalent — Prereq 3 names the exact call), and GuestDecoded is
  NOT sound until a DEPLOYED-Workers probe confirms: compressed
  bytes observed in-guest, encoding metadata preserved, exactly one
  decode, oversized-WBITS rejected. Rust decoder code existing in
  the adapter proves nothing about what bytes reach it.

  **⚠ Round-43 H2 — the pinned CF adapter does NOT
  satisfy the GuestDecoded contract. NEW BLOCKING
  PREREQ.** Its `transform_stream` matches only exact
  single tokens:

  ```rust
  // pinned edgezero-adapter-cloudflare/src/proxy.rs:137
  match encoding {
      Some("gzip") => decode_gzip_stream(stream).boxed_local(),
      Some("br")   => decode_brotli_stream(stream).boxed_local(),
      _            => stream.map(...).boxed_local(),   // PASSTHROUGH
  }
  ```

  …while `convert_response` unconditionally strips the
  header whenever ANY encoding was present:

  ```rust
  // pinned proxy.rs:112
  if encoding.is_some() {
      proxy_response.headers_mut().remove(header::CONTENT_ENCODING);
      proxy_response.headers_mut().remove(header::CONTENT_LENGTH);
  }
  ```

  So for `Content-Encoding: deflate`, or any stacked
  chain like `gzip, br`, the body is **passed through
  still compressed** but arrives at auctioneer labelled
  identity. auctioneer then hands compressed bytes to the
  JSON parser → garbage → `BadServerResponse` at best,
  silent misparse at worst. A bidder that negotiates
  deflate silently drops out of every auction.

  **Prereq 2 REQUIRES an EdgeZero-side fix.** Either:
  - **(a) preferred — fail loud:** `transform_stream`
    returns an error for any encoding it cannot
    decode, so the adapter never strips a header it
    didn't honour. auctioneer surfaces it as
    `TypedProxyError::Other` → code 4.
  - **(b) complete the decoder:** handle `deflate`
    (zlib-framed per RFC 9110 §8.4.1.2 — see bounded fanout's
    round-35 H4 note), case-insensitive token
    matching, OWS trimming, and right-to-left stacked
    chains up to `LAYERS_MAX`. This is auctioneer's own
    pipeline logic duplicated in the adapter, which is
    an argument for (a) plus having auctioneer request
    `RawCompressed` instead.
    Until this lands, CF's `supported_body_sources()`
    may only claim `GuestDecoded` for responses whose
    `Content-Encoding` is exactly `gzip`, `br`, or
    absent — which is not a contract auctioneer can rely on.
    Task Fanout-3 regression:
    `cloudflare_deflate_response_does_not_reach_parser_as_identity()`.

- Fastly: already emits `Body::Stream` at
  [`proxy.rs:169`](https://github.com/IABTechLab/edgezero/blob/9e661ae/crates/edgezero-adapter-fastly/src/proxy.rs#L169)
  AND already uses `Body::read_chunks(8 * 1024)` at
  [`proxy.rs:149`](https://github.com/IABTechLab/edgezero/blob/9e661ae/crates/edgezero-adapter-fastly/src/proxy.rs#L149).
  Round-39 B8: Fastly reports `&[RawCompressed,
GuestDecoded]` — its decoder runs inside the Rust
  guest, so `GuestDecoded` still costs the full
  decoder state per stream (charged via
  `guest_decoder_memory_bound()` — see the trait
  extension above).
- Spin: buffers-then-decodes via
  `flate2::read::GzDecoder` and returns `Body::Once`.
  Round-39 B8: same `&[RawCompressed, GuestDecoded]`
  as Fastly.

So the Prereq 2 work reduces to (round-42 B8 canonical,
retires the earlier CF-HostDecoded + Axum-includes-
HostDecoded lines that survived round-40's sweep):

- (a) Axum switches to `Body::Stream` and reports
  `&[RawCompressed, GuestDecoded]`. `HostDecoded` is
  RETIRED — no pinned adapter runs an out-of-guest
  decoder, and keeping the variant available on Axum
  would silently allow a misconfiguration.
- (b) Cloudflare stays on the current identity-decoded
  stream and reports `&[GuestDecoded]` (round-40 B2
  canonical — the pinned CF adapter runs `flate2` +
  `brotli_decompressor` in-guest at
  `crates/edgezero-adapter-cloudflare/src/proxy.rs:4`).
- (c) Fastly reports `&[RawCompressed, GuestDecoded]`
  (round-135 #1: its internal chunked read is the collect
  loop's implementation detail, not a auctioneer-visible stream).
- (d) Spin reports `&[RawCompressed, GuestDecoded]` (same
  note).

Round-42 B8: `BodySource::HostDecoded` REMAINS in the
enum as a reserved-for-future-use variant, but NO
bounded fanout-shipping adapter declares support for it. Every
regression test that fixtures a `HostDecoded` adapter
today (`admission_default_axum_host_decoded_ships`
etc.) is renamed to the `guest_decoded` variant.

### Prereq 2 — ProxyResponse accessors + round-34 B3/B4 error-mapping pins

**`ProxyResponse` accessors.** `ProxyResponse` at
[`proxy.rs:154`](https://github.com/IABTechLab/edgezero/blob/9e661ae/crates/edgezero-core/src/proxy.rs#L154)
has PRIVATE fields (`body`, `extensions`, `headers`,
`status`) and no `into_parts()`. Round-33 B2: auctioneer reads
the response through the existing pub accessors —
`.body()`, `.body_mut()`, `.headers()`, `.status()` — and
swaps the body out via `.body_mut()` where needed. The
round-32 draft implied a `let ProxyResponse { body,
  headers, status, .. } = ...;` destructure that doesn't
compile.

This preserves:

- **Dyn-compatibility.** `ProxyHandle` stores
  `Arc<dyn ProxyClient>` at
  [`edgezero-core/src/proxy.rs:16`](https://github.com/IABTechLab/edgezero/blob/9e661ae/crates/edgezero-core/src/proxy.rs#L16);
  the pinned `send() -> Result<ProxyResponse, EdgeError>`
  shape stays fully dyn-compatible.
- **HTTP status and headers**, accessed via `.status()` and
  `.headers()`.
- **The existing `Body::Stream` variant.**
  [`edgezero-core/src/body.rs:14`](https://github.com/IABTechLab/edgezero/blob/9e661ae/crates/edgezero-core/src/body.rs#L14)
  defines `Body::Stream(LocalBoxStream<'static,
Result<Bytes, anyhow::Error>>)`. Stream item type is
  `Result<Bytes, anyhow::Error>` — auctioneer keeps it as-is.

**Complete error mapping (round-34 B3 + B4 canonical
block — supersedes round-33 B2's inlined `EdgeError` /
`anyhow::Error` variants which broke `FanoutError: Clone`
and the auctioneer-exchange neutrality invariant).** The
variant shape is defined once at the top of Task Fanout-2 in
the "Typed error surface" block; every failure crosses into
auctioneer-exchange as one of those Clone-safe variants. The
classifier lives in auctioneer-core and produces the exact
variants at the boundary — see
`classify_send_error` / `classify_stream_error` in Task
Fanout-2's Typed error surface block above.

Wire mapping — see the canonical block in Task Fanout-2
above (round-37 H2). Two pins here so this section
doesn't drift again: `Unreachable` → 999 + 103 (round-36
H2 restored the variant from the round-35 B9 removal);
`Other(_)` → 999 + 100 (round-37 H2 preserves the
current auction core baseline at
[`crates/auctioneer-exchange/src/run.rs:585-591`](../../../crates/auctioneer-exchange/src/run.rs#L585-L591)).

### Prereq 2 — historic per-adapter streaming asks (Axum/Cloudflare/Fastly/Spin)

(Historic streaming asks:)

- Axum: `edgezero-adapter-axum/src/proxy.rs::send` currently
  calls `response.bytes().await` (buffered `Body::Once`).
  Return `Body::Stream(reqwest_response.bytes_stream().
map_err(anyhow::Error::from).boxed_local())` instead.
  **Round-41 H1 correction (retires round-33 H2's
  ClientBuilder-frame-cap claim):** `reqwest::ClientBuilder`
  does NOT expose a documented
  response-frame ceiling. `bytes_stream()` yields chunks
  the underlying `hyper::body` produced, and any
  downstream rechunking caps only the SIZE of a yielded
  Bytes (retained data), NOT the allocation hyper made
  while receiving the frame from the socket. Auctioneer's
  honest guarantee under Axum is:
  - `observed <= max_transport_bytes + MAX_CHUNK_SIZE`
    on YIELDED bytes counted at auctioneer's decoder feed
    point.
  - NO cap on hyper's transient buffer between socket
    receive and `bytes_stream` yield. This is bounded
    only by hyper's default HTTP/1.1 max header + max
    body-frame settings (~4 GB — the guest process's
    overall memory model has to cover it).
  - **ROUND-138 #3 — this is exactly why G6 stays OPEN and what
    the certificate EXCLUDES:** EdgeZero's proposed
    `max_chunk_bytes` rechunks OUTPUT only — it does not bound
    the allocation made while RECEIVING a provider-controlled
    source chunk, and has no effect on buffered `send_all`. The
    managed-memory certificate's `MAX_CHUNK_SIZE` staging terms
    cover AUCTIONEER'S OWN pipeline chunking only; upstream
    source-read allocation is EXPLICITLY EXCLUDED from the
    certified bound and documented as a platform-side allocation
    outside auctioneer's allocator, until EdgeZero guarantees a
    bounded source-read allocation (the standing G6 ask).
    NAMING (round-139 #6): with this exclusion, what Fanout-2
    certifies is the **MANAGED-MEMORY certificate** — the
    "whole-isolate certificate" label is RETIRED (it would
    overclaim); the equation section's historical "whole-isolate
    budget" title survives as a section name only, and egress
    completion (Prereq 9) remains a SEPARATE
    production-certification prerequisite it never subsumes.
  - **Round-42 H5 CORRECTION** (retires round-41 H1's
    nginx/envoy/HTTP2 hints — none of those actually
    cap aggregate response allocation):
    - `nginx proxy_buffer_size` caps the size of a
      SINGLE buffer used to receive the response
      headers, NOT the total response body allocation.
    - `envoy max_request_bytes` bounds REQUEST body
      buffering; the RESPONSE path uses a separate
      (larger) limit not exposed by that key.
    - HTTP/2 `SETTINGS_MAX_FRAME_SIZE` bounds one
      frame's payload, not the aggregate stream
      allocation the receiver may accumulate.
      **Round-117 P1-7 — with only the yielded-byte guarantee, the
      Axum certificate is a certificate of AUCTIONEER-MANAGED memory,
      not of the whole process**, and it must say so: hyper's
      parser-stage allocations sit outside the bound, so the 512 MB
      whole-process figure is a deployment target, not a proven
      envelope. Round-118 F13 RETRACTS the round-117 claim that the
      `Limited` wrapper upgrades the certificate to whole-process:
      `Limited` polls its inner body first and length-checks the frame
      only AFTER it was produced, so receive/parser allocation below
      the wrapper is spent before the check runs. The wrapper remains
      documented as a total-yielded-bytes bound; a `whole_process`
      certificate additionally requires independent, verified bounds
      on hyper's receive buffering and per-connection/stream state
      (with an oversized-inner-frame allocation test), or it is not
      issuable on Axum. The machine-readable certificate (Fanout-10
      acceptance) carries a `scope: managed | whole_process` field
      recording which claim was proven.
      Auctioneer therefore accepts the WEAKER yielded-byte
      guarantee under Axum as the honest managed-memory bound. Deployers
      who need a genuinely bounded transport allocation
      must either (a) run auctioneer on Fastly/Spin/CF
      whose runtimes enforce hard heap ceilings, or (b)
      build a custom hyper client with a
      `http_body_util::Limited` wrapper around the
      response body — that IS documented and does cap
      total allocation, at the cost of losing reqwest's
      convenience layer. Fanout-2 lands the weaker
      guarantee; the Limited-wrapper alternative is a
      documented follow-on for deployers who need it.
- Cloudflare: already emits `Body::Stream` at
  [`proxy.rs:70`](https://github.com/IABTechLab/edgezero/blob/9e661ae/crates/edgezero-adapter-cloudflare/src/proxy.rs#L70).
  Round-40 B2 canonical (retires round-38 B5's
  HostDecoded-only claim): the pinned CF adapter
  imports Rust `flate2::read::GzDecoder` +
  `brotli_decompressor::DecompressorReader` at
  [`proxy.rs:4`](https://github.com/IABTechLab/edgezero/blob/9e661ae/crates/edgezero-adapter-cloudflare/src/proxy.rs#L4)
  and runs them at
  [`proxy.rs:137`](https://github.com/IABTechLab/edgezero/blob/9e661ae/crates/edgezero-adapter-cloudflare/src/proxy.rs#L137).
  That is IN-GUEST Rust decompression, not host-side.
  Cloudflare's canonical `supported_body_sources()`
  therefore reports `&[GuestDecoded]` (round-40 B2
  fix). The identity bytes auctioneer consumes are the
  output of a Rust decoder that already charged
  its measured decoder state to the isolate heap;
  admission MUST include that term. auctioneer still does
  NOT inspect / reject on `Content-Encoding` (the
  guest decoder handles it) and auctioneer's header
  handling per B6 below strips `Content-Encoding` +
  `Content-Length` before passing headers downstream.
  - `max_transport_bytes` on Cloudflare bounds
    **runtime-delivered bytes** (per round-37 B7).
    The Workers runtime enforces the count as bytes
    reach auctioneer's Body::Stream — the only observable
    ceiling regardless of on-wire size. The plan does
    not claim an on-wire byte bound on Cloudflare
    because the platform doesn't expose one.
  - New regression tests in Task Fanout-3:
    `cloudflare_body_source_is_runtime_decoded()`,
    `cloudflare_transport_bound_counts_runtime_delivered_bytes()`
    .
- Fastly: `edgezero-adapter-fastly/src/proxy.rs` — expose
  the raw compressed stream via a
  `Body::Stream` construction sourced from a chunked read
  over the `pending_request` result. Fastly's platform
  surface is unusual; this is the piece that most
  constrains Prereq 2's landing.
- Spin: `edgezero-adapter-spin/src/proxy.rs` stops calling
  `flate2::read::GzDecoder`; returns
  `Body::Stream(...)` sourced from the raw HTTP body.

### Prereq 2 — Source negotiation (round-40 B1) + header normalisation on decoded sources (round-38 B6)

**Source negotiation** (round-40 B1 canonical, retires
round-37 B7's `RawCompressed | RuntimeDecoded` shape.
Round-38 B5 + round-39 B8 replaced that with the
three-way `RawCompressed | GuestDecoded | HostDecoded`
split earlier in this task; leaving the old two-way
shape live below produced duplicate incompatible APIs).
Strategies call `client.supported_body_sources()` before
invoking `send_bounded`, pick the strongest supported
source, and pass that into
`ResponseBodyPolicy.body_source`:

```rust
// ROUND-133 #6 — PER-REQUEST SOURCE SELECTION IS RETIRED. The
// round-40 B1 preference ladder that stood here described a
// negotiation upstream does not support: bare gzip/br are
// ALWAYS adapter-decoded and every other encoding ALWAYS
// passes through (the alignment cap-ownership table) — the
// disposition is a property of each RESPONSE — read from its
// RETAINED Content-Encoding (round-139 #3; the G14 metadata ask
// is downgraded to analytics-only) — never a request-time choice.
// `BodySource` survives as the STATIC capability description
// budgeting consumes, and the budget charges the MAX decoder
// state across every disposition the adapter can produce:
//   per_stream decoder charge =
//     max(mode_chain(RawCompressed),   // auctioneer passthrough
//         guest_decoder_bound_bytes)   // adapter in-guest decode
// — a single boot DecodeMode undercharges an adapter that
// guest-decodes gzip while auctioneer decodes deflate/stacked.
let available = client.supported_body_sources(); // capability, not choice
// Round-111 B3: ALL SEVEN fields — the earlier three-field
// literal omitted even the existing brotli fields and could
// not compile. (Round-132 #3: seven was the STREAMING struct's
// count — this example is requirements-record; the compilable
// facade struct in the Fanout-2 block adds max_decoded_bytes as an
// eighth field, both caps present.)
let policy = ResponseBodyPolicy {
    max_chunk_bytes: MAX_CHUNK_BYTES,
    max_body_bytes: budgets.max_transport_bytes,
    // body_source: RETIRED, round-133 #6 (see the note above)
    max_brotli_wbits: budgets.max_brotli_wbits,
    max_brotli_layers: budgets.max_brotli_layers,
    max_header_bytes: MAX_RESPONSE_HEADER_BYTES,
    max_header_count: MAX_RESPONSE_HEADER_COUNT,
};
```

The admission gate charge (round-133 #6, replacing the
round-39 B8 per-mode formula): every adapter's per-stream
decoder charge is `max(mode_chain(RawCompressed),
  guest_decoder_bound_bytes)` — passthrough responses are always
possible, so auctioneer's own chain is always in the max; adapters
that decode in-guest contribute their reported bound as the
other arm. There is no adapter that ships
`HostDecoded` today (round-40 B2 corrected the
Cloudflare misclassification); the branch exists for
future host-decoded platforms.

**Header normalisation on decoded sources**
(round-38 B6 canonical, applies to `HostDecoded` AND
`GuestDecoded` per round-40 B1). Round-37 B7 required
the adapter to set `Content-Length` to the decoded
stream length at the bounded-response boundary. That's
impossible: the adapter cannot know the delivered
length before the stream is consumed, and Cloudflare's
`Response` object actively rejects manually-supplied
`Content-Length` on arbitrary streams (only
`FixedLengthStream` fixes the length upfront, which
precludes bounded streaming). Round-38 B6 shape:

- Adapters that report `HostDecoded` OR `GuestDecoded`
  MUST strip BOTH `Content-Encoding` AND `Content-
Length` from the response headers before returning
  `BoundedProxyResponse`. The stream length is
  unknowable at header-arrival time.
- Auctioneer MAY set `Content-Length` on the response it
  emits DOWNSTREAM once the bounded body has been
  fully collected. Downstream header rewriting, not
  adapter-side.
- Downstream code that reads response headers between
  `send_bounded` return and body-stream exhaustion
  MUST NOT assume `Content-Length` is populated.

Task Fanout-3 regressions (round-38 B6):

- `cloudflare_guest_decoded_headers_omit_content_encoding()`
  (round-40 B2 rename).
- `cloudflare_guest_decoded_headers_omit_content_length()`.
- `auctioneer_downstream_response_sets_content_length_after_body_collection()`.

### Prereq 2 — item 4, bounded per-chunk reads at the adapter transport

4. **Bounded per-chunk reads AT THE ADAPTER TRANSPORT
   LAYER — REQUIRES A NEW EDGEZERO PRIMITIVE.** Round-34
   B5: `Stream<Item = Result<Bytes, _>>` places NO upper
   bound on individual `Bytes` chunk sizes, and no
   configuration knob on any of the pinned adapters
   currently caps response-frame yield size. Concretely:
   - **Axum / reqwest.** The round-32 draft named
     `reqwest::ClientBuilder::pool_max_idle_per_host` (a
     connection-pool knob unrelated to response framing)
     and a fictional `http1_max_buf_size` on the reqwest
     side. Reqwest at the pinned version exposes NO
     response-frame ceiling; further, `Response::bytes_stream`
     is only available when the `stream` feature
     is enabled on reqwest, which EdgeZero does not enable
     at
     [`Cargo.toml:56`](https://github.com/IABTechLab/edgezero/blob/9e661ae/Cargo.toml#L56).
     Prereq 2 therefore REQUIRES an EdgeZero-side change
     that (a) enables reqwest's `stream` feature, and (b)
     wraps `bytes_stream()` behind a bounded-pull primitive
     that guarantees each `Bytes` yield is ≤ 8 KiB — either
     by copying to fixed staging buffers or by a
     chunked-read helper. Reqwest / hyper themselves do not
     expose that guarantee.
   - **Cloudflare / Workers.** The Workers Streams API's
     yield size is bounded by the runtime's internal read
     buffer, but the guaranteed max is not documented;
     Prereq 2 requires EdgeZero to WRAP the Workers stream
     in an 8-KiB chunked-copy shim so auctioneer gets a
     provable per-frame bound.
   - **Fastly.** Already uses `Body::read_chunks(8 * 1024)`
     at
     [`proxy.rs:149`](https://github.com/IABTechLab/edgezero/blob/9e661ae/crates/edgezero-adapter-fastly/src/proxy.rs#L149).
     This is the only adapter that satisfies the bounded-
     pull contract today; no new EdgeZero work needed
     here.
   - **Spin.** Spin's HTTP body reader accepts a
     chunked-read primitive; Prereq 2 requires EdgeZero to
     pass 8 KiB explicitly through it.

   **Round-150 #8: the round-36 B4 "canonical shape" (`send_bounded`
   returning a streaming `BoundedProxyResponse`, `TypedBodyError`
   stream items, `TypedProxyError`, `guest_decoder_memory_bound`,
   `send_one_bounded` retirement) — already ⚠ STRUCK since round-130
   #2 / round-134 #5 — is MOVED to the historical appendix §8. The
   ONE normative path is the buffered facade in the Fanout-2
   alignment section (`ProxyClientApi::send_bounded_all`,
   `BoundedSlotResult`, `BoundedSlotError`, the round-130 mapping
   table).**

## 11. Round-120 gap assessment for Prereq 2 (G5 / G6 / G7 / G10 / G11 / body-source negotiation) — moved round-152 #4

> Every gap but G6 is closed at `120dd2aa`; see the plan's Prereq 2 stub and the compatibility matrix.

- **GAP vs upstream outbound spec (round-120) — feedback G5:** no
  transport-side byte cap exists upstream (`max_response_bytes` counts
  decoded bytes only; early Content-Length reject is identity-only,
  §3.4.1) and `ResponseTooLarge` carries no `origin` discriminator —
  round-118 F10's independent caps and round-119 F8's `OverflowOrigin`
  have no upstream carrier.
- **GAP (round-120) — feedback G6:** no per-chunk yield bound; §3.4.1
  "the spec does not rechunk", chunk size is source-controlled, §8
  risk 11 defers the cap. `max_chunk_size`/`MAX_CHUNK_SIZE` has no
  upstream mechanism.
- **GAP (round-120) — feedback G7:** no response header/trailer
  byte/count caps and no oversize discriminator — platform header-limit
  signals are flattened to generic `bad_gateway` (Fastly
  `HttpResponseHeaderSectionTooLarge`, §4.3 stage-2 table; Spin §4.4),
  so `ResponseHeadersOversize`/`TrailersOversize` (round-114/115 B1)
  cannot be produced from upstream's error surface.
- **GAP (round-120) — feedback G10:** upstream collapses every
  transport failure into one `BadGateway { message }` — the
  `Unreachable` discriminator behind the spec's code 999 /
  seatnonbid 103 mapping does not survive to the `EdgeError` boundary.
- **GAP (round-120) — feedback G11:** multi-member gzip semantics
  (round-119 F7: decode ALL members, cumulative cap, drain to
  transport EOF, trailing junk fails) are unspecified for §3.4.1's
  shared decoder.
- **Body-source negotiation:** there is NO per-request
  RawCompressed/GuestDecoded negotiation upstream — bare single
  gzip/br is ALWAYS adapter-decoded (with the incremental decoded cap)
  and everything else is always passthrough. The three-way `BodySource`
  becomes descriptive of upstream's fixed behaviour rather than a
  negotiated policy; the admission consequences (decoder-state
  charging, WBITS) are Prereq 8's round-120 block.
  _Blocks:_ Tasks Fanout-2 through Fanout-5 and Fanout-7 (round-133 #8 —
  matching the banner's round-117 P2-13 list: Fanout-3/4/5 consume the
  facade and cancellation shapes too).

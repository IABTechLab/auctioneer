# M3b — Multi-adapter Production Fanout + Fail-loud Config + Bounded Body Cap

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **2026-09-29 M4 integration correction:** the `m4-privacy` worktree's
> retained-node guard disproved the 64-byte estimate carried in older proof
> rows below. Use `NODE_RETAINED_BYTES = 384`, including a checked 20% margin
> for nested arrays and single-key-object forests. This supersedes older
> 64-byte node-charge claims, not unrelated 64-byte header/metadata charges.
> Caps and warn-only boot policy are unchanged. Exact native measurements,
> updated adapter totals, and remaining live-accounting gaps are recorded in
> [the verification record](2026-09-28-m4-verification.md#retained-node-accounting-correction).

## Current Review Corrections (2026-10-05)

These decisions supersede conflicting historical copy formulas and probe
coordination descriptions below. Implementation repair tracking lives in
[the review-fix plan](2026-10-05-m3b-review-fixes.md). Auctioneer API changes
need no backward-compatibility layer. The EdgeZero pin was updated on
2026-10-07 to PR #275's head,
`ea66eff045512cccc60646d6ec339e83ff44fed7`.
The current migration and Q1/Q3/Q4 evidence boundaries are tracked in
[the bounded-ingress/config update](2026-10-07-edgezero-bounded-ingress-config-update.md).
The latest update and merge-readiness evidence are recorded in
[the prior SDK delta review](2026-10-06-edgezero-shared-policy-update.md).
Additional repair tracking lives in
[the auction review follow-up](2026-10-06-auction-review-followup.md).
The six additional implementation findings are tracked in
[the review-fix checklist](2026-10-06-m3b-review-fixes.md).

The 2026-10-08 review supersedes the obsolete profile-installer and copied
proof-row requirements below. `OUTBOUND_PROFILE` is a static declaration checked
against each adapter's expected capabilities, not installed state consumed by
the scheduler. Runtime behavior comes from resolved limits, SDK response metadata,
and the adapter's concrete timer/cooperator. The shipped native row is proved
in `auctioneer-adapter-axum/src/envelope.rs`; generic runtime fixtures are synthetic
arithmetic examples. Adapter conformance binaries must link and exercise their
own timer/cooperator with a controlled client, not repeat the core suite.
These local tests do not certify SDK/provider transport or hosted cancellation.
See [the consolidated review repairs](2026-10-08-m3b-consolidated-review-repairs.md)
for findings, verification evidence, and external CI limitations.

The October 10 response and guard corrections are tracked in
[the response-classification repairs](2026-10-10-response-classification-review-repairs.md).
Response media classification is per bid, shares standard `mtype` handling across
adapters, and uses the bidder's post-FPD, privacy-scrubbed context. Good siblings
survive malformed media hints; each rejected bid contributes its own bounded
diagnostic. The funded impression cap governs both preparation and outbound
metadata. Spent entry deadlines refuse before compression, timers, or submission.

- Admit a returned bid only when its `impid` belongs to the surviving
  observable bidder's split group. Nonempty `covered_imp_ids` additionally
  restrict admission to the originating call; metadata cannot enlarge
  participation. Empty metadata uses bidder participation as its fallback.
  Drop invalid bids before targeting, preserve valid bids from mixed responses,
  and emit one fixed-size code-4 error per affected call under the observable
  identity, without echoing returned IDs. Reuse the retained split group rather
  than allocating another membership index or request copy.
- Custom targeting prefixes must leave `MIN_KEY_LENGTH` (12) UTF-8 bytes
  within the selected key cap (at most 20); otherwise fall back to `hb`
  with bounded warning 10015. Enabled-only catalog views resolve eligible
  host aliases before compiled-name entries; base-only views retain compiled
  bidder semantics. Runtime `[bidders]` map keys obey the same 64-byte bound
  before validating values or formatting diagnostics.
- The quoted targeting-prefix diagnostic is bounded after escaping control
  characters as well as before formatting. Root map validation must not retain
  entire bidder, account, or alias maps in error parameters; keep field keys,
  error codes, and nested validation while discarding that unnecessary copy.
- Aliases named `prebid` remain valid. At the shared diagnostic wire key,
  preserve bidder entries followed by auction-wide entries instead of replacing
  either array. Production config allocation acceptance measures peak live
  overlap through store extraction, envelope integrity verification, and typed
  resolution against the separately funded extraction and parse terms.

- Bidder and alias identifiers are limited to 64 UTF-8 bytes before
  resolution or diagnostic construction. Targeting keys remain capped at
  20 bytes without retaining the original allocation.
- Dropped aliases remain denied even when their names shadow compiled
  bidders: splitting checks the deny set before direct-registry fallback
  in modern, hoisted and legacy parameter locations. Catalog aliases use
  the same name/base byte bounds and registered-base eligibility.
- Request accounting funds the original parsed tree, two retained
  request representations, grouping subtrees, and two sequential scoped
  working representations. Wire bytes and syntactic-node heap are charged
  separately. Total request-level supplement expansion for one bidder is
  preflighted against `max_parsed_nodes` and `max_outbound_raw_bytes`, before
  copying; exceeding either limit refuses that materialization. Thus the
  working set is input plus one bounded supplement, not one input-sized
  supplement per impression. No request tree is retained per bidder.
- Before privacy, sampling or cap diagnostics copy impression IDs, the
  auction preflights request-derived response bookkeeping against independent
  `max_outbound_raw_bytes` and `max_parsed_nodes` ledgers. Each candidate
  identity reserves `2 * name.len() + 128` content bytes and 16 nodes;
  each potential non-bid row reserves `imp_id.len() + 32` bytes and 8 nodes.
  Disabled-warning identities also reserve a ledger entry. The conservative
  candidate upper bound is checked before allocating the output maps;
  exceeding either ledger returns code 2 without echoing identifiers.
  Privacy/expired-tmax rows are not built unless status rows are requested
  and enabled. Separately fund four live representations (typed metadata,
  named-map serialization and JSON assembly scratch) with
  `bookkeeping_term = 4 * (max_outbound_raw_bytes +
  max_parsed_nodes * NODE_RETAINED_BYTES)` using checked arithmetic. Add
  this to `per_auction_peak`; outbound call count does not bound this population.
- Runtime host `[auction]`, selected `[accounts.<id>]` and observable
  `[bidders.<name>]` expose independent `max_transport_bytes` and
  `max_decoded_bytes`, each validated in 1 KiB..=100 MiB. Resolve each field
  bidder-own, alias-base, account, then host default (the legacy response
  cap defaults unset host caps). Scoped overrides may raise host defaults;
  only the installed boot envelope is a funded ceiling. `AuctionPlan`
  carries resolved requested rows and optional funded ceilings. Missing
  observable/base rows are internal invariant errors, not silent host fallbacks.
  Call construction independently clamps both caps and emits one code-10020
  warning under the observable bidder when either is clamped. Both resolved
  caps reach the production transport/decode policy; host defaults must not
  re-narrow higher-priority account/bidder values. The development-only
  buffered fanout enforces the decoded cap, not a streaming transport guarantee.
- Every real probe config resolution owns a counted slot. Cold saturation
  waits asynchronously for a published verdict or a released slot; it
  never creates an uncounted read. The capacity floor is one. Cancellation
  removes registered wakers and returns resolution permits.
- Aux/probe extraction populations are charged as
  `aux_route_permits + max(aux_route_permits, PROBE_CONFIG_MIN_PERMITS)`;
  zero aux capacity still funds the probe's minimum config slot.
- Complete status bodies, including diagnostics, are limited to 8192
  UTF-8 bytes. Baked/runtime banners use byte validation. Rendering reserves
  diagnostic space before truncating a banner. Cached renderers borrow the
  stored verdict instead of cloning its banner.
- Config-work permits do not fund output retention. A separate term is
  `8192 + PROBE_RESPONSE_PERMITS * (8192 * egress_handoff_factor +
  transmit_state_bytes)`, with `PROBE_RESPONSE_PERMITS = 4`. This independent
  process-global pool exists before bootstrap and holds output permits through
  terminal egress. Auction and aux capacity cannot shed probes; probe-pool
  saturation aborts without rendering an unfunded response. The resident
  verdict cache is separately funded. Provisional provider ingress estimates
  do not establish an output bound.
- Application-owned logging applies EdgeZero's independent boot target
  filter (`BOOT_LOG_TARGET`, WARN/ERROR) and configured ordinary filter
  from the start, including Fastly's inner endpoint filters. The facade
  admits both; no temporary widening or post-bootstrap reset is used.
  Fastly's runtime-env result carries `.env` plus deferred diagnostics,
  which are emitted after backend installation. The environment is resolved
  once for health and dispatch, through the SDK's documented custom entrypoint
  (`App::build` with Fastly metadata, then `send_with_registries_and_hooks`).
  EdgeZero retains conversion, registries, admission, routing and egress;
  no manual handler or router assembly is introduced. The latest upstream
  update additionally validates driver EOF after the final outbound slot,
  including late failures and malformed trailing events. Auctioneer keeps polling
  until explicit termination and publishes no partial auction on a driver
  failure. Axum validates declared-length stream EOF before handing off the
  final frame; `App::build` rejects replacement of adapter-selected platform
  metadata. These checks do not upgrade provider qualification or change the
  config-read/decoder caps used by the resource accounting.
- RawValue refusal classification depends on an observed refusal; a client
  string containing an internal marker remains an ordinary type error.

Budget-over-ceiling remains warn mode. Neither these corrections nor local
WASM checks imply hosted-provider qualification.

Final review repairs and actionable qualification ownership are recorded in
[the final review checklist](2026-10-06-m3b-final-review-fixes.md). Both explicit
impression bidder maps enforce the identifier limit, matched non-body routes
drain under the SDK read deadline and 4096-byte cap before handler work, and
borrowed bidder responses no longer expose raw-byte extraction. Object FPD
patches recursively normalize non-object targets. Cloudflare handoff charges
three overlapping buffers, including the JavaScript heap copy. These decisions
supersede historical zero-permit probe and two-copy Cloudflare descriptions.

> **Execution status:** implementation begins on the `m3b-fanout`
> branch. Step tracking lives in the SDD execution ledger
> (`docs/superpowers/sdd/<plan-basename>/progress.md`), not in this file's
> checkboxes — the same convention the M3a plan records. Per the
> round-117 DAG: Task M3b-9 is Auctioneer-only and startable immediately
> — and so is the M3b-2a pure-foundations slice (round-134 #7;
> round-135 #3: its ALGORITHMS only — the concrete target-envelope
> constants wait for the §8 measurements);
> **⚠ P0 — THE UPSTREAM BATCH API CHANGED AT HEAD; EVERY `send_all`
> CONTRACT IN THIS PLAN AND THE MASTER SPEC IS PIN-SCOPED
> (round-159 #1/#2).** At the PINNED `120dd2aa` the contract is
> <!-- PIN-SCOPED record of what 120dd2aa published (batch-rebase-ok) -->
> `OutboundHttpClient::send_all(Vec<OutboundRequest>) ->
> Vec<OutboundSlotResult>` — verified present at
> `edgezero-core/src/outbound.rs:98`. **EXACTLY THREE auctioneer types
> implement `OutboundHttpClient` (round-160 #2 corrects round-159's
> count of four):** `TestProxyClient`
> (`crates/auctioneer-core/tests/conformance/mod.rs:237`), `NeverResponds`
> (`crates/auctioneer-core/tests/axum_production_strategy.rs:35`) and
> `MockProxyClient` (`crates/auctioneer-core/tests/production_fanout.rs:65`).
> `conformance.rs:317` is `NoOutboundCapabilityFanout`, an impl of
> AUCTIONEER'S OWN `BidderFanout`, not a second outbound client; and the
> production bridge (`host_primitives/edgezero_proxy.rs`) implements
> `ProxyClientApi` over the SINGLE-request `HttpClient::send`, so it
> does not implement `OutboundHttpClient` at all and the pin move
> does not force it to change.
>
> **At EdgeZero head that method is HARD-REMOVED** and replaced by a
> batch handle. ROUND-160 #1 — THE SIGNATURE BELOW, first recorded at
> `3d87d5bc` and carried forward to the RECORDED TARGET `351698c9` (round-184 #1; `4790a140` superseded)
> (round-167 #3; M3b-0 re-verifies it against that revision when it
> runs — round-159's `61457d7d` shape, where cutoff was `None` from
> `next()`, is ALREADY OBSOLETE and must not be implemented):
> ```rust
> start_batch_until(..) -> OutboundBatch
> OutboundBatch::next()    -> Result<OutboundBatchNext, EdgeError>
> OutboundBatch::collect() -> Result<OutboundBatchResults, OutboundBatchFailure>
> //   round-163 #1 — the failure arm is NOT a bare EdgeError:
> //   struct OutboundBatchFailure { error: EdgeError, slots: .. }
> //   It PRESERVES the slots that completed before the driver failed.
> OutboundBatch::cancel(self) -> Vec<usize>   // unresolved indices
> enum OutboundBatchNext {            // #[non_exhaustive]
>     Item(..),
>     Finished(OutboundBatchTermination),  // Completed | Cutoff
> }
> ```
> `None` no longer signals cutoff: it appears ONLY as an absent
> positional slot inside `OutboundBatchResults` when termination was
> `Cutoff`. Duplicate or out-of-range slot indices, and premature
> driver termination, surface as `EdgeError::Internal`.
>
> **⚠ READ AGAINST HEAD `5eccff8e` ON 2026-09-18 — THREE CONTRACT
> DETAILS THIS PLAN DID NOT CARRY. M3b-0 must fold them in:**
> 1. **`Item` carries an INDEX, not just a result.**
>    `OutboundBatchItem { index: usize, result: OutboundSlotResult }`.
>    The pseudocode's `Ok(Item(slot)) => record(slot)` hides that the
>    index comes FROM THE ITEM — so restoring original positions is
>    `out[map[item.index]] = item.result`, and auctioneer's compaction map
>    is keyed by the batch index upstream reports, not by arrival
>    order. Writing it as `slot` invites an implementer to assume
>    arrival order IS input order, which completion-order delivery
>    explicitly breaks.
> 2. **`cancel(self) -> Vec<usize>` RETURNS THE UNRESOLVED INDICES**
>    and consumes the batch; the plan had documented it as returning
>    nothing, which is corrected. **ROUND-171 #1 — BUT AUCTIONEER DOES
>    NOT CALL IT, so this banner states the SIGNATURE ONLY and
>    imposes NO timeout-mapping requirement.** The round-169 text
>    here said those indices "still need a `Timeout(stored
>    selected_cause)`", contradicting the canonical cancellation
>    block below: auctioneer's three real paths are `Finished(Cutoff)`
>    for deadlines, the outer error for fatal failures, and the
>    batch's own `Drop` for handler teardown — none of which calls
>    `cancel()`. EdgeZero offers it for callers that HAVE a defined
>    non-fatal early-stop policy; auctioneer currently has no such
>    trigger, so the mapping would be dead code. If a future round
>    defines one, it adds the mapping THERE, not here.
> 3. **A FAILED BATCH IS POISONED.** After any `Err` from `next()`,
>    the handle sets `poisoned` and every subsequent `next()` returns
>    "polled after an invariant failure". auctioneer's atomic-publication
>    policy already stops at the first `Err`, so this is compatible —
>    but it must be STATED, because a future round tempted to salvage
>    `failure.slots` cannot do it by polling on.
> Also verified: `Finished(Completed)` is set WHEN THE LAST ITEM
> RESOLVES, and `next()` then returns it on the FOLLOWING call. The
> loop must keep polling after the final `Item` to observe
> termination — a loop that exits when its own tally is full never
> sees `Completed` and never runs the positional check.
>
> **REFERENCE STATE — UPDATED 2026-10-06 (logging pin move).**
> Two revisions matter and they are DIFFERENT; every citation in this
> plan names which one it means:
> - **PINNED (what auctioneer compiles against today):**
>   `dfd439c613af4f6ed295e71c032881b15bf7247c`, from `Cargo.lock`'s
>   `edgezero-core` entry, pinned by `rev =` in the workspace
>   manifest rather than by branch, and re-asserted by
>   `codec_pins.rs`'s `EDGEZERO_REV`. The pin has moved five times
>   since `120dd2aa`: M3b-11 took it to `6904869b`; a one-commit move
>   to `cff8a85e` took upstream's `fix(cli): isolate template
>   dependency feature`, which makes `edgezero-cli`'s `handlebars`
>   optional; `ab333f4e` added `PlatformMetadata`, which publishes
>   each target's `MemoryCeiling` with its scope
>   (`PerInstance` vs `PerExecution`) and its `MemoryCeilingSource`;
>   `2b4d22b4` is the move described next; `491df844` is the
>   platform-resource move described after it; `74f864bd` is the
>   content-encoding move described last, which CLOSED F10; and
>   `c84996a4` is the config-binding move described below.
>   The latest move from `5527b58` to `dfd439c` is one logging-only
>   commit: independent boot filtering, native logger initialization
>   before configuration, and deferred Fastly runtime-env diagnostics.
>   Older revision-specific measurements below remain historical;
>   decoder and resource-contract implementations are unchanged by this move.
>
>   **The move to `c84996a4` WAS a clean fast-forward** — exactly one
>   commit, `fix: preserve declared config bindings and defer Spin
>   opens`, which CLOSED both EdgeZero findings from auctioneer's fourth
>   review round. Nothing in auctioneer broke:
>   `cargo check --workspace --all-targets` passed unchanged, because
>   every signature auctioneer calls kept its shape. The commit changes
>   WHICH typed error a declared-but-unopenable config store produces,
>   not how it is reported.
>
>   Upstream now adds `ConfigStoreOpenFailure`
>   (`Unavailable` / `InvalidConfiguration`) and
>   `ConfigStoreHandle::failed_open(failure)`, which returns a handle
>   whose every read fails with that classification. Cloudflare and
>   Fastly register that handle instead of omitting the binding, and
>   Spin binds its label with `SpinConfigStore::for_label` and opens
>   inside `get_bounded` rather than during `build_config_registry`.
>
>   **auctioneer's classification needed no loosening or tightening, and
>   the reason is worth recording** rather than re-derived: a failing
>   handle returns `Err`, and `map_config_store_error` sends
>   `Internal` to `BackendFailure` and `Unavailable` to
>   `BackendUnavailable`. `MissingBlob` is produced at exactly one
>   site, from a SUCCESSFUL read whose value is `None`, so a failed
>   open can never reach the `allow_missing_config` rule. The flag
>   governs absence; a failure is already a fault under either policy.
>   What WAS missing is proof: `malformed_reasons_ignore_the_bootstrap_policy`
>   now pins the malformed family across both policies, the companion
>   to the transient row that already did, and
>   `tests/config_store_failed_open.rs` drives `/status` through the
>   real `failed_open` constructor. That binary cannot compile against
>   `74f864bd`, where neither symbol exists.
>
>   **Finding 2's `/status` consequence is now observable.** Spin's
>   open previously aborted `dispatch_with_registries` before any
>   route ran, so no degraded verdict could be reported; deferring the
>   open into the timed read means the failure arrives as an extraction
>   error and `/status` answers 200 with
>   `config: degraded (malformed_config)` and the `degraded` header.
>   The open is now also subject to extraction's deadline, which
>   upstream covers with
>   `bounded_read_timer_cancels_pending_store_open`.
>
>   **The move to `2b4d22b4` was NOT a fast-forward.** Upstream
>   REBASED `docs/outbound-http-spec`, so `ab333f4e` is not an
>   ancestor of `2b4d22b4`: they diverge at `98930917`, and "expose
>   platform memory ceilings" appears on both sides at different SHAs
>   — `ab333f4e` on the discarded tip, `8b29079f` on the rebased one.
>   The pin auctioneer held was an orphan on a tip upstream threw away.
>   Two upstream signatures moved, and both broke auctioneer:
>   `OutboundResponse::new` gained a fifth `MonotonicClock` parameter
>   and absorbed the removed `new_with_monotonic_clock`, and
>   `Hooks::build_app` was removed from both the trait and
>   `edgezero-macros` in favour of `App::build::<A>(platform)`.
>   Upstream's reason for the clock is that a response's deferred
>   body deadlines must sit in the application's monotonic domain, so
>   every auctioneer call site passes the clock the surrounding scheduler
>   already runs on rather than `MonotonicClock::default()`.
>   `MonotonicClock::deadline_after` is also new; auctioneer has no site
>   that needs it yet, because nothing here composes a deadline from
>   a bare `Duration` at the moment it builds a response.
>   No envelope total moved: axum still validates at `437_836_672` of
>   `512_000_000` and Cloudflare, Fastly and Spin still refuse boot.
>
>   **This falsifies the plan's assertion that `outbound.rs` is
>   unchanged across heads.** `scripts/check-batch-rebase.sh` still
>   exits 0, because that gate scans for the retired `send_all`
>   contract in prose rather than diffing upstream source — so the
>   stale claim is recorded here rather than caught there.
>   That last move is what lets the four adapter envelopes read their
>   ceiling from the adapter instead of hand-copying a literal, and it
>   is why Spin's binary `134_217_728` differs from the decimal
>   `128_000_000` Cloudflare and Fastly publish. Before it,
>   `handlebars` enabled `serde_json/preserve_order` for EVERY
>   workspace member, swapping `serde_json::Map` from `BTreeMap` to
>   `IndexMap` and doubling retained bytes per node on the auction
>   path — 104.6 B/node against the 64 the isolate budget charges.
>   Production artifacts were never affected (every adapter build is
>   package-scoped, so `auctioneer-cli` is not in their graphs), but
>   `cargo test --workspace` was, and `parsed_node_charge_guard` now
>   asserts its map backing before measuring. The "@120dd2aa" citations below, the compatibility
>
>   **The move to `491df844` WAS a clean fast-forward** — `2b4d22b4`
>   is an ancestor — and its point was not the version bump. Nine
>   commits, two of which answer questions auctioneer had recorded as
>   unresolved.
>
>   `InboundRequestPopulationBound` publishes "maximum live inbound
>   request executions charged to one memory ceiling" as a
>   `PlatformFact` that may be unknown. Fastly and Akamai Functions
>   publish a known `1`; Cloudflare publishes
>   `Unknown(ProviderUnpublished)`. **Upstream confirmed auctioneer's
>   derivations rather than contradicting them**, so no envelope total
>   moved. The Fastly and Spin rows now READ the published `1` instead
>   of restating the derivation from the scope, and Cloudflare keeps
>   its borrowed `6` with the provenance restated: upstream has
>   formally recorded the figure as unavailable, which is a
>   confirmation of the borrowing's weakness and not a licence to drop
>   the charge to zero. `the_population_bound_is_unpublished_upstream`
>   pins the unknown so a future publication fails a test rather than
>   leaving a stale constant.
>
>   `HostIngressMemoryAccounting` (`CountsTowardCeiling` vs
>   `OutsideCeiling`) was expected to settle whether host-side framing
>   memory counts against the guest ceiling. **It did not settle it.**
>   Every adapter publishes `Unknown` — `ProviderUnpublished` on
>   Cloudflare, Fastly and Akamai, `RuntimeConfigured` on generic
>   Spin. The only `OutsideCeiling` values upstream are in test
>   fixtures and generated scaffolding, not vendor facts. So auctioneer's
>   conservative charge stands on all four rows, now cited to
>   upstream's recorded unknown rather than to auctioneer's own inability
>   to find an answer. `parser_transient_bytes` stays funded
>   everywhere.
>
>   Three signatures broke and all three were mechanical.
>   `PlatformMetadata::memory_ceiling()` returns
>   `PlatformFact<MemoryCeiling>` instead of `Option<MemoryCeiling>`;
>   `MemoryCeilingSource` is renamed `PlatformResourceSource` and
>   moved OFF `MemoryCeiling` onto the fact, so provenance is read
>   from the fact rather than the value; and `PlatformMetadata::new`
>   takes three facts instead of one `Option`, for which
>   `PlatformMetadata::default()` — all facts `Unknown(Unspecified)`
>   — is the faithful successor to the old `new(None)` the core tests
>   passed. `auctioneer-adapter-fastly` and `auctioneer-adapter-spin` promoted
>   `edgezero-core` from a dev-dependency to a normal one so their
>   envelope rows can name `PlatformFact` in a `const`, matching what
>   `auctioneer-adapter-cloudflare` already did.
>
>   `start_batch_until`'s parameter is renamed `cutoff` to
>   `observation_cutoff` and its semantics are now spelled out: it
>   "stops batch observation regardless of any remaining per-request
>   budget or later per-request deadline", and "passing an earlier
>   observation cutoff intentionally leaves unresolved slots".
>   **This confirms the F9 fix in `82cfce9`**: the wave cutoff is the
>   LATEST dispatched deadline, restricted to calls that cleared
>   conversion, precisely because an earlier one would truncate
>   siblings that were given longer. auctioneer's `start_wave` adopts the
>   upstream parameter name so the vocabulary matches across the seam.
>
>   Upstream also added `MemoryEnvelope`, `validate_memory_envelope`
>   and `MemoryEnvelopeValidation`. auctioneer did NOT migrate to them in
>   this unit — that is a separate decision — but upstream's
>   validator DOES close auctioneer's known gap:
>   `validate_memory_envelope` refuses a `ScopeMismatch` between the
>   envelope's scope and the ceiling's, which is exactly the
>   per-instance-row-on-a-per-execution-target error
>   `validate_isolate_limits` cannot express. It also reports
>   `Indeterminate` with a reason rather than silently charging a
>   guess when a fact is unknown. Both are strictly better than what
>   auctioneer has; what it does NOT carry is auctioneer's per-term
>   decomposition (the dominating-term breakdown an operator reads in
>   the boot warning), so a migration would complement rather than
>   replace.
>
>   **The move to `74f864bd` WAS a clean fast-forward** —
>   `491df844` is an ancestor — and it **CLOSED F10**. Three commits.
>   `97b1a7e2 feat(outbound): decode deflate and type passthrough
>   reasons` implements, in the shape auctioneer asked for, the
>   requirement recorded in
>   `scratchpad/edgezero-content-encoding-requirement.md`:
>   `ContentEncoding` gains a `Deflate` variant that upstream DECODES
>   and strips, and `Passthrough` now carries a typed
>   `PassthroughReason` (`Malformed` / `Stacked` /
>   `Unsupported(String)`).
>
>   **Why that closes F10 rather than merely enabling it.** The
>   finding was that passthrough bodies reach bidder parsers as
>   compressed bytes and get rejected. `deflate` was the common case
>   and upstream now decodes it, so it never arrives as a passthrough
>   at all. What remains is a residue that NO decoder in this
>   workspace could finish either — a stacked chain, a coding nothing
>   here implements, a malformed header — so the residue is a REFUSAL,
>   not deferred work, and closing F10 needed a classification rather
>   than the layered decoder earlier rounds scoped. `CapRow` therefore
>   became `RawPassthrough(UndecodableEncoding)` and the body is
>   refused with its cause instead of published still encoded.
>
>   `auctioneer-core` **stopped re-parsing `Content-Encoding`**. The old
>   `cap_row_of` ran its own header scan, which had already drifted
>   from upstream: it classified `deflate` — a coding upstream now
>   decodes and strips — as still-encoded, and it classified a
>   retained `br` as a passthrough, which upstream can no longer
>   produce. It now reads `classify_content_encoding` and maps the
>   typed reason once.
>
>   **`FanoutError::UndecodableResponse(UndecodableEncoding)` is the
>   new wire-bearing variant** — code 4 / `seatnonbid` 100, the pair
>   the spec reserved for `UnsupportedContentEncoding`. It was
>   unimplementable before this upstream commit because the spec's
>   `actual: String` field had nothing honest to fill it with; the
>   typed `Unsupported(token)` now does, and the token is upstream's
>   PARSED coding — lowercased and validated against the HTTP token
>   grammar — rather than free upstream text. `public_message`
>   still renders fixed category text and reads no payload, so the
>   token never reaches the wire.
>
>   `UndecodableEncoding` lives in `auctioneer-exchange` beside
>   `OversizeOrigin` and mirrors its construction exactly: one
>   explicit arm per recognised upstream reason plus `Unclassified`
>   for the `#[non_exhaustive]` wildcard, in the crate that carries no
>   platform dependency.
>
>   **Two breaking renames.** `max_brotli_decoder_bytes` became
>   `max_decoder_bytes` on the request builder and now bounds EVERY
>   decoder; auctioneer's own same-named fields on `ResponseBodyPolicy` and
>   `DecodePolicy` followed suit, because a brotli-specific name is
>   misleading for a value that also bounds gzip and deflate. The
>   figure itself did not move: it stays `brotli_state_bytes(wbits)`,
>   which at its 16,778,240-byte floor exceeds upstream's
>   1 MiB `FLATE_DECODER_FIXED_CHARGE_BYTES` by an order of magnitude,
>   so widening the cap's scope costs auctioneer nothing and **no envelope
>   total moved.** `decode_gzip_stream` gained a `max_decoder_bytes`
>   parameter and `decode_deflate_stream` is new; auctioneer calls neither
>   directly.
>
>   **Three stale test fixtures used `gzip` to mean "passthrough"** —
>   the core's own decode-expiry row and the Cloudflare and Spin
>   cooperator rows. All three would still have compiled and passed
>   while silently exercising the ready-to-parse path, since upstream
>   now decodes `gzip`. They use `zstd` instead, a coding upstream
>   genuinely refuses.
>
>   `69b86ca1` adds `RuntimeVariables` (`from_vars` / `get` /
>   `variable`); auctioneer's config extraction does not touch it. The
>   `docs/guide/capabilities.md` rows changed upstream, but no auctioneer
>   test asserts against them, so there was nothing to regenerate.
>   matrix and the prerequisite checklist's pinned column are a
>   HISTORICAL record of the revision those reviews examined — they
>   stay as written, and none of them describes the current build.
> - **HEAD of `docs/outbound-http-spec` (what M3b-0 rebases onto and
>   the pin move targets): `6904869bd0787ba8e6c36e732e3a447c45e99201`
>   — verified by `ls-remote` 2026-09-25 (round-208). ONE commit
>   past `325134e4`: `fix(core): preserve mandatory error headers`.
>   **The head has now moved in EIGHT consecutive review rounds.**
>   Superseded: `325134e4`, `46f85958`, `0d84247e`, `1e7e0562`,
>   `03cdba98`, `351698c9`, `4790a140`.
>   **`325134e4` ADDED `App::set_error_response_renderer(Fn(EdgeError)
>   -> Response)`**, which hands the application the whole error
>   body. auctioneer installs the TWO-ARM renderer (Appendix F's pinned
>   bodies verbatim, `error.kind()` for the rest — round-207 #2),
>   bounded well under the 256 charged per permit against ~840 for
>   the default renderer on a 768-byte path — so **G29 is CLOSED
>   and the round-205 framework-error exemption is WITHDRAWN**
>   (round-206 #1).
>   **`6904869b` THEN MADE THAT RENDERER SAFE TO INSTALL
>   (round-208).** It re-extends `error.required_response_headers()`
>   onto the rendered response, so a custom renderer **CANNOT DROP**
>   a mandatory header: canonical `Allow` survives 405 rendering and
>   `Retry-After: 60` survives `config_out_of_date`. Without it,
>   auctioneer's renderer would have silently stripped both — a
>   correctness bug our own tests would not have caught, since they
>   assert BODY bytes. **2a.7b asserts the headers too:**
>   `renderer_preserves_allow_and_retry_after()`.
>   `outbound.rs` is unchanged across `325134e4..6904869b` — and
>   across every head since `46f85958` — so M3b-0's batch rebase
>   again needs no rework.
>   **`ResponseEgressDeadline` IS NOW AN ENUM, AND IT CLOSES A GAP
>   AUCTIONEER FILED ONE ROUND EARLIER (G26):**
>   ```rust
>   ResponseEgressDeadline::after(Duration)  // resolved at egress start
>   ResponseEgressDeadline::at(Deadline)     // absolute, as before
>   ```
>   `begin()` resolves the relative form against `egress_started_at`,
>   clamps oversized durations and keeps the earlier-of
>   application/policy rule. auctioneer uses `after()` for the
>   static-response budget, which RETIRES the round-193
>   marker-header scheme (round-194 #4 — that scheme could not have
>   worked: the policy callback receives IMMUTABLE headers, so the
>   marker could not be both visible to the callback and stripped
>   before transmission).
>   `crates/edgezero-core/src/outbound.rs` is UNCHANGED across
>   `0d84247e..46f85958`, so M3b-0's batch rebase again needs no
>   rework — the fourth consecutive head where that has held.
>   **THE LIFECYCLE API CHANGED AGAIN, AND TWO OF THESE ARE THINGS
>   AUCTIONEER ASKED FOR:**
>   1. **`ResponseEgressCompletion::join(self, other) -> Self`**
>      (`38f61a19`) — composes two completions, each with its own
>      panic boundary, releasing both resources exactly once. **This
>      did NOT exist at `1e7e0562`**, where round-187 #1 verified
>      "exactly three constructors, no join/combine" and built a
>      composite `T` around its absence. That workaround is now
>      RETIRED (round-188 #2).
>   2. **`DetachedResponseEgressDecision::Send { completion,
>      deadline }`** (`ce61f92a`) — the deadline is now a MANDATORY
>      FIELD, not an extension auctioneer inserts. **G25 CLOSED**, and
>      round-187 #3's extension workaround is retired with it:
>      EdgeZero constructs the detached response and installs the
>      returned deadline itself.
>   3. **`ResponseEgressHead::write_deadline_after(Duration)`**
>      anchors at `request_start`, NOT at permit-acquisition time.
>      auctioneer ACCEPTS that anchor: it is strictly EARLIER than
>      acquisition, so the static-pool drainage bound holds a
>      fortiori. No upstream change requested, and auctioneer does not
>      need to capture the shared clock to compute an absolute
>      acquisition-time deadline (round-188 #1).
>   `crates/edgezero-core/src/outbound.rs` is UNCHANGED across
>   `1e7e0562..0d84247e`, so M3b-0's batch rebase still needs no
>   rework — the third consecutive head where that has held.
>   Historical: `1e7e0562` was FOUR
>   commits past `03cdba98`, which is superseded in turn. The head
>   has now moved in four consecutive review rounds; **M3b-0's
>   `ls-remote` re-check before the pin move is not ceremony — it has
>   fired every time.**
>   **`ac69b359` — "allow detached ingress abort decisions" — CLOSES
>   ROUND-187 #2.** The detached pre-admission path is no longer a
>   completion FACTORY but a DECISION:
>   ```rust
>   pub enum DetachedResponseEgressDecision {
>       Abort,                            // adapter's non-response abort boundary
>       Send(ResponseEgressCompletion),
>   }
>   ```
>   So when auctioneer's static pool is exhausted, a pre-admission
>   400/414/431 can now RESET rather than emit an ungated response.
>   The universal static-egress invariant is satisfiable for this
>   class after all — the round-186 #1 claim that it already was
>   ("G23 closed") was PREMATURE: the factory could supply a
>   completion but could not refuse, so exhaustion still sent
>   ungated. It is true at `1e7e0562`, not at `03cdba98`.
>   Historical: `03cdba98` was ONE commit
>   past `351698c9` ("feat(core): harden lifecycle and bounded
>   reads"), which is now superseded along with `4790a140`,
>   `fc8d7e9f` and `5eccff8e`. **THE M3b-0 STOP RULE FIRED AS
>   DESIGNED:** the plan recorded `351698c9`, the live head had
>   moved, and the pin move was held rather than run against a stale
>   SHA. **DELTA `351698c9..03cdba98` — 35 files, 2_654 insertions,
>   288 deletions (full-range diff, no pathspec), and it CLOSES TWO
>   MORE GAPS auctioneer filed:**
>   1. **`ResponseEgressCompletion::late_bound<T>()`** returning
>      `(ResponseEgressResource<T>, ResponseEgressCompletion)`, with
>      `install()` → `Result<(), ResponseEgressResourceInstallError
>      { AlreadyInstalled, Closed }>` and nine tests covering
>      install/close races, poison recovery, duplicate install,
>      abandonment and competing terminal signals. auctioneer's
>      hand-rolled permit slot is DELETED in favour of it
>      (round-186 #3).
>   2. **`DetachedResponseEgressCompletionFactory`** +
>      `App::set_detached_response_egress_completion_factory` —
>      **G23 CLOSED.** Pre-admission normalized-head rejections
>      (400/414/431) can now carry an application completion, so the
>      static-egress exemption round-184 #5 declared is REMOVED.
>   3. **`ConfigStore::get_bounded` has NO default** — "there is no
>      compatibility default because awaiting an unbounded provider
>      call can hide a never-ready future". **G20 CLOSED**, and
>      auctioneer's outer config race is retired (round-186 #7).
>   The batch API is again UNCHANGED (`git diff 351698c9..03cdba98
>   -- crates/edgezero-core/src/outbound.rs` is empty), so M3b-0's
>   rebase still needs no rework. Superseded head history: **THIS IS A BREAKING LIFECYCLE CHANGE, NOT A
>   DOCUMENTATION MOVE**, and it is why M3b-11 and M3b-12 are
>   COLLAPSED into one atomic migration (round-184 #1). Verified in
>   the upstream tree, not inferred from commit subjects:
>   1. `ResponseEgressCompletion` — a RESPONSE-SCOPED
>      `FnOnce(&ResponseEgressReport)`, fired exactly once at
>      terminal egress, panic-guarded
>      (`edgezero-core/src/response_egress.rs`). **G18 CLOSED.**
>   2. `AdmissionDecision` gains `Abort` (closes or resets the native
>      request with NO response) and carries `completion` on `Admit`,
>      `ReadBodyBeforeFallback` AND `Refuse`
>      (`edgezero-core/src/ingress.rs`). **G21 and G22 CLOSED** —
>      refusals DO get a completion callback, which is stronger than
>      the ask.
>   3. `configure` now returns `Result<(), EdgeError>` and
>      `build_app()` returns `Result<App, EdgeError>`
>      (`edgezero-macros/src/app.rs`). **G19 CLOSED**, and this
>      BREAKS every adapter entry point.
>   Delta `4790a140..351698c9` is **45 files, 2_698 insertions, 523
>   deletions** (full-range diff, no pathspec, per the round-173 rule)
>   — `app.rs` alone is 477 lines changed.
>   **THE BATCH API ITSELF IS UNCHANGED:** `git diff
>   4790a140..351698c9 -- crates/edgezero-core/src/outbound.rs`
>   is EMPTY. So M3b-0's documentation rebase, which was written
>   against `4790a140`, needs NO rework for the new target — every
>   batch signature, the four-outcome mapping and the cancellation
>   contract carry over verbatim. The pin move adds the lifecycle
>   migration ON TOP of that work; it does not invalidate it. M3b-0's
>   `ls-remote` re-check still runs, and still stops M3b-11 if head
>   has moved again.
>   **REMAINING GAP (round-184 #5):** `ResponseEgressCompletion` does
>   NOT cover pre-admission normalized-head validation failures;
>   EdgeZero emits 400/414/431 through a detached response with an
>   EMPTY completion before application admission runs. G23 is filed
>   for that, and auctioneer's static-egress guarantee is explicitly
>   weakened for those responses rather than claimed. Superseded head
>   history: **This commit changes
>   upstream's own documentation contract and adds a guard; it does
>   NOT change the batch API or the capability matrix** — verified by
>   diffing `5eccff8e..4790a140`. **ROUND-173 #1 — THE EVIDENCE THIS
>   NOTE ORIGINALLY GAVE WAS FALSE AND IS CORRECTED HERE.** It said
>   "ONE file with six added lines, all rustdoc", because the diff
>   behind it had been restricted to source paths and never saw the
>   rest. The ACTUAL delta is **three files, 21 insertions, 11
>   deletions**:
>   1. `crates/edgezero-core/src/outbound.rs` — six rustdoc lines
>      (the two corroborations below);
>   2. `docs/superpowers/specs/2026-05-21-outbound-http-design.md` —
>      batch-lifecycle clarification, and REMOVAL of a duplicate,
>      STALE `BudgetSource` definition whose variant list
>      (`RequestDeadline, Default, PerCallTimeout, Unspecified`)
>      OMITTED `BatchCutoff`;
>   3. `scripts/check_outbound_docs_contract.mjs` — a guard failing
>      the build unless the spec defines `BudgetSource` exactly once.
>   **The CONCLUSION is unchanged and re-verified: no batch-API and
>   no capability-matrix change.** One consequence for auctioneer, checked
>   rather than assumed: the canonical enum in `error.rs` is
>   `{ BatchCutoff, Default, PerCallTimeout, RequestDeadline,
>   Unspecified }`, so this plan's round-159 #5 migration to
>   `RequestDeadline` / `BatchCutoff` names variants that DO exist —
>   the definition upstream deleted was the one that would have
>   contradicted it. Notably the rustdoc lines CORROBORATE two
>   findings this plan already carries: upstream now documents that
>   "after the final `Item`, callers must poll once more to receive
>   `Finished(Completed)`; completion is not inferred from the number
>   of items observed" (round-169 M4), and that a caller mapping
>   cancellation "still owes every returned slot an outcome because
>   EdgeZero observed no terminal result for those indices"
>   (round-169 M2 — which is why auctioneer, having no non-fatal
>   early-stop trigger, simply does not call `cancel()`: round-170
>   #1). So every
>   contract this plan records from `5eccff8e` — `OutboundBatchItem
>   { index, result }`, `cancel(self) -> Vec<usize>`, poisoning after
>   an `Err`, `Finished(Completed)` on the poll after the last item,
>   and the capability drift highlights — stands unchanged. M3b-0
>   still RE-VERIFIES against whatever head is live when it runs;
>   that rule is never satisfied by this note. Superseding
>   `fc8d7e9f`, `5eccff8e` (round-167's CI-fix revision, which the
>   round-169 API review was read against; the batch contract
>   recorded from it — `OutboundBatchItem { index, result }`,
>   `cancel(self) -> Vec<usize>`, poisoning, and
>   `Finished(Completed)` on the poll after the last item — is
>   carried forward and RE-VERIFIED by M3b-0 against this head).** This is THE RECORDED TARGET: the round-166 hold
>   ("do not record `543b782d`, a CI fix is coming") is discharged,
>   because that fix has landed and this is it.
>   **EVERY EARLIER SNAPSHOT IS SUPERSEDED AND NONE OF THEM
>   AUTHORIZES ANYTHING:** `543b782d`, `3d87d5bc`, `1d47bc0a`,
>   `61457d7d`. Where text below still reads "rebased onto
>   `3d87d5bc`" or "if it no longer returns `3d87d5bc`", substitute
>   the recorded target above — those sentences predate this entry
>   and are corrected here rather than in six places.
>   **THE DISPATCH-TIME RECHECK STILL RUNS AND STILL BINDS**
>   (round-162 #7): M3b-0 re-runs `ls-remote` when it is actually
>   dispatched and writes what it finds. If the branch has moved
>   again by then, the recorded SHA above is what it was checked
>   against, not a guarantee — the task rebases onto whatever is
>   live and updates this entry.
>   Previously `3d87d5bc`, confirmed live via
>   `git ls-remote https://github.com/IABTechLab/edgezero.git
>   refs/heads/docs/outbound-http-spec` on 2026-09-17 — the branch
>   head still resolves to the revision round-163 recorded, so the
>   round-162 #7 SHA assertion HOLDS and M3b-0 may proceed against
>   it. `1d47bc0a` and `61457d7d` are superseded snapshots, retained
>   below only as history.
> **HOW TO VERIFY AN UPSTREAM DELTA (round-173 #1 — the method, not
> just the correction): diff the FULL revision range with no
> pathspec** (`git diff --stat <old>..<new>` and
> `git diff --name-only <old>..<new>`), THEN narrow. A pathspec-first
> diff answers only the question you already thought to ask, and it
> silently hides spec and tooling changes — which is exactly how this
> note came to claim "one file, six lines" for a three-file delta
> that removed a stale enum definition and added a build guard.
>
> RE-CHECK RULE: M3b-11's **Step 1** (round-168 #2 — it was Step 2
> before the round-167 reorder) re-runs that `ls-remote` before
> touching the lockfile; if it no longer returns the RECORDED TARGET
> (`351698c9`, round-184 #1 — `4790a140` superseded), M3b-11
> STOPS and M3b-0 re-runs against the new head (round-162 #7).
>
> **⚠ ROUND-163 #1 — THE SNAPSHOT MOVED (head was `3d87d5bc` then;
> the recorded target is now `03cdba98`, round-186 #1) AND
> `collect()`'s failure arm CHANGED SHAPE.** It no
> longer returns a bare `EdgeError`; it returns
> `OutboundBatchFailure { error, slots }`, which CARRIES THE SLOTS
> THAT ALREADY COMPLETED. Two consequences auctioneer must state
> explicitly rather than inherit by accident:
> 1. **`.await?` NO LONGER EXPRESSES THE CONTRACT.** The `?` on a
>    collected call discards `failure.slots` — every bidder that
>    answered before the driver broke. Any collected example in this
>    plan or the master spec must destructure
>    `Err(OutboundBatchFailure { error, slots })` and say what it
>    does with BOTH fields.
> 2. **THE `next()` LOOP MAY STILL FAIL THE WHOLE AUCTION on a
>    driver error — but that is now a DELIBERATE DISCARD, not the
>    only option the API offers.** auctioneer's choice stands — outer
>    `Internal` ⇒ 500 — on ATOMIC PUBLICATION grounds (round-165 #4:
>    a wave publishes whole or not at all; the earlier "no per-slot
>    outcome is trust""worthy" rationale contradicted upstream, whose
>    `slots` are completed TERMINAL outcomes). The plan must SAY SO
>    in those words:
>    *auctioneer deliberately discards otherwise-valid completed slots
>    when the batch driver fails, trading a partial auction for a
>    loud one.* Silence would read as ignorance of the new shape.
> M3b-0 IS REBASED ONTO THE RECORDED TARGET ABOVE
> (`03cdba98`, round-186 #1; the round-163 text said `3d87d5bc`,
> since superseded twice): it records that
> SHA, adds `OutboundBatchFailure` to its contract ledger, and its
> drift gate additionally detects (a) the obsolete
> <!-- the gate's own pattern list (batch-rebase-ok) -->
> `Result<OutboundBatchResults, EdgeError>` return type and (b)
> slot-invalidating language. **ROUND-164 #7 — M3b-0 ITSELF IS
> EXECUTABLE.** The round-163 sentence that listed M3b-0 among the
> tasks "not executable as specified" contradicted its own task body,
> which is blocked by nothing; with the round-164 #1/#2 corrections
> (both collected forms handling the failure arm, and a gate that
> cannot match its own documentation) M3b-0 is THE UNBLOCKER for the
> OUTBOUND track and may be dispatched now. **M3b-11 and M3b-2b core
> remain blocked until it lands.**
> **ROUND-186 #8 — M3b-0 IS NOT THE ONLY DISPATCHABLE TASK.**
> **M3b-1c** (`/status` reports what is misconfigured, D-1) is also
> startable today: it touches one handler and its tests, owns no
> roster member, and needs no clock, no batch API and no pin. It is
> on the `/status` track, which the outbound rebase does not gate.
> Any sentence reading "M3b-0 is the only dispatchable task" is
> stale.
>
> CONSEQUENCES, binding:
> 1. **Nothing in M3b may be implemented from the `send_all`
>    contract as if it were durable.** The algorithm in master-spec
>    §5.2 and every `BoundedFetch` / `run_wave` paragraph here
>    describe the PINNED API and are correct only for `120dd2aa`.
> 2. **M3b-11 CANNOT be dispatched from its current body.** Moving
>    the pin deletes a trait method auctioneer implements three times;
>    the task would fail to compile on its first build. M3b-11 is
>    therefore BLOCKED pending the documentation rebase below.
>    **OWNERSHIP, corrected (round-160 #2): M3b-11 does NOT own
>    `BoundedFetch` or `run_wave` — those are M3b-2b's core
>    deliverables and were double-assigned by round-159 #6.** The
>    pin move owns only what the pin itself breaks. THE ORDER IS:
>    (i) the P0 documentation rebase (this banner's item 4);
>    (ii) **M3b-11** — pin + Cargo.lock, the three SDK bumps, the
>    Fastly entrypoint, the THREE mock/test client migrations named
>    above, the regenerated capability manifest and profiles, and the
>    codec tuple; (iii) **M3b-2b** — `BoundedFetch`, the
>    completion-driven scheduler and decoder integration, written
>    against the batch API from the start because by then it is the
>    only API there is.
> 3. **The API-independent work is unaffected** and may proceed:
>    2a.1–2a.3, 2a.4a, 2a.4, 2a.6 (sink, builder, budget, admission,
>    prepare-limits), and 2b.0/2b.1 (the clock-domain migration and
>    code-12 attribution) touch no batch API.
> 4. **THE REBASE (a documentation task, before M3b-2b core and
>    M3b-11 — round-159 #2: a semantic redesign, not a rename).**
>    `run_wave` must be respecified against the batch handle:
>    - the wave CUTOFF passed to `start_batch_until` is the LATEST
>      composed deadline among that wave's members, while each
>      request still carries its OWN earlier absolute
>      `Deadline::at_instant(composed_at)` — the cutoff bounds
>      observation, the per-request deadline bounds the call;
>    - EdgeZero batch indices are COMPACT over the dispatched
>      subset, so the facade keeps the same `(batch_index →
>      original auction index)` map preflight compaction already
>      builds, and restores positions from it;
>    - a COMPLETED slot is TERMINAL: reaching the cutoff must never
>      rewrite a sibling that already answered (the round-153 #2
>      rule, now enforced by the API rather than by auctioneer
>      restraint);
>    - a slot UNRESOLVED at cutoff becomes
>      `Timeout(stored selected_cause)` — auctioneer's stored cause, not
>      upstream's provenance;
>    - **THE FOUR OUTCOMES OF `next()` (round-160 #1), each mapped
>      explicitly, with a wildcard because both enums are
>      `#[non_exhaustive]`:**
>      `Ok(Item(slot))` → lift and record that slot; it is TERMINAL.
>      `Ok(Finished(Completed))` → every dispatched slot resolved;
>      any slot still unfilled is a genuine positional breach ⇒
>      `FanoutInternalError::BatchContractViolation { expected, got }`.
>      `Ok(Finished(Cutoff))` → the observation window closed; every
>      slot still unfilled becomes `Timeout(stored selected_cause)` —
>      NOT a contract violation, and completed siblings are untouched.
>      `Err(EdgeError)` → BATCH-LEVEL failure (duplicate or
>      out-of-range index, premature driver termination ⇒
>      `EdgeError::Internal`): on the `next()` path the Items already
>      recorded ARE terminal and valid, but auctioneer publishes a wave
>      whole or not at all, so ATOMIC PUBLICATION (round-165 #4) maps
>      this to the outer `FanoutInternalError::Internal` ⇒ 500, and
>      the wave lease releases by RAII.
>      `_ =>` (a future variant of either enum) → treated as
>      `Err`-equivalent: outer `Internal` ⇒ 500, never a silent
>      per-slot default. A new upstream variant is a pin-move review
>      item, exactly like the `BadGatewayReason` wildcard.
>      In the `collect()` shape the same distinction appears as an
>      ABSENT positional slot when termination was `Cutoff`; an
>      absent slot under `Completed` is the contract breach;
>    - **`cancel()` SEMANTICS, corrected (round-161 #4).** The
>      round-160 wording named two triggers, and both were wrong:
>      (a) ON HANDLER TEARDOWN no auctioneer code runs — the auction
>      future is dropped, so nothing can CALL `cancel()`. Teardown
>      works through `OutboundBatch`'s own `Drop`, which is exactly
>      what makes round-156 #1's "handler teardown only" rule work
>      without an explicit call. If auctioneer ever needs teardown to do
>      more than upstream's drop does, that requires a deliberate
>      RAII owner holding the batch, designed as such — not a
>      `cancel()` call on a path where no code executes.
>      (b) AFTER EVERY SLOT RESOLVES the scheduler must keep polling
>      until `Finished(Completed)`, NOT call `cancel()` — cancelling
>      there would skip the termination signal that distinguishes a
>      complete batch from a cutoff, discarding the very validation
>      the four-outcome mapping depends on.
>      **ROUND-170 #1 — AUCTIONEER NEVER CALLS `cancel()` ON THE NORMAL
>      FANOUT PATH. THERE IS NO NON-FATAL TRIGGER, AND THE ROUND-169
>      M2 TEXT THAT STOOD HERE WAS INCOHERENT.** It named one trigger
>      — "the auction has already failed fatally" — and then required
>      the returned indices to become positional
>      `Timeout(selected_cause)` results. Those two cannot both hold:
>      a fatal auction answers a single atomic 500 with NO per-bidder
>      results on the wire, so positional timeouts synthesized there
>      are published nowhere. Every real path is already covered
>      without an explicit call:
>      - **deadline reached** ⇒ the driver emits
>        `Finished(Cutoff)`; unresolved slots become
>        `Timeout(stored selected_cause)` through the CUTOFF arm,
>        which IS published. No `cancel()`.
>      - **fatal failure** (driver `Err`, contract violation) ⇒ outer
>        `FanoutInternalError` ⇒ 500, atomic publication. Nothing
>        per-bidder reaches the wire, so there is nothing to fill.
>      - **handler teardown** ⇒ the auction future is dropped and no
>        auctioneer code runs; the batch's own `Drop` releases adapter
>        work (round-161 #4).
>      `cancel(self) -> Vec<usize>` therefore exists upstream and
>      auctioneer simply does not use it in `run_wave`. IF a future round
>      introduces a NON-FATAL early stop — one where the auction still
>      publishes per-bidder results — that round must define the
>      trigger FIRST, and only then may it consume the returned
>      indices as positional timeouts. Until such a trigger exists,
>      specifying the mapping was specifying dead code;
>    - **ROUND-169 M3 — A FAILED BATCH IS POISONED.** After any `Err`
>      from `next()`, upstream marks the handle poisoned and every
>      later `next()` returns "polled after an invariant failure".
>      auctioneer's atomic-publication policy already stops at the first
>      `Err`, so the two agree — but a future round tempted to salvage
>      `failure.slots` CANNOT do it by polling on. Salvage, if ever
>      adopted, must read the slots already delivered plus
>      `OutboundBatchFailure.slots`, never resume the handle;
>    - the wave LEASE is held until network work AND all
>      auctioneer-owned decoding finish, unchanged.
>    PREFERRED SHAPE: completion-driven — poll `batch.next()`
>    alongside a decode `FuturesUnordered`, so decoding one raw
>    response does not delay observing a sibling's completion.
>    THE VALID CALL FORMS (round-161 #4 — `send_all_until(..)
>    .collect()` is NOT one of them; that was a malformed chain of
>    two different entry points):
>    ```rust
>    // ROUND-164 #1 — BOTH collected forms handle the failure arm.
>    // `.await?` is FORBIDDEN on either: it coerces
>    // `OutboundBatchFailure` into an `EdgeError` and throws away
>    // `failure.slots`, the completed bidders.
>    //
>    // convenience form: dispatch and collect in one await
>    let results = match client.send_all_until(requests, cutoff).await {
>        Ok(results) => results,
>        // `..` is REQUIRED: OutboundBatchFailure is #[non_exhaustive],
>        // so an exhaustive field pattern does not compile outside
>        // EdgeZero's own crate (round-164 #1).
>        Err(OutboundBatchFailure { error, slots, .. }) => {
>            return Err(driver_failure(error, slots));
>        }
>    };
>    // handle form, collected at the end — same contract
>    let results = match client.start_batch_until(requests, cutoff)
>                              .collect().await {
>        Ok(results) => results,
>        Err(OutboundBatchFailure { error, slots, .. }) => {
>            return Err(driver_failure(error, slots));
>        }
>    };
>    // THE ONE POLICY, written once so neither form can drift:
>    fn driver_failure(error: EdgeError, slots: ..) -> FanoutInternalError {
>        // ROUND-165 #4 — THE RATIONALE, CORRECTED. auctioneer
>        // DELIBERATELY discards `slots`, but NOT because they are
>        // untrustworthy: upstream's contract says the opposite —
>        // `slots` holds COMPLETED TERMINAL outcomes. Claiming
>        // otherwise contradicts the very API this plan targets. The
>        // real reason is ATOMIC PUBLICATION: a wave publishes whole
>        // or not at all, so an auction that cannot account for every
>        // dispatched slot answers 500 rather than emit a partial bid
>        // set whose gaps are indistinguishable from no-bids. A future
>        // round may salvage them — the API allows it; only auctioneer's
>        // own policy does not. `error` is logged as a typed
>        // category; neither field reaches the wire.
>        FanoutInternalError::Internal("outbound batch driver failed".to_owned())
>    }
>    // handle form, completion-driven (PREFERRED). ROUND-162 #5:
>    // the loop is an EXHAUSTIVE MATCH, never `while let Item(..)`,
>    // which would silently collapse Completed and Cutoff into the
>    // same exit and discard the distinction the four-outcome
>    // mapping is built on.
>    let mut batch = client.start_batch_until(requests, cutoff);
>    loop {
>        match batch.next().await {
>            // ROUND-169 M1: the item carries its OWN INDEX —
>            // `OutboundBatchItem { index, result }`. Delivery is in
>            // COMPLETION order, so arrival order is NOT input order:
>            // the position comes from `item.index`, mapped back
>            // through the compaction map, never from a counter.
>            Ok(OutboundBatchNext::Item(item)) => {
>                let original = keys[item.index].0;   // (index, deadline, decode)
>                out[original] = Some(lift(item.result, &keys[item.index]));
>            }
>            // ROUND-169 M4: this arrives on the poll AFTER the last
>            // item resolves — upstream sets Completed when the final
>            // slot lands and delivers it on the NEXT call. So the
>            // loop MUST keep polling after its own tally is full; a
>            // loop that exits on "all slots filled" never observes
>            // termination and never runs the positional check.
>            Ok(OutboundBatchNext::Finished(Completed)) => {
>                // every dispatched slot resolved; an unfilled index
>                // is a positional breach ⇒ BatchContractViolation
>                break finish_completed(&mut out)?;
>            }
>            Ok(OutboundBatchNext::Finished(Cutoff)) => {
>                // unfilled slots ⇒ Timeout(stored selected_cause);
>                // completed siblings untouched
>                break finish_cutoff(&mut out);
>            }
>            // round-163 #1 (blockquote escape fixed, round-164 #1):
>            // on the next() path a driver error has no slots to
>            // carry — the Items already recorded stay in `out`, and
>            // auctioneer DELIBERATELY discards them by failing the
>            // auction rather than publishing a partial wave. Same
>            // policy as the collected form: ATOMIC PUBLICATION
>            // (round-165 #4), not a claim that the recorded Items
>            // are untrustworthy — they are terminal and valid.
>            Err(err) => return Err(FanoutInternalError::Internal(..)),
>            _ => return Err(FanoutInternalError::Internal(..)), // non_exhaustive
>        }
>    }
>    ```
>    Either collected form is permitted but MUST be documented as
>    deliberately retaining post-batch decode latency.
>
> **SPIN TARGET MISMATCH — BLOCKS M3b-4 (round-154 #4).** auctioneer
> builds `auctioneer-adapter-spin` for `wasm32-wasip1`
> (`edgezero.toml:167` + `:172`, `crates/auctioneer-adapter-spin/
> spin.toml:15`, `.github/workflows/test.yml:149,154`, the master
> spec's §11 build matrix and README), but the PINNED EdgeZero at
> `120dd2aa` builds Spin on **`wasm32-wasip2`** (`CLAUDE.md:74`,
> `README.md:40`). auctioneer is therefore compiling its Spin adapter
> for a target upstream no longer targets. FIX, as the first unit of
> M3b-4 (before any fanout work): move every Spin build site to
> `wasm32-wasip2` — `edgezero.toml`, `spin.toml`, the CI matrix
> rows, the spec's build-matrix commands and the README row — verify
> `cargo check -p auctioneer-adapter-spin --target wasm32-wasip2` and the
> crate's conformance test compile, and keep Fastly on
> `wasm32-wasip1` (upstream still targets wasip1 there:
> `CLAUDE.md:72`). Pinned by `spin_builds_on_wasip2()` in CI.
>
> STATUS (round-151 #7, authoritative over every older sentence in
> this file): the pin IS `120dd2aa` (Cargo.lock), the outbound
> prerequisites (Prereq 2, incl. the former Prereq 4) are LANDED and
> checked below, so M3b-2b through M3b-7 wait on NO upstream work —
> only on their own expansion + re-review; M3b-10's adapter-level
> ingress surfaces remain genuinely absent and G6 limits the
> certificate SCOPE, not any task. Round-141 #5 detail: at `120dd2aa` config
> classification, bounded outbound fanout, route classes, grants,
> deadlines, and Brotli limits have LANDED upstream, so for the
> tasks consuming them the gate is now the ATOMIC pin move plus
> per-task re-verification and body expansion, not absent upstream
> work; M3b-10's adapter-level surfaces remain genuinely absent.

> **M3b's outbound prerequisites are LANDED (round-151 #7 supersedes
> the round-117 "upstream-gated" claims):** (a) **Task M3b-9 is
> DONE** (e414a1e); (b) **Prereq 2 is checked** — buffered dual-cap
> the batch handle, per-slot elapsed, typed reasons — so it blocks nothing;
> (c) **the former Prereq 4** (Fastly native fanout) is satisfied by
> dispatch-before-harvest (round-142) and is retired; the only OPEN
> upstream item touching outbound is G6, which bounds the certificate
> scope to `managed`. Tasks M3b-3..5 CONSUME the
> prerequisite deliverables rather than reimplementing them — any
> overlap reads as integration, not duplication. Task-DAG summary:
> upstream deliverables (Prereqs 1-3, 5, 8-10) → Auctioneer integration
> (round-121 #10: M3b-1 is split — 1a pure mapping is free-standing;
> 1b lifecycle integration follows the M3b-2 foundations it consumes)
> (M3b-1..M3b-7, M3b-9 free-standing) → certification (M3b-8,
> M3b-10). Otherwise every task depends on
> at least one EdgeZero prerequisite that is not yet available in
> the pinned snapshot — now EdgeZero **`docs/outbound-http-spec`**
> @ **`120dd2aa`** (round-145 #6 — moved atomically from `6feeae42`;
> **PIN POLICY THROUGH M3b (round-154 #5 — decided, not open):
> auctioneer STAYS on `120dd2aa` for the whole of M3b.** The newer
> PR-275 commits are NOT consumer-neutral: they bump the provider
> SDKs (Fastly `0.12.1`→`0.13.1`, Worker `0.8.3`→`0.8.5`, spin-sdk
> `6.0.0`→`7.0.0`) and change the Fastly entrypoint from a
> request/response `run_app(req)` to a no-argument runtime
> entrypoint — both of which reach auctioneer's adapter crates
> (`crates/auctioneer-adapter-fastly/src/main.rs:28` calls
> `run_app::<auctioneer_core::App>(req)`, and the root `Cargo.toml`
> pins `fastly = "0.12"`, `worker = "0.8"`, `spin-sdk = "~6.0"`).
> At `120dd2aa` those pins are compatible and nothing in M3b needs
> the newer commits. Moving to them is therefore its OWN atomic task
> — **M3b-11, NOT part of M3b's outbound work** — covering, in ONE
> commit per the atomic pin-move rule: the lockfile move, the three
> SDK version bumps, the Fastly entrypoint rewrite, and the Spin
> wasip2 move if it has not already landed under M3b-4. Until that
> task runs, any statement that a newer commit is
> surface-neutral is FALSE and must not be used to justify a pin
> move. The older text below described the `f3fe8da9`-era commits: Round-143 #1 had
> moved from `f3fe8da9`; the newer PR-275 commits make config extraction,
> outbound response decoding and response-egress deadlines all use
> the ONE application clock, and correct HEAD/304 response
> normalization and Cloudflare response ownership. Round-142 #1's
> move landed — the
> outbound client (`OutboundHttpClient` / `OutboundRequest` /
> `OutboundResponse` / the batch handle with per-slot elapsed), the typed
> `ResponseLimitReason` / `BadGatewayReason` (now including
> `Unreachable` and `Decode(..)`), `StoreExtractionReason`, the
> string-class ingress surface and the app-owned monotonic clock are
> all IN CODE at this SHA; `Cargo.lock` moved by a deliberate
> `cargo update` in the same commit as the RequestContext + bridge
> migration). The per-area status is the COMPATIBILITY MATRIX
> below — verification notes elsewhere citing `9e661ae` /
> `fcefdde3` / `c8d6e414` / `12965da2` are HISTORICAL records of
> those snapshots;
> evidence links dated `9e661ae` in this plan remain valid citations
> of that snapshot, and the load-bearing ones were spot-checked
> unchanged. **The branch now CARRIES the upstream design specs for
> the outbound/inbound surface** — `docs/superpowers/specs/
> 2026-05-21-outbound-http-design.md` and
> `2026-08-22-inbound-body-design.md` in the edgezero repo — and a
> dedicated alignment pass of auctioneer's prerequisite contracts
> against those specs is queued as its own follow-up; the
> prerequisite statements below describe the CODE surface and are
> unaffected by it. A branch ref floats: `Cargo.lock` is what pins
> the build (`120dd2aa` since round-142), so moving the pin is a
> deliberate `cargo update`.

> **PIN-MOVE RULE (round-139 #1; migration list COMPLETED round-142
> #1 — a clean trial update to `120dd2aa` produced ELEVEN compile
> errors, not only the outbound ones).** The move is ONE atomic
> commit that migrates ALL of the following, with the tests named:
> 1. `edgezero_core::proxy` is DELETED — `host_primitives/
>    edgezero_proxy.rs` re-homes onto `OutboundHttpClient`
>    (`proxy_handle()` → the context's `http_client()`). Round-143
>    #2 CORRECTION: this re-homing kept the M3a bridge shape —
>    `EdgeZeroProxyAdapter` calling `HttpClient::send` once per
>    call, uncapped — it did NOT introduce `BoundedFetch`; the
>    facade is M3b-2b's deliverable.
> 2. `RequestContext::request()` is DELETED (auction.rs:82 and every
>    sibling read) — callers move to the new context accessors.
> 3. `into_request()` now returns `Result<Request, EdgeError>` —
>    every call site handles the `Err`.
> 4. `proxy_handle()` is DELETED (auction.rs:323) — replaced by 1.
> 5. Test-extension access (`TestFanoutHandle` / `TestClockHandle` /
>    `TestRngHandle` and the store-registry insertions) migrates to
>    the new context API.
> 6. The fanout conformance suite and the Axum transport-cancellation
>    tests migrate to the buffered client.
> Gate for the commit: fmt; `clippy -D warnings` on both feature
> combos; the conformance suite under its feature; the Axum
> transport tests; the full workspace suite — all green in the
> same commit.

> **COMPATIBILITY MATRIX (round-139 #2 two-snapshot form; round-142
> #1 collapsed it; round-143 #1 re-pinned — the BUILT pin IS the
> reviewed head `120dd2aa`). "Design Yes" alone unblocks nothing;
> the code column is what tasks may consume; "usable" means
> consumable, NOT consumed (round-143 #2). Rows re-verified against
> code when the consuming task starts:**
>
> | Requirement | Design | Code @12965da2 (previous pin) | Code @120dd2aa (BUILT) | M3b usable |
> |---|---|---|---|---|
> | `Deadline` / `BudgetSource` / `BadGatewayReason` / gateway-error primitives | Yes | **Yes** | Yes | Partially (primitives only) |
> <!-- PINNED-state capability row (batch-rebase-ok) -->
> | Buffered positional batch + empty-batch (`OutboundHttpClient` etc.) | Yes | No | **Yes** | Consumable — **NOT YET CONSUMED** (round-143 #2): the M3a bridge issues one `HttpClient::send` per call under `join_all` and reads `into_bytes_bounded(usize::MAX)`; the positional `send_all` batch, dual caps, slot metadata and the bounded facade are M3b-2b, UNIMPLEMENTED |
> | Encoded-byte / header caps (`ResponseLimitReason` typed reasons, incl. `BufferedBody` / `DecoderMemory`) | Yes | No | **Yes** | **Yes** |
> | Encoding-sensitive dual caps (G5 — independent encoded + decoded) | Yes | No | **Yes** | **Yes** — the five-clause facade contract applies (the max() rule covers only the buffered memory bound) |
> | Per-slot elapsed (G15) | Yes | No | **Yes** | **Yes** |
> | Unreachable / decode-corruption discriminants (G10) | Yes | No | **Yes** — `BadGatewayReason::{Unreachable, Decode(Brotli \| Gzip \| Json)}` (round-142) | **Yes** — mapped in the round-137 table |
> | Brotli window gate; multi-member gzip/EOF (G11) | Yes | No | **Yes — verified by source inspection @120dd2aa (round-146 #2)**: `edgezero-core/src/compression.rs` — `brotli_decoder_memory_charge(window_bits)` L79-92; `window_bits > max_window_bits` ⇒ `ResponseLimitReason::BrotliWindow` L181-190; `GzipDecoder::multiple_members(true)` L110-111 pinned by `decode_gzip_drains_every_member_to_native_eof` L485; brotli trailing-input recovery L231-233 | **Yes** — consumed through the round-137 mapping table; M3b-2b and M3b-3/4/5 cite these sites |
> <!-- PINNED-state capability row (batch-rebase-ok) -->
> | CF manual encoding + abort; Spin owned exchange; Fastly batch dispatch | Yes | No | **Yes — verified by source inspection @120dd2aa (round-146 #2)**: `edgezero-adapter-cloudflare/src/outbound.rs` — `classify_content_encoding` / `decode_brotli_stream` / `decode_gzip_stream` L104, `web_sys::AbortController` L222/L240, `controller.abort()` L347, `abort_guard` L421-539; `edgezero-adapter-spin/src/outbound.rs` — `mod exchange` L28, `run_exchange` L131, exchange future L363; `edgezero-adapter-fastly/src/outbound.rs` — `send_all` L496 dispatches every request (`send_async` L389/L456) before harvesting (`wait_pending` L392; `DISPATCH_SLACK_MESSAGE` L181) | **Yes** — the per-adapter rows in M3b-3/4/5 bind to these sites |
> | Frozen-clock fairness (G12) | Yes | Yes (primitive-level) | Yes | Partially |
> | Source-chunk read-allocation bound (G6) | **No — OPEN** (`max_chunk_bytes` rechunks OUTPUT only) | No | No | No — certificate exclusion documented |
> | Ingress route-aware classification / grant / ctx access / parser caps (G1/G16 residue) | Yes (revised shape) | No | **Major surfaces added at 120dd2aa** (string route classes, route-table introspection/resolution, app-owned clock, request start, transferable grants) — re-verify; adapter wiring still absent | M3b-10 blocked pending the round-140 #6 task rewrite + pin move |
> | Config classification / deadline (G4) | Yes | No | **Yes — `StoreExtractionReason`, 16-variant `#[non_exhaustive]` (error.rs)** | **Yes** — M3b-1a expands against it now (round-140 #4 total mapping) |
> | Egress lifetime (G3) | Draft/partial | No | Surfaces added at 120dd2aa — re-verify | No until re-verified |
> | Fastly outbound/deadlines/slot isolation; Spin deadlines | BestEffort BY DESIGN | — | — | Per-target capability policy below is the contract |

> **Per-target capability policy (round-137 #7 — a CONTRACT choice,
> stated, never an implementation placeholder):** upstream marks
> Fastly outbound HTTP / deadlines / slot isolation and Spin
> deadlines BestEffort by design. Auctioneer's choice: Fastly and Spin
> ship behavior-complete with DOCUMENTED BestEffort deadline
> semantics — auctioneer's own absolute deadline checks remain
> authoritative for cause attribution and code-12 — and (round-155
> #2) that authority covers AUCTIONEER-OWNED WORK ONLY: the wave wait and
> auctioneer's passthrough decode. An upstream slot outcome is terminal
> on every target, Fastly and Spin included; a BestEffort host that
> ran long reports its own `GatewayTimeout`, which auctioneer classifies
> through the stored cause. Auctioneer does NOT re-compare the clock
> after the batch to drop "late-looking" results — that would
> discard a bidder that answered inside its deadline whenever a
> sibling delayed the batch. NEITHER
> target may claim the M3b deadline-enforcement guarantee Axum and
> Cloudflare claim, and their conformance rows assert the
> documented BestEffort behavior. WHAT "CONCURRENT" MEANS
> (round-145 #3, one definition for every document; round-152 #5
> tightens it): every SUCCESSFUL dispatch call for every eligible
> slot COMPLETES BEFORE THE FIRST HARVEST BEGINS. Dispatch is
> sequential, so this does NOT promise simultaneous overlap — an
> early request may finish before a later one is dispatched; overlap
> is the provider's timing, never a auctioneer claim. That is the whole
> achievable Fastly guarantee — it
> adds NO complete-exchange concurrency and NO independent terminal
> observation (serial, input-ordered harvest; BestEffort slot
> isolation); Axum/Cloudflare/Spin additionally observe each slot's
> terminal independently. ELAPSED SEMANTICS ON FASTLY
> (round-138 #4 — the decision the G15/no-degraded-metric pair
> forced): Fastly's proposed harvest drains pending requests
> SERIALLY in harvest order, so a completed slot's completion can
> be OBSERVED late — true completion timestamps are not available
> there. Auctioneer's choice: on Fastly, per-slot elapsed is an
> OBSERVATION-TIME metric — WRITTEN TO `responsetimemillis[<bidder>]`
> AS-IS (round-148 #6: an upper bound on true completion, never an
> under-count; the wire field is a bare integer with no degradation
> flag, so the meaning is declared out-of-band, not withheld) —
> declared as such in Fastly's capability
> row and its deployment documentation (the metric NAME in
> diagnostics is `elapsed_observed`); the round-134 #3
> no-degraded-fallback rule is thereby SCOPED to targets claiming
> true completion semantics (Axum, Cloudflare, Spin) — on those,
> G15's real timestamps remain a behavior-complete prerequisite,
> and no target silently reports one semantic as the other. The whole-isolate memory
> certificate is unaffected (it does not depend on deadline
> enforcement); Fastly's BestEffort slot isolation is recorded in
> its certificate as a documented caveat.
>
> **`[capabilities]` manifest — ONE concrete model (round-143 #4
> replaces the round-142 per-target required/optional table, which
> EdgeZero's manifest cannot express: there is ONE global
> `[capabilities]` declaration, `required` rejects BestEffort, and
> the enum has no "config store" entry — the config entries are
> `config-read-deadlines` and `config-read-allocation-bounds`).**
> Chosen model: a COMMON manifest declaring as `required` only the
> guarantees that are Native on all four adapters, every degraded
> capability as `optional` (logged, never blocking), and nothing
> for capabilities that are Unsupported everywhere (auctioneer tracks
> those as production blockers, not manifest entries); target
> deviations are pinned by TARGET-SPECIFIC CI ASSERTIONS, not by
> the manifest:
>
> **ROUND-190 #5 — EdgeZero has TWENTY-THREE capabilities at
> `6904869b`, not 22** (counted from upstream's
> `docs/guide/capabilities.md`); `ingress-admission-abort` is the
> one this plan had never named, and it is now in both the drift
> table and the manifest below. M3b-11's exact-regeneration check
> compares EVERY row, so an omitted capability fails it.
> ROUND-160 #3: the block below is VALID TOML — the round-159 edit
> left two `required` keys and prose inside a value, which no parser
> accepts. `outbound-cache-bypass` is REQUIRED (Native on all four
> targets; a cacheable bidder call is a correctness AND privacy
> defect). `send-all-slot-isolation` is RETIRED in favour of the
> three `outbound-batch-*` rows; M3b-11 regenerates every cell from
> an adapter dump (round-156 #5), so these names are what the
> regenerated table must use.
> ```toml
> [capabilities]
> required = ["ingress-admission", "outbound-cache-bypass"]
> optional = [
>   # ROUND-193 #4 — THE FOUR RESPONSE-EGRESS ROWS. M3b CONSUMES
>   # these (permit release rides the completion; the static-class
>   # budget rides the write deadline), and M3b-11's instruction is
>   # that every CONSUMED degraded capability appears as optional.
>   # All four are BestEffort on all four targets, so none may be
>   # `required`; omitting them also defeats M3b-11's
>   # exact-regeneration check, which compares every row.
>   "response-egress-completion",  # permit release on terminal egress
>   "response-write-deadlines",    # the static-class refusal budget
>   "response-egress-abort",       # exhausted-pool reset
>   "response-egress-backpressure",
>   "ingress-admission-abort",    # ROUND-190 #5: Axum Native, CF /
>                                 #   Fastly / Spin BestEffort. M3b USES
>                                 #   it (an exhausted
>                                 #   terminal_response_permits pool
>                                 #   aborts at admission), so it must
>                                 #   appear; OPTIONAL because only Axum
>                                 #   is Native and a BestEffort target
>                                 #   may close rather than reset.
>   "outbound-http",              # Fastly BestEffort (dynamic backends)
>   "outbound-deadlines",         # Cloudflare + Fastly + Spin BestEffort
>                                 #   (round-177 #2: Cloudflare was
>                                 #   missing from this comment)
>   "outbound-batch-cancellation",      # the `cancel()` half of the handle
>   "outbound-batch-completion-order",  # `next()` yields as slots complete
>   "outbound-batch-slot-isolation",    # replaces send-all-slot-isolation
>   "outbound-flexible-phase-budget",
>   "outbound-header-fidelity",   # Cloudflare BestEffort
>   "streamed-upload-deadlines",
>   "inbound-read-deadlines",     # only Axum Native
>   "config-read-deadlines",      # BestEffort on all four
> ]
> ```
> **NOT DECLARED — every row named, with pinned and head status
> separately (rewritten 2026-09-18; the previous wording used a
> `response-egress-*` wildcard that named nothing a regeneration
> could compare, and asserted "Unsupported on every adapter" as a
> single status for rows that no longer share one):**
>
> **ROUND-194 #6 — THIS TABLE IS "DEGRADED OR UNDECLARED", NOT
> "NOT DECLARED".** Four response-egress rows are DECLARED OPTIONAL
> in the manifest above AND listed here; the `Why` column now says
> which is which, so the two cannot be read as contradicting.
>
> | Capability | @`120dd2aa` (pinned) | @head `46f85958` (round-194 #3; round-191 #6 — this header once said `03cdba98`; rows verified at `4790a140`, and no later delta changes them — M3b-11 regenerates every cell regardless. NOTE: `config-read-deadlines` is **BestEffort on all four** at head, round-186 #7, and there are **23 capabilities**, round-190 #5) | Why not declared |
> |---|---|---|---|
> | `config-read-allocation-bounds` | Unsupported | Unsupported | no surface to consume |
> | `outbound-complete-resource-accounting` | Unsupported | Unsupported | G6 residue; bounds certificate SCOPE, not a task |
> | `raw-ingress-head-limits` | Unsupported | Unsupported | Prereq 5/10 — M3b-10 |
> | `raw-ingress-framing-validation` | Unsupported | Unsupported | Prereq 5/10 — M3b-10 |
> | `response-egress-abort` | Unsupported | **BestEffort** | **DECLARED OPTIONAL (round-194 #6)** — M3b CONSUMES it (exhausted-pool reset); only its NATIVE certification is deferred to M3b-10 |
> | `response-egress-backpressure` | Unsupported | **BestEffort** | **DECLARED OPTIONAL (round-194 #6)** |
> | `response-egress-completion` | Unsupported | **BestEffort** | **DECLARED OPTIONAL (round-194 #6)** — permit release rides it |
> | `response-write-deadlines` | Unsupported | **BestEffort** | **DECLARED OPTIONAL (round-194 #6)** — the static-class budget rides it |
> | `outbound-authority-override` | n/a (new) | Axum **Native**, Cloudflare **BestEffort**, Fastly **Native**, Spin **Unsupported** (round-175 #2 — verified per adapter; "Native (all four)" here was wrong) | **KNOWINGLY absent:** auctioneer REJECTS caller-supplied authority (round-159 #3), so it has nothing to override |
> | `lazy-streamed-response-passthrough` | varies | Axum **Native**, Cloudflare **Native**, Fastly **BestEffort**, Spin **BestEffort** (round-175 #2 — verified; "varies" named no values a regeneration could compare) | **KNOWINGLY absent:** auctioneer BUFFERS every bidder response (round-136 #1) |
>
> The four egress rows plus the two ingress rows are the M3b-10 /
> Prereq-9 production blockers. **BestEffort at head does NOT unblock
> them** — it is not a certifiable guarantee — but it DOES mean
> Prereq 9 is IMPLEMENTED-BUT-NOT-CERTIFIABLE rather than absent,
> which is what the prerequisite ledger's Prereq-9 row says and what
> the old "Unsupported on every adapter" sentence contradicted. The
> last two rows are deliberate auctioneer choices, listed so a
> regeneration reads them as decided rather than forgotten.
>
> EXPECTED SUPPORT TABLE — REGENERATED AT `6904869b`, NOT HAND-WRITTEN.
> Produced by `scripts/dump-capabilities.py`, which reads the checkout
> `Cargo.lock` resolved, so it cannot describe a revision the build does
> not carry. It lists EVERY variant of upstream's `Capability` enum (23
> at this revision), including the ones auctioneer does not consume: a row
> present upstream and absent here is a regeneration failure, not an
> omission to tolerate. Regenerate with
> `./scripts/dump-capabilities.py` and paste the result; do not edit
> cells by hand.
>
> Against the previous `120dd2aa` table, the consumer-visible changes
> are: Cloudflare's `outbound-deadlines` and `streamed-upload-deadlines`
> demote to BestEffort; `outbound-batch-cancellation` is Native on Axum
> only; the four `response-egress-*` rows and `ingress-admission-abort`
> now exist at BestEffort (Axum Native for the abort); and
> `outbound-cache-bypass` is Native on all four, which is what lets the
> manifest `require` it.
>
> <!-- generated by scripts/dump-capabilities.py from 6904869bd0787ba8e6c36e732e3a447c45e99201 -->
> | Capability | Axum | Cloudflare | Fastly | Spin |
> |---|---|---|---|---|
> | config-read-allocation-bounds | Unsupported | Unsupported | Unsupported | Unsupported |
> | config-read-deadlines | BestEffort | BestEffort | BestEffort | BestEffort |
> | inbound-read-deadlines | Native | BestEffort | BestEffort | BestEffort |
> | ingress-admission | Native | Native | Native | Native |
> | ingress-admission-abort | Native | BestEffort | BestEffort | BestEffort |
> | lazy-streamed-response-passthrough | Native | Native | BestEffort | BestEffort |
> | outbound-authority-override | Native | BestEffort | Native | Unsupported |
> | outbound-batch-cancellation | Native | BestEffort | BestEffort | BestEffort |
> | outbound-batch-completion-order | Native | Native | BestEffort | Native |
> | outbound-batch-slot-isolation | Native | Native | BestEffort | Native |
> | outbound-cache-bypass | Native | Native | Native | Native |
> | outbound-complete-resource-accounting | Unsupported | Unsupported | Unsupported | Unsupported |
> | outbound-deadlines | Native | BestEffort | BestEffort | BestEffort |
> | outbound-flexible-phase-budget | Native | Native | BestEffort | BestEffort |
> | outbound-header-fidelity | Native | BestEffort | Native | Native |
> | outbound-http | Native | Native | BestEffort | Native |
> | raw-ingress-framing-validation | Unsupported | Unsupported | Unsupported | Unsupported |
> | raw-ingress-head-limits | Unsupported | Unsupported | Unsupported | Unsupported |
> | response-egress-abort | BestEffort | BestEffort | BestEffort | BestEffort |
> | response-egress-backpressure | BestEffort | BestEffort | BestEffort | BestEffort |
> | response-egress-completion | BestEffort | BestEffort | BestEffort | BestEffort |
> | response-write-deadlines | BestEffort | BestEffort | BestEffort | BestEffort |
> | streamed-upload-deadlines | Native | BestEffort | BestEffort | BestEffort |
>
> There is NO app-level boot assertion API (round-143 #4 retires
> the round-142 "boot-time assertions"): the EdgeZero CLI compares
> the manifest with the selected adapter at build/serve/deploy, and
> auctioneer's per-target CI row
> (`capability_table_matches_<target>()`, one per adapter crate)
> asserts the adapter's reported support against the table above —
> a BestEffort cell silently becoming Native, or vice versa, fails
> CI, never boot.

> **Round-137 #8 (accepted; mechanical follow-up):** the struck /
> historical journal blocks in this plan migrate to an appendix
> file in a dedicated commit; the SHA-pinned matrix above is the
> maintained summary until then.
> **Start rule (round-113 M5 — the per-task rule is canonical;
> the old all-or-nothing wording is retired): an M3b task may start
> when ITS OWN prerequisites have landed upstream AND its body has
> been expanded + re-reviewed per the expansion protocol at the top
> of the Tasks section — no task waits on another task's
> prerequisites.**
>
> **DISPATCH ORDER — THE ONE LIST (self-review finding 4; the
> constraints were correct but spread across six places, and the
> DOCUMENT order misleads: 2b.0/2b.1 are printed before the 2a units,
> and M3b-0 sits between M3b-5 and M3b-11. Task NUMBERS have never
> been execution order. Read this table, not the page order):**
>
> **⚠ ROUND-167 #1, SCOPED CORRECTLY IN ROUND-168 #1 — "GATED BY:
> nothing" MEANS NO *TASK* BLOCKS IT, NOT THAT IT IS DISPATCHABLE
> TODAY.** A unit that OWNS MEMBERS OF THE AUTHORITATIVE ROSTER
> carries one further precondition: its roster subset must be
> enumerated — every test named, with its exact assertion — before it
> is handed to an implementer. **THE CANONICAL COUNTS (round-172 #1,
> updated round-181 #4 — identical wording at every site that states
> them): 175 total = 168 owned by M3b-2 + 7 owned by M3b-10; 80
> currently NAMED with an assertion; ~95 still requiring per-unit
> recovery or removal.**
> **WHY THE TOTAL MOVED (round-181 #4 — the question is ADDITIVE vs
> RECOVERED, and the answer here is ADDITIVE):** 2a.7 and its
> response-write checkpoints introduce NINE tests that did not exist
> in any earlier count — five gate/boot rows, three egress-deadline
> rows, and the extraction-limit ordering row. They are NOT
> recoveries of unnamed members: the ~95 outstanding all trace to
> group labels in the head's summary ("7 weighted-gate", "3
> elapsed-timing", …), and none of those groups covers egress gates,
> boot ordering or extraction limits — surfaces that had no owner at
> all until round 180. So both the total and the named count rise by
> nine, and the outstanding ~95 is unchanged. Any future addition
> must state which of the two it is, for exactly this reason.**
> **NAMED COUNT = 80 (round-183 #8 states it ONCE, here, and every
> other site defers to this line).** Derivation: 62 literal bullets,
> 3 of them `<target>` templates expanded four ways = 71, plus the
> nine round-181/182 members named with assertions = 80. The earlier
> text asserted 71 in one sentence and 80 in the next without
> retiring the 71; 71 is the PRE-181 subtotal and is not a current
> count. Round-183 #3 substitutes one 2a.7a member for another
> (`egress_gate_full_refuses_without_a_permit()` in, the static-class
> test moved to 2a.7b), which changes no total. So the ~95 members
> that exist only as category counts ("7 weighted-gate", "3
> elapsed-timing", …) block the units that own them.
>
> **⚠ THE RECONCILIATION IS NOT DONE, AND NO ROSTER-OWNING UNIT IS
> DISPATCHABLE UNTIL IT IS — BUT IT NOW HAS AN OWNER: TASK
> M3b-2a.0 (round-200 #1).** Thirty rounds of restating this gap
> without assigning it is what kept it open; the task below carries
> the method, the steps and a mechanical acceptance.
> **ROUND-209 (M3b-2a.0) — RESOLVED. §9's blocks sum to 175, as
> declared.** The apparent two-member gap was a stale spelled-out
> word, not an arithmetic defect: the inline block reads **97**
> (round-181 #4 added the nine 2a.7/egress members, taking it from
> 88 to 97), while one prose mention still said "eighty-eight".
> With 97 the sum is **65 base + 12 (rounds-122..125, under M3b-8)
> + 1 (`config_parse_factor_guard`) + 97 (rounds 127..169, the
> parenthesised group list) + 7 (M3b-10) = 175**, which is what
> `scripts/check-roster.sh` reads and what §9 declares. No member
> was invented and none was struck; see the inline block's own
> round-209 note for the full derivation.
> `scripts/check-roster.sh` checks that arithmetic — the property
> twenty rounds of hand-counting kept getting wrong — rather than
> counting backticked names, because a name count answers a
> different question: a `<target>` template is ONE name and FOUR
> members, and "7 weighted-gate" is seven members discharged by
> fewer named tests.
> **AND THE GAP IS NOT MISSING TESTS. IT IS MISSING CITATIONS.**
> Walking every group label to the round it cites found **no
> unrecoverable member**: each label's tests are already NAMED with
> assertions, in the TASK BODIES, and §9's enumeration simply does
> not cite them. Four examples that looked missing on a §9-scoped
> scan: `wave-expiry` is
> `wave_wait_does_not_expire_before_deadline_<target>()`;
> `3 time-domain` is `auction_deadline_composes_on_instant_without_
> projection()` + `fake_clock_drain_wakes_every_sleeper()` + the
> instant-identity case; `validator-regeneration` is
> `validator_regenerates_the_synthetic_worked_row()`;
> `layout-constants-parity` is in 2a.5's body. The plan names **347
> unique tests** overall — far more than the roster's 175 — so the
> roster was never short of TESTS.
> **CONSEQUENCE: the reconciliation is a CITATION pass, not a
> recovery-or-strike pass.** §9 gains an explicit cross-reference
> from each group label to the owning unit and the test names that
> discharge it. Nothing is invented and nothing is struck; the
> arithmetic closes because the names were always there.
> **`scripts/check-roster.sh` must therefore count what §9 CITES**,
> which is the number an implementer can act on — a name that exists
> only in a task body two thousand lines away does not tell the 2a.4
> implementer which tests are theirs.
> **THE STALE FIGURES, for the record:** `scripts/check-roster.sh`
> counts §9's enumerated members mechanically: **138** — 126 literal
> `name()` bullets plus 3 `<target>` templates expanding four ways —
> against the declared **175**. The enumeration GREW across rounds
> (each round that added a test added a bullet) while the "80 named"
> figure stayed frozen from round 186, and every round since repeated
> it. This is precisely the hand-counting failure the script exists to
> stop, and it is why 2a.0's acceptance is a script rather than a
> recount. **37 members remain to recover or strike.** Superseded
> arithmetic, retained so the drift is visible:
> **ROUND-186 #8:** 175 total = 80 named + ~95 recoverable
> only from group labels. Those two facts have coexisted for eighteen
> rounds, and restating them is not progress. Stated once, plainly:
> **every unit bound by the roster rule — 2a.1–2a.6, 2a.7a, 2a.7b,
> 2a.4a, 2b.0–2b.2, M3b-10 — is BLOCKED on recovering its own
> subset**, and that recovery is the first action of its dispatch
> brief. The unblocked work is exactly the units that own no roster
> member: **M3b-0, M3b-1c, M3b-1d, M3b-11** (and M3b-1a/M3b-9,
> done). A
> milestone plan that lists 175 tests and can name 80 is not ready
> for its implementation phase, whatever the DAG rows say.
> **WHICH UNITS THAT IS — AND WHICH IT IS NOT (round-168 #1; the
> round-167 wording said "EVERY UNIT" and "NO UNIT meets it", which
> was wrong and blocked the declared unblocker):** the authoritative
> roster is explicitly the list of tests that LAND WITH M3b-2 and
> M3b-10 (see its head: "+175 total — 168 land with M3b-2, and seven
> land with M3b-10"). So the rule binds M3b-2's units — 2a.1–2a.6,
> **2a.7a and 2a.7b** (round-183 #8: the round-182 note claimed the
> list included them while the list itself did not; they own the nine
> round-181/182 members and are bound like every other 2a unit),
> 2a.4a, 2b.0–2b.2 — and M3b-10. It does NOT bind **M3b-0**, which
> owns no roster member and carries its own fully enumerated tests
> (the gate script's negative, positive, `send_all_until`-positive
> and frozen-section fixtures — round-199 #4: this named the
> "appendix-exclusion" fixture, which round-191 #1 REPLACED because
> it pointed at a file the gate no longer scans; the inventory is
> now the ten ordered cases (a)-(j) in the task body) plus explicit
> acceptance criteria;
> nor **M3b-11**, whose acceptance is the capability diff, the
> lockfile assertion and the codec pin test, all named in its body.
> **Round-197 #7: "dispatchable on their own terms" means NEITHER
> owns a roster member — it does NOT mean both are startable now.
> M3b-0 is; M3b-11 is BLOCKED ON M3b-0** (DAG row 7, and its own
> body: the head hard-removes `send_all`, so the pin breaks the
> build until the documentation rebase lands). The roster rule and
> the DAG are separate gates, and a unit must clear both.
> THE GATING QUESTION for a roster-owning unit: *can I list this
> unit's tests by name and assertion?* If not, it is NOT startable,
> whatever its row says. Reconciling a subset is the first action of
> that unit's dispatch brief, and it is cheap: the group labels cite
> the rounds that introduced each member, so recovery is lookup, not
> invention. A member that cannot be recovered is STRUCK and the
> total corrected downward — the count is not sacred; the
> correspondence between count and enumeration is.
>
> | # | Unit | Gated by |
> |---|---|---|
> | 1 | M3b-9, M3b-1a | DONE |
> | 2 | 2a.1, 2a.3, 2a.4a, 2a.6 | nothing — API-independent; **task-unblocked, roster reconciliation pending** (round-168 #1) |
> | 2b | 2a.2 | **2a.1** (round-166 #5 — 2a.2 MODIFIES `admission.rs`, which 2a.1 CREATES; grouping them as co-startable invited two implementers into one file); roster reconciliation pending |
> | 3 | 2a.4 | 2a.4a (builder primitives) |
> | 4 | 2a.5 | 2a.3 + 2a.4a (consumes their constants) |
> | 4b | **2a.7a** — API-INDEPENDENT half: aux + egress GATE construction, bounded serialization, boot lifecycle | **2a.1 + 2a.5** |
> | 8b | **2a.7b** — INTEGRATION half: the egress DEADLINE checkpoints, `ResponseEgressDeadline` attachment, AND the response-terminal egress-permit binding | **2b.0** (the shared monotonic clock they measure on) **+ M3b-11** (the ONE pin that makes both `ResponseEgressDeadline` and `ResponseEgressCompletion` exist) **+ 2a.7a** (round-201 #3 — it CREATES the gates, the permit types and `AuctioneerIngressLease`; this row omitted its own producer) — round-182 #3: one undivided 2a.7 sat at row 4b while consuming both, which is backwards. **Round-184 #2: with a single pin this row is now satisfiable — the two-pin split had 2a.7b consuming lifecycle APIs that the pin below it did not yet carry** |
> | 5 | 2b.0 | nothing — the FIRST clock-domain unit, no batch API; roster reconciliation pending |
> | 5b | 2b.1 | **2b.0** (round-166 #5 — code-12 attribution is written against the MIGRATED instant representation; before 2b.0 it would target the retired Duration shape) |
> | 6 | **M3b-0** | nothing — THE UNBLOCKER for the outbound track, and **genuinely startable now**: it owns no roster member and enumerates its own tests (round-168 #1) |
> | 6b | **M3b-1c** — `/status` reports what is misconfigured (D-1) | nothing — **startable today, in parallel with M3b-0** (round-186 #8): one handler + its tests, no roster member, no clock, no batch API, no pin. It is what unblocks M3b-8 Step 0 |
> | 6d | **M3b-2a.0** — ROSTER RECONCILIATION | nothing — **startable today, in parallel with M3b-0, M3b-1c and M3b-1d** (round-200 #1). It reads and edits §9 only; no source changes. **It is the sole precondition for every roster-owning unit**, so it is on the critical path to the central fanout wave |
> | 6c | **M3b-1d** — fold `host-defaults.toml` + `account-defaults.toml` into `auctioneer.toml` (round-192; redesigned rounds 193–197) | nothing in the DAG, but **NOT the small task the round-192 row described** (round-198 #3): it now creates `auctioneer-config-schema`, adds a CLI wrapper type, migrates `config validate` to the typed form, and touches the root manifest, two crate manifests and `Cargo.lock`. Startable in parallel with M3b-0 and M3b-1c, and it owns no roster member |
> | 7 | **M3b-11** — ONE atomic pin move to `6904869b`: batch API + the full G18–G25 lifecycle adoption (round-188 #1 retargets from `03cdba98`) (round-184 #1 merges the former M3b-12 here; there is no row 11b) | M3b-0; also re-asserts the head SHA first |
> | 8 | **2b.2** (the "M3b-2b core" unit) | M3b-0 + M3b-11 **+ 2b.0 (the `RequestClock`/`FakeClock` its own steps use and the composed `deadline_at` it asserts) + 2a.2 (the weighted gate the wave scheduler waits on) + 2a.4/2a.4a (bounded request construction) + 2a.5 (the resolved limits it charges against)** — round-201 #3: this row listed only the two API-shape blockers, so following the start rule could dispatch the scheduler before the clock, gate and builders it calls exist |
> | 9 | M3b-3, M3b-4, M3b-5 | 2b.2 (M3b-4 carries the Spin wasip2 move first) |
> | 10 | M3b-6, M3b-7 | 2b.2 |
> | 11 | M3b-1b | **2a.7a + 2a.7b + 2a.1 + 2a.5 + 2b.0 + 2b.2 + M3b-11** (round-201 #3 adds 2a.7b: M3b-1b's capacity-recovery case tests the RESPONSE-TERMINAL binding that 2a.7b creates) (round-184 #2: "G21" named a landing, not a task; it arrives with the single pin, so the dependency is M3b-11) (round-182 #6 — this row said "2b.2" alone while the task body lists five; the body is right. Round-183 #4 adds G21: the capacity-recovery acceptance is not demonstrable for refusal/static responses until upstream's completion path covers detached egress) |
> | 12 | 2a.FINAL | GATE REWRITTEN (baseline measured; the two host-state constants are unmeasurable on wasm by construction) |
> | 13 | **M3b-8** | everything above — runs LAST |
> | — | M3b-10 | Prereqs 5 + 9 + 10; NOT part of behavior-complete |
>
> **EXPANSION STATUS (round-153 #8, refreshed round-156 #6 — what is
> executable TODAY vs what is still a requirements list; upstream
> gates nothing here, this is auctioneer-side expansion work. This table
> is re-checked every review round; the "awaiting re-review" note on
> the 2a units reads **round-164**, the latest round whose findings
> are incorporated):**
>
> | Unit | State |
> |---|---|
> | M3b-9 | DONE (e414a1e) |
> | M3b-1a | DONE (f461f8c) |
> | M3b-1b (config-fetch deadline + admission lease lifecycle — round-178 #5 relabels it; "streaming body cap track" described different work) | **EXPANDED** (round-177 #1) — files, interfaces, four tests, steps; blocked by **2a.7a + 2a.1 + 2a.5 + 2b.0 + 2b.2 + G21** (round-183 #8: this cell listed two of the six the task body names; the body and the DAG row are authoritative) |
> | M3b-2a.1 – 2a.4a, 2a.4, 2a.6 | EXPANDED — implementation-grade steps, carrying the round-164 corrections, awaiting re-review |
> | **M3b-2a.7a** (gate construction, bounded serialization, boot lifecycle) | **EXPANDED** — files, interfaces, five tests, steps; blocked by 2a.1 + 2a.5. Round-183 #3 scopes it to what needs no clock and no response-completion API |
> | **M3b-2a.7b** (egress deadline integration + response-terminal permit binding) | **EXPANDED** (round-184 #6 gives it files, interfaces, the permit-ownership flow, failure cases and a commit boundary) — blocked by **2a.7a** (its gates and permit types) **+ 2b.0 + M3b-11**, the single pin carrying both `ResponseEgressDeadline` and `ResponseEgressCompletion` (round-201 #3) |
> | **M3b-2a.0** (roster reconciliation) | **EXPANDED** (round-200 #1) — files, method, interfaces, steps and MECHANICAL acceptance (`named == total`, no label standing in for members, one owning unit each). Blocked by NOTHING and dispatchable today |
> | **M3b-1d** (fold the baked defaults into `auctioneer.toml`) | **EXPANDED** (round-192; REDESIGNED rounds 193–197) — files, interfaces, steps, and the test list in its body is authoritative (round-198 #3/#4: the earlier "four tests" count predates the schema crate, the CLI wrapper and the typed-validate migration). Blocked by NOTHING and dispatchable once its documentation corrections land. `runtime-bootstrap.toml` is explicitly out of scope (self-referential key) |
> | **M3b-1c** (`/status` degraded reporting — D-1) | **EXPANDED** (round-185; round-186 #5 replaces the public `detail` with a closed `ConfigStatusCode`) — files, interfaces, five tests, steps; blocked by NOTHING and dispatchable today |
> | M3b-2a.5 | EXPANDED but **ORDERED AFTER 2a.3 and 2a.4a** (round-161 #6 / round-162 #7 — it consumes their constants; it is not independently startable) |
> | M3b-2b.0, 2b.1 | EXPANDED — implementation-grade steps |
> | **M3b-0 (batch-API documentation rebase)** | EXPANDED — see its task body below (round-161 #3); REQUIRED FIRST, blocks M3b-2b core and M3b-11 |
> <!-- blocked-reason cell: why 2b.2 waits (batch-rebase-ok) -->
> | M3b-2b core = **unit 2b.2** (`BoundedFetch` + the wave scheduler) | **EXPANDED** (self-review finding 1 — it was blocked-but-undefined; it now has files, interfaces, steps and tests) but **BLOCKED on M3b-0 + M3b-11** (its whole contract is the removed `send_all`) **+ 2b.0 + 2a.2 + 2a.4/2a.4a + 2a.5** — the producers its scheduler calls (round-201 #3) |
> | M3b-3, M3b-4, M3b-5 | **EXPANDED** (round-177 #1) — each has files (timer, cooperator, profile, tests), interfaces, per-target build commands and steps; all blocked by 2b.2 |
> | M3b-6, M3b-7 | **EXPANDED** (round-177 #1) — fixture files and assertions; fixtures only, so a failure is a defect in its owning task |
> | M3b-8 | **EXPANDED** (round-177 #1) — its own Step 0 preconditions, matrix run, milestone flip |
> | M3b-2a.FINAL | GATE REWRITTEN — `BASELINE_BYTES` measured at 27,328,512; `TRANSMIT_STATE_BYTES` / `PARSER_TRANSIENT_BYTES` are host h2/hpack state that no wasm guest allocator can observe, so the gate now requires their provenance be visible in code rather than re-demanding an impossible measurement |
> | Decoder measurement (`brotli_state_measured`) | BLOCKED on M3b-11 (round-158 #1: the head codec tuple must be in the lockfile before the constant is measured); the harness itself is an M3b-2b deliverable, not a 2a one |
> | M3b-10 | BLOCKED on Prereqs **5 + 9 + 10** (round-164 #6 — the production gate includes the response-egress prerequisite; earlier rows listed only 5 and 10 while the task body already named 9) — **round-162 #7: at PR HEAD those surfaces EXIST but are not certifiable** (raw ingress framing/head guarantees Unsupported; egress BestEffort under Prereq 9). At the PINNED revision they are absent. M3b-10's task body still describes the pinned state; M3b-11 refreshes it along with the ledger |
> | M3b-11 (EdgeZero PR-head pin move) | **BLOCKED on M3b-0** — head hard-removes `send_all`, which auctioneer implements in exactly THREE places (round-161 #2), so the move breaks the build on its first compile. Scope: pin + SDKs + Fastly entrypoint + those three client migrations + capabilities/profiles + codec tuple + cache bypass + the capability/`BudgetSource` renames. NOT `BoundedFetch`/`run_wave`, which are M3b-2b's. Round-158 #1 ordering stands: it RUNS BEFORE the decoder measurement |
>
> **ROUND-179 #1 — THE ROUND-177 CLAIM THAT "NO UNIT REMAINS IN THE
> REQUIREMENTS-LIST STATE" WAS TOO BROAD, AND IS WITHDRAWN.** Every
> named TASK now carries files, interfaces, tests and steps — that
> part holds. But M3b-2's normative deliverable list names surfaces
> that NO expanded unit owns, verified by searching every 2a/2b file
> list:
>
> | Deliverable | Owner | Status |
> |---|---|---|
> | `aux_gate` (aux-route admission) | **2a.7a** | OWNED (round-180 #2; round-183 #8 — the split makes 2a.7a the owner) |
> | `egress_gate` + `PermitBody` | **2a.7a** (gate + admission-body permit) / **2a.7b** (response-terminal egress binding) | OWNED — round-183 #2: these are two permits with two release conditions, so the row names both halves |
> | bounded final-response serialization | **2a.7a** | OWNED |
> | boot-envelope installation | **2a.7a**, in `bootstrap_adapter` (2a.5 computes it; round-183 #7 — NOT in `configure`) | **OWNED, BUT NOT WIRED — see below** |
>
> **✅ RESOLVED — THIS WARNING IS STALE AND ITS PREMISE NO LONGER
> HOLDS. All four adapters now call
> `bootstrap_adapter_with_envelope`; grep for `bootstrap_adapter(`
> outside the envelope variant returns nothing. Axum validates at
> 437_836_672 of 512_000_000. The three wasm adapters install an
> envelope and REFUSE, with the overshoot pinned in their own tests:
> Cloudflare 258_725_760, Fastly 219_310_464, Spin 213_092_736 over
> their ceilings. That is a different state from the one this warning
> describes — not "no envelope", but "an envelope that correctly
> declines to boot". The reasoning below about `2a.FINAL` rests on the
> old premise and should be re-read with that in mind.**
>
> **The superseded warning, kept so the correction is legible:**
> **⚠ THE BOOT ENVELOPE IS NOT INSTALLED IN ANY SHIPPED BINARY
> (M3b-1b, grep-verified).** 2a.7a built
> `bootstrap_adapter_with_envelope`, but ALL FOUR adapters still call
> the envelope-less `bootstrap_adapter`:
> `auctioneer-adapter-axum/src/main.rs:23`,
> `auctioneer-adapter-cloudflare/src/lib.rs:36`,
> `auctioneer-adapter-spin/src/lib.rs:41`,
> `auctioneer-adapter-fastly/src/lib.rs:51`. So `boot_limits()` answers
> `None` everywhere and every envelope-derived BYTE CAP resolves to a
> host default instead of the value 2a.5 computed. The config-fetch
> TIMEOUT is unaffected — it is baked, not envelope-derived, and is
> live on all four.
> **This matters for 2a.FINAL and M3b-8: the §8 measurements would
> certify an envelope that no shipped binary installs.** No task in
> this plan owns the adapter-side wiring — the row above assigns
> installation to 2a.7a, which built the entry point but did not
> switch the call sites. NEEDS AN OWNER before behaviour-complete.
> | adapter boot ordering | **2a.7a** | OWNED |
> | `response_write_deadline_ms` enforcement — checkpoints (1)-(3) | **2a.7b** | OWNED (round-182 #6 — this row said all four were deferred, contradicting 2a.7's scope since round-181 #2) |
> | `response_write_deadline_ms` checkpoint (4), write-complete | M3b-10 / production | **DEFERRED** — the BestEffort gap, plus feedback G18's missing per-response completion identity |
>
> **ROUND-180 #2 — 2a.7 IS NOW A REAL TASK** (files, interfaces,
> five named tests with assertions, DAG row 4b), not the provisional
> label round-179 left. Five of the six surfaces are owned; the
> sixth is deferred with a stated reason rather than silently
> dropped. **M3b-2 is planned to the extent its surfaces exist** —
> what remains is the roster gap below. What gates each unit
> today: a roster-owning unit needs its subset enumerated (~95
> outstanding); the batch-API units need M3b-0 then M3b-11; and the
> six surfaces above need an owner. M3b-0 is unblocked on all counts
> and remains the one genuinely dispatchable task. M3a's plan
> (`2026-07-10-m3a-privacy-and-axum-fanout.md`) is the executable
> slice for the M3 milestone and is not blocked by anything here.
>
> **⚠ Round-46, banner corrected round-128 #8 — the round-38→45
> admission drafts below were found NOT design-ready by round-46 and
> the redesign has since LANDED: the buildable design is the
> "Admission + whole-isolate budget — the round-98 design" section
> (as consolidated through round-128), which supersedes every
> `AdmissionSemaphore` / `install_admission` /
> per-stream-memory-proof block below.** Treat those blocks as
> historical record carrying the four round-46 defects, never as
> buildable pseudocode:
> 1. **Not WASM-portable (B6).** The `tokio::select!` +
>    `tokio::sync::Semaphore` gate cannot compile for
>    `wasm32-unknown-unknown`: the workspace `tokio` at
>    [Cargo.toml:74](../../../Cargo.toml#L74) pulls
>    `rt-multi-thread`, which Tokio rejects on wasm, and
>    `auctioneer-core` is compiled by all three wasm adapters. The gate
>    must be **runtime-neutral** — a portable counting primitive
>    plus the existing `Timer` abstraction, not Tokio. The drafted
>    snippets also have concrete compile errors (`AdmissionSemaphore::
>    new(u64,u32,u64)` called with `usize`; `AuctionDeadline` has no
>    `sleep_until`; `drive_one` returns `BidderCallResult` but a
>    snippet returns bare `Err(..)`; production `.expect(..)` under
>    the denied restriction lints).
> 2. **The retained-GVL bound is unsound (B4).** Privacy resolution
>    runs BEFORE stream admission, so `max_active_body_streams ×
>    max_gvl_heap` does not bound the `Arc<GvlInner>`s held by
>    queued auctions. Fix requires auction-level admission BEFORE
>    privacy resolution, or materializing compact permissions and
>    dropping the GVL Arc before fanout.
> 3. **Budgets are not shared "by construction" (B5).** The GVL
>    parse-cache ceiling and `AdmissionBootConfig` are generated
>    independently; `AdmissionBootConfig` carries no raw-GVL,
>    parsed-cache, or retained-GVL fields, so raising the cache
>    override leaves fanout's allowance unchanged. One validated
>    boot budget must own every reservation and reject inconsistent
>    overrides.
> 4. **Admission queueing escapes the bidder timeout (H1).** The
>    drafted `acquire` races only the auction deadline; a call can
>    wait past its per-bidder `call.timeout` and still get a fresh
>    send budget. The gate must race BOTH deadlines and subtract
>    queue-elapsed from the remaining budget.
>
> **Round-95 B1 / round-96 / round-97 revisions — REDESIGNED. The
> section "Admission + whole-isolate budget — the round-98 design"
> below is the AUTHORITATIVE design; it closes the four round-46
> defects, the round-96 blockers (false wasm single-request premise,
> admission-after-body ordering, missing active-stream bound,
> raw-JSON-as-parsed-bound fallacy), AND the round-97 blockers
> (StreamGate cross-auction coordination via a waker queue, retained
> request/response bodies + retained-GVL terms in the budget,
> submit-time absolute bidder deadlines, reproducible expansion/
> overhead bounds, and the canonical boot lifecycle). Every earlier
> `AdmissionSemaphore` / `AdmissionBootConfig` / `install_admission` /
> per-stream-memory-proof block in this document is STRUCK (and,
> round-150 #8, MOVED to the historical appendix §7) — Task
> M3b-2's normative deliverables are the round-98 design's types and
> tests, and the struck blocks MUST NOT be implemented. The track is
> design-ready; implementation remains upstream-gated like the rest
> of M3b.** M3a still ships WITHOUT a hard whole-isolate memory proof (its
> GVL cache is count-bounded, its Axum fanout buffered — see M3a's
> dev-complete banner); M3b's memory numbers below become binding when
> M3b-2 lands against this design.

## Admission + whole-isolate budget — the round-98 design

> **CERTIFICATE SCOPE (round-151 #7 — authoritative everywhere):**
> the machine-readable certificate carries `scope: managed |
> whole_process`. On EVERY current target the issued scope is
> `managed` — auctioneer-managed memory only; the provider's source-read
> allocation is excluded while G6 is open — and `whole_process` is
> issuable only with independently verified receive-buffer bounds
> (round-118 F13). "Whole-isolate" in this section's title and in
> older sentences is a SECTION NAME, never a scope claim.

**Round-98 revision (fourth iteration; supersedes round-95/96/97).
Closes the round-98 blockers: StreamGate cancellation-safety, the
source-of-truth budget shape, call cardinality + defined inputs,
pipeline allocations in the per-auction proof, permit-vs-response
lifetime, the boot circularity, and the whole-isolate measurement
contract. Owner: Task M3b-2; pure types testable immediately.**

1. **Admission is acquired in EDGEZERO INGRESS, before any bounded
   body read — round-117 P0-1 (retires "first HANDLER action", which
   was too late for the whole-isolate claim).** The handler-first
   design let EdgeZero's ingress buffer up to the route cap per
   concurrent NOT-YET-ADMITTED request, so the un-admitted population
   escaped the envelope; it also placed the body read before
   `request_start`, excluding slow-upload time from `tmax` contrary to
   spec §5.1 step 1. Corrected contract (Prereq 10): EdgeZero ingress
   (a) captures `request_start` on the EdgeZero APPLICATION clock
   (`ctx.request_start()`, round-143 #3 — auctioneer's Duration clock
   is not the source) and (b)
   try-acquires an ADMISSION LEASE from the `AdmissionGate` BEFORE
   performing any `BoundedRead`, and injects both into
   `RequestContext`; a request refused a lease is answered from the
   static-egress class (round-117 P0-3) having buffered ZERO body
  bytes. **⚠ ROUND-186 #4 — THE ADMISSION PERMIT IS NOT INSERTED
  INTO `IngressGrant` ON ITS OWN.** `IngressGrant` carries ONE
  type-erased value, and this plan also gives the grant a
  response-egress resource (2a.7b). Both travel as fields of the
  single `AuctioneerIngressLease { admission_permit, route_egress_resource }`
  defined in 2a.7b; two independent inserts would not type-check.
  Admission ALSO reserves this request's ONE static-egress permit at
  the same moment (round-186 #2), so no post-admission response can
  find the class exhausted. The lease-crossing protocol is the Prereq-10 consolidated
  contract's step 3 (opaque `IngressGrant` newtype in a dedicated
  `RequestContext` field — NOT `http::Extensions`, whose Clone bound
  a boxed Any cannot meet; one definition, there).
   `AdmissionGate` capacity comes from the compile-time boot
   envelope (§2) — NOT from per-request config and NOT from the config
   store, so the ingress hook needs no store round-trip. The handler's
   former `try_acquire()` becomes a debug assertion that the lease is
   present (portable fallback: an adapter that predates Prereq 10 may
   run handler-first admission in DEV builds only; the whole-isolate
   certificate (M3b-10) is UNOBTAINABLE on such an adapter). Full ⇒
   immediate
   503 with the exact wire shape `503`, `content-type: text/plain;
   charset=utf-8`, body `overloaded\n` (recorded in the spec Appendix F
   wire table). There is NO admission queue.

2. **Immutable boot envelope — request-independent, no circularity
   (round-98 H6).** The round-97 "budget from resolved host config"
   was circular on wasm (host config needs the store; the store needs
   a request context; admission must precede both) and unimplementable
   on Axum process boot (the canonical resolver takes
   `&RequestContext`). The envelope is therefore built from
   REQUEST-INDEPENDENT inputs only:
   - per-target constants (`isolate_total_bytes`, the measured
     `baseline_bytes` table — §8),
   - the BAKED host-defaults (compile-time `auctioneer-config` values:
     caps, `max_outbound_requests`, brotli limits, GVL cache entries),
   - the baked GVL archive metadata (`max_raw_blob_bytes` = the
     LARGEST manifest entry's byte size, a build.rs-emitted constant).
   `ResolvedIsolateLimits` (§7) is constructed + validated ONCE per
   isolate/process from these (wasm: first-request init BEFORE serving,
   no store access; Axum: process start), installed via the
   host-primitives `OnceLock` seam, fail-loud on validation failure.
   **Round-107 B5 — boot ORDER within that init:**
   `install_host_primitives` → `let _ = Gcl::baked();` (explicit, in
   EVERY adapter's bootstrap — the GCL heap must exist before any
   baseline-relevant point) → `validate_isolate_limits` + admission
   install → serve. `scripts/measure-baseline.sh` boots through the
   SAME production bootstrap, so the §8 run-A baseline includes the
   GCL static by construction; a per-adapter bootstrap test asserts
   the factory initializes the GCL static before the budget install.
   **Request-time store/account overlays may only LOWER envelope
   values** — any overlay above an envelope cap is CLAMPED to it; the
   envelope itself never changes after install. Nothing in the struck
   drafts needs re-deriving; this section owns installation.

3. **`StreamGate` v3 — cancellation-safe FIFO with identified waiter
   nodes (round-98 B1); WEIGHTED per round-126 #2.** A waiter node
   carries `requested: u32` (1 for single-permit callers;
   k for `acquire_wave(k)`). Semantics: `k == 0` → `Err(ZeroWave)`;
   `k > capacity` → `Err(WaveExceedsCapacity)` (callers clamp waves to
   capacity BEFORE acquiring, so this is a bug signal, not flow
   control). Grant is ATOMIC: the head waiter is granted only when
   `available >= requested`, taking all k in one state transition —
   partial reservation never exists, so cancellation while WAITING
   releases nothing (unchanged). STRICT FIFO with head-of-line
   blocking is deliberate: a large wave at the head blocks smaller
   later waves until it grants — that is the anti-starvation choice,
   and the alternative (small-wave overtaking) would starve full
   waves forever at saturation. Cancellation AFTER grant releases the
   FULL weight (the wave permit is one RAII object of weight k;
   release is all-k, matching whole-wave batch completion —
   round-125 #9). Tests: zero/over-capacity errors, atomic
   no-partial-hold, mixed-size FIFO order, head-of-line demonstrated,
   cancel-before-grant releases nothing, cancel-after-grant releases
   k, weighted release restores exactly k. The round-97 waker-only queue lost permits
   when a woken future was cancelled before repoll, and let new
   callers steal the freed permit. v3 uses the standard
   identified-node design:
   ```rust
   // auctioneer-runtime/src/admission.rs — std Mutex + Waker; wasm-clean.
   // Round-127 #6 / round-128 #5 — the sketch IS weighted, with
   // SEPARATE waiting-order and waiter-state storage and THREE
   // INDEPENDENT weight counters, so the invariant can detect leaks
   // instead of restating a definition.
   struct Waiter { requested: u32, state: WaiterState }
   //   WaiterState = Waiting(Waker) | Granted
   //   (a Granted node holds exactly `requested` permits; cancelled
   //   nodes are REMOVED from `nodes`, never a state)
   pub struct StreamGate { capacity: u32, inner: Mutex<GateInner> }
   // GateInner {
   //   free: u32,                  // unassigned weight
   //   held: u32,                  // weight inside live RAII guards
   //                               //   (+k when a Granted node
   //                               //    repolls into a guard;
   //                               //    -k on guard drop)
   //   granted: u32,               // weight parked on Granted nodes
   //   order: VecDeque<u64>,       // WAITING order only — ids;
   //                               //   granted ids are popped off
   //   nodes: HashMap<u64, Waiter>,// id → state; Waiting AND
   //                               //   Granted nodes both live
   //                               //   here, so cancellation and
   //                               //   repoll always find their
   //                               //   node by id
   //   next_id: u64 }
   // INVARIANT (debug-asserted on every transition, and each
   // counter is mutated INDEPENDENTLY — none is derived from the
   // others, so a lost release or a double-grant trips the assert):
   //   free + held + granted == capacity
   //   granted == Σ requested over Granted nodes
   ```
   Rules (each is load-bearing; every quantity is a WEIGHT):
   - **Grant scan:** on any weight arriving in `free` (guard drop:
     `held -= w, free += w`; granted-cancel: `granted -= k,
     free += k`): while `order` is non-empty AND the front id's node
     is `Waiting` AND `free >= node.requested`: `free -=
     node.requested; granted += node.requested`, set the node
     `Granted`, POP its id off `order` (the node STAYS in `nodes`
     under its id), collect its waker — the scan's next head is the
     next waiter, so the scan continues until the head is
     unsatisfiable or the queue empties; a front id with no `nodes`
     entry is a cancelled leftover, popped and skipped. Wakers fire
     OUTSIDE the lock. A too-big head stops the scan (head-of-line,
     deliberate).
   - **No stealing:** `acquire(k)` polls: if `order` is empty AND
     `free >= k` → `free -= k; held += k`, return the guard;
     otherwise insert a `Waiting` node into `nodes` and push its id
     onto `order` (or repoll its own node). A new caller behind a
     non-empty queue always enqueues — FIFO by arrival, regardless
     of weight.
   - **Repoll after grant:** a `Granted` node's future, when
     repolled, removes its entry from `nodes`,
     `granted -= requested; held += requested`, and returns the
     weight-k RAII guard.
   - **Cancellation-safe:** `StreamAcquire::drop` looks its id up in
     `nodes`: `Waiting` → remove the entry **AND REMOVE ITS ID FROM
     `order` IN THE SAME OPERATION** (round-201 #1 — see below) AND
     RE-RUN THE GRANT SCAN —
     round-129 #3: a cancelled too-big HEAD may have been the only
     thing blocking satisfiable successors (capacity 8, free 4, head
     wants 8, next wants 1: dropping the head must wake the
     1-waiter, and without the rescan nothing ever would — no weight
     moved, so no other transition fires); `Granted` (woken, never
     repolled) → remove the entry, `granted -= requested; free +=
     requested`, re-run the grant scan — a cancelled winner cannot
     strand weight, and the next satisfiable waiter inherits it.
     BOTH cancellation arms end in the same scan.
   - **⚠ ROUND-201 #1 — LEAVING A CANCELLED ID IN `order` IS AN
     UNBOUNDED MEMORY LEAK, AND THE PERMIT INVARIANT DOES NOT CATCH
     IT.** The earlier rule said a cancelled waiter's id "goes stale
     and is skipped by the scan". But the scan pops stale ids only
     at the FRONT and stops at an unsatisfiable head, so every stale
     id BEHIND a blocked head is never reclaimed. **REPRODUCED by
     simulating these exact rules:** capacity 8 fully held, one
     blocked head wanting 1, then 100_000 enqueue/cancel pairs
     behind it — `order` reached **100_001 entries with ONE live
     waiter**, while `free + held + granted == 8` still held. The
     whole-isolate memory proof bounds live CONCURRENCY, so a leak
     that keeps concurrency at 1 slips straight through it, and
     every existing gate test passes.
     **THE RULE: cancellation removes the id from `order` as well as
     from `nodes`.** Cost is O(live waiters) worst case, which is
     the bound the envelope already charges — a `VecDeque::retain`
     or an index-carrying node makes it cheaper, but ANY form that
     reclaims immediately is acceptable; what is NOT acceptable is
     deferring reclamation to a scan that may never reach the entry.
     **RE-SIMULATED WITH THE FIX: `order` stays at 1**, and the new
     invariant `order.len() <= live waiter count` holds.
     **Test (§9): `cancellation_churn_behind_blocked_head_does_not_
     grow_storage()`** — drive ≥ 10_000 enqueue/cancel pairs behind
     an unsatisfiable head and assert **RETAINED STORAGE**
     (`order.len()`), not merely the permit counts. A test that
     checks only `free + held + granted` passes against the broken
     rule and is why this survived to round 201.
   - **Waker replacement:** repolling from a new task replaces the
     stored waker in place (same node id, no requeue).
   Tests (§9): head cancellation, middle cancellation, waker
   replacement, granted-then-dropped handoff, no-stealing, and
   **`cancellation_churn_behind_blocked_head_does_not_grow_storage()`**
   (round-201 #1 — asserts retained storage, not permit counts).

4. **Absolute per-call deadlines — submission is BATCH ENTRY;
   equality is BIDDER-owned (round-98 M9).** At batch entry every
   call captures `call_deadline_at = min(auction_deadline_at,
   entry_instant + call.timeout)` — later waves do NOT re-anchor.
   Permit acquisition and the send both race that absolute instant.
   Cause attribution keeps M3a Task 9.5's tie rule: **equality ⇒
   `TimeoutCause::Bidder`** (the round-97 "bidder only if strictly
   first" phrasing is retired; the two documents now agree). Test:
   a later-wave call whose permit wait crosses `entry + timeout`
   yields `Timeout(Bidder)` even though its wave dispatched late.

5. **Permit lifetime + adapter-owned egress buffers (round-98 B5,
   corrected round-99 B4).** `PermitBody` (the `AdmissionPermit` moved
   into a `Body::Stream`) is NECESSARY but NOT SUFFICIENT: the pinned
   Axum and Spin adapters DRAIN the stream into a `Vec` (dropping the
   stream — and the permit — before transmission), and even a
   streaming adapter may poll EOF while the final chunk sits in a
   transport buffer. Two-part fix:
   - **A dedicated EGRESS GATE (round-100 B4; acquisition point +
     capped serializer round-107 B2).** A fixed
     `max_concurrent_auctions x final_response_bytes` reservation does
     NOT bound post-permit responses: once an adapter drains the stream
     and the auction permit releases, a NEW auction can start while the
     old response is still buffered for a slow client — repeat and the
     buffered responses exceed any concurrent-auctions multiple. The
     bound therefore needs a REAL second counter: `egress_permits`
     (envelope-owned); the budget charges
     `egress_permits x final_response_bytes`. **Round-107 B2 — the
     permit is acquired BEFORE serialization, and serialization is
     CAPPED (the earlier "acquired when the body is handed to the
     adapter" point let already-serialized bodies wait OUTSIDE the
     egress term, and `final_response_bytes` was a hope while
     `serialize_response` (serialize.rs:11) ran an unbounded
     `serde_json::to_vec`):**
     - **Round-117 P0-3 — EVERY response path holds bounded egress
       ownership, not only the success path.** The egress gate as
       previously drafted covered only serialized auction bodies;
       admission-refused 503s, ingress `Rejection` bodies, queue-expiry
       503s, serializer-overflow 500s, and every other static 4xx/5xx
       were transmitted ungated, so a population of slow clients could
       accumulate adapter-owned copies of those buffers without limit.
       **⚠ ROUND-186 #2 — HOW THE STATIC CLASS IS ACQUIRED, AND WHY
       RESET-ON-EXHAUSTION CANNOT BE `AdmissionDecision::Abort` FOR
       POST-ADMISSION RESPONSES.** Round-184 adopted upstream's
       `AdmissionDecision::Abort` as the answer to "what happens when
       the static pool is exhausted". That is correct ONLY for
       responses decided DURING admission. `AdmissionDecision` is
       returned once, synchronously, BEFORE the body is read and
       before the handler runs — so a config-fetch 503, a
       serializer-overflow 500, an egress-queue-expiry 503 or any
       other POST-admission response has no admission decision left
       to return, and cannot retroactively abort. Verified at
       upstream head `03cdba98`: `ResponseEgressOutcome` has no
       application-selectable abort, and
       `ResponseEgressFallbackDisposition::Aborted` is a REPORTED
       disposition, not a decision the application makes.
       **⚠ ROUND-188 #4 — RESERVING PER ADMITTED REQUEST MAKES THE
       POOL A UNIVERSAL TERMINAL-RESPONSE RESERVATION, AND IT MUST BE
       SIZED AND NAMED AS ONE.** The round-186 resolution below is
       right that reservation must happen at admission. But it left
       the pool described as a REFUSAL pool while every admitted
       request — including every SUCCESSFUL auction — now holds one
       of its permits to terminal egress. Two consequences the
       earlier text got wrong:
       - **Auction traffic can starve refusals.** With 64 permits and
         a healthy load of 64 concurrent auctions, the next request's
         admission finds the pool empty and ABORTS — under no
         overload condition at all. A pool sized for refusals cannot
         absorb the success path.
       - **`max_concurrent_auctions` does not bound the class.** Aux
         routes and pre-handler fallbacks are admitted too and hold
         permits, so they are outside that bound by construction.
       **SIZING RULE, CORRECTED — ONE FIELD, NO RESIDUAL RESERVE
       (round-189 #4).** The pool is a UNIVERSAL terminal-response
       reservation and the field is named
       **`terminal_response_permits`** EVERYWHERE — envelope,
       formula, defaults, task interfaces and proof rows.
       `static_egress_permits` is its RETIRED SPELLING and must not
       appear as a live field name; round-188 renamed it in one
       sentence and left nine uses behind, which is how two names for
       one semaphore got into the same document.
       ```
       terminal_response_permits >= max_concurrent_auctions
                                  + aux_route_permits
       ```
       **`fallback_reserve` IS REMOVED (round-189 #4).** Round-188
       introduced it with no field, value, budget term, owner or
       test — a term in a boot assertion that nothing defines cannot
       be asserted. It is also unnecessary: pre-handler fallback and
       body-timeout requests have ALREADY passed an admission gate
       and are therefore already counted in `max_concurrent_auctions`
       or `aux_route_permits`. Adding a separate reserve would
       DOUBLE-COUNT them. A pre-admission detached rejection, by
       contrast, never took an admission slot — and it is bounded by
       the adapter's own connection limits, not by this pool, which
       is exactly why `DetachedResponseEgressDecision::Abort` is the
       response to exhaustion there rather than a reserved permit.
       **This is a BOOT ASSERTION** (both terms are baked per
       round-188 #3), so a deployment cannot configure itself into
       the starvation above. Default: 64 + 4 = 68, from the existing
       `max_concurrent_auctions` and `aux_route_permits` defaults.
       **The alternative — releasing the static permit when an
       auction permit is acquired — is REJECTED:** the two permits
       would have to be exchanged atomically at a point where the
       response is not yet constructed, and any gap between release
       and acquire is a window where a terminal response has no
       reservation, which is the exact failure the reservation
       exists to prevent.
       **RESOLUTION — RESERVE AT ADMISSION, ONE PER ADMITTED
       REQUEST.** Every admitted request reserves ONE static permit
       during admission and carries it to completion. That permit is
       the request's guaranteed right to emit exactly one terminal
       response, whatever that response turns out to be. Consequences,
       each deliberate:
       - **Exhaustion is decided where it CAN be decided.** If no
         terminal-response permit is available at admission,
         admission itself refuses or aborts — the one place
         upstream's API allows it.
       - **No post-admission response can ever be blocked**, because
         its permit was reserved before it was needed. A 500 that
         cannot be emitted is the failure mode this whole section
         exists to prevent; reserving up front makes it unreachable.
       - **The pool is sized by ADMITTED CONCURRENCY**, not by
         response count: one live request holds at most one
         terminal-response permit. **Round-189 #4: that bound is
         `max_concurrent_auctions + aux_route_permits`, NOT
         `max_concurrent_auctions` alone** — this line previously
         said the latter, which omits every aux-route request and
         understates the pool by exactly the aux capacity.
       - **Release is on terminal egress**, through the same
         completion the auction class uses.
       The alternative — asking upstream for a post-handler egress
       decision so the application can select Abort after the handler
       returns — is filed as **G24** and is NOT required for M3b: it
       would let auctioneer reclaim capacity more aggressively, but the
       reserve-at-admission model is correct without it.
       Corrected model — two egress classes, both bounded:
       (a) the AUCTION class (the `StreamGate` permits below), for
       serialized auction bodies, `final_response_bytes` each;
       (b) a STATIC class (consolidated contract, rounds 117-127) for
       every refusal and error body — `overloaded\n`,
       `Invalid request: …\n`, the standard 500, all ingress
       `Rejection`s: each of the `terminal_response_permits`
       (boot-envelope field, default 68 — round-190 #6: this said 64,
       the pre-round-189 refusal-only figure; the universal pool is
       `max_concurrent_auctions + aux_route_permits` = 64 + 4)
       reserves ONE fixed
       `refusal_body_bytes = 256` render buffer.
       **✅ ROUND-206 #1 — THE ROUND-205 EXEMPTION IS WITHDRAWN:
       UPSTREAM SHIPPED A RENDERER, SO THE 256-BYTE CHARGE HOLDS
       FOR EVERY RESPONSE.** Round-205 found that
       `EdgeError::NotFound { path }` embeds the full unmatched path
       and concluded framework errors must sit OUTSIDE the
       per-permit byte charge, bounded instead by auctioneer's URI
       limit. Two things were wrong with that:
       - **The URI-based bound under-counted.** It omitted the JSON
         envelope the default renderer wraps the message in —
         measured, a 768-byte path yields ~840 response bytes, not
         768.
       - **It is no longer necessary.** `325134e4` adds
         **`App::set_error_response_renderer(Fn(EdgeError) ->
         Response)`**, so the application owns the entire error
         body. auctioneer installs a BOUNDED renderer — **measured at 9
         bytes for the fallback arm**, against 256 charged and ~840
         unbounded.
       **⚠ ROUND-207 #2 — IT IS NOT A BLANKET `error.kind()`
       RENDERER: APPENDIX F PRESCRIBES EXACT WIRE BODIES.** The
       round-206 rule said emit `error.kind()` for everything, which
       would have BROKEN two shipped contracts: a 414 must be
       `uri too long\n`, not `uri_too_long`, and a 431 must be
       `too many header bytes\n`, not
       `request_header_fields_too_large`. Those are
       application-visible normalized errors with pinned wire
       formats, not raw-parser behaviour auctioneer does not own.
       **THE RENDERER IS A MATCH, IN TWO ARMS:**
       1. **PRESCRIBED — a fixed, exhaustive mapping** from the
          `EdgeError` variants Appendix F pins to their exact
          bytes, including the trailing newline and
          `content-type: text/plain; charset=utf-8`:
          `UriTooLong` → `uri too long\n`,
          `RequestHeaderFieldsTooLarge` →
          `too many header bytes\n`, and every other row Appendix F
          names. **Each body is a `&'static str`**, so its length is
          known at compile time and cannot exceed the charge.
       2. **FALLBACK — `error.kind()` alone** for variants Appendix
          F does NOT pin (`NotFound`, `MethodNotAllowed`,
          `Internal`, …). The kind is a closed vocabulary, so this
          arm is bounded by its longest member
          (`request_header_fields_too_large`, 31 bytes) — far under
          256 and independent of caller input.
       **NEITHER ARM INTERPOLATES CALLER TEXT.** The Appendix F rows
       that carry `<encoding>` are emitted by AUCTIONEER'S OWN handlers
       through the bounded serializer, not by this renderer, so the
       no-echo rule below stands unqualified for everything the
       renderer produces.
       Tests assert **EXACT BYTES for the prescribed arm** — a
       length-only assertion passes while the wire format is wrong,
       which is precisely how round 206 shipped this defect — and
       the ≤ 256 ceiling for the fallback arm.
       **CONSEQUENCE: no exemption, no upstream ask.** The
       per-permit `refusal_body_bytes = 256` is a true ceiling for
       every terminal response again, framework-generated included,
       and **G29 is CLOSED** rather than pending. The renderer is
       installed in `configure` alongside the egress policy (M3b-11
       adopts it; see its lifecycle bullet).
       **⚠ DELIBERATELY NOT INTERPOLATED:** the renderer must NOT
       echo the path, the allowed-set, or any other caller-supplied
       text — that is what made the default unbounded, and it is
       also the `/status` leak rule (round-186 #5) applied to a
       second surface. The KIND is a closed vocabulary.
       Tests: `framework_404_body_is_bounded_by_the_renderer()` —
       drive an unmatched path at the URI limit and assert the
       emitted body is ≤ `refusal_body_bytes`, which fails against
       the default renderer; `error_renderer_never_echoes_caller_
       input()`, which plants a canary path and asserts it appears
       in neither body nor headers; **`prescribed_error_bodies_
       match_appendix_f_exactly()`** (round-207 #2) — byte-for-byte
       over every pinned row, 414 and 431 included, with the
       trailing newline; and **`renderer_preserves_allow_and_retry_
       after()`** (round-208) — a 405 keeps its canonical `Allow`
       and a `config_out_of_date` keeps `Retry-After: 60` THROUGH
       custom rendering. `6904869b` guarantees this upstream by
       re-extending `required_response_headers()`, but auctioneer
       asserts it: every other renderer test checks BODY bytes, so a
       regression that stripped a mandatory header would pass all of
       them.
       Constant bodies and
       bounded-dynamic rejection bodies alike render into it (tokens
       clipped per the Rejection contract). Per-permit charge:
       `refusal_body_bytes x EGRESS_HANDOFF_FACTOR +
       transmit_state_bytes` — identical shape to auction egress.
       **⚠ ROUND-187 #3 — DETACHED RESPONSES DO NOT INHERIT AUCTIONEER'S
       REFUSAL BUDGET, AND THE DRAINAGE BOUND MUST BE MADE TRUE
       RATHER THAN ASSUMED.** The static class's whole saturation
       argument is that a held permit drains within
       `refusal_write_deadline_ms` (envelope-owned, default 2_000).
       That holds for responses auctioneer emits through its own egress
       policy. It does NOT hold for DETACHED pre-admission responses:
       `ResponseEgressEnvelope::detached` hardcodes
       `Arc::new(default_response_egress_policy)` — verified at head
       — which applies `DEFAULT_RESPONSE_WRITE_BUDGET =
       Duration::from_secs(30)`. A slow client on a detached 400
       would hold a static permit **fifteen times longer** than the
       bound the pool is sized against, so 64 permits drain in 30s,
       not 2s.
       **THE FIX — UPSTREAM MADE IT A REQUIRED FIELD (round-188 #7;
       G25 CLOSED).** Round-187 specified auctioneer constructing the
       detached response and inserting a `ResponseEgressDeadline`
       extension. That is no longer how it works, and the workaround
       is retired: at `ce61f92a` the decision became
       ```rust
       DetachedResponseEgressDecision::Send {
           completion: ResponseEgressCompletion,
           deadline: Deadline,          // MANDATORY
       }
       ```
       **EdgeZero constructs the detached response and installs the
       deadline auctioneer returns** — auctioneer supplies the value, not the
       response and not an extension. auctioneer computes it as
       `head.write_deadline_after(refusal_write_deadline_ms)`.
       **ANCHORING, ACCEPTED DELIBERATELY (round-188 #1):**
       `write_deadline_after` measures from `request_start`, while
       auctioneer's own static-class prose measures from permit
       ACQUISITION. auctioneer takes the upstream anchor: request start
       is strictly EARLIER than acquisition, so the resulting
       deadline is strictly tighter and the drainage bound holds a
       fortiori. Capturing the shared clock to return an absolute
       acquisition-anchored deadline would BUY NOTHING and add a
       clock dependency to a synchronous pre-admission callback.
       Test: `detached_refusals_carry_the_refusal_write_deadline()`,
       asserting the effective deadline is the auctioneer value and NOT
       the 30s default.
       **This is why `refusal_write_deadline_ms` stays envelope-owned
       (round-187 #9): the value now has to match a pool size, so an
       operator who raises one must be able to raise the other
       without a rebuild.**
       **⚠ ROUND-190 #6 — THAT SENTENCE IS WITHDRAWN.** Both
       `terminal_response_permits` and `refusal_write_deadline_ms`
       are declared in `auctioneer.toml` at the TOP LEVEL with the other
       host settings (round-193 #1) and are BAKED by `build.rs`: the envelope is
       built before store access and the pre-admission factory is
       synchronous, so no store read is possible at that instant.
       **Changing either requires REBUILD + REDEPLOY. `config push`
       cannot change them, and neither can a restart** — round-193
       #2: an earlier line here said a pushed change "takes effect on
       the next PROCESS START", which is false, since the same binary
       re-reads the same compiled-in constant. The
       interdependence is real and is why they are checked by a BOOT
       ASSERTION rather than left to an operator to keep in step. The
       withdrawn wording survives only as the reason the assertion
       exists. The
       class is sized so refusals can always be answered up to the
       counter; auction egress can never starve it; permits release
       only on the Prereq-9 completion/disconnect signal, under the
       fresh `refusal_write_deadline_ms` budget.
       **Round-118 F05 — saturation is defined, bounded, and
       nonrecursive:** "refusals can always be answered" holds only up
       to the counter; at capacity the next rejection does NOT queue
       and does NOT respond ungated — the adapter CLOSES/RESETS the
       transport with no body (a close costs no charged buffer, so the
       model stays whole; the client experiences connection reset, the
       honest signal of an isolate past even its refusal capacity).
       Round-119 F4 corrects the deadline inheritance: a static
       refusal born FROM an expired auction deadline cannot reuse that
       deadline (it is already in the past — the 503 would abort
       before its first byte). Every static-class response gets its
       OWN fresh, boot-owned budget (`refusal_write_deadline_ms`,
       default 2_000).
       **⚠ ROUND-190 #2 — THE ANCHOR DEPENDS ON WHEN THE RESPONSE IS
       BORN, AND ROUND-189 #8 COLLAPSED TWO CASES THAT MUST STAY
       APART.** Round-188 accepted upstream's request-start anchor
       for DETACHED pre-admission rejections, where it is correct and
       strictly tighter. Round-189 then applied that anchor to the
       WHOLE static class — which destroys the property this very
       paragraph exists to state. A config timeout at t=2.5s or an
       egress-queue expiry at t=3s would be handed a deadline of
       `request_start + 2_000ms`, i.e. ALREADY IN THE PAST, so the
       503 would abort before its first byte and the client would get
       nothing. That is exactly the round-119 F4 failure this
       paragraph names — "a static refusal born FROM an expired
       auction deadline cannot reuse that deadline" — reintroduced
       through a different route.
       **THE SPLIT, BINDING:**
       - **PRE-ADMISSION detached response** (400/414/431 from a
         normalized-head failure): `head.write_deadline_after(
         refusal_write_deadline_ms)` — REQUEST-START anchored. These
         are born microseconds after request start, so the anchor
         costs nothing, and upstream constructs the response anyway.
       - **POST-ADMISSION static response** (config-fetch 503,
         serializer-overflow 500, egress-queue expiry, admission
         refusal): a FRESH deadline, NEVER `request_start + _`.
         **⚠ ROUND-191 #5 — "COMPUTED WHEN TERMINAL EGRESS BEGINS"
         IS NOT ATTACHABLE AS WRITTEN.** auctioneer builds and annotates
         the response BEFORE handing it back; EdgeZero calls
         `ResponseEgressEnvelope::begin` afterwards, so auctioneer has no
         way to evaluate an extension at upstream's egress-start
         instant. Two places can express it, and auctioneer uses the
         SECOND:
         - *Response construction* (`ResponseEgressDeadline` in the
           extensions, `now + refusal_write_deadline_ms` at the
           moment the response is built). Legal, and STRICTLY
           TIGHTER than egress start, since construction precedes
           it. Acceptable, but it silently shortens the budget by
           however long the response waits to be picked up.
         - *The response-egress POLICY CALLBACK*, which upstream
           invokes with `egress_started_at` — exactly the instant
           needed. auctioneer's installed policy returns
           `egress_started_at + refusal_write_deadline_ms` for
           static-class responses, so the budget is measured from
           the real transmission start with no extension needed.
           **✅ ROUND-194 #3/#4 — NEITHER OF THESE IS NEEDED:
           UPSTREAM SHIPPED `ResponseEgressDeadline::after`.** At
           `46f85958` (commit `feat(core): add response-relative
           egress deadlines`) the type became an enum with
           `after(Duration)` and `at(Deadline)`, and `begin()`
           resolves the relative form against `egress_started_at`
           (`response_egress.rs`: `.remove::<ResponseEgressDeadline>()
           .map(|d| d.resolve_at(egress_started_at))`). **G26
           CLOSED** — auctioneer filed it one round earlier.
           **auctioneer inserts
           `ResponseEgressDeadline::after(refusal_write_deadline_ms)`
           into the static response's extensions.** Upstream
           resolves it at the true transmission start, so the budget
           is neither shortened by construction-time anchoring nor
           dependent on classifying the response in a global
           callback.
           **⚠ ROUND-202 #4 — THE EXTENSION ALONE DOES NOT COVER
           EVERY STATIC RESPONSE, SO THE INSTALLED POLICY IS ALSO
           SPECIFIED.** `ResponseEgressDeadline::after` is inserted
           by the HANDLER, so it cannot reach responses EdgeZero
           generates itself — body-timeout and overflow replies
           never pass through auctioneer's handler. Withdrawing the
           policy-based approach (round-194 #4) left those paths
           with no deadline at all.
           **BOTH mechanisms, with a stated precedence:** auctioneer
           installs a response-egress policy via
           `set_response_egress_policy` that returns
           **`head.application_deadline()` when the response carries
           one** (the handler-inserted `after(..)`, already resolved
           by `begin()`), **otherwise `egress_started_at +
           refusal_write_deadline_ms`**. The handler keeps inserting
           the extension for responses it builds; the policy is the
           floor for everything else. Upstream enforces the earlier
           of the two, so the policy can only tighten.
           Test: `framework_generated_response_gets_the_refusal_
           budget()` — drive a BODY-TIMEOUT response (no handler
           involvement) and assert it carries a deadline, which a
           handler-only test would never catch.
           **⚠ ROUND-203 #2 — THE INSTALLED POLICY DOES NOT REACH
           ADMISSION REFUSALS EITHER, SO THEY CARRY THE DEADLINE
           THEMSELVES.** `AdmissionDecision::Refuse` goes out through
           DETACHED egress, and `ResponseEgressEnvelope::detached`
           hardcodes `Arc::new(default_response_egress_policy)` — the
           30-SECOND default — so auctioneer's installed policy is never
           consulted. A refusal would hold its terminal-response
           permit fifteen times longer than the pool is sized for,
           which is the exact failure round-187 #3 fixed for
           pre-admission rejections and which returns here by a
           different route.
           **THE RULE: the admission callback attaches
           `ResponseEgressDeadline::after(refusal_write_deadline_ms)`
           to EVERY `Refuse.response` before returning it.**
           `Refuse.response` is a full `Response`, so the extension
           is attachable at construction, and `begin()` resolves the
           relative form against the true egress start. No upstream
           change is needed.
           Test: `admission_refusal_carries_the_refusal_budget()` —
           drives the REAL refusal-to-egress path and asserts both
           that the effective deadline is auctioneer's value and that
           **the installed application policy was NOT called** (a
           test that only checks the deadline would pass if the
           policy silently started applying).
           **THE ROUND-193 MARKER-HEADER SCHEME IS WITHDRAWN, AND IT
           COULD NOT HAVE WORKED.** It required auctioneer to leave
           `x-auctioneer-egress-class` on the response for the policy
           callback AND strip it before transmission. There is no
           such point: the callback receives IMMUTABLE headers from
           the response upstream is about to send, so removing the
           header hides it from the callback and keeping it leaks a
           private marker to the client. `after()` removes the need
           for a response class in the callback entirely.
           Test: `static_response_uses_a_relative_egress_deadline()`
           — asserts the extension carries the RELATIVE form, so a
           response constructed long before transmission still gets
           its full budget.
         **auctioneer installs the policy** (`set_response_egress_policy`
         at boot) and does NOT rely on a construction-time
         extension for this class. Where both exist, upstream
         enforces the earlier, so the policy can only tighten, never
         loosen.
       **The pool-drainage bound survives either way**: each held
       permit still releases within `refusal_write_deadline_ms` of
       the moment ITS response starts transmitting, which is what the
       sizing argument needs. It never needed the deadline measured
       from request start. Tests:
       `post_admission_refusal_gets_a_future_deadline()` — a 503 born
       after `refusal_write_deadline_ms` has already elapsed still
       has a deadline strictly in the future and still emits its
       body, asserted through the POLICY CALLBACK's
       `egress_started_at` (round-191 #5)
       — and `pre_admission_refusal_is_request_start_anchored()`.
       If THAT
       expires, the transport is reset — no renewal, no recursion,
       terminal. A slow client holding a static permit is thus aborted
       by the refusal budget, so saturation is transient by
       construction — **bounded by the write deadline PLUS upstream's
       fallback budget, not by the write deadline alone (round-202
       #5).** Tests: egress-queue expiry answered with free
       static capacity; expiry immediately after serialization;
       admission rejection before any auction begins; a static writer
       stalling to ITS deadline and resetting.
       Conformance rows: capacity+1 (reset observed, no allocation
       growth), and 64 never-reading clients drained by deadline
       abort.
     - Handler order after `run_auction` returns `Ok(BidResponse)`:
       (1) ACQUIRE an egress permit — an async FIFO wait on the
       `egress_gate` (same `StreamGate` type: cancellation-safe,
       identified waiters). While waiting, the auction holds only its
       TYPED response — inside the per-auction parsed-bids terms; no
       serialized buffer exists yet. (2) Only under the permit,
       serialize through the REPLACED bounded serializer:
       `serialize_response_bounded(&resp, final_response_bytes) ->
       Result<Vec<u8>, SerializeError>` — `serde_json::to_writer`
       into a `CappedWriter` over a `Vec::with_capacity(cap)` that
       errors the moment the cap would be crossed (M3b-2 REPLACES
       serialize.rs's unbounded `serde_json::to_vec`; the M3a/M2
       call sites migrate mechanically). (3) Hand the serialized
       response to the adapter. **⚠ ROUND-183 #2 — STEP (3) DOES
       NOT ATTACH THE EGRESS PERMIT TO `PermitBody`, AND THIS
       NORMATIVE SECTION IS THE PLACE THAT SAYS SO.** `PermitBody`
       carries the ADMISSION permit (item 5 above defines it that
       way); the egress permit is a DIFFERENT permit with a
       DIFFERENT release condition. Body EOF/drop is not terminal
       transmission: an adapter that drains the stream into a `Vec`
       hits EOF while every byte is still in a transport buffer, so
       releasing egress capacity there would hand the slot to the
       next auction before the current response left the process —
       precisely the over-admission the gate exists to prevent. The
       egress permit is released on EdgeZero's terminal
       `ResponseEgressReport`, delivered through the RESPONSE-SCOPED
       `ResponseEgressCompletion` callback that upstream landed at
       `03cdba98` (G18/G21 — verified in
       `edgezero-core/src/response_egress.rs`: a `FnOnce(&Response
       EgressReport)` held by the response, fired exactly once at
       terminal egress, panic-guarded). The permit moves INTO that
       callback, so release is terminal by construction.
       **⚠ ROUND-202 #5 — "RELEASED WITHIN
       `refusal_write_deadline_ms`" IS NOT TRUE, AND THE POOL SIZING
       MUST NOT ASSUME IT.** Upstream's commit boundary says that
       before headers commit, an expired deadline "may be replaced
       by an adapter-private bounded 500 or 504", and the original
       cause is reported "only after the fallback reaches finish or
       abort" (response-egress design §4.1). That fallback carries
       its OWN bounded budget, and the SAME completion holds the
       permit until it finishes. So the application WRITE DEADLINE
       and the TERMINAL RESOURCE-RELEASE instant are different
       quantities:
       - **write deadline** = `refusal_write_deadline_ms` — when
         auctioneer's own write must be done.
       - **release instant** = terminal report, which in the
         worst case is `refusal_write_deadline_ms` PLUS upstream's
         fallback budget.
       **CONSEQUENCE FOR SIZING:** drainage is bounded by
       `refusal_write_deadline_ms + fallback_budget`, not by the
       first term alone. The pool is still bounded — both terms are
       finite — but any assertion phrased as "the permit is released
       within 2s" is false and must read "within the write deadline
       plus upstream's fallback budget". Tests assert ORDERING
       (release happens at the terminal report, after any fallback)
       rather than a wall-clock bound auctioneer does not own.
       **⚠ ROUND-184 #4 — THERE IS NO HANDLER-FUTURE FALLBACK, AND
       THE EARLIER ONE WAS UNSOUND.** A previous revision said that
       until the binding landed, the permit could be held by the
       handler's response future and released on its completion,
       calling that a conservative approximation that "over-holds
       rather than over-admits". That reasoning was BACKWARDS: the
       handler future completes when it RETURNS the response, which
       is BEFORE transmission — strictly earlier than body EOF, the
       release point this same paragraph rejects as too early. It was
       an UNDER-hold that over-admits egress, i.e. the exact failure
       the gate exists to prevent, and it is removed rather than
       retained as a fallback. If a target cannot deliver the
       response-scoped completion, the correct response is to
       DECLARE that target's egress bound BestEffort in its
       capability row — never to approximate with an earlier signal.
       2a.7a owns the admission-body permit; 2a.7b owns the egress
       binding.
     - Defined behavior: EXACT-CAP output is accepted (the cap is
       inclusive). OVER-CAP is `AuctionError::Internal` → the standard
       500 body transmitted UNDER THE SAME PERMIT the serialization
       held (round-117 P0-3 — the release-then-ungated-500 draft let a
       slow client hold the adapter's 500 buffer outside any gate; the
       permit now travels to completion/disconnect for the error body
       too, and only then releases). **Round-108 H4 — over-cap is a LEGITIMATE
       (rare) outcome, not "provably a bug": the
       `final_response_bytes >= max_calls x max_decoded x
       ASSEMBLY_FACTOR` validation is a SIZING FLOOR, not an
       output-expansion theorem — ASSEMBLY_FACTOR pins allocator
       scratch, and JSON string-escaping of embedded creative markup
       can expand raw bytes on the wire (up to 6x for pathological
       control-character content). The WIRE bound is the enforced
       cap itself; the default 2_097_152 vs the 1_572_864 content
       floor leaves 33% escaping headroom, and operators sizing
       `final_response_bytes` are pointed at this escaping note.**
       WAIT-CANCELLATION (client disconnect while queued) removes the
       waiter node without consuming a permit (StreamGate
       cancel-handoff semantics).
       **Round-108 H4 — `ResponseWriteDeadline`, a DISTINCT deadline
       (the round-107 text said "the same §5 deadline", but §5's only
       deadline was the auction tmax — reusing it would turn every
       budget-consuming auction's legitimate timeout RESPONSE into a
       503):** a monotonic absolute instant anchored when the handler
       ENTERS the egress phase (immediately after `run_auction`
       returns — including a tmax-expired auction returning its
       timeout response: the write deadline starts FRESH there), with
       duration `resource_limits.response_write_deadline_ms`
       (host-config value, default `10_000`; range `100..=120_000`).
       **Round-109 H3 — the COMPLETE source-to-sink dataflow (the
       round-108 definition named the value but wired nothing):**
       `crates/auctioneer-config/src/types.rs` (`ResourceLimits` gains
       `response_write_deadline_ms: u64`);
       `crates/auctioneer-config/data/host-defaults.toml`
       (`response_write_deadline_ms = 10000`);
       `crates/auctioneer-config/build_codegen.rs` (the `ResourceLimits`
       renderer emits it; validation range `100..=120_000`, out of
       range fails codegen); NOT in the `[auction]` runtime-overlay
       DTO — an overlay attempt is rejected as an unknown key by the
       existing overlay validation (host-only, no account/runtime
       override; the spec §9 `[auction]` bullet records this);
       `ResolvedIsolateLimits` gains `response_write_deadline_ms:
       u64` (a TIME source, not a memory term — validation only
       range-checks it; carried on the envelope so the egress phase
       reads ONE installed struct); the §2 boot envelope copies it
       from the baked host-defaults; the handler's egress phase
       reads it from the installed envelope on every target. Spec
       §9.1 resource-limits table carries the row. Tests (roster):
       `response_write_deadline_codegen_default_10000()`,
       `response_write_deadline_range_rejected_pair()` (99 and
       120_001 fail codegen),
       `response_write_deadline_not_overlayable()` (an `[auction]`
       overlay carrying the key is rejected),
       `envelope_propagates_response_write_deadline()` (configured
       non-default value reaches the installed envelope). It covers the WHOLE egress phase at FOUR enforced
       checkpoints (round-110 H6 — the round-109 text claimed
       queue+serialize+transmit coverage but only defined the queue
       and mid-write arms, leaving serialization outside the
       deadline): (1) the QUEUE wait races it; (2) rechecked ON
       PERMIT GRANT, before serialization begins; (3) rechecked
       AFTER serialization, BEFORE the first byte is handed to the
       adapter (serialization itself is CPU-bounded by the byte cap,
       so bracketing checks suffice — no mid-serialize polling.
       ROUND-118 F12 caveat: on DEPLOYED Workers, Date.now/
       performance.now are FROZEN across CPU-only work — both
       bracketing reads can return the same instant around a
       serialization that in fact crossed the deadline, and local
       Wrangler behaves differently, so native/local timing proves
       nothing here. The CF adapter must therefore re-evaluate the
       deadline THROUGH A CLOCK-REFRESHING HOST BOUNDARY (an awaited
       host I/O point — Prereq 3 names the exact yield) before
       committing the first byte, and decode loops take a cooperative
       yield budget so an always-ready stream cannot monopolize one
       poll with the clock frozen. Conformance: a frozen-clock test
       plus one DEPLOYED-Workers probe);
       (4) transmission through the Prereq-9 write-complete signal
       (the §5 slow-client stall bound is this same deadline's
       transmission leg). Tie rule: `now >= deadline_at` ⇒ expired
       (equality expires, matching the GCL comparator convention).
       Wire behavior is split at the FIRST BYTE (round-110 H6):
       expiry at checkpoints 1-3 — nothing transmitted yet — → drop
       the typed response / the serialized buffer, release the
       AUCTION permit, and return 503 `overloaded\n` THROUGH THE
       STATIC-EGRESS CLASS (round-117 P0-3, below — never ungated);
       once transmission has
       begun no status rewrite is possible — expiry or stall
       mid-write → ABORT the connection + release the permit (the
       Prereq-9 `egress_write_deadline_enforced_on_stall()` case). RELEASE
       stays Prereq-9 write-complete/disconnect — over-cap error,
       wait-cancel, and both expiry arms each tested.
   - **EdgeZero Prereq 9 BLOCKS the whole-isolate memory
     certification:** without the egress completion/drop hook the
     release point does not exist, so M3b-2 implements the gate but the
     memory PROOF is certified only once Prereq 9 lands (the M3b-8 gate
     records this as an explicit open item until then). Adapter-level
     tests (with Prereq 9): EOF, client-disconnect, slow-client, and a
     response-write DEADLINE so a stalled client cannot pin an egress
     permit indefinitely.

6. **GVL + pipeline retention terms (round-97 B2/B3, corrected
   cardinality round-98 B3).**
   - `max_calls = max_outbound_requests` (**100** — the GLOBAL
     outbound-call cap from the spec resource limits; one bidder may
     emit MULTIPLE calls from `make_requests`, so the round-97
     `min(max_bidders, …) = 50` under-counted).
   - **Outbound-body caps — TWO stages, charged transiently
     (round-99 H9 / round-100 H6 — a single post-compression check is
     too late: the adapter can allocate an unbounded raw body first,
     and compression transiently holds raw + encoded together):**
     `max_outbound_raw_bytes` (round-101 B3 / round-102 B4 /
     round-103 B6 — enforced by an ENFORCING SINK, because an
     advisory-limits parameter beside an API that still returns an
     unrestricted `Vec` bounds nothing, and `Result<Vec<_>, Vec<_>>`
     cannot express partial success. M3b-2 REPLACES the
     `make_requests` return channel:
     ```rust
     // auctioneer-bidder — the sink is the ONLY constructor of
     // BidderHttpRequest (its fields become pub(crate); round-103 B6 —
     // a public Vec<u8> body field would bypass the cap):
     /// Round-106 B6 — ONE auction-global call budget, SHARED by every
     /// per-bidder sink (max_calls is an AUCTION cap; per-bidder sinks
     /// enforcing it independently would have allowed
     /// bidders x max_calls). The exchange creates it once per auction
     /// from `limits.max_outbound_requests`; the bidder loop is
     /// SEQUENTIAL in §5.9 registry order, so a `Cell<u32>` suffices
     /// (no atomics) and exhaustion is DETERMINISTIC: budget is
     /// consumed in registry order, then per-bidder request order —
     /// exactly the spec's existing `max_outbound_requests` counting
     /// rule, now enforced AT CONSTRUCTION (this RETIRES the M2
     /// post-hoc global-cap truncation in run.rs — the cap can no
     /// longer materialize-then-drop).
     pub struct CallBudget { remaining: Cell<u32> }
     impl CallBudget {
         pub fn new(max_calls: u32) -> Self { .. }
         /// Round-108 B1 — RAII reservation (replaces the round-107
         /// bool try_reserve + a private rollback that auctioneer-exchange
         /// could never call): dropping an uncommitted reservation
         /// re-credits the budget AUTOMATICALLY, so no cross-crate
         /// rollback API exists at all.
         pub fn try_reserve(&self) -> Option<CallReservation<'_>> { .. }
     }
     pub struct CallReservation<'a> { budget: &'a CallBudget, committed: bool }
     impl CallReservation<'_> { fn commit(mut self) { .. } }
     impl Drop for CallReservation<'_> {
         fn drop(&mut self) { /* if !committed: re-credit one slot */ }
     }
     /// Round-146 #1 — the bidder-owned limits the sink enforces.
     /// The EXCHANGE copies these two values out of the resolved
     /// envelope (`max_outbound_raw_bytes`,
     /// `max_impressions_per_auction`) when it constructs a sink, so
     /// auctioneer-bidder never depends on the envelope type. The spec's
     /// earlier `OutboundLimits` parameter is superseded (spec
     /// limits table) and is NOT a type anywhere in this plan.
     #[derive(Clone, Copy, Debug)]
     pub struct SinkLimits {
         pub max_outbound_raw_bytes: usize,    // the CappedWriter cap
                                               // (round-160 #5: usize,
                                               // checked-converted from
                                               // the envelope's u64 at
                                               // boot — wasm32 is
                                               // 32-bit)
         pub max_impressions_per_auction: u16, // sizes the imp-id offset table
     }
     /// Round-148 #3 — an accepted request and its STILL-UNCOMMITTED
     /// reservation travel together: committing inside push_request
     /// would make a later semantic-Err discard unrefundable, and
     /// dropping the local reservation would refund immediately.
     /// `into_outcome()` commits every retained reservation;
     /// `into_outcome_discarding_accepted()` (or simply dropping the
     /// sink) drops them uncommitted and the RAII Drop re-credits.
     pub struct AcceptedRequest<'a> {
         request: BidderHttpRequest,
         reservation: CallReservation<'a>,
     }
     pub struct OutboundRequestSink<'a> {
         limits: SinkLimits,
         budget: &'a CallBudget,
         validator: &'a dyn EndpointValidator,
         accepted: Vec<AcceptedRequest<'a>>,
         errors: Vec<OutboundBuildError>, // capacity fixed at 16, never grows
         suppressed_error_count: u32,     // 17th+ typed error
         rejected_call_count: u32,        // drives code 10999
     }
     impl OutboundRequestSink<'_> {
         /// The ONLY way to add a request. Round-107 B3 — the
         /// transaction order is RESERVE → METADATA → VALIDATE →
         /// BODY → COMMIT/ROLLBACK (round-146 #1: this is the ONE
         /// contract; the earlier single-closure `build(meta, body)`
         /// shape is retired from every document), and BOTH meta and
         /// body are constructed INSIDE the closures the sink
         /// invokes (the round-106 shape took `meta` as an argument,
         /// so Rust evaluated — and allocated — it before the budget
         /// check could run):
         /// 1. RESERVE: `budget.try_reserve()` → `None` means
         ///    exhaustion: increment `rejected_call_count`, record
         ///    CallBudgetExhausted at most ONCE in the error list,
         ///    return WITHOUT invoking EITHER closure — truly nothing
         ///    allocated.
         /// 2. METADATA: allocate the bounded meta byte pool
         ///    (`OUTBOUND_META_BYTES`, derived below) and run
         ///    `build_metadata(&mut meta_builder)`.
         /// 3. VALIDATE: the finalized URL is checked against `info`
         ///    through the exchange-owned `EndpointValidator`; a
         ///    rejection rolls the reservation back and `build_body`
         ///    is NEVER invoked.
         /// 4. BODY: allocate the `CappedWriter` and run
         ///    `build_body(&mut body_writer)`.
         /// 5. Any closure Err / cap breach / incomplete meta →
         ///    the `CallReservation` DROPS uncommitted (auto
         ///    re-credit); record the typed error; partial buffers
         ///    dropped. Incomplete meta is TYPED (round-148 #4):
         ///    after `build_metadata` returns Ok, a never-set method
         ///    is `OutboundBuildError::MissingMethod`, a never-set url
         ///    is `MissingUrl` (checked in that order, one error per
         ///    request); both are detected BEFORE validation and
         ///    BEFORE `build_body`, so the body closure is never
         ///    invoked. A SECOND `set_method`/`set_url` call is
         ///    `SinkAbort::DuplicateMeta { field }` (the byte pool is
         ///    append-only; an overwrite would strand bytes and let
         ///    a URL change after the adapter believed it final).
         /// 6. Ok → the sink assembles `BidderHttpRequest` from
         ///    byte pool + body and pushes `AcceptedRequest { request,
         ///    reservation }` — the STILL-UNCOMMITTED reservation
         ///    stored WITH the request (round-108 B1 / round-148 #3) — commitment
         ///    happens only in `into_outcome()`, so a later
         ///    semantic-Err discard refunds by simply DROPPING the
         ///    sink.
         /// Round-105 B7 / round-106 H7: `meta` is CAPPED AT
         /// CONSTRUCTION as an ALLOCATION bound, not a logical
         /// string-length tally (summing lengths after the strings
         /// exist bounds nothing — the allocations already happened).
         /// `OutboundRequestMetaBuilder` (allocated by the SINK
         /// inside push_request, AFTER the budget reservation.
         /// Round-118 F15: endpoint validation MOVES INSIDE this
         /// transaction — `validate_endpoint` runs before a
         /// reservation COMMITS (policy stays exchange-owned; the
         /// sink calls a validator handle), so an invalid-endpoint
         /// request never consumes a committed call slot. Under the
         /// Round-124 #8 makes the ownership boundary CONSTRUCTIBLE:
         /// the sink CONSTRUCTOR takes the validator —
         /// `OutboundRequestSink::new(limits: SinkLimits, call_budget:
         /// &CallBudget, endpoint_validator: &dyn EndpointValidator)`
         /// — and
         /// (round-145 #1) `push_request` is TWO-STAGE, taking the
         /// `&BidderRequestInfo` the validator needs per call:
         /// reserve → build METADATA (URL + headers) → validate the
         /// finalized URL against `info` → only then invoke the BODY
         /// closure → commit; any failure rolls the reservation back
         /// and the body closure is never invoked after a rejected
         /// URL. A single closure building meta AND body could not
         /// satisfy "rejected before any body is built". Where
         /// `trait EndpointValidator { fn validate(&self, url: &Url,
         /// info: &BidderRequestInfo) -> Result<(),
         /// EndpointRejectReason>; }` is implemented by the EXCHANGE
         /// (policy stays exchange-owned; the bidder-owned sink only
         /// holds the handle). `enum EndpointRejectReason {
         /// SchemeNotAllowed, HostNotAllowed, IpLiteralRejected,
         /// TemplateUnresolved, PortNotAllowed,
         /// MalformedUrl, UserinfoRejected, AuthorityMismatch }`
         /// (round-125 #13: exhaustive over every validate_endpoint
         /// failure class) (non_exhaustive) is
         /// defined HERE — it was referenced by the error enum but
         /// never defined.
         /// Round-119 F5 makes the rejection REPRESENTABLE:
         /// `OutboundBuildError` gains a per-request
         /// `EndpointRejected { reason }` variant. It is NOT the
         /// bidder-fatal semantic channel — the sink records it,
         /// rolls back ONLY that request's reservation, and
         /// construction of sibling requests continues; the exchange
         /// maps each occurrence to a code-5 entry for that bidder
         /// while dispatching the survivors. Tests assert, at
         /// capacity one, invalid-then-valid within AND across
         /// bidders: surviving dispatch, refund, the retained
         /// endpoint error, NO spurious budget warning, and no lost
         /// sibling calls. Under the
         /// M3a shape validation preceded the cap; letting the sink
         /// commit first would regress capacity-1 deployments: an
         /// invalid first request would refuse a valid later one with
         /// no refund path inside that bidder's construction loop.
         /// Fixtures: invalid-then-valid within one bidder AND across
         /// bidders at max_outbound_requests = 1 —
         /// round-107 B3) owns ONE private byte pool,
         /// `Vec::with_capacity(META_BYTES_CAPACITY = 3_584)` — split
         /// (round-108 B2) into the ADAPTER region (3_328) and the
         /// EXCHANGE FINALIZE RESERVE (256, the byte pool tail the
         /// adapter cannot reach; `finalize_for_transport` appends
         /// the exchange-owned headers — `Sec-GPC` and
         /// `Content-Encoding`, NEVER `Content-Length` (round-147
         /// #2: EdgeZero's `normalize_for_dispatch` strips Host and
         /// Content-Length and the provider transport frames the
         /// body; the reserve derivation is `Content-Encoding: gzip`
         /// = 20 bytes + `Sec-GPC: 1` = 8 bytes = 28 bytes of
         /// content, so the 256 tail is an unchanged, generous pin
         /// and the former Content-Length digits are no longer a
         /// term) — there, so
         /// finalization needs NO second allocation and NO field
         /// mutation) — plus a fixed-capacity offset table
         /// (component COUNT capped: <= 32 adapter headers
         /// (`MAX_BIDDER_HEADERS`; the 33rd `header()` call
         /// returns `SinkAbort::TooManyHeaders { limit: 32 }`
         /// without growing anything — round-147 #4) + 4 reserved
         /// exchange slots (2 appended today; 2 headroom, pinned by
         /// the offset-table constant, not by a header list) + up to
         /// `max_impressions_per_auction` imp ids — round-123 #7: the
         /// former hard 32 was arbitrary; no spec or validation bounds
         /// auctions at 32 imps, and mocktioneer legitimately covers
         /// EVERY imp in one call. ROUND-124 #9 — the offset table is
         /// SIZED FROM the field, not fixed. **ROUND-202 #1 — THE
         /// ENTRIES ARE NOT ALL THE SAME WIDTH, AND THE EARLIER
         /// FORMULA CHARGED THEM AS IF THEY WERE.** A header entry
         /// is `(n_start, n_len, v_start, v_len)` = **8 bytes**
         /// (`size_of::<(u16,u16,u16,u16)>()`, compiled probe), not
         /// the 4 the old `(2 + 36 + max_imps) x
         /// size_of::<(u16,u16)>()` charged. Corrected:
         /// `offset_table_bytes = 2*4 + 36*8 + max_impressions_per_
         /// auction*4`, rounded up to 64 — one 4-byte
         /// `(start, len)` each for method and url, 36 EIGHT-byte
         /// header entries (32 adapter + 4 reserved exchange), then
         /// 4-byte entries for the imp
         /// ids (round-147 clarification: the method/url pair was
         /// implicit before; the 64-byte rounding absorbs it at
         /// both the default and the 1_000-imp maximum, so no
         /// constant moves) — for the
         /// default 100 that is (138 x 4 -> 552 ->) 576 bytes, and
         /// the former fixed 512 is retired (it under-sized the
         /// default). Dataflow: `prepare()` gains the limit as an
         /// argument threaded from the envelope; over-limit requests
         /// are rejected as `BadInput` with the exact body
         /// `Invalid request: too many impressions (<n> > <limit>)\n`
         /// before any per-imp work runs; codegen range for the field
         /// is `1..=1_000`. Boundary tests: exactly-at-limit passes,
         /// limit+1 rejects with that body, and the sink covers
         /// all-limit imp ids without `MetaOverCap`. The envelope
         /// field (default 100,
         /// validated at prepare(): over-limit requests are rejected
         /// as BadInput with a documented code) sizes the offset
         /// table, so covering all imps can never overflow it). Every appended component
         /// is written INTO the byte pool through a bounded append that
         /// errors (`SinkAbort::MetaOverCap` — round-110 B3: every
         /// builder method returns `Result<(), SinkAbort>`) BEFORE
         /// exceeding capacity — the
         /// byte pool never reallocates and no free-standing component
         /// String is built first. The finished
         /// `OutboundRequestMeta` stores the byte pool + typed offset
         /// ranges; accessors return `&str` slices. Allocation peak
         /// per meta = 3_584 byte pool + offset_table_bytes (576 at the
         /// default 100-imp limit) + 64 struct overhead
         /// = 4_384 (`OUTBOUND_META_BYTES` at defaults; 8_032 at the
         /// 1_000-imp codegen max — round-126 #7: the old fixed
         /// "<= 512 overhead = 4_096" under-sized the default and is
         /// retired everywhere). The allocator-guard test pins ACTUAL
         /// allocations <= the DERIVED value at exact-cap, and the over-cap
         /// case errors without growing the byte pool.
         pub fn push_request(
             &mut self,
             info: &BidderRequestInfo<'_>,
             build_metadata: impl FnOnce(&mut OutboundRequestMetaBuilder<'_>)
                 -> Result<(), SinkAbort>,
             build_body: impl FnOnce(&mut CappedWriter<'_>)
                 -> Result<(), BodyWriteError>,
         ) -> Result<(), OutboundBuildError>;
     }
     /// Round-108 B2 / round-110 B3 — the ONE builder error model.
     /// The round-109 draft was internally incompatible: MetaBuilder
     /// methods returned `Result<(), MetaOverCap>` while `header()`
     /// was required to yield ReservedHeader — impossible without a
     /// shared enum. Fixed: EVERY `OutboundRequestMetaBuilder` method
     /// returns `Result<(), SinkAbort>` DIRECTLY (`set_method`,
     /// `set_url`/`expand_into`, `header`, `imp_id` — over-cap ⇒
     /// `MetaOverCap`, a reserved name in `header` ⇒
     /// `ReservedHeader`); body writes return
     /// `Result<(), BodyWriteError>` with `From<BodyWriteError> for
     /// SinkAbort`, so `?` works uniformly inside either closure
     /// (`build_body` returns `BodyWriteError` directly; the `From`
     /// impl serves adapter helpers shared by both stages).
     /// push_request maps SinkAbort onto the matching
     /// OutboundBuildError variant (MetaOverCap / ReservedHeader /
     /// TooManyHeaders / TooManyImpIds / Body; OversizeRaw comes
     /// from the CappedWriter's cap). Round-147 #4: the concrete
     /// definitions, files and exports of `CappedWriter`,
     /// `BodyWriteError`, `OutboundRequestMetaBuilder`,
     /// `EndpointValidator` and `EndpointRejectReason` are in
     /// M3b-2a.4a.
     pub enum MetaField { Method, Url }                  // round-148 #4
     pub enum SinkAbort { MetaOverCap { limit: u64 },
                          ReservedHeader { name: String },  // round-109 B2
                          TooManyHeaders { limit: u16 },    // round-147 #4
                          TooManyImpIds { limit: u16 },     // round-147 #4
                          DuplicateMeta { field: MetaField }, // round-148 #4
                          Body(BodyWriteError) }
     ```
     **Round-108 B2 — ONE bounded finalization path (the sink alone
     did not cover the real request pipeline; these are the M2 sites
     that today allocate free-standing strings or mutate the request
     after construction, all migrated in M3b-2):**
     ```rust
     // auctioneer-bidder — the ONLY post-accept transform. Consumes the
     // accepted request, returns the transport-ready request; every
     // step works inside existing bounded storage:
     pub struct TransportFinalize<'a> {
         pub gpc: Option<&'a str>,          // "1" ⇒ append Sec-GPC
         pub compression: Option<Compression>,
         pub max_encoded_bytes: u64,
     }
     /// Round-109 B2 — finalization has its OWN error type (the
     /// round-108 draft returned OutboundBuildError, none of whose
     /// variants can represent an encoded-size failure):
     pub enum FinalizeError {
         OversizeEncoded { limit: u64 },  // encoded body (gzip output,
                                          // or the raw body itself on
                                          // an uncompressed endpoint)
                                          // crossed max_encoded_bytes
         ReserveOverflow,                 // the 256-byte finalize
                                          // reserve overflowed —
                                          // unreachable for the three
                                          // fixed appends (< 96 bytes
                                          // worst case) but defined
     }
     pub fn finalize_for_transport(req: BidderHttpRequest,
                                   opts: &TransportFinalize<'_>)
         -> Result<BidderHttpRequest, FinalizeError>;
     // Exchange mapping: OversizeEncoded → the positional
     // FanoutError::OversizeRequest { stage: Encoded } (per-call slot
     // Err, never dispatched); ReserveOverflow →
     // FanoutError::Other("finalize reserve overflow") — the
     // internal-bug channel, same positional slot.
     ```
     **Reserved headers (round-109 B2, set EXPANDED round-110 H4 —
     prevention at CONSTRUCTION, not removal at finalize):** the
     RESERVED set is `Content-Encoding`, `Sec-GPC` (exchange-owned,
     appended only by `finalize_for_transport` into the finalize
     reserve), `Content-Length` (round-147 #2: reserved because the
     TRANSPORT owns framing — EdgeZero's `normalize_for_dispatch`
     strips Host and Content-Length before platform conversion and
     the provider generates them from the body; auctioneer never writes
     it either) PLUS the routing/framing fields RFC 9110
     assigns to the connection layer: `Host` (and the `:authority`
     pseudo-header form — Host influences application ROUTING and
     must derive from the validated URL at transport conversion,
     never from an adapter header), `Transfer-Encoding`,
     `TE` (round-111 H5 — RFC 9110 §7.6.1 lists it with the
     connection-specific fields; an adapter must not advertise
     trailer capability when the finalizer owns framing),
     `Connection`, `Trailer`, `Upgrade`, `Proxy-Connection`, and
     `Keep-Alive` (conflicting framing fields enable request
     smuggling). Fields NOMINATED by a `Connection` header need no
     separate handling — `Connection` itself is rejected, so no
     nomination can enter. The
     `OutboundRequestMetaBuilder::header(name, value)` append REJECTS
     every reserved name at push time, compared CASE-INSENSITIVELY
     (`eq_ignore_ascii_case` — "content-LENGTH", "hOsT", and
     "transfer-encoding" alike), via `SinkAbort::ReservedHeader` /
     `OutboundBuildError::ReservedHeader { name }` → the per-bidder
     PRE-FANOUT code-5 entry, like every other build error. Because
     reserved names cannot enter the meta byte pool at all, the finalizer
     never removes or overwrites anything — it only appends
     (`Content-Encoding: gzip` when it compressed; `Sec-GPC: 1` when
     gpc is set; no Content-Length, ever — round-147 #2), and the
     transport conversion (the pinned edgezero_proxy.rs forwards ALL
     request headers verbatim; EdgeZero's own dispatch normalization
     then strips Host/Content-Length as its framing pass) is SAFE
     precisely because construction already excluded the reserved
     set — no second filter needed, but only under sink-constructed
     requests.
     - **URL expansion moves INTO the build closure**: `EndpointBuilder`
       gains `expand_into(&mut OutboundRequestMetaBuilder<'_>) ->
       Result<(), SinkAbort>` and the macro values become a
       fixed-capacity slice of `(&str, &str)` pairs borrowed from
       `BidderInfo`/request fields — the current
       `BTreeMap<String, String>` value store (endpoint_builder.rs:52)
       and the `String`-returning `build()` (endpoint_builder.rs:156)
       are RETIRED for adapters (the template scan/allowlist/
       traversal checks are unchanged; only the output target moves
       into the byte pool). The url component is written once, directly
       into the adapter region.
     - **GPC insertion** (today run.rs:434 pushes an owned
       `("Sec-GPC","1")` pair after cloning): `finalize_for_transport`
       appends it into the FINALIZE RESERVE via the same bounded
       builder — inside auctioneer-bidder where the pub(crate) fields are
       writable; auctioneer-exchange only chooses `opts.gpc`.
     - **Compression + entity-header rewrite** (today fanout.rs:50
       clones the request, gzip-encodes into a fresh Vec, and
       retains/pushes header Strings): the gzip path stays
       `GzEncoder::new(CappedWriter::new(&mut encoded, max_encoded_bytes), ..)`
       (round-102 B5; the 2a.4a writer borrows a
       `Vec::with_capacity(max_encoded_bytes)` the finalizer owns)
       writing into that bounded buffer, which REPLACES the raw body
       in place;
       `Content-Encoding: gzip` goes into the finalize reserve (no
       Content-Length — round-147 #2); the raw body is dropped after the
       swap (the transient raw+encoded overlap stays charged by the
       round-100 H6 term).
     - **Transport conversion** (BidderHttpRequest → the platform
       outbound request) CONSUMES the request: `into_parts()` yields
       the meta (byte pool, `&str` accessors) and the body `Vec<u8>`,
       which MOVES into `Body::from_bytes` (round-149 #3 — no second
       outbound-body set exists, so the certificate charges one);
       method/url are parsed from the byte pool slices and headers are
       appended into the platform `HeaderMap` — the one copy the
       platform API itself requires.
     **⚠ ROUND-202 #2 — CONSUMING THE REQUEST LEAVES `make_bids`
     WITHOUT ITS ARGUMENT. THE RETAINED CONTEXT IS
     `OutboundRequestMeta`, AND THE PARSER CONTRACT CHANGES TO SAY
     SO.** `finalize_for_transport` MOVES the body into EdgeZero, so
     no `&BidderHttpRequest` survives the dispatch — yet the master
     spec's `make_bids(&self, request, http_request:
     &BidderHttpRequest, response)` requires one, and positional
     timeout handling separately needs the covered impression ids.
     Nothing in the plan said which side gives.
     **THE DECISION: the exchange retains `OutboundRequestMeta` per
     slot — it already does, for the memory charge — and that IS the
     response-processing context.** It holds method, url, headers
     and `covered_imp_ids()`; the only thing it does not hold is the
     BODY, which `make_bids` must not need: a bidder that re-reads
     its own request body to parse a response is re-deriving state
     it should have encoded in the request.
     **CONSEQUENCES, each explicit:**
     - **The trait changes**, and the master spec changes with it in
       the SAME commit: `make_bids(&self, request: &BidRequest,
       sent: &OutboundRequestMeta, response: &BidderHttpResponse)`.
       `OutboundRequestMeta` is the auctioneer-owned type the exchange
       already retains, so this adds no allocation.
     - **Ownership:** the exchange owns the meta for the slot's
       lifetime and lends it to `make_bids`; it is dropped with the
       slot. The memory charge is unchanged — already counted.
     - **Timeout handling** reads `covered_imp_ids()` off the same
       meta, so the seatnonbid path needs no separate retention.
     - **A bidder fixture MUST exercise this**, not just compile
       against it: `bidder_reads_request_context_in_make_bids()`
       asserts a fixture bidder reading `sent.headers()` and
       `sent.covered_imp_ids()` during response processing, so the
       retained context is proven live rather than assumed.
     **Owned by 2a.4a (the meta) and M3b-2b (the trait migration);
     the spec edit lands with the trait.**
     The exchange calls `finalize_for_transport` once per accepted
     call, AFTER `into_outcome()` and BEFORE `BidderCall`
     construction (it holds `BidderInfo.endpoint_compression` and the
     GPC signal right there — round-150 #6: the M3a
     `BidderCall.endpoint_compression` field is DELETED in M3b-2b, its
     only consumer having been the retired in-drive gzip); a finalize
     error on call *i* becomes that call's
     POSITIONAL `FanoutError::OversizeRequest { stage: Encoded }`
     result — the slot is recorded as Err and never dispatched, other
     calls proceed (the spec's Encoded-stage ownership is unchanged).
     **Naming reconciliation (round-108 B2): M3a's
     `prepare_outbound_request(call)` fanout-drive helper (M3a Task
     11 — gzip inside the drive) is RETIRED by this migration: its
     gzip/entity-header work moves INTO `finalize_for_transport` at
     the exchange step above, the drive sends `call.request` as-is,
     and the per-call transient raw+encoded overlap stays within the
     already-charged `max_calls x (raw + encoded)` term (all calls
     were charged concurrently regardless).** The M2 mutation sites
     (fanout.rs gzip rewrite, run.rs GPC push) are DELETED in the
     same task — read-only fields plus this one funnel replace them.
     ```rust
     pub enum OutboundBuildError { OversizeRaw { limit: u64 },
                                   MetaOverCap { limit: u64 },
                                   ReservedHeader { name: String },
                                   CallBudgetExhausted { max_calls: u32 },
                                   // Round-123 #6 — the variants the
                                   // round-119 F5 / round-121 flows
                                   // require but the enum never
                                   // carried:
                                   EndpointRejected { reason: EndpointRejectReason },
                                   MissingMethod,                   // round-148 #4 —
                                   MissingUrl,                      //   replace the
                                                                    //   stringly
                                                                    //   MissingEndpointMeta
                                   DuplicateMeta { field: MetaField }, // round-148 #4
                                   TooManyHeaders { limit: u16 },   // round-147 #4
                                   TooManyImpIds { limit: u16 },    // round-147 #4
                                   Body(BodyWriteError) }
     // Both are PER-REQUEST and non-fatal to siblings: the sink rolls
     // back exactly that request's reservation, records the error for
     // the bidder's code-5 entry, and construction continues.
     // make_requests fills the sink; PARTIAL SUCCESS is structural —
     // accepted requests and typed build errors accumulate SIDE BY
     // SIDE. Round-104 B6: the SEMANTIC error channel is preserved —
     // business failures (missing placement id, unsupported imp, …)
     // still return through Result exactly as the spec's trait
     // documents today:
     fn make_requests(&self, req: &BidRequest, info: &BidderRequestInfo,
                      sink: &mut OutboundRequestSink<'_>)
         -> Result<(), Vec<BidderError>>;
     // Round-105 B6: BidderHttpRequest fields become pub(crate) WITH
     // public read-only accessors (method()/url()/headers()/body()/
     // covered_imp_ids()) so auctioneer-exchange/auctioneer-core keep read
     // access without a construction bypass.
     // Round-108 B1 — the COMPLETE terminal API (replaces
     // into_parts(), whose two-tuple omitted the counts the 10999
     // warning needs):
     //   pub struct SinkOutcome {
     //       pub accepted: Vec<BidderHttpRequest>,
     //       pub errors: Vec<OutboundBuildError>,   // first 16
     //       pub suppressed_error_count: u32,
     //       pub rejected_call_count: u32,
     //   }
     //   impl OutboundRequestSink<'_> {
     //       /// Ok path: COMMITS every held reservation, returns
     //       /// everything the exchange needs.
     //       pub fn into_outcome(self) -> SinkOutcome { .. }
     //       /// Semantic-Err path: accepted requests are DROPPED and
     //       /// their uncommitted reservations refund via Drop (RAII
     //       /// — no cross-crate rollback call); errors + both
     //       /// counts are still returned for per-bidder reporting.
     //       pub fn into_outcome_discarding_accepted(self) -> SinkOutcome { .. }
     //   }
     // Exchange usage — NO `?` before the outcome is taken (the
     // spec's §5.2 snippet shows this exact match):
     //   match bidder.make_requests(&req, &info, &mut sink) {
     //       Ok(())        => { let out = sink.into_outcome(); .. }
     //       Err(semantic) => { let out =
     //           sink.into_outcome_discarding_accepted(); .. }
     //   }
     // Round-107 B3 — BOUNDED accumulation: the sink keeps the FIRST
     // 16 typed build errors verbatim plus a saturating
     // `suppressed_error_count` (an adapter looping push_request
     // cannot grow memory through the error list);
     // CallBudgetExhausted appears at most once in `errors`, and
     // `rejected_call_count` counts EVERY budget-rejected attempt.
     // Round-108 B1 — ONE warning message, both documents verbatim:
     // the 10999 entry's N is `rejected_call_count` and the message
     // is EXACTLY
     //   "N outbound request attempt(s) rejected by max_outbound_requests"
     // (attempt-count semantics: construction-time enforcement counts
     // rejected attempts, not truncated materialized requests; an
     // adapter that stops at its first exhaustion yields N = 1 — its
     // unexpressed further intent is unknowable by construction. The
     // spec §9.1 partial-bidder contract carries this same string;
     // the older "truncated by" wording is retired there).
     // Round-104 B6: sink-rejection accumulation is UNCONDITIONAL —
     // push_request records every rejection into the sink's error
     // list BEFORE returning it; the returned OutboundBuildError is
     // advisory (lets an adapter stop early) and CANNOT be silently
     // discarded, because the exchange reads the sink's accumulated
     // outcome, not the adapter's handling of the return value.
     // Accepted calls proceed to fanout; each OversizeRaw or
     // MetaOverCap build error becomes the per-bidder PRE-FANOUT
     // ext.errors code-5 entry (spec OversizeRequestStage::Raw
     // ownership) — the declared adapter error type never needs to
     // carry FanoutError. Round-106 B6 — CallBudgetExhausted maps
     // DIFFERENTLY: over-cap-by-COUNT is the spec's existing
     // `max_outbound_requests` partial-bidder contract, so the
     // bidder stays LIVE with the calls that fit and gets ONE
     // non-fatal `ext.warnings[<bidder>]` code-10999 entry
     // (the message above; never a code-5 ext.errors entry, never
     // seatnonbid). Build errors and the budget warning are
     // preserved SIDE BY SIDE per bidder: a bidder can carry a
     // code-5 oversize entry AND the 10999 truncation warning from
     // the same make_requests run.
     ```
     **Round-102 B5 — the ENCODED stage keeps its own bounded sink:**
     the gzip path writes through
     `GzEncoder::new(CappedWriter::new(&mut encoded, max_encoded_bytes), ..)`
     (the 2a.4a writer over a finalizer-owned bounded `Vec`; the
     unbounded `Vec` sink in the M3a gzip helper is replaced in this
     task), so compression output is bounded at write time.
     Per-adapter counting-allocator conformance tests pin both; the
     check-after remains defense-in-depth) and
     `max_outbound_request_bytes` (the final ENCODED body,
     post-serialization AND post-compression — gzip expansion on
     incompressible payloads counted). Both are
     `resource_limits.*` config/codegen fields — **round-101 B5, the
     COMPLETE file enumeration (M3b-2 config subtask):**
     `crates/auctioneer-config/src/types.rs` (`ResourceLimits` gains
     `max_outbound_raw_bytes: u64` + `max_outbound_request_bytes:
     u64`); `crates/auctioneer-config/data/host-defaults.toml` (both
     default `262144`); `crates/auctioneer-config/build_codegen.rs` (the
     `ResourceLimits` renderer emits both; validation range `1_024..=
     16_777_216` — over/under fails codegen);
     `crates/auctioneer-core/src/config.rs` (the `[auction]` overlay DTO
     gains both as `Option<u64>`, spec §9 `[auction]` table row
     updated); the runtime resolver applies the DOWNWARD-ONLY clamp
     (an overlay above the envelope value is clamped to it, round-98
     §2); tests: codegen render pair, range-rejection pair,
     overlay-clamp pair. Then -> boot envelope ->
     enforced in `finalize_for_transport` (round-108 B2 — was
     `prepare_outbound_request`, retired); either over-cap yields
     the per-call `FanoutError::OversizeRequest` -> per-bidder
     `ext.errors[bidder]` code 5 (`FailedToRequestBids` family; the
     spec's FanoutError enum, resource-limits table, and Appendix G
     code-5 row all carry the variant — round-100 H6). The per-auction
     formula charges the TRANSIENT overlap:
     `max_calls x (max_outbound_raw_bytes + max_outbound_request_bytes)`
     replaces the single outbound term. Tests: raw over-cap rejected
     pre-encoding; encoded over-cap rejected; exact-cap accepted;
     compressed-larger-than-raw edge counted.
   - `max_raw_blob_bytes` is DEFINED: the largest GVL manifest entry,
     emitted by build.rs as a constant (§2).
   - Retained-GVL term: `max_concurrent_auctions x max_parsed_gvl_bytes`
     (`max_raw_blob_bytes x GVL_PARSE_EXPANSION_FACTOR`).

7. **`ResolvedIsolateLimits` — normalized SOURCES + checked DERIVED
   totals (round-98 B2 + B4).** Aggregate byte totals cannot be a
   source of truth (consumers cannot recover caps from sums). The
   installed type carries the sources; totals are derived + checked:
   ```rust
   // auctioneer-runtime/src/budget.rs — pure, wasm-clean
   pub struct ResolvedIsolateLimits {
       // ---- normalized SOURCE limits (consumers read these) ----
       pub isolate_total_bytes: u64,
       pub baseline_bytes: u64,            // §8: ceil(max-of-30 measured
                                           // whole-isolate baseline x 6/5) —
                                           // round-107 B1: the 20% margin is
                                           // INSIDE this pin; validation uses
                                           // it AS IS (no further budgeting).
                                           // INCLUDES runtime/JS heap AND the
                                           // embedded raw archive (no separate
                                           // archive term — round-98 H8)
       pub inbound_request_bytes: u64,     // auction body cap (256 KiB)
       pub max_outbound_request_bytes: u64,
       // Round-99 B5: BOTH live caps restored as sources — the
       // round-98 single `per_call_response_bytes` collapsed the
       // two-cap contract this same plan requires:
       pub max_transport_bytes: u64,       // compressed on-the-wire cap
       pub max_decoded_bytes: u64,         // post-decode cap
       // Round-100 B3: NO `gcl_parsed_bytes` field — the `Gcl` static
       // is boot-initialized BEFORE baseline measurement, so its heap
       // is inside `baseline_bytes` by definition (a separate term
       // would double-count).
       pub final_response_bytes: u64,      // HARD cap on the serialized
                                           // response = the EGRESS-buffer
                                           // bound (charged ONCE, as
                                           // egress_permits x this, in
                                           // TOTAL). Round-106 B5: the
                                           // in-auction assembly SCRATCH
                                           // is the DERIVED max_calls x
                                           // max_decoded x ASSEMBLY_FACTOR
                                           // term inside per_auction_peak;
                                           // validation enforces
                                           // final_response_bytes >= that
                                           // derived term (2_097_152 >=
                                           // 1_572_864 in the CF default)
                                           // as a SIZING FLOOR — round-108
                                           // H4: NOT an output-size proof
                                           // (JSON escaping can expand
                                           // wire bytes); the enforced
                                           // bounded serializer is the
                                           // wire bound, over-cap = 500.
       pub max_outbound_raw_bytes: u64,    // round-101 B4: pre-encoding
                                           //   construction cap (CappedWriter)
       pub egress_permits: u32,            // round-101 B4: egress-gate
                                           //   capacity (§5)
       pub response_write_deadline_ms: u64,// round-109 H3: the §5
                                           //   ResponseWriteDeadline
                                           //   duration — a TIME source
                                           //   (validation range-checks
                                           //   100..=120_000 only; no
                                           //   memory term); egress
                                           //   phase reads it here
       pub guest_decoder_bound_bytes: u64, // round-134 #4 — REPLACES the
                                           //   round-102/103 `decode_mode:
                                           //   DecodeMode` field: one mode
                                           //   cannot describe an adapter
                                           //   that guest-decodes gzip
                                           //   while auctioneer decodes
                                           //   deflate/stacked (round-133
                                           //   #6). The envelope stores
                                           //   the adapter-reported guest
                                           //   decoder bound (0 where the
                                           //   adapter never decodes
                                           //   in-guest); the per-stream
                                           //   decoder charge is the
                                           //   CHECKED
                                           //   max(auctioneer_chain(wbits,
                                           //   layers), this field).
                                           //   `DecodeMode` and the
                                           //   per-adapter
                                           //   `decode_mode_of` mappings
                                           //   are RETIRED from the
                                           //   envelope — `BodySource`
                                           //   remains the static
                                           //   capability description
                                           //   only.
       pub max_aliases: u32,               // round-102 H11 / round-103 H7 —
                                           //   ONE semantic: an [aliases]
                                           //   overlay with MORE entries than
                                           //   max_aliases is REJECTED at
                                           //   config validation (fail-loud;
                                           //   "clamp" would silently drop
                                           //   aliases — never). Feeds the
                                           //   /info/bidders/all aggregate
                                           //   bound. Dataflow: config
                                           //   types.rs + host-defaults.toml
                                           //   (64) + codegen range 0..=1024
                                           //   + validation-reject tests +
                                           //   the spec §9 runtime-config
                                           //   table row (round-103 H7) —
                                           //   same file list as the outbound
                                           //   caps (round-101 B5).
       pub aux_route_permits: u32,         // round-100 B3/B5: aux gate
       pub aux_route_peak_bytes: u64,      //   config blob x parse factor
                                           //   + info-response assembly/
                                           //   egress bound
       // (round-135 #2: the round-100 B3 `guest_decoder_bound_bytes`
       // declaration that stood here is MERGED into the single
       // round-134 #4 field above — one field, one struct.)
       pub max_brotli_wbits: u8,
       pub max_brotli_layers: u8,
       pub gvl_cache_entries: u32,
       pub max_raw_blob_bytes: u64,        // build.rs constant (largest manifest entry)
       pub max_bidders_per_auction: u32,   // 50 baked default; round-117
                                           // P1-9: per-target HARD CAP
                                           // clamps this (see below)
       pub max_outbound_requests: u32,     // 100 == max_calls; same clamp
       pub max_concurrent_auctions: u32,   // AdmissionGate capacity
       pub max_active_body_streams: u32,   // StreamGate capacity
       // ---- fields added by the round-117..123 amendments; every
       // term the TOTAL formula charges has its limit HERE, or the
       // formula is charging air (round-123 #2). Defaults are the
       // amendment values; the CF table re-derivation re-chooses
       // them per target. ----
       pub max_runtime_config_bytes: u64,     // 1_048_576; pre-deser envelope cap
       pub max_parsed_nodes: u32,             // JSON node budget — round-128
                                              // #4 renames max_parsed_nodes:
                                              // the COUNT IS SYNTACTIC, not
                                              // retention-dependent, because a
                                              // generic serde Deserializer
                                              // cannot know what an arbitrary
                                              // visitor retains. NODE: every
                                              // syntactic JSON value (scalar,
                                              // string, array, object) AND
                                              // every object key, SKIPPED
                                              // FIELDS INCLUDED, counted as
                                              // parsed — a wrapping
                                              // Deserializer counts these
                                              // without visitor knowledge,
                                              // which is what makes the
                                              // generic form implementable.
                                              // API: `bounded_json::
                                              // from_slice_within::<T>(bytes,
                                              // max_nodes)` (generic;
                                              // `value_within` is the
                                              // T = Value specialization,
                                              // nothing more). ERROR DOMAIN
                                              // (round-131 #5 — it must
                                              // carry ordinary JSON errors
                                              // too, or no caller can adopt
                                              // it):
                                              // pub struct
                                              //   NodeBudgetExceeded {
                                              //     pub budget: u32 }
                                              // (a REAL standalone type —
                                              // round-132 #4: it is also
                                              // the prelookup's error, so
                                              // it cannot be only an enum
                                              // variant)
                                              // enum BoundedJsonError {
                                              //   Syntax(serde_json::Error),
                                              //   NodeBudget(
                                              //     NodeBudgetExceeded),
                                              // }
                                              // WIRE BODY, exact
                                              // (round-132 #4): BadInput
                                              // carries ONLY the message
                                              // "too many JSON nodes"; the
                                              // handler's existing wrapper
                                              // (auction.rs) supplies the
                                              // "Invalid request: " prefix
                                              // and trailing \n, producing
                                              // the canonical
                                              // `Invalid request: too many
                                              // JSON nodes\n` — no new
                                              // wrapper, no double prefix;
                                              // pinned by
                                              // node_budget_wire_body_
                                              // exact_prefix_and_newline().
                                              // Syntax maps wherever the
                                              // replaced serde_json call's
                                              // error went (the existing
                                              // Invalid-request 400 at the
                                              // typed parse; deferral at
                                              // the prelookup). Precedence:
                                              // after the
                                              // byte-size cap, before typed
                                              // validation. CHARGE: the
                                              // equation's max_parsed_nodes x
                                              // NODE_RETAINED_BYTES (64) term
                                              // bounds RETAINED Value memory
                                              // — retained <= parsed by
                                              // construction, and 64 B/node
                                              // (32 B Value + 32 B amortized
                                              // map-bucket / vec-header /
                                              // key-String-header overhead)
                                              // is a claim about
                                              // serde_json::Value
                                              // materialization ONLY, pinned
                                              // by `parsed_node_charge_guard()`
                                              // over FOUR
                                              // adversarial shapes
                                              // (small-key map flood, deep
                                              // array nesting at the depth
                                              // cap, many-small-strings
                                              // flood, empty-container
                                              // flood), each allocator-
                                              // measured; key/string
                                              // CONTENTS are input copies
                                              // bounded by the inbound
                                              // parse-factor term. Typed
                                              // T != Value deserialization
                                              // is bounded by its own struct
                                              // caps — for it the node
                                              // budget bounds parser WORK,
                                              // and only its unknown-field
                                              // Value captures charge the
                                              // 64 B term. Users: the typed
                                              // request parse AND the
                                              // account-id pre-lookup — the
                                              // full serde_json::Value
                                              // materialization in
                                              // config/resolve.rs is a
                                              // listed migration site —
                                              // WITH its signature
                                              // migration (round-130 #5):
                                              // today's
                                              // account_id_from_body(&[u8])
                                              // -> Option<String> swallows
                                              // every JSON error (.ok()?),
                                              // leaving NO channel for the
                                              // canonical 400. It becomes
                                              // account_id_from_body(
                                              //   &[u8], max_nodes: u32)
                                              // -> Result<Option<String>,
                                              //           NodeBudgetExceeded>
                                              // (internally value_within;
                                              // BoundedJsonError::Syntax ⇒
                                              // Ok(None), deferred to typed
                                              // parsing exactly as today;
                                              // NodeBudgetExceeded ⇒ Err).
                                              // THE FULL CALLER CHAIN
                                              // (round-131 #5):
                                              // resolve_account(raw_body,
                                              //   app, default, max_nodes)
                                              // -> Result<
                                              //      ResolvedAccountOwned,
                                              //      AuctionError>
                                              // keeps its error type and
                                              // propagates the budget
                                              // breach as AuctionError::
                                              // BadInput(canonical node
                                              // body) BEFORE the disabled
                                              // check can conclude; the
                                              // handler's separate typed
                                              // parse (auction.rs)
                                              // migrates from
                                              // serde_json::from_slice to
                                              // from_slice_within::<_>,
                                              // mapping Syntax to its
                                              // existing Invalid-request
                                              // 400 and NodeBudgetExceeded
                                              // to the same canonical node
                                              // body.
                                              // PRECEDENCE IS EXPLICIT: the
                                              // budget 400 outranks
                                              // account-state responses —
                                              // over-budget input naming a
                                              // DISABLED account yields the
                                              // 400, never the 503,
                                              // because the prelookup runs
                                              // before account resolution
                                              // can conclude.
                                              // IMPLEMENTATION CONSTRAINT
                                              // (round-129 #6): the pinned
                                              // serde_json consumes an
                                              // ENTIRE skipped subtree
                                              // through one
                                              // deserialize_ignored_any call
                                              // (de.rs ignore_value), so a
                                              // wrapper that merely
                                              // delegates it counts that
                                              // subtree as ONE node. The
                                              // implementation must
                                              // intercept ignored input —
                                              // its own counting
                                              // ignored-value walker in
                                              // place of IgnoredAny, or a
                                              // separate bounded token pass
                                              // — and COUNTING SEMANTICS
                                              // are pinned by their own
                                              // behavior tests (§9), not
                                              // only by the allocation
                                              // guard.
       pub max_copied_header_bytes: u32,      // 4_096; UA+Referer copies at prepare()
       pub max_admitted_header_bytes: u32,    // 16_384; whole inbound header block
       pub max_admitted_header_count: u16,    // 128; both enforced at the ingress
                                              // seam pre-routing -> 431 on breach
       pub max_impressions_per_auction: u16,  // 100; round-123 #7 — replaces the
                                              // sink's ad-hoc 32-id table cap;
                                              // validated at prepare(), so
                                              // covered_imp_ids sizes to THIS
       pub terminal_response_permits: u32,        // 68 = max_concurrent_auctions + aux_route_permits
       pub refusal_body_bytes: u32,           // 256 — the ONE static-permit buffer
                                              // size (the earlier 128 draft is
                                              // superseded; see §5)
       pub transmit_state_bytes: u32,         // Prereq-9 measured per-response const
       pub max_ingress_read_ms: u32,          // Prereq-10 drain deadline
       pub max_config_fetch_ms: u32,          // loader deadline (M3b-1b)
       pub refusal_write_deadline_ms: u32,    // 2_000; static-class write budget
   }

   // Round-124 #3 — TARGET_PLATFORM_CONSTANTS: per-target facts the
   // formula charges that are NOT operator-tunable limits. One row
   // per shipped target; every value CITED to platform documentation
   // (or measured, with the measuring commit recorded). The CF
   // re-derivation fills the numbers; the formula may not reference
   // an input that has no row here or field above.
   // Round-125 #3: consumed by validation —
   // `validate_isolate_limits(limits: &ResolvedIsolateLimits,
   // target: &TargetPlatformConstants)` takes BOTH; every
   // formula input lives in one of the two structs.
   // Round-126 #3 — the FIRST numeric row (axum), with its
   // implementation owner. These are REQUIRED serve-config values,
   // owned by auctioneer-adapter-axum's server builder (it sets hyper's
   // http2_max_concurrent_streams and wraps accept in a semaphore;
   // a deploy overriding them re-runs validation with its values):
   //   AXUM_ROW = TargetPlatformConstants {
   //       platform_concurrent_ceiling: 384,     // 96 connections
   //                                             // (accept semaphore)
   //                                             // x 4 h2 streams —
   //                                             // round-128 #1 lowered
   //                                             // 1_024 → 512; round-129
   //                                             // #1's honest parser-
   //                                             // derived entry charge
   //                                             // (676 x 64/slot) needs
   //                                             // 384 to keep the 512 MB
   //                                             // envelope
   //       header_entry_overhead_bytes: 64,      // measured HeaderMap
   //                                             // per-entry cost
   //       parser_header_ceiling_bytes: 21_632,  // 16_384 + (128+4)x32
   //                                             // + 1_024 (h2 field-
   //                                             // section accounting).
   //                                             // ENFORCING KNOB =
   //                                             // 21_633 — h2 rejects at
   //                                             // headers_size >= max
   //                                             // (EXCLUSIVE, round-115
   //                                             // B1), so the knob sits
   //                                             // ONE above; admitted
   //                                             // <= 21_632
   //       parser_max_header_entries: 676,       // DERIVED: floor(21_632
   //                                             // / 32) — no h2 count
   //                                             // knob exists
   //                                             // (round-129 #1)
   //       parser_header_knob_bytes: 21_633,     // configured; validation
   //                                             // asserts ceiling + 1
   //       max_request_target_bytes: 768,        // 768 + 192 <= the 1_024
   //                                             // allowance (checked)
   //       h1_max_buf_size_bytes: 24_576,        // >= 16_384 + 768 +
   //                                             // 1_024 (checked)
   //       h1_max_headers: 128,                  // == admitted count
   //                                             // (checked)
   //       pseudoheader_allowance_bytes: 1_024,
   //       parser_transient_bytes: 16_384,       // hpack 4_096 + scratch;
   //                                             // MEASURED-PENDING
   //       egress_handoff_factor: 2,             // no zero-copy proof
   //                                             // yet
   //   }
   // CF/Fastly/Spin rows land with the measurement re-derivation
   // (their ceilings are platform-documented, cite required).
   pub struct TargetPlatformConstants {
       pub platform_concurrent_ceiling: u32,  // max concurrently-live
                                              // requests the PLATFORM
                                              // admits per isolate/
                                              // process BEFORE any
                                              // auctioneer gate. CF:
                                              // documented isolate
                                              // concurrency (cite
                                              // required at
                                              // re-derivation). Axum
                                              // (round-125 #3): a
                                              // listener bound does
                                              // NOT bound h2 streams
                                              // or parsed requests —
                                              // the value is the
                                              // REQUIRED serve
                                              // config: hyper
                                              // max_concurrent_streams
                                              // x connection cap via
                                              // an accept-side
                                              // semaphore, both set
                                              // explicitly; an axum
                                              // deploy without them
                                              // has NO ceiling and
                                              // cannot validate.
       pub header_entry_overhead_bytes: u32,  // measured per-entry map cost
       pub parser_header_ceiling_bytes: u32,  // round-128 #2 / round-130
                                              // #6: the ADMITTED ceiling —
                                              // the largest field section
                                              // accepted (field-section
                                              // accounting, pseudoheaders
                                              // included; h1: max_buf_size
                                              // where larger); derived per
                                              // target from the admitted
                                              // caps (INBOUND_H2_MAX_FIELD_
                                              // SECTION above). This is the
                                              // CHARGED bound. A target
                                              // without an enforcing knob
                                              // charges its platform-
                                              // documented ceiling (cite
                                              // required) or cannot validate.
       pub parser_header_knob_bytes: u32,     // round-130 #6: the value
                                              // actually CONFIGURED into
                                              // `http2_max_header_list_size`
                                              // — a SEPARATE field so the
                                              // exclusive-boundary +1 can
                                              // never be silently dropped:
                                              // validation asserts
                                              // knob == ceiling + 1 on h2
                                              // (h2 rejects at
                                              // headers_size >= max).
       pub max_request_target_bytes: u32,     // round-130 #6: the
                                              // request-target (URI) cap —
                                              // the pseudoheader allowance
                                              // was leaning on a
                                              // NONEXISTENT URI cap; this
                                              // field makes it real.
                                              // NORMALIZED per protocol
                                              // (round-131 #6): ONE budget
                                              // value, counted as the h1
                                              // request-target's bytes on
                                              // h1 and as
                                              // len(:path) + len(:authority)
                                              // on h2. Round-132 #5 — the
                                              // two accountings CAN reject
                                              // the same URL differently
                                              // (h2 always counts the
                                              // authority; h1 counts it
                                              // only in absolute-form
                                              // targets): ACCEPTED and
                                              // documented — each
                                              // protocol's boundary is
                                              // pinned by its own M3b-10
                                              // test, and no cross-
                                              // protocol equivalence is
                                              // claimed. Enforced at
                                              // ingress: h1 — the
                                              // connection driver checks
                                              // the request-target length
                                              // during head parse
                                              // (max_buf_size bounds the
                                              // whole head, not the
                                              // target); h2 — :path +
                                              // :authority length at
                                              // header decode. Breach ⇒
                                              // 414 (`uri too long\n`) via
                                              // the static class on h1,
                                              // stream reset on h2 —
                                              // Appendix F row added.
       pub h1_max_buf_size_bytes: u32,        // round-131 #6: the h1 head
                                              // buffer knob (hyper
                                              // max_buf_size) gets its own
                                              // VALIDATED field — it was
                                              // named by the ingress
                                              // contract but had no row.
                                              // Validation asserts
                                              // h1_max_buf_size_bytes >=
                                              // max_admitted_header_bytes
                                              // + max_request_target_bytes
                                              // + 1_024 head slack, so a
                                              // policy-valid h1 head can
                                              // never be parser-rejected.
       pub h1_max_headers: u32,               // round-131 #6: the h1 count
                                              // knob (hyper http1_max_
                                              // headers) as a validated
                                              // field — hyper defaults to
                                              // 100, below the 128 auctioneer
                                              // admits; validation asserts
                                              // h1_max_headers ==
                                              // max_admitted_header_count.
       pub parser_max_header_entries: u32,    // round-129 #1: DERIVED, not
                                              // configured — h2 exposes NO
                                              // header-COUNT knob (the same
                                              // reality the outbound
                                              // round-112 B2 layer pins), so
                                              // the parser-time entry
                                              // maximum is
                                              // floor(parser_header_ceiling_
                                              // bytes / 32): ~676 tiny
                                              // fields fit a 21_632 section
                                              // before the LOGICAL 128-count
                                              // check can run (that check is
                                              // post-materialization
                                              // defense-in-depth in
                                              // decide()). This derived
                                              // value — never
                                              // max_admitted_header_count —
                                              // is what the equation's
                                              // pre-admission map-overhead
                                              // charge uses; validation
                                              // recomputes and asserts it.
       pub pseudoheader_allowance_bytes: u32, // :method/:scheme/:authority/
                                              // :path byte allowance (1_024
                                              // default). Round-130 #6:
                                              // DERIVED-CHECKED, not hoped —
                                              // validation asserts
                                              // allowance >=
                                              // max_request_target_bytes
                                              // + 192. Round-131 #6 makes
                                              // the 192 arithmetic real:
                                              // the four pseudo names
                                              // (≈32 B) + :method value
                                              // <= 16 B + :scheme value
                                              // <= 8 B (both enforced at
                                              // header decode, over ⇒
                                              // stream reset) + 4 x 32
                                              // per-field overhead = 184
                                              // <= 192; and EXTENDED
                                              // CONNECT (:protocol, RFC
                                              // 8441 — a FIFTH
                                              // pseudoheader) is REJECTED
                                              // at decode on auctioneer
                                              // routes, so the
                                              // four-pseudo derivation
                                              // stands.
       pub parser_transient_bytes: u32,       // hpack dynamic table + decode
                                              // scratch; measured per target
       pub egress_handoff_factor: u32,        // round-121 #5: 1 ONLY
                                              // with verified owned
                                              // zero-copy handoff for
                                              // that adapter (and the
                                              // destination built via
                                              // with_capacity — a
                                              // growing Vec voids the
                                              // proof); else 2, or the
                                              // measured value.
                                              // Applies to the AUX
                                              // egress term too.
   }
   /// DERIVED, all checked arithmetic (overflow ⇒ Err). Round-99 B5:
   /// per_stream_peak comes from the CANONICAL per-stream charge
   /// helper (transport staging + decoded body + decoder state — the
   /// same fn the admission weights use; never recomputed ad hoc):
   ///   per_stream_peak  = charge(max_transport_bytes, max_decoded_bytes,
   ///                             decoder(wbits, layers))
   ///                      + HEADER_STREAM_CHARGE           // rounds 110-113: retained
   ///                                                       // initial map (12_288) +
   ///                                                       // a TRAILER block's
   ///                                                       // parser-worst transient
   ///                                                       // (10_336 + 313 x 64 =
   ///                                                       // 30_368) CONCURRENT with
   ///                                                       // it = 42_656 (round-113
   ///                                                       // H1 — dominates both the
   ///                                                       // initial-parse transient
   ///                                                       // 30_368 alone and the
   ///                                                       // post-check live+copy
   ///                                                       // 24_576)
   /// Round-99 B6 — the per-auction terms are COMPOSITIONAL, each with
   /// its OWN measured factor (a single inbound-scaled multiplier could
   /// not track response-side scaling — raising max_decoded_bytes must
   /// raise the reservation):
   ///   per_auction_peak =
   ///       runtime_config_term                          // round-117 P0-2: the fetched
   ///                                                    // envelope + deserialized
   ///                                                    // AuctioneerAppConfig were previously
   ///                                                    // UNACCOUNTED. Round-183 #5
   ///                                                    // RE-BASES this onto upstream's
   ///                                                    // extraction limits. Term =
   ///                                                    //   max_total_bytes
   ///                                                    //   + max_blob_bytes
   ///                                                    //     x CONFIG_PARSE_FACTOR
   ///                                                    // max_total_bytes is upstream's
   ///                                                    // RAW pre-parse budget (it is
   ///                                                    // charged before the value is
   ///                                                    // retained), so it cannot stand
   ///                                                    // in for deserialization; only
   ///                                                    // the root blob is parsed into
   ///                                                    // AuctioneerAppConfig, so the factor
   ///                                                    // scales max_blob_bytes.
   ///                                                    //   — round-126 #11:
   ///                                                    //   CONFIG_PARSE_FACTOR = 6,
   ///                                                    //   guarded like every factor
   ///                                                    //   (config_parse_factor_guard()
   ///                                                    //   joins the authoritative
   ///                                                    //   roster: adversarial TOML,
   ///                                                    //   max maps at their caps).
   ///                                                    // max_runtime_config_bytes is a
   ///                                                    // NEW pre-deserialization cap on
   ///                                                    // the raw envelope (default
   ///                                                    // 1 MiB, boot-envelope field,
   ///                                                    // enforced by the store read
   ///                                                    // before parsing); the factor is
   ///                                                    // measured with its own guard
   ///                                                    // test like every factor below.
   ///                                                    // Boundedness of the parsed form
   ///                                                    // additionally requires the
   ///                                                    // round-117 field caps:
   ///                                                    // status_response <= 4096 bytes,
   ///                                                    // bidders map <= registry size +
   ///                                                    // 64 alias slots, accounts map
   ///                                                    // <= 4096 entries, aliases map
   ///                                                    // <= max_aliases (the envelope
   ///                                                    // field, default 64 — round-126
   ///                                                    // #11: the flat "256" this line
   ///                                                    // once carried disagreed with
   ///                                                    // the field and is retired; ONE
   ///                                                    // limit, the field's)
   ///                                                    // — all rejected by
   ///                                                    // validate(), so a pushed blob
   ///                                                    // cannot widen the term.
   ///     + inbound                                      // raw inbound body (round-104 B7)
   ///     + admitted_header_bytes                        // round-121 #4: the FULL
   ///                                                    // inbound header block
   ///                                                    // (originals, not just the
   ///                                                    // copied pair) lives in
   ///                                                    // memory from ingress
   ///                                                    // through config lookup and
   ///                                                    // normalization — bounded by
   ///                                                    // an admitted-header
   ///                                                    // byte+count cap enforced at
   ///                                                    // the ingress seam (or, where
   ///                                                    // the platform caps it,
   ///                                                    // charge THAT documented
   ///                                                    // ceiling), charged for its
   ///                                                    // full lifetime. Truncation
   ///                                                    // applies ONLY to the copied
   ///                                                    // advisory pair — protocol
   ///                                                    // and privacy-relevant
   ///                                                    // headers are never
   ///                                                    // truncated, over-cap blocks
   ///                                                    // are REJECTED.
   ///     + header_copy_bytes x (1 + max_bidders)        // round-119 F2: normalization
   ///                                                    // copies User-Agent/Referer
   ///                                                    // into the typed request and
   ///                                                    // every bidder clone carries
   ///                                                    // them — body-scaled factors
   ///                                                    // never saw this input. The
   ///                                                    // term is bounded by NEW
   ///                                                    // ingress header caps
   ///                                                    // (max_copied_header_bytes,
   ///                                                    // envelope const, default
   ///                                                    // 4_096 across the copied
   ///                                                    // set; over-cap headers are
   ///                                                    // TRUNCATED at prepare() —
   ///                                                    // they are advisory metadata,
   ///                                                    // not protocol state), and
   ///                                                    // allocator fixtures must
   ///                                                    // drive normalization with
   ///                                                    // max-cap UA/Referer plus a
   ///                                                    // near-cap structural body
   ///                                                    // and full survivor set.
   ///     + inbound x INBOUND_PARSE_FACTOR
   ///       + max_parsed_nodes x 64                      // typed req + serde graph
   ///                                                    // (NODE_RETAINED_BYTES) —
   ///                                                    // the syntactic parser-time
   ///                                                    // node budget (bounded_json,
   ///                                                    // round-128 #4), charged
   ///                                                    // HERE in the equation, not
   ///                                                    // only described at the
   ///                                                    // field
   ///     + inbound x GROUPING_FACTOR                    // round-108 B3: grouping +
   ///                                                    // resolved-alias bookkeeping
   ///                                                    // at input-derived worst-case
   ///                                                    // cardinality (~13k names)
   ///     + max_bidders x inbound x TYPED_CLONE_FACTOR   // bidder-scoped clones are
   ///                                                    // TYPED objects, not raw bytes.
   ///                                                    // Round-106 B4: a TRUE bound
   ///                                                    // only because M3b-2
   ///                                                    // restructures the split into
   ///                                                    // group-then-materialize —
   ///                                                    // clones exist ONLY for the
   ///                                                    // post-cap survivor set (see
   ///                                                    // the split restructure below)
   ///     + max_calls x (max_outbound_raw_bytes
   ///                    + max_outbound_request_bytes)   // transient raw+encoded
   ///                                                    // outbound bodies (round-100 H6)
   ///     + max_calls x max_decoded_bytes                // retained positional bodies
   ///     + max_calls x HEADER_MAP_CHARGE                // round-110 H5 / 111 H4:
   ///                                                    // retained response headers
   ///                                                    // (12_288 each, incl. entry
   ///                                                    // overhead) until make_bids
   ///     + max_calls x max_decoded_bytes x RESPONSE_PARSE_FACTOR // parsed bids +
   ///                                                             // creative markup
   ///     + max_calls x OUTBOUND_META_BYTES              // request meta (round-104 B7)
   ///     + max_calls x OUTBOUND_HEADER_MAP_CHARGE       // round-150 #5: the platform
   ///                                                    // `http::HeaderMap` the transport
   ///                                                    // conversion fills from the byte pool
   ///                                                    // (<= 36 entries -> capacity 64:
   ///                                                    // 3_584 name+value bytes + 64 x
   ///                                                    // HEADER_ENTRY_OVERHEAD_BYTES =
   ///                                                    // 7_680), retained by the
   ///                                                    // OutboundRequest until its slot
   ///                                                    // completes. It OVERLAPS the
   ///                                                    // byte pool (OUTBOUND_META_BYTES stays
   ///                                                    // alive for the response phase),
   ///                                                    // which is why BOTH are charged
   ///     + max_calls x max_decoded_bytes
   ///               x ASSEMBLY_FACTOR                    // assembly/serialization
   ///                                                    // SCRATCH (round-106 B5:
   ///                                                    // the DERIVED term, not the
   ///                                                    // final_response_bytes field
   ///                                                    // — that field bounds the
   ///                                                    // EGRESS buffer and appears
   ///                                                    // ONCE, in TOTAL below;
   ///                                                    // validation requires
   ///                                                    // final_response_bytes >=
   ///                                                    // this term so the hard
   ///                                                    // response cap can never
   ///                                                    // truncate a within-budget
   ///                                                    // assembly)
   ///   gvl_terms  = cache_entries x parsed + concurrent x parsed
   ///   TOTAL: baseline                              // incl. boot-init Gcl static
   ///        + gvl_terms
   ///        + max_concurrent_auctions x per_auction_peak   // incl. the assembly
   ///                                                       // SCRATCH term above
   ///        + egress_permits
   ///          x (final_response_bytes x EGRESS_HANDOFF_FACTOR
   ///             + transmit_state_bytes)                    // round-124 #4: the
   ///                                                        // per-response transmit
   ///                                                        // state is charged on
   ///                                                        // AUCTION permits exactly
   ///                                                        // as on static ones.
   ///                                                        // round-121 #5: Axum/Spin
   ///                                                        // response conversion can
   ///                                                        // hold source AND
   ///                                                        // destination buffers
   ///                                                        // simultaneously; the
   ///                                                        // factor is 1 only where
   ///                                                        // an owned zero-copy
   ///                                                        // handoff is verified per
   ///                                                        // adapter, else 2 (or the
   ///                                                        // measured value), with
   ///                                                        // the ownership/drop
   ///                                                        // invariant stated —
   ///                                                        // unspecified headroom is
   ///                                                        // not proof.
   ///                                                        // §5: the EGRESS gate —
   ///                                                       // a REAL counter (permits
   ///                                                       // held until write-complete/
   ///                                                       // disconnect via Prereq 9),
   ///                                                       // NOT concurrent-auctions
   ///                                                       // (round-100 B4: drained
   ///                                                       // responses outlive permits)
   ///        + aux_route_permits
   ///          x (aux_route_peak_bytes x EGRESS_HANDOFF_FACTOR
   ///             + transmit_state_bytes)                    // §7b (round-124 #4: aux
   ///                                                        // egress pays handoff +
   ///                                                        // transmit state too)
   ///        + terminal_response_permits
   ///          x (refusal_body_bytes x EGRESS_HANDOFF_FACTOR
   ///             + transmit_state_bytes)                    // round-125 #12: the
   ///                                                        // 256 B render buffer
   ///                                                        // and the adapter's copy
   ///                                                        // can coexist, exactly
   ///                                                        // as on auction egress
   ///                                                        // §5 static class —
   ///                                                        // round-123 #3: was
   ///                                                        // introduced but never
   ///                                                        // charged in THIS
   ///                                                        // authoritative formula
   ///        + max(H2_SLOT_PEAK, H1_SLOT_PEAK)              // round-132 #5: the
   ///                                                        // validator computes BOTH
   ///                                                        // protocol peaks and
   ///                                                        // charges their MAX — the
   ///                                                        // round-131 formula
   ///                                                        // substituted the h2
   ///                                                        // ceiling everywhere and
   ///                                                        // was unsound for a target
   ///                                                        // where h1 dominates.
   ///          where
   ///            H2_SLOT_PEAK =
   ///              PARSER_HEADER_CEILING_BYTES                // encoded field section:
   ///                                                        // INBOUND_H2_MAX_FIELD_
   ///                                                        // SECTION =
   ///                                                        // max_admitted_header_bytes
   ///                                                        // + (count + 4 pseudo) x 32
   ///                                                        // + PSEUDOHEADER_ALLOWANCE
   ///            + PARSER_TRANSIENT_BYTES                     // hpack table + decode
   ///                                                        // scratch (measured)
   ///            + PARSER_HEADER_CEILING_BYTES                // materialized map bytes,
   ///            + PARSER_MAX_HEADER_ENTRIES                  // up to floor(ceiling/32)
   ///              x HEADER_ENTRY_OVERHEAD_BYTES              // entries (no h2 count
   ///                                                        // knob — round-129 #1)
   ///            H1_SLOT_PEAK =
   ///              H1_MAX_BUF_SIZE_BYTES                      // the whole h1 head sits
   ///                                                        // in hyper's read buffer
   ///            + max_admitted_header_bytes                  // materialized map bytes
   ///            + max_admitted_header_count                  // (h1 HAS a count knob —
   ///              x HEADER_ENTRY_OVERHEAD_BYTES              // h1_max_headers — so the
   ///                                                        // logical count IS the
   ///                                                        // parser-time entry bound)
   ///                                                        // max_admitted_header_count
   ///                                                        // otherwise governs only
   ///                                                        // decide()'s logical check
   ///                                                        // and the retained
   ///                                                        // per-auction charge
   ///          x PLATFORM_CONCURRENT_CEILING                 // round-123 #8: headers
   ///                                                        // materialize BEFORE
   ///                                                        // admission; the
   ///                                                        // pre-admission
   ///                                                        // population is bounded
   ///                                                        // by the platform's own
   ///                                                        // concurrency ceiling
   ///                                                        // (named + cited per
   ///                                                        // target), not by our
   ///                                                        // gate
   ///        + max_active_body_streams x per_stream_peak <= isolate_total
   pub fn validate_isolate_limits(
       l: &ResolvedIsolateLimits,
       t: &TargetPlatformConstants,   // round-126 #3: the canonical
                                      // signature takes BOTH structs
                                      // (the round-125 comment said
                                      // so; the signature now agrees)
       outbound_meta_bytes: u32,      // round-162 #3: THE ONE bidder-
                                      // derived input. auctioneer-runtime
                                      // owns every RUNTIME constant
                                      // internally (NODE_RETAINED_BYTES,
                                      // the H1/H2 slot peaks, the
                                      // factors) and must not accept
                                      // them from callers; the ONLY
                                      // value it cannot compute is the
                                      // per-call outbound metadata
                                      // charge, because auctioneer-bidder
                                      // owns the byte pool layout. It
                                      // arrives PRE-COMPUTED as a
                                      // single u32 — byte pool + offset
                                      // table + struct overhead —
                                      // from `auctioneer_bidder::
                                      // outbound_meta_bytes(max_imps)`,
                                      // so no layout constant is
                                      // duplicated or re-derived here.
   ) -> Result<Derived, BudgetError>
   ```
   Each factor (`INBOUND_PARSE_FACTOR`, `GROUPING_FACTOR`,
   `TYPED_CLONE_FACTOR`, `RESPONSE_PARSE_FACTOR`, `ASSEMBLY_FACTOR`)
   is INDEPENDENTLY pinned
   by its own allocator guard test over the dimension it scales with
   (round-99 B6; the round-98 single `REQUEST_PIPELINE_EXPANSION_FACTOR`
   is retired). The `Gcl` static is boot-initialized BEFORE baseline
   measurement — so its heap lives INSIDE `baseline_bytes`; round-100 B3
   removed the separate `gcl_parsed_bytes` term (double-count). **Round-106 B4 — the split restructure that makes the clone term a
   TRUE bound (REPLACES the round-99 "structural, clones per bidder
   today" note, which under-counted):** today's `split_by_bidder`
   (run.rs:59 / split.rs:40) fully materializes a typed
   `PerBidderRequest` (a whole-request clone) for EVERY participating
   bidder BEFORE the privacy filter, sampling, and
   `apply_max_bidders` run — a request naming 30 registered bidders
   would hold 30 typed clones concurrently while the budget charges
   `max_bidders (8) x inbound`. M3b-2 therefore splits the function in
   two. Round-107 B4 refines the round-106 shape into a THREE-stage
   pipeline — DECIDE → MATERIALIZE → SCRUB — because M3a's privacy
   stage is not a pure name filter: it both DECIDES per-bidder
   permissions AND MUTATES the surviving typed requests
   (`scrub_gdpr_id` / `scrub_geo_and_device_ip`, M3a step 6), and the
   scrub can only run on a materialized clone:
   - `group_by_bidder(req, aliases, registry) -> GroupedBidders` — the
     existing `process_one_imp` walk recording per-bidder imp INDICES
     (`BTreeMap<BidderName, Vec<usize>>`) and the disabled-bidder
     warnings. Round-107 B4 (ownership fixed): the function ALSO
     serializes the request once (`serde_json::to_value`) and
     `GroupedBidders` OWNS that `request_value: Value` — there is no
     separate caller-threaded value and there are NO stored "FPD
     pointers" (no self-borrow): materialization LOOKS UP each
     bidder's params from the owned `request_value` by OBJECT
     INDEXING — `request_value["ext"]["prebid"]["bidderparams"]
     .get(name)` / the imp object's maps keyed by `name` — NEVER a
     JSON Pointer path (round-108 B3: spec §5.1 step 7's round-80
     rule — `BidderName` is a free `Box<str>` and alias validation
     does not reject `/` or `~`, so a pointer path would misparse
     alias names like `my/alias`). NO per-bidder `Value` or typed materialization here;
     pre-cap scratch = the single owned `request_value` (inside
     `inbound x INBOUND_PARSE_FACTOR`) + the grouping/alias
     bookkeeping, which round-108 B3 charges EXPLICITLY as
     `inbound x GROUPING_FACTOR` (budget 8, its own allocator
     guard): a 256 KiB request can name ~13k distinct minimal
     participants (~20 input bytes each: one alias declaration + one
     `imp.ext.bidder` key), and each costs ~100 heap bytes across
     the grouping map node + `BidderName` + index Vec + the
     handler's resolved-alias map — bookkeeping that plainly does
     NOT fit inside another factor's "slack".
   - DECIDE — the per-bidder permission decisions (6a: `AllowBidRequest`
     → keep/drop; PassID/PassGeo recorded per bidder), sampling (6b),
     and `apply_max_bidders` (6c) run on the grouped NAME set in the
     same §5.9 order as today; decisions need bidder identity +
     consent, never the clone. The hoisted warning/bookkeeping maps
     are populated here exactly as in M3a.
   - MATERIALIZE — `materialize_per_bidder(grouped, survivors) ->
     Result<Vec<PerBidderRequest>, AuctionError>` builds typed clones
     ONLY for the post-cap survivor set (≤ `max_bidders_per_auction`)
     from the grouped-owned `request_value`; the FPD merge-patch AND
     the code-15 `InvalidImpFirstPartyData` round-trip/`validate()`
     check move here with it.
   - SCRUB — each materialized survivor is then scrubbed per its
     RECORDED decision (`PassID=false` → `scrub_gdpr_id`,
     `PassGeo=false` → `scrub_geo_and_device_ip`) before fanout.
     Decision inputs and scrub outputs are bidder-identical to M3a's
     combined filter+scrub step — only the point of clone existence
     moves; a snapshot test asserts per-bidder wire equality against the
     M3a-shaped pipeline on a mixed fixture (allowed / scrubbed /
     dropped / sampled-out / cap-dropped bidders).
   Behavior delta (deliberate, tested): the code-15 FPD validation now
   fires only for FANOUT-ELIGIBLE survivors — a malformed FPD object
   scoped to a bidder that was privacy-filtered, sampled out, or
   cap-dropped no longer fails the auction (it was never asked; the
   spec §5.1 step 7 code-15 contract carries the same round-107
   note). Tests: `fpd_validation_runs_for_survivors_only()` and the
   allocator guards at WORST-CASE cardinality (round-107 B4,
   corrected round-108 B3 — `max_aliases` bounds ONLY the host
   `[aliases]` overlay per spec §9, NOT request-defined aliases,
   which are bounded by nothing but the 256 KiB input; there is
   deliberately NO new request-alias cap — the bound is DERIVED from
   the input maximum instead): the guard fixture is generated to the
   INPUT-DERIVED maximum — a request at exactly
   `inbound_request_bytes` packed with minimal distinct participants
   (alias declaration + one-imp membership each, ~13k names at the
   256 KiB default) plus the full registry and a max_aliases host
   overlay — and asserts (a) the grouping stage allocates NO typed
   clone and its bookkeeping peak stays within
   `inbound x GROUPING_FACTOR` (the `grouping_factor_guard()`), and
   (b) the whole pipeline peak stays within
   `max_bidders x inbound x TYPED_CLONE_FACTOR` on top of the
   grouping term. An `Arc` reduction
   for the survivor clones stays a listed post-M3b optimization.

7b. **Aux-route gating (round-99 B7 / round-100 B5; round-117 P1-10
   adds `/internal/health/gvl`).** `/status`,
   `/info/bidders`, `/info/bidders/{name}` (round-100 B5 — the
   correct route shape; there is no `/info/bidder`), and
   `/internal/health/gvl` (an active M3 route whose JSON assembly +
   egress lifetime previously escaped the proof; it loads no config
   but its response buffer and counters are allocations like any
   other) are ALL gated, so their concurrent allocations are inside
   the proof: a third portable counter, `aux_gate` (`aux_route_permits`,
   default 4, envelope-owned), gates EVERY config-consuming
   non-auction route with the same immediate-503 `overloaded\n`
   semantics — the aux-gate check is the FIRST action on those routes,
   so overload takes precedence over every other rejection (tested).
   `aux_route_peak_bytes` charges the FULL route footprint using the
   SAME defined pair as the auction term (round-117 P0-2 — "config
   blob x its parse factor" previously named neither a cap nor a
   factor): the SAME `runtime_config_term` the auction path uses —
   **round-183 #5 re-bases it to `max_total_bytes + max_blob_bytes x
   CONFIG_PARSE_FACTOR`, and this site tracks that change rather than
   keeping the retired `max_runtime_config_bytes x (1 +
   CONFIG_PARSE_FACTOR)` form** — + the info-response
   ASSEMBLY/EGRESS bound.
   **Round-124 #10 (supersedes round-101 H7's ASSUMPTION that
   `/info/bidders/all` is largest): `aux_route_peak_bytes` is the
   CHECKED MAXIMUM over every gated route's computed bound** — a boot
   assertion evaluates all four and takes the max; no route's bound is
   assumed dominant. Per-route bounds: `/info/bidders/all` as below
   (with alias NAME length now independently bounded:
   `max_alias_name_bytes = 64`, enforced at config validation, so
   `per_bidder_detail_bytes` is a true constant);
   `/internal/health/gvl` gets structural bounds — its version vectors
   are bounded by NAMED generated constants (round-125 #11: build.rs
   emits `GVL_SPEC3_MAX_RETAINED` and `GVL_SPEC2_ENTRIES` from the
   manifest retention config; the health bound uses them, not prose)
   and its serialization moves to the bounded serializer
   (`serialize_response_bounded`, the M3b-2 replacement — the current
   unbounded `serde_json::to_vec` calls are listed migration sites);
   `/status` and `/info/bidders/{name}` are constants + the config
   term. The four equations (round-125 #11 — each independently
   derivable and boot-asserted; `MAX_ALIAS_NAME_BYTES = 64` is a
   config-validation constant, no longer prose):
   `status_peak = config_term + status_response_cap(4_096)
   + STATUS_DIAGNOSTIC_LINE_BYTES(64)` — **round-187 #7: the
   degraded path APPENDS `config: degraded (<code>)\n` to the
   configured body, so charging only `status_response_cap` would
   under-reserve on exactly the path that runs during an incident.
   64 bytes covers the longest form (`store_key_rejected`) with
   headroom; it is a constant, not a config knob, because the
   vocabulary is closed and compile-time known.**
   (round-183 #5: `config_term` is the single re-based
   `runtime_config_term` above, so the four equations cannot drift
   from the auction path);
   `info_name_peak = config_term + per_bidder_detail_bytes`;
   `info_all_peak = config_term + (registry_bidder_count + max_aliases)
   x per_bidder_detail_bytes`;
   `health_peak = (GVL_SPEC3_MAX_RETAINED + GVL_SPEC2_ENTRIES)
   x HEALTH_VERSION_ROW_BYTES + HEALTH_FIXED_JSON_BYTES` (both
   HEALTH_* are named measured constants). `aux_route_peak_bytes =
   checked_max(status_peak, info_name_peak, info_all_peak,
   health_peak)`. The old text follows for `/info/bidders/all`:** (it
   aggregates EVERY base bidder AND every runtime alias, so it is
   config-dependent, not a constant): the bound is COMPUTED as
   `(registry_bidder_count + max_aliases) x per_bidder_detail_bytes`
   where `registry_bidder_count` is compile-time known,
   `max_aliases` is a NEW envelope source limit clamping the runtime
   `[aliases]` overlay's entry count (overlays past it are rejected
   at config validation), and `per_bidder_detail_bytes` is a pinned
   measured constant. Aux permits are held through egress exactly
   like auction permits (released on the Prereq-9 signal), so the
   charge covers the buffered aggregate response. Round-121 #8: the
   SUCCESS path's write deadline is explicit, not implied — the
   installed `response_write_deadline_ms` budget is anchored at aux
   egress entry (covering serialization, pre-first-byte failure,
   mid-write stall → abort, exactly-once release), reusing the shared
   egress machinery; without it, stalled /status or /info readers
   retain all aux capacity indefinitely. The spec Appendix F gains overload rows for `/status` and both
   info routes (same `503` / `text/plain` / `overloaded\n` shape as
   the auction row). Tests: `aux_routes_gated_and_charged()` +
   `aux_overload_precedes_other_rejections()`.

8. **Measurement contract — whole-isolate, per-target, defaults must
   validate (round-98 H8).**
   - `baseline_bytes` procedure (round-99 H8 — REPRODUCIBLE, not
     drive-until-OOM): baseline is measured, per target, as the
     whole-isolate resident memory AFTER boot-init (host primitives +
     budget install + `Gcl` static) and ONE no-op request, using the
     platform's memory introspection — per-target tools PINNED
     (round-100 H8), all committed as `scripts/measure-baseline.sh`:
     - **Cloudflare (round-102 H12 — a heap snapshot alone is NOT
       whole-isolate memory):** the baseline is the SUM of (a) the
       Wasm linear memory, reported by the probe worker itself via a
       `/__mem` endpoint returning
       `core::arch::wasm32::memory_size(0) * 65_536` (the module can
       read its own memory size), and (b) the JS-heap total from
       `scripts/cf-heap-snapshot.mjs` driving the DevTools Protocol
       over `wrangler dev --inspector-port <p>`
       (`HeapProfiler.takeHeapSnapshot`) — automated, with the
       interactive `D` hotkey as manual fallback. Native allocations
       outside both are covered by the 20% margin; the DEFINITIVE
       whole-isolate check remains the production error-1102 threshold
       validation (a separate exercise, §8).
     - **Fastly:** `fastly compute serve` and record the Viceroy
       process's OS RSS (no in-runtime API).
     - **Spin:** `spin up` and record the spin process's OS RSS.
     - **Axum/native:** process RSS after warm-up.
     TWO SEPARATE runs so workload memory never contaminates the
     baseline (round-100 H8): run A = no-op only → `baseline_bytes`;
     run B = one max-shaped auction → validates the per-auction
     FACTORS (§7), never added to baseline. **N = 30 repetitions**
     per run, take the MAX, add a **20% safety margin**, commit to
     the per-target default table (`auctioneer-runtime/src/budget.rs`
     constants). **Round-107 B1 — the margin lives INSIDE the
     committed constant and NOWHERE else:** `baseline_bytes` =
     `ceil(max_observed x 6/5)` (checked integer `(max x 6 + 4) / 5`),
     and `validate_isolate_limits` consumes `baseline_bytes` AS IS —
     it derives NO further budgeted value (the former ceil(x 11/10)
     step double-margined). ONE margin, ONE rule. Driving concurrency until OOM is a separate
     VALIDATION exercise (it checks the formula's prediction), never
     the baseline source. The embedded raw
     GVL archive ships INSIDE the module, so it is in the baseline's
     VIRTUAL size by definition — but round-117 P2-14: on native,
     mmap'd pages fault lazily, so a no-op request does NOT prove the
     archive is resident, and the two-run measurement must explicitly
     TOUCH every archive page (read one byte per 4 KiB of
     `gvl_archive()` + `baked_gcl_json()`) before the baseline
     snapshot; on wasm the module's data segment is materialized at
     instantiation and no touch pass is needed. The budget still has
     no separate archive term.
   - Per-target pinned table (round-125 #2 — validation is gated on
     having VALID numbers): the every-target assertion iterates a
     `VALIDATED_TARGETS` list that starts as `[axum]` (native, 512 MB
     — the binding arithmetic is the WORKED AXUM PROOF ROW beside the
     struck Cloudflare table: TOTAL = 497_075_072 <= 512_000_000 at
     max_calls 64 / ceiling 384 / 4 streams (round-140 #1), its
     three measured placeholders
     re-run under §8 before certificate issuance); the CF/Fastly/Spin
     rows JOIN the list in the same commit as the measurement
     re-derivation that produces their real numbers. A target not in
     the list cannot claim any memory certificate. Original rule:
     every SHIPPED target's default
     `ResolvedIsolateLimits` MUST pass `validate_isolate_limits` — a
     table-driven test iterates the four targets' default envelopes.
     **Round-102 B1 / round-103 H8 — the GLOBAL spec defaults (100
     calls x 256 KiB caps) are ARITHMETICALLY IMPOSSIBLE inside
     Cloudflare's 128,000,000-byte isolate, so the per-target DEFAULT
     envelopes clamp them DOWNWARD. The Cloudflare default is
     BYTE-EXACT (every input pinned; the table test reproduces this
     arithmetic verbatim):**
     ```text
     isolate_total            = 128_000_000
     baseline_bytes (PINNED)  =  50_331_648   // round-107 B1: the pin CARRIES the §8
                                              // 20% margin INSIDE it — the committed
                                              // constant is ceil(max_observed x 6/5),
                                              // so this default REQUIRES the §8 run-A
                                              // max-of-30 to satisfy
                                              //   ceil(max x 6/5) <= 50_331_648,
                                              // i.e. max_observed <= 41_943_040
                                              // (40 MiB); a larger measurement forces
                                              // re-deriving this envelope. Validation
                                              // uses the pin DIRECTLY — the round-105
                                              // extra ceil(x 11/10) budgeting was a
                                              // SECOND margin on top of §8's 20%: at
                                              // the honest 20% it produced
                                              // 130_787_994 > 128_000_000, so it is
                                              // RETIRED (no baseline_budgeted line)
     max_raw_blob             =   3_000_000; factor 6 -> parsed 18_000_000
     gvl_cache_entries        = 1; max_concurrent_auctions = 1
       gvl_terms              = (1 + 1) x 18_000_000        = 36_000_000
     inbound = 262_144; max_bidders = 8; max_calls = 12     // round-105 B8: calls
                                                            // 16 -> 12 to fund the
                                                            // second stream.
                                                            // Round-117 P1-9: 8/12 are
                                                            // NOT ad-hoc table values —
                                                            // they are the Cloudflare
                                                            // rows of a new per-target
                                                            // TARGET_HARD_CAPS table
                                                            // (boot-envelope consts per
                                                            // adapter: axum 50/64
                                                            // (the 100 input default
                                                            // clamps to the round-128
                                                            // certificate's 64 —
                                                            // round-133 #7), CF
                                                            // 8/12, Fastly+Spin sized in
                                                            // their own worked sections).
                                                            // Precedence: hard cap wins
                                                            // over baked AND runtime
                                                            // values; a clamped value
                                                            // emits one boot-time WARN
                                                            // and a `clamped_caps` field
                                                            // on /internal/health/gvl,
                                                            // so an operator setting 50
                                                            // on CF can SEE the clamp.
     max_outbound_raw = max_outbound_request = 262_144
     max_transport = max_decoded = 131_072
     OUTBOUND_META_BYTES = META_BYTES_CAPACITY(3_584)
                         + offset_table_bytes + 96  // round-126 #7, corrected
                                            // round-202 #1 (fixes
                                            // round-148 #3: the 64 is
                                            // the AcceptedRequest
                                            // handle. ROUND-202 #1:
                                            // this charged 64 for
                                            // TWO Vec headers, but
                                            // the struct holds THREE
                                            // (byte pool, headers,
                                            // imp_ids) = 24 x 3 = 72
                                            // + CallReservation 16
                                            // = 88, and the compiled
                                            // probe measures
                                            // AcceptedRequest at 136
                                            // AS BUILT (2a.4 probed
                                            // it; see the assertion
                                            // note below — the struct
                                            // holds a
                                            // BidderHttpRequest, five
                                            // handles not three)
                                            // (round-203 #3: 96 was
                                            // the size WITHOUT the
                                            // body Vec handle, which
                                            // the type carries inside
                                            // `request`). CHARGE 96
                                            // HERE, because the body
                                            // handle belongs to the
                                            // separately-charged body
                                            // term:
                                            // byte pool 24 + headers 24
                                            // + imp_ids 24 + retained
                                            // CallReservation 16
                                            // (&CallBudget + bool,
                                            // padded); the body Vec
                                            // header (24) rides in the
                                            // body term's 20% margin;
                                            // `const _: () = assert!(
                                            // size_of::<AcceptedRequest
                                            // <'_>>() == 120)` pins
                                            // the WHOLE representation
                                            // (round-203 #3 — the
                                            // earlier `<= 88` and
                                            // `<= 96` both asserted a
                                            // subtracted figure and
                                            // would have failed)
                                            // so a field added
                                            // later fails compile, not
                                            // the certificate
                                            // round-125 #4's 3_520 typo,
                                            // which contradicted the
                                            // sink's own 3_584 byte pool):
                                            // DERIVED, not fixed —
                                            // offset_table_bytes =
                                            // 2*4 + 36*8 + max_imps*4,
                                            // rounded up to 64
                                            // (ROUND-202 #1 — header
                                            // entries are 8 bytes,
                                            // not 4). At the default
                                            // 100 imps this is 704
                                            // and the total
                                            // 3_584+704+96 = 4_384;
                                            // at the codegen
                                            // maximum 1_000 it is
                                            // 4_160 and the total
                                            // 3_584+4_352+96=8_032
                                            // — the
                                            // charge scales WITH the
                                            // limit instead of
                                            // silently overflowing
                                            // past a fixed 512.
     INBOUND_PARSE_FACTOR = 2   // *** STILL DISPROVEN AFTER THE
                                //     MANDATED REPAIRS — 2a.5
                                //     MEASURED IT ON THE BUILT TREE
                                //     AND IT NEEDS A DECISION.
                                //     `bounded_json` (2a.3) landed
                                //     repair (1), and the factor is
                                //     STILL wrong by an order of
                                //     magnitude, in RETAINED bytes:
                                //     dense imp array x18.6,
                                //     retained unknown ext x11.9,
                                //     many small strings x25.0.
                                //     The cause is structural and a
                                //     node budget cannot fix it:
                                //     `Imp`, `Banner` and siblings
                                //     each carry a
                                //     `#[serde(flatten)]`
                                //     `serde_json::Map`, so a ~44-byte
                                //     impression retains a map header
                                //     and buckets whatever the input
                                //     density. The ratio is set by the
                                //     TYPE, not the body, and does not
                                //     fall as bodies grow (22 flatten
                                //     fields across the OpenRTB hot
                                //     types — verified).
                                //     2a.5 did NOT change the
                                //     constant: raising it would push
                                //     axum's `inbound parse` term ~10x
                                //     past the 512 MB ceiling and
                                //     invalidate the certified
                                //     envelope, and reshaping the
                                //     types is outside an
                                //     arithmetic-only unit.
                                //     `inbound_parse_factor_guard`
                                //     therefore RECORDS the
                                //     measurement and asserts a x48
                                //     regression ceiling instead.
                                //     **OPEN — two ways out, both
                                //     outside 2a.5: raise the factor
                                //     and recertify every envelope, OR
                                //     take the flattened maps off the
                                //     hot types. Until one lands, the
                                //     inbound term is understated.**
                                // *** ROUND-118 F02: DISPROVEN. A
                                //     measured native probe parsed a
                                //     VALID 240,072-byte request whose
                                //     retained unknown-field array
                                //     (120,000 zeroes, 32B/node) costs
                                //     >= 3,840,000 B — 16x input, vs
                                //     this factor's 524,288 B
                                //     allowance. Two mandated repairs,
                                //     BOTH land before any envelope is
                                //     trusted: (1) a parser-time
                                //     RETAINED-NODE BUDGET — unknown /
                                //     passthrough serde_json::Value
                                //     content is counted per node at
                                //     parse and the request rejected
                                //     as BadInput past
                                //     max_parsed_nodes (boot-envelope
                                //     const; sized so node_budget x
                                //     32B fits the factor's term);
                                //     (2) every factor below is
                                //     re-derived from ADVERSARIAL
                                //     structural fixtures (dense
                                //     arrays, tiny objects, deep
                                //     nesting, retained unknowns) —
                                //     not from benign round-trip
                                //     bodies. The 2 here is retained
                                //     ONLY as the post-node-budget
                                //     target the rederivation must
                                //     prove or replace. Guard tests
                                //     assert the MEASURED value fits
                                //     the CHARGED value — never that
                                //     a constant equals 2. ***
     GROUPING_FACTOR = 8 (round-108 B3)
     TYPED_CLONE_FACTOR = 2
     RESPONSE_PARSE_FACTOR = 2; ASSEMBLY_FACTOR = 1
     MAX_RESPONSE_HEADER_BYTES = 8_192; MAX_RESPONSE_HEADER_COUNT = 64
     HEADER_MAP_CHARGE = 8_192 + 64 x 64 = 12_288   // round-111 H4 (retained maps)
     OUTBOUND_HEADER_MAP_CHARGE = 3_584 + 64 x 64 = 7_680 // round-150 #5 (request maps:
                                                    // byte pool bytes + 64-slot entry table)
     H2_MAX_FIELD_SECTION = 8_192 + 65 x 32 + 64 = 10_336   // round-112 B2
     H2_FIELD_SECTION_KNOB = 10_337   // round-115 B1: h2 rejects at
                                      // headers_size >= max (exclusive
                                      // boundary) — set ONE above the
                                      // ceiling; admitted <= 10_336
     HEADER_MAP_TRANSIENT_CHARGE = 10_336 + 313 x 64 = 30_368 // parser worst case
     HEADER_STREAM_CHARGE = 12_288 + 30_368 = 42_656 // round-113 H1: retained
                                                     // initial map + concurrent
                                                     // trailer-block transient
       per_auction_peak = 262_144 + 262_144 x 2 + 262_144 x 8
         + 8 x 262_144 x 2
         + 12 x (262_144 + 262_144) + 12 x 131_072 + 12 x 12_288
         + 12 x 131_072 x 2 + 12 x 131_072 + 12 x 4_096
         + 12 x 7_680                     // round-151 #4: request maps
         = 19_949_568
     assembly_scratch (derived, INSIDE per_auction_peak above)
                              =  12 x 131_072 x 1 = 1_572_864
     final_response_bytes     =   2_097_152; egress_permits = 1
                              // round-106 B5: the CONFIGURED egress cap;
                              // sizing floor: 2_097_152 >= 1_572_864 ✔
                              // (round-108 H4: a floor, not an output-
                              // size proof — enforcement is the bounded
                              // serializer); charged once, next TOTAL line
     per_stream = 131_072 + 131_072 + 5_767_168 (GuestDecoded)
                + 42_656 (header stream charge, round-113 H1) = 6_071_968
     max_active_body_streams  = 2  -> 12_143_936   // round-105 B8: >= 2 so CF
                                                   // fanout stays CONCURRENT
                                                   // (bounded, not serialized)
     aux_route_permits = 1; aux_route_peak = 2_621_440 -> 2_621_440
     TOTAL = 50_331_648 + 36_000_000 + 19_949_568 + 2_097_152
           + 12_143_936 + 2_621_440 = 123_143_744
     MARGIN = 4_856_256  (<= 128_000_000 ✔; the §8 20% safety margin
                          is INSIDE baseline_bytes — the residual
                          margin is headroom beyond it)
     ```
     **AXUM proof row (round-127, RE-DERIVED round-128 #1 — the
     prior row's subterms did not satisfy the authoritative equation;
     this row is computed FROM that equation term-by-term, its
     subterms sum EXACTLY to the stated aggregates, and M3b-2's
     `the_over_ceiling_row_names_the_terms_that_dominate()` checks the
     actual Axum envelope inputs with
     `validate_isolate_limits` so the checked-in table cannot drift
     from the code. Fitting the 512 MB budget forced default
     reductions: max_calls 100 → 64 (round-128) and
     platform_concurrent_ceiling 1_024 → 512 (round-128) → 384
     (round-129 #1 — the honest parser-derived entry charge).
     VALIDATED_TARGETS = [axum]. The three
     MEASURED-PENDING placeholders (baseline, transmit_state,
     parser_transient) re-run under §8 before certificate issuance:**
     **⚠ AS BUILT — VALIDATED_TARGETS IS STILL [axum], AND THE THREE
     PLACEHOLDERS CANNOT BE MEASURED THE WAY §8 ASSUMES. Axum now
     installs this row for real (`auctioneer-adapter-axum/src/envelope.rs`,
     read back through `boot_limits()`), but the envelope-wiring unit
     established three things that change what §8 can deliver:
     (1) `parser_transient_bytes` and `transmit_state_bytes` are the
     HOST's hpack table, decode scratch and write-path state. No h2 or
     hpack code exists in any wasm guest — the host hands it a parsed
     `Request` — so those bytes are not in guest linear memory and NO
     guest-side allocator can observe them, however cleanly scoped. The
     counting-allocator technique does not reach them.
     (2) `baseline_bytes` is isolate RSS, and no local run yields a
     valid one: `spin up` and `fastly compute serve` both abort on the
     `auctioneer_runtime` config store, and host RSS is not a substitute —
     three runs of ONE unchanged Spin binary read 2_144 / 2_160 /
     50_432 KiB, tracking page-cache residency of the mmap'd module
     rather than anything about auctioneer.
     (3) **CLOUDFLARE'S WORKED ROW IS ARITHMETICALLY INFEASIBLE.** Fed
     to `validate_isolate_limits` it requires 209_979_328 bytes against
     the documented 128 MB isolate ceiling — 64% over. Supplying
     deliberately MINIMAL parser constants instead of axum's still
     requires 173_627_328, so the overrun is in the row's own caps, not
     in the borrowed fields. Fitting it means REDUCING per-auction
     capacity on Cloudflare, which is a product decision about how many
     concurrent auctions that target serves, not a derivation. **DECISION
     NEEDED before Cloudflare can install an envelope.** Fastly and Spin
     lack a documented `isolate_total_bytes` at all; `platform_concurrent_ceiling`
     is the one field a wrong value does not fail loudly on, since the
     validator uses it only in the final margin subtraction, so choosing
     it to make the arithmetic close would make the proof circular.
     Also: `scripts/measure-baseline.sh`, referenced throughout §8, does
     not exist in the tree.**
     ```
     axum ResolvedIsolateLimits (native, host budget 512_000_000):
       inbound 262_144; max_bidders 50
       max_calls 64   (round-128 #1: the axum-target default
                       max_outbound_requests — the baked 100 exceeds
                       this envelope; boot validation clamps DOWN)
       max_transport = max_decoded = 262_144; wbits 22; layers 1
       max_active_body_streams 4 (round-140 #1 — the corrected
         20 MiB brotli charge funds 4, not 16); max_concurrent_auctions 1
       max_runtime_config_bytes 1_048_576; CONFIG_PARSE_FACTOR 6
       max_parsed_nodes 65_536; max_copied_header_bytes 4_096
       max_admitted_header_bytes 16_384; count 128 (overhead 64/entry)
       max_impressions_per_auction 100 (OUTBOUND_META 4_384)
       terminal_response_permits 68; refusal_body_bytes 256
       transmit_state_bytes 4_096 (MEASURED-PENDING placeholder)
       egress_permits 1; aux_route_permits 4; handoff factor 2
       final_response_bytes 16_777_216 (>= the 64 x 262_144 x 1
                                        assembly floor — exact)
     axum TargetPlatformConstants:
       platform_concurrent_ceiling 384 (96 conns x 4 h2 streams)
       parser_header_ceiling_bytes 21_632 (ADMITTED, the charged
         bound = 16_384 + (128 + 4 pseudo) x 32 + 1_024 allowance)
       parser_header_knob_bytes 21_633 (CONFIGURED; validation
         asserts knob = ceiling + 1 — h2 exclusive boundary)
       parser_max_header_entries 676 (derived: 21_632 / 32)
       max_request_target_bytes 768; pseudoheader_allowance 1_024
         (checked: 768 + 184 <= 1_024; :method <= 16, :scheme <= 8,
          extended CONNECT rejected)
       h1_max_buf_size_bytes 24_576 (checked: >= 16_384 + 768
         + 1_024); h1_max_headers 128 (checked: == admitted count)
       parser_transient_bytes 16_384 (hpack table 4_096 + decode
         scratch; MEASURED-PENDING placeholder)
       header_entry_overhead_bytes 64; egress_handoff_factor 2
     per_auction_peak terms (bytes):
       config 1_048_576 x 7                             7_340_032
       inbound raw                                        262_144
       admitted headers 16_384 + 128 x 64                  24_576
       header copies 4_096 x (1 + 50)                     208_896
       inbound parse 262_144 x 2                          524_288
       parsed nodes 65_536 x 64                         4_194_304
       grouping 262_144 x 8                             2_097_152
       typed clones 50 x 262_144 x 2                   26_214_400
       outbound raw+encoded 64 x 524_288               33_554_432
       retained positional 64 x 262_144                16_777_216
       response headers 64 x 12_288                       786_432
       response parse 64 x 262_144 x 2                 33_554_432
       meta 64 x 4_384                                    280_576
       outbound request maps 64 x 7_680                   491_520
       assembly scratch 64 x 262_144 x 1               16_777_216
       per_auction_peak (exact sum)                   143_087_616
     TOTAL terms (bytes):
       baseline (§8, MEASURED-PENDING; provisional)    96_000_000
       gvl_terms                                       36_000_000
       per_auction_peak x 1                           143_087_616
       egress: 1 x (16_777_216 x 2 + 4_096)            33_558_528
       aux: 4 x (7_725_056 x 2 + 4_096)                61_816_832
       static: 64 x (256 x 2 + 4_096)                     294_912
       # ⚠ INCONSISTENT (found by 2a.5's
       # validator_regenerates_the_synthetic_worked_row, which reproduces
       # 14 of 15 subterms exactly): this line charges 64, but the
       # row above STATES terminal_response_permits 68. Charging the
       # stated 68 gives 313_344, a TOTAL of 497_595_264 and a margin
       # of 14_404_736 — the 512 MB bound still holds comfortably, so
       # this is a bookkeeping error, not a budget breach. The
       # validator charges the STATED limit and asserts the
       # recomputed totals; the arithmetic here is what needs
       # correcting. Note also that the field's definition
       # (= max_concurrent_auctions + aux_route_permits) does not
       # hold for this row either: its values are 1 and 4.
       pre-admission parser peak (max of protocol peaks —
       round-132 #5):
         h2 slot: 21_632 + 16_384 + 21_632 + 676 x 64 = 102_912
         h1 slot: 24_576 + 16_384 + 128 x 64          =  49_152
         384 x max(102_912, 49_152)                    39_518_208
       streams: 4 x per_stream(21_825_184)             87_300_736
         (round-140 #1 — the brotli term now carries upstream's
          audited 16 MiB fixed charge: per_stream =
          helper(262_144 transport + 262_144 decoded
          + 21_258_240 max-rule decoder [chunk staging 24_576 +
          brotli(22) 20_971_520 + flate2 x 2 262_144] = 21_782_528)
          + HEADER_STREAM_CHARGE 42_656 = 21_825_184. The
          round-135 16-stream row at 5_572_256/stream rode the
          undercharged window+512 KiB model and was UNSAFE;
          max_active_body_streams drops 16 → 4. Axum's own chain
          dominates the guest bound, boot-asserted, else the max
          applies and the row is regenerated)
     TOTAL (exact sum)  497_576_832  <= 512_000_000  ✔
                        (margin 14_423_168 ≈ 13.8 MiB; round-202 #1
                        re-derived both after the metadata layout
                        correction — the +10_240 delta stays inside
                        the margin, so the bound still holds;
                        round-150 #5
                         added the 491_520 request-map term)
     ```
     **⚠ ROUND-118 F03 — THE CLOUDFLARE TABLE BELOW IS INVALID PENDING
     REDERIVATION and must not be implemented as-is. STATUS (round-152
     #7): still invalid — it blocks 2a.FINAL, the certified CF/Fastly/
     Spin stream counts and the final per-target envelope defaults, and
     nothing else (the 2a.1–2a.6 algorithms and the numberless
     `TargetOutboundProfile` literals do not depend on it).** Independent
     recomputation shows the round-117 mandatory x1.5 cross-target
     margin on the five native-measured factors adds 5,767,168 B,
     taking this total to 128,818,752 B — OVER the 128,000,000 ceiling
     — before the round-117 runtime-config and static-egress terms are
     even added, and it lifts assembly scratch past the 2,097,152 B
     egress sizing floor. The table is retained as the worked EXAMPLE
     of the method only. Before M3b-2 freezes: (1) one AUTHORITATIVE
     source-struct default table gains rows for every named-but-unplaced
     round-117/118 input — `max_runtime_config_bytes`,
     `CONFIG_PARSE_FACTOR`, `terminal_response_permits`,
     `transmit_state_bytes`, `max_parsed_nodes`, and the Cloudflare
     host-header term as its own scalable line; (2) the CF defaults are
     re-chosen so the FULL amended equation fits with the margins
     applied (smaller caps, measured-not-margined wasm factors, or
     both); (3) acceptance tests assert THE EQUATION HOLDS against the
     authoritative table — the round-105 instruction that tests
     reproduce these exact constants is RETIRED.
     Axum's RESOLVED default envelope (native, host-configured
     total ≥ 512 MB) pins max_calls = 64 with the 256 KiB caps
     (round-133 #7 — the global 100 survives only as the baked
     INPUT default, clamped down by boot validation per the
     round-128 certificate; the earlier "keeps the global
     100-call contract" sentence is retired). The
     default-envelope test asserts EXACTLY the RESOLVED
     constants (64), matching the validator-generated proof row.
     **Accounting model for the CF measurement (round-103 H8 /
     round-104 B7 — SOUND upper bound, no inclusion inference):** the
     `snapshot >= wasm_linear` test cannot prove inclusion (a large
     unrelated JS heap masks an omitted Wasm heap), so the baseline
     ALWAYS SUMS both components: `baseline = snapshot_total +
     wasm_linear` (from the `/__mem` probe). If the snapshot already
     tracked the Wasm `ArrayBuffer` this DOUBLE-COUNTS it — which only
     OVER-reserves; for an upper-bound memory proof the conservative
     direction is correct by construction. V8-external native
     allocations ride the 20% margin; Cloudflare's 128 MB limit covers
     JS heap AND Wasm together, and the production error-1102
     threshold validation remains the definitive whole-isolate check;
     an M3b-8 CI gate re-measures baselines (same N=30-max procedure)
     and FAILS when `ceil(new_max x 6/5)` exceeds the committed
     `baseline_bytes` pin (round-107 B1 — the gate re-applies the ONE
     20% margin rule; there is no separate drift percentage).
   - `GVL_PARSE_EXPANSION_FACTOR` guard: every bundled manifest entry
     + the synthetic worst case — **generator corrected (round-98 H8):
     GVL-side fields only** (65,535 vendors; full purposes /
     legIntPurposes / flexiblePurposes / specialFeatures /
     specialPurposes / features sets; 128-byte pinned name strings;
     maximal `dataDeclaration`/URL fields) — publisher restrictions are
     TC-STRING data and are OUT of the GVL generator. Native
     measurement asserts <= 4x; the budget uses 6x (documented 1.5x
     cross-target margin). Round-127 reconciliation: the factor-guard
     set is SIX — `inbound_parse_factor_guard`,
     `grouping_factor_guard`, `typed_clone_factor_guard`,
     `response_parse_factor_guard`, `assembly_factor_guard`,
     `config_parse_factor_guard` — matching one-to-one the factors
     the equation charges; a factor without a guard, or a guard
     without an equation term, fails this reconciliation by
     definition.

9. **Tests (land with M3b-2 — round-101 H9 / round-102 H13 /
   round-107 H7 / round-108 / round-109; round-126 #12: the twelve
   rounds-122..125 additions listed under M3b-8 are MEMBERS of this
   roster (one list, +139 total: 132 land with M3b-2 and
   7 land with M3b-10: the six inbound parser-boundary cases (rounds
   129 #7 / 130 #6 / 131 #6) plus the manifest-class semantic
   assertion (round-136 #4): all members are named below or in the
   LABEL -> OWNER index. Historical round-by-round block counts
   are retired; the named enumeration determines the total.
   (round-146
   #3: a `<target>` template row counts ONCE PER TARGET — four
   concrete tests, axum / cloudflare / spin / fastly — never as one
   logical row)
   (7 weighted-gate + waiting-head-cancel + wave-expiry +
   same-poll recheck + 3 elapsed-timing + 3 time-domain +
   parsed-node guard +
   2 node-counting +
   node-budget-vs-503 precedence + node-budget wire body +
   heterogeneous batch +
   batch-contract violation + header-preflight +
   preflight-compaction + 6 inbound parser
   [M3b-10] +
   manifest-class semantics [M3b-10] +
   validator-regeneration +
   2 stream-gate wake + redirect classifier +
   4 per-adapter redirect-destination +
   7 sink-primitive boundary (round-147 #4: capped-writer exact-cap
   + write_json over-cap + 32-header admit + 33rd-header reject +
   reserved-name case + meta over-cap + reject-reason
   exhaustiveness) + finalize-no-content-length (round-147 #2) +
   slot-error-mapping totality (round-147 #6) +
   3 clock-domain migration (round-148 #1: instant type identity +
   deadline composition + fake-clock drain) +
   3 incomplete/duplicate meta (round-148 #4) +
   retained-reservation refund (round-148 #3) +
   request-body cap equality (round-148 #2) +
   one-clock-source (round-149 #1) +
   code-12-by-clock-only (round-151 #1) +
   2 deadline-retention (round-152 #1: decode-expiry + round-153 #2's
   no-rewrite replacement) +
   4 per-target profile-vs-capability-table + limits-not-profile
   (round-152 #3) +
   decode-policy-per-slot (round-153 #1) +
   frozen-clock decode expiry (round-153 #4) +
   4 per-target wave-wait (round-153 #3) +
   deadline-overflow policy (round-154 #9) +
   fastly-gate-besteffort (round-154 #6) +
   spin-wasip2 (round-154 #4) +
   capped-writer-under-capacity (round-155 #6) +
   cf-batch-never-dropped-on-deadline (round-155 #7 as corrected
   round-156 #1) + codec-pins-match-lockfile (round-156 #2) +
   brotli-state-arithmetic (round-159 #4) +
   cache-bypass (round-159 #3) +
   sink-limits-usize-boundary (round-160 #5) +
   brotli-wbits-range (round-160 #6) +
   layout-constants-parity (round-161 #6 as corrected round-163 #3) +
   3 batch-contract (round-169 M1/M4 + round-170 #1: item-index
   ordering, completed-after-last-item, and
   no-explicit-cancel — the round-169
   `cancel-unresolved timeouts` member is RETIRED, round-171 #2) +
   collect-failure-preserves-slots (round-163 #1) +
   never-responds-pending-driver (round-164 #4)) — the split "+65 here, +12 there"
   presentation is retired): this list is the ONE
   authoritative roster, matching the Task M3b-2 deliverable list.

   **⚠ ROUND-209 (M3b-2a.0) — EVERY GROUP LABEL ABOVE IS
   DISCHARGED BY NAMED TESTS IN ITS OWNING UNIT'S BODY. THE LABELS
   ARE AN INDEX, NOT A BACKLOG.** Walking each label to the round it
   cites found no unrecoverable member: the tests exist, with
   assertions, in the task bodies. What was missing is the pointer
   from label to name, which is what made the roster read as "~95
   unnamed" for twenty-odd rounds. Nothing below is invented and
   nothing is struck.
   **THE RULE, so this cannot regress:** a group label may appear
   here ONLY if the same line names the unit that owns it. A label
   with a count and no owner is the defect
   `scripts/check-roster.sh` fails on — not a placeholder to fill
   in later.
   **LABEL → OWNER, for the labels whose names live furthest from
   this section:**
   - `wave-expiry` + `4 per-target wave-wait` → **M3b-3/4/5**,
     `wave_wait_does_not_expire_before_deadline_<target>()` (one per
     adapter, four concrete tests).
   - `3 clock-domain migration` / `3 time-domain` → **2b.0**,
     `auction_deadline_composes_on_instant_without_projection()`,
     `fake_clock_drain_wakes_every_sleeper()`, and the
     instant-type-identity case in the same step.
   - `validator-regeneration` → **M3b-9**,
     `validator_regenerates_the_synthetic_worked_row()` (generic runtime
     arithmetic fixture, not a second shipped Axum row).
   - `layout-constants-parity` → **2a.5**, in its Step 1 list.
   - `7 weighted-gate` + `waiting-head-cancel` + `same-poll
     recheck` + `2 stream-gate wake` → **2a.2**, whose Step 1
     enumerates head cancellation, middle cancellation, waker
     replacement, granted-then-dropped handoff, no-stealing, and
     `cancellation_churn_behind_blocked_head_does_not_grow_storage()`.
   - `7 sink-primitive boundary` → **2a.4/2a.4a**, enumerated in
     round-147 #4 and listed in those units' steps.
   - `6 inbound parser` + `manifest-class semantics` → **M3b-10**,
     the seven members that land with it rather than M3b-2.
   **ACCEPTANCE RECONCILIATION (2026-10-06): NAMED == TOTAL.**
   This section names 130 distinct declarations: 127 literal names
   and three `<target>` templates, each expanded to four adapters,
   for **139** concrete roster members. References do not declare
   additional members. The former 175 total came from stale
   historical addends, not 36 missing Rust tests. No named member
   is added or removed by this correction. The 80/~95 figures from
   the earlier self-review are also retired.
   `scripts/check-roster.sh` requires the landing split to sum to
   the declared total AND the named, expanded roster to equal that
   total, with unique ownership and counted-label checks retained.
   Deleting any member, including one outside the counted-label
   table, must fail acceptance.
   **⚠ ROUND-200 #4 — RECONCILIATION IS CENTRAL, NOT PER-UNIT. THE
   PER-UNIT RULE BELOW IS RETIRED AND M3b-2a.0 REPLACES IT.** The
   round-166 #2 instinct was right — reconciling at the final gate
   is too late — but pushing the work into each unit's dispatch
   brief gave it no owner, no steps and no acceptance, which is why
   it stayed open for thirty rounds. **M3b-2a.0 produces ONE
   authoritative enumeration BEFORE any roster-owning unit is
   dispatched**, so each unit reads its subset rather than
   recovering it. A unit's dispatch brief still QUOTES its subset —
   that is cheap once the enumeration exists — but it no longer
   performs recovery, and no unit may strike or invent a member.
   HISTORICAL (round-166 #2), retained for the reason the timing
   matters: deferring wholesale to M3b-8 was too
   late: an acceptance case discovered at the final gate arrives
   AFTER the code it constrains is written, forcing redesign exactly
   when redesign costs most. THE RETIRED RULE: a unit's dispatch brief
   must enumerate ITS OWN roster subset — every test name plus its
   exact assertion — and any member the head's group summary implies
   but no bullet names is recovered from the round that introduced it
   (the group labels cite those rounds) or struck with the total
   corrected downward. A unit whose subset cannot be enumerated is
   NOT dispatchable, however finished its code steps look. M3b-8
   keeps a Step-0 arithmetic check as the BACKSTOP — that gate counts
   this roster and cannot certify a total it cannot enumerate — but
   it is no longer where the work happens.
   The base arithmetic is retired. The declarations below and in
   the index above, expanded per target, determine the total):**
   - `admission_full_returns_503_wire_shape_no_alloc()` (503 +
     `overloaded\n`, no config fetch, no timers, no body).
     **Prereq-10-gated, and NOT 2a.1's:** the wire response is
     emitted at EdgeZero ingress through the injected-grant path,
     not inside `admission.rs`, so this member lands with the unit
     that wires ingress admission — the same rule the six
     `inbound_*` members follow. 2a.1 owns the unit-level
     counterpart, `admission_full_returns_none_without_allocating()`,
     which asserts the gate returns `None` and allocates nothing;
     the two are not duplicates and both are roster members.
   - `admission_permit_releases_on_drop_including_panic()`.
   - `permit_body_holds_admission_until_transmission_complete()` —
     round-98 B5 / round-100 B4: dropping the handler does NOT
     release; the EGRESS permit releases only on the Prereq-9
     write-complete/disconnect signal (Prereq-9-gated).
   - `stream_gate_head_cancellation_hands_grant_to_next()` —
     round-98 B1: head cancelled after wake-before-repoll → next
     waiter granted; no stranded permit.
   - `stream_gate_middle_cancellation_removes_node()`.
   - `stream_gate_waker_replacement_repolls_same_node()`.
   - `stream_gate_notifies_only_the_latest_waker_exactly_once()` —
     round-145 #4: counting wakers A then B; only B is notified,
     exactly once.
   - `stream_gate_wakes_outside_the_lock_reentrant_waker()` —
     round-145 #4: a re-entrant waker completes, proving wakers fire
     outside the gate lock.
   - `stream_gate_granted_then_dropped_returns_permit()`.
   - `stream_gate_new_caller_cannot_steal_past_queue()`.
   - Weighted-gate cases (round-126 #2 / round-128 #5, seven):
     `stream_gate_zero_wave_errors()`,
     `stream_gate_wave_exceeding_capacity_errors()`,
     `stream_gate_weighted_grant_is_atomic_no_partial_hold()`,
     `stream_gate_mixed_size_fifo_order_head_of_line()`,
     `stream_gate_cancel_before_grant_releases_nothing()`,
     `stream_gate_cancel_after_grant_releases_full_weight()`,
     `stream_gate_invariant_free_held_granted_equals_capacity()`
     (drives all transitions; the independent counters make a lost
     release or double-grant trip the assert).
   - `parsed_node_charge_guard()` — the four adversarial shapes,
     allocator-measured (round-128 #4; proves the 64 B ALLOCATION
     claim only — counting semantics are the next two).
   - `bounded_json_counts_nested_skipped_subtrees_at_exact_boundary()`
     — round-129 #6: nested unknown objects/arrays inside skipped
     fields, sized to land exactly AT and one OVER the budget; fails
     against a wrapper that delegates deserialize_ignored_any (the
     pinned serde_json consumes the whole subtree in one call).
   - `bounded_json_counts_keys_and_ignored_any_via_both_entry_points()`
     — round-129 #6: object keys count as nodes, IgnoredAny paths
     count per syntactic value, asserted through BOTH
     `from_slice_within::<T>` (typed) and `value_within` (Value)
     entry points at exact boundaries.
   - `stream_gate_waiting_head_cancel_unblocks_satisfiable_next()`
     — round-129 #3: capacity 8, free 4, head requests 8, next
     requests 1; dropping the waiting head must wake and grant the
     1-waiter via the cancellation rescan.
   - `wave_acquisition_expiry_emits_expired_calls_and_shrinks_wave()`
     — round-129 #4: an early bidder deadline expiring during
     `acquire_wave(k)` cancels the acquisition, emits positional
     `Timeout` results (stored cause) for every expired call, and
     re-enters the loop with recomputed k; no re-anchoring.
   - `wave_permit_and_timer_ready_same_poll_drops_lease_and_times_out()`
     — round-130 #4: acquisition and the deadline timer resolve on
     the SAME poll; the post-acquisition recheck must drop the whole
     lease and emit the due timeouts — expired work is never
     dispatched.
   - `slot_elapsed_carried_on_ok_and_err_outcomes()` — round-135 #8:
     both a successful slot and a failed slot carry the G15 elapsed;
     a passthrough Ok adds auctioneer's local decode time to
     responsetimemillis.
   - `queued_then_successful_slot_elapsed_excludes_wait()` —
     round-136 #8: a call that waits a full wave then succeeds
     reports the G15 dispatch time only — the wait is excluded,
     asserted against a driven multi-wave fixture.
   - `wave_timer_and_recheck_share_app_clock()` — round-136 #6,
     re-targeted round-142 #3: the race's sleep and the
     post-acquisition recheck read the SAME EdgeZero application
     clock; a fixture advancing any other clock must not move the
     race.
   - `deadline_composition_preserves_preprocessing_elapsed()` —
     round-136 #6 / round-142 #3: request_start read from the app
     clock, time advanced during preprocessing, deadline composed —
     composed_at equals request_start + budget exactly (no elapsed
     time lost or double-counted; no projection exists to lose it).
   - `handler_and_fanout_read_one_app_clock()` — round-142 #3
     (replaces the broken-domain test, since no bridge exists to
     break): the handler's request_start and the fanout's
     fanout_entry_at come from the same clock instance; asserted
     by driving the test clock and observing both.
   - `queue_expired_call_elapsed_measures_wait_not_zero()` —
     round-135 #8: a call expiring during wave-wait gets the
     auctioneer-synthesized elapsed (now − fanout_entry_at), asserted
     nonzero and >= the driven wait.
   - `invalid_header_fails_slot_preflight_not_silently_dropped()` —
     round-138 #7: a call carrying an unparseable header name or
     value fails its slot BEFORE dispatch with a typed error; the
     wire sees nothing, and no header is silently removed.
   - `on_time_slot_is_not_rewritten_by_batch_or_publication_time()`
     — round-153 #2 (REPLACES round-152's
     `late_ok_after_deadline_is_timeout_not_on_time`, whose recheck
     would have blamed a fast bidder for a slow sibling): a two-slot
     wave where slot A's upstream outcome is `Ok` well inside its
     deadline while slot B delays the batch past A's deadline — A
     publishes as `Ok`, and only a slot whose own upstream outcome is
     `GatewayTimeout` classifies as `Timeout(selected_cause)`. A
     preflight failure recorded in wave one is likewise unchanged
     after a later wave runs.
   - `brotli_state_bytes_is_fixed_charge_plus_window()` — round-159
     #4: 16_778_240 / 20_971_520 / **33_554_432** at wbits 10/22/24,
     asserted SEPARATELY from `per_stream_bytes`'s 34_365_440 total,
     which the earlier text conflated with the decoder state.
   - `validate_isolate_limits_rejects_raw_bytes_above_usize_max()` —
     round-160 #5 as relocated round-161 #1: the conversion lives in
     `auctioneer-runtime`, so a configured `u64` above `usize::MAX` on
     the target fails at BOOT with `LimitExceedsUsize`, never
     truncating or panicking at the allocation site; the boundary
     value is accepted and lands converted in `Derived`.
   - `outbound_meta_bytes_matches_worked_values()` — round-161 #6 as
     corrected round-162 #3: `auctioneer_bidder::outbound_meta_bytes`
     returns 4_384 at the default 100 imps and 8_032 at the 1_000-imp
     maximum, so the ONE value the budget validator receives cannot
     diverge from the byte pool the sink allocates.
   - `never_responds_driver_stays_pending()` — round-164 #4: the
     migrated `NeverResponds` batch never yields `Item` and never
     yields `Finished`, so the strategy's own timer remains what
     ends the test; an immediately-finished empty batch fails it.
   - `batch_item_index_drives_position_not_arrival_order()` —
     round-169 M1: a driver delivering terminal indices 2, 0, 1
     still publishes results in INPUT order, proving position comes
     from `OutboundBatchItem.index` and never from arrival order.
   - `no_explicit_cancel_on_the_normal_fanout_path()` — round-170
     #1 (REPLACES `cancel_unresolved_indices_become_positional_timeouts`,
     which specified dead code — see the cancellation
     block: no non-fatal trigger exists, so its positional timeouts
     were never publishable): a batch whose handle panics on
     `cancel()` completes a full wave — dispatch, cutoff and fatal
     paths alike — without that panic firing, proving `run_wave`
     reaches cutoff through `Finished(Cutoff)` and fatal failure
     through the outer error, never through an explicit cancel.
   - `completed_is_observed_on_the_poll_after_the_last_item()` —
     round-169 M4: the loop keeps polling once its own tally is full
     and still observes `Finished(Completed)`, so the positional
     check always runs.
   - `collect_failure_preserves_completed_slots_is_documented()` —
     round-163 #1: a fixture whose batch driver fails after two
     slots completed yields `OutboundBatchFailure { error, slots }`
     with both slots present; auctioneer's classifier still answers 500,
     and the test pins that the DISCARD is deliberate rather than an
     unnoticed `?`.
   - `proxy_adapter_sets_cache_bypass()` — round-162 #1: the request
     `EdgeZeroProxyAdapter::send_one` builds carries
     `OutboundCachePolicy::Bypass`, never `PlatformDefault`.
   - `brotli_state_bytes_rejects_out_of_range_wbits()` — round-160
     #6: wbits 9, 25, 64 and 255 yield `BrotliWbitsOutOfRange`; 10
     and 24 succeed — an undefined shift is unreachable.
   - `bidder_calls_set_cache_bypass()` — round-159 #3: every
     converted `OutboundRequest` carries
     `OutboundCachePolicy::Bypass`, and no bidder call is left at
     `PlatformDefault`.
   - `codec_pins_match_lockfile()` — round-156 #2 / round-157 #1+#3:
     all six asserted codec crates (`async-compression`, `brotli`,
     `brotli-decompressor`, `compression-codecs`, `compression-core`,
     `flate2`) match `Cargo.lock` exactly AND appear exactly once
     each, so a transitive bump — or a second resolved copy — cannot
     silently invalidate the measured decoder-state bound. It is the
     ONLY witness for the four crates auctioneer does not name as direct
     dependencies.
   - `capped_writer_rejects_under_capacity_buffer()` — round-155 #6:
     a backing buffer whose capacity is below the cap is refused at
     construction, so no writer can reallocate mid-write.
   - `cf_batch_is_never_dropped_on_a_deadline()` — round-156 #1: the
     batch runs to completion through an early member's expiry, and
     only handler teardown drops it.
   - `deadline_overflow_is_internal_error_not_saturation()` —
     round-154 #9: `checked_add` overflow at BOTH composition sites
     (auction deadline in `tmax.rs`'s `resolve_tmax`, per-call in
     `compose_all`) yields the internal 500, never a far-future
     instant. **As built: 2b.0 landed the auction half — the
     `compose_all` half lands with 2b.2, which creates that
     function; the test grows its second arm there.**
   - `fastly_gate_wait_is_besteffort_and_documented()` — round-154
     #6: the Fastly wave-gate wait can overrun a slot deadline; the
     test pins that auctioneer emits the due positional timeouts on the
     next permit wake and that the capability row declares the gap.
   - `spin_builds_on_wasip2()` — round-154 #4: the Spin adapter and
     its conformance test compile for `wasm32-wasip2`, the target
     the pinned EdgeZero uses.
   - `decode_policy_travels_with_the_slot()` — round-153 #1: a
     passthrough slot whose decoded output exceeds ITS OWN
     `max_decoded_bytes` yields `OversizeResponse { source: Decoded }`
     while a sibling with a larger cap succeeds in the same wave —
     proving per-slot caps, not a batch-wide value, reach the decoder.
   - `decode_expiry_under_frozen_cpu_clock_is_timeout()` — round-153
     #4: with a `DecodeCooperator` whose `yield_to_host` is the ONLY
     point the test clock advances, a decode that crosses
     `deadline_at` returns `Timeout(selected_cause)`; a decoder that
     never yields is caught by the same test hanging the budget.
   - `wave_wait_does_not_expire_before_deadline_<target>()` —
     round-153 #3, one per adapter (FOUR roster members): a contended
     `acquire_wave(k)` under a far-future deadline emits NO
     pre-dispatch timeout, which `ImmediateResolveTimer` cannot
     satisfy.
   - `decode_expiry_is_timeout_not_decoder_error()` — round-152 #1:
     a passthrough (deflate) body whose cooperative decode crosses
     `deadline_at` on the driven clock classifies as
     `Timeout(selected_cause)` — not `DecoderFailure`, not
     `OversizeResponse` — and the wave lease still releases.
   - `outbound_profile_matches_capability_table_<target>()` — round-152
     #3, one per adapter (FOUR roster members): profile semantics
     equal the expected-support table cells.
   - `bounded_fetch_reads_limits_not_profile()` — round-152 #3: every
     number `BoundedFetch` charges comes from `ResolvedIsolateLimits`.
   - `preflight_compaction_restores_indices_and_skips_empty_upstream()`
     — round-140 #5: a mixed batch dispatches only the valid
     siblings and restores original indices (preflight slots carry
     the auctioneer-MEASURED elapsed from wave entry to the failure —
     round-141 #4 / round-149 #5 / round-151 #6: on the driven clock
     the test advances one step between wave entry and preflight and
     asserts elapsed == that step; a second case with NO advance
     asserts elapsed == 0 — a legal measured value, not a fabricated
     one); an all-invalid batch never
     calls upstream.
   - `node_budget_400_wins_over_disabled_account_503()` — round-130
     #5: over-budget input naming a DISABLED account yields the
     canonical node-budget 400 (the prelookup errors before account
     resolution can conclude), never the account 503.
   - `node_budget_wire_body_exact_prefix_and_newline()` — round-132
     #4: the wire body is byte-exact
     `Invalid request: too many JSON nodes\n` — BadInput carries
     only the message, the existing handler wrapper supplies prefix
     and newline, no double-prefixing.
   - `send_bounded_all_heterogeneous_policies_positional()` —
     per-call caps and deadlines all differ; out[i].outcome answers
     calls[i], and every slot carries its own elapsed
     (rounds 127 #1 / 134 #3).
   - `redirect_3xx_is_bad_server_response_without_parsing()` —
     round-145 #2: 302/307/308 classify as `BadServerResponse`
     (code 4, seatnonbid 100); the 307's valid-looking bid JSON is
     never parsed.
   - `redirect_destination_never_contacted_<target>()` — round-145
     #2, one per adapter (FOUR roster members: axum, cloudflare,
     spin, fastly — round-146 #3): the Location target's listener
     sees zero connections.
   - `batch_contract_violation_maps_to_typed_internal_error()` —
     length mismatch / EmptyBatch on a non-empty fanout batch ⇒
     `FanoutInternalError::BatchContractViolation`, standard 500,
     wave lease released by RAII (round-128 #6).
   - `inbound_h1_header_overflow_writes_431_and_closes()`,
     `inbound_h2_field_section_breach_resets_stream()`,
     `inbound_h1_header_count_boundary_admits_128_rejects_129()`,
     `inbound_h2_field_section_boundary_admits_21632_rejects_21633()`,
     `inbound_h1_buf_size_boundary_admits_cap_rejects_over()`, and
     `inbound_h2_pseudoheader_bounds_method_scheme_connect_rejected()`
     — the protocol-split parser-cap behaviors (round-128 #3), the
     exact knob boundaries (round-130 #6), and the h1-buffer /
     pseudoheader-value bounds (round-131 #6). ROSTER
     MEMBERS, but they LAND WITH TASK M3b-10, not M3b-2 (round-129
     #7): all six need Prereq 10 and the connection-driver
     interception seam, which M3b-1..9 explicitly do not depend on.
   - `the_over_ceiling_row_names_the_terms_that_dominate()` — Axum's
     envelope test runs `validate_isolate_limits` on its actual
     `envelope::limits` and `envelope::target` inputs and checks the
     worked component charges and total. The runtime arithmetic fixture
     is synthetic, not another copy of the shipped row.
   - `stream_gate_bounds_concurrent_live_bodies()` (100-call fanout,
     8 permits: high-water <= 8, positional results intact).
   - `later_wave_deadline_anchored_at_send_all_entry_bidder_wins_tie()`
     — round-98 M9.
   - `validate_isolate_limits_rejects_overcommit_overflow_and_bad_defaults()`
     — incl. the per-target defaults table (every VALIDATED_TARGETS
     member's envelope must validate; round-128 #8 — a shipped
     target absent from the list ships WITHOUT a memory certificate,
     and the table asserts that absence is EXPLICIT, never implied).
   - `per_auction_formula_charges_clones_outbound_and_retained_responses()`
     — the round-98 B3/B4 arithmetic (100 x 256 KiB = 25 MiB retained;
     50 x inbound clones; checked ops).
   - The SIX factor guards (round-100 B7 / round-108 B3 / round-126 —
     replace
     the retired single pipeline factor test). Round-117 P2-14: the
     factors are measured NATIVE-ONLY today; until each guard also
     runs on at least one wasm target (wasmtime for wasip1 is
     sufficient), every factor used in a WASM target's certificate
     carries an explicit x1.5 cross-target margin, recorded in that
     certificate — remove the margin only by replacing it with a
     wasm-measured value:
     `inbound_parse_factor_guard()`, `grouping_factor_guard()` (the
     input-derived maximal-cardinality fixture),
     `typed_clone_factor_guard()`, `response_parse_factor_guard()`,
     `assembly_factor_guard()`, `config_parse_factor_guard()` (the
     max-size envelope fixture against CONFIG_PARSE_FACTOR = 6) —
     each allocator-measured over the
     dimension it scales with (native-only).
   - `aux_routes_gated_and_charged()` +
     `aux_overload_precedes_other_rejections()` (round-100 B5).
   - The outbound-cap quartet (round-100 H6 / round-101 B3):
     `raw_over_cap_errors_during_construction_no_overalloc()`,
     `encoded_over_cap_rejected()`, `exact_cap_accepted()`,
     `compressed_larger_than_raw_counted()`.
   - Egress cases (Prereq-9-gated, FOUR — round-102 H13 aligns this
     roster with the deliverable list): `egress_eof_releases_permit()`,
     `egress_client_disconnect_releases_permit()`,
     `egress_slow_client_progresses_within_deadline()`,
     `egress_write_deadline_enforced_on_stall()`.
   - `parsed_gvl_expansion_within_pinned_factor()` — every manifest
     entry + the corrected generator (native-only).
   - **Round-107 H7 — the SIXTEEN round-106/107 contract cases
     (previously specified in the design section but missing from
     this roster):**
     - CallBudget (B3/B6): `call_budget_shared_across_sinks_exhausts_in_registry_order()`
       (two bidders, one budget: the second
       sink's first over-budget attempt rejects; order deterministic),
       `call_budget_rolls_back_on_build_failure()` (OversizeRaw →
       slot re-credited; a later bidder gets it),
       `call_budget_rolls_back_on_semantic_err_discard()` (accepted
       requests discarded ⇒ their slots return),
       `sink_error_list_bounded_with_suppressed_count()` (17th typed
       error suppressed, counter increments, memory flat),
       `budget_rejection_count_drives_10999_n()` (N =
       rejected_call_count; stop-at-first-exhaustion ⇒ N = 1).
     - Meta byte pool (round-106 H7): `meta_bytes_exact_cap_within_default_cap_allocation()`
       (allocator-pinned actual allocations
       — the default cap is the DERIVED 4_384, not a literal — at the
       exact-cap layout), `meta_over_cap_errors_without_pool_growth()`.
     - **⚠ ROUND-204 #2 — THE SPLIT RESTRUCTURE HAS NO OWNING UNIT,
       AND 2a.5 CANNOT SATISFY ITS GUARDS WITHOUT ONE.** 2a.5's
       acceptance requires all six factor guards green, including
       the two below, but 2a.5 is explicitly ARITHMETIC-ONLY — it
       computes the equation, it does not restructure the exchange.
       Neither of its prerequisites does either: `split.rs` still
       CLONES and materializes each per-bidder request inside its
       loop (`crates/auctioneer-exchange/src/split.rs`, the
       `br_value = request_value.clone()` in the `for (bidder_name,
       imps)` body) BEFORE any survivor pruning, so a typed-clone
       guard fails there by construction.
       **THE RESTRUCTURING IS ASSIGNED TO 2b.2**, which already owns
       the exchange-side fanout drive and is the only unit whose
       scope reaches `split.rs`: group → decide → materialize, so
       only SURVIVORS are materialized. **The two guards below move
       with it and are NOT 2a.5 acceptance.** 2a.5 keeps the four
       purely-arithmetic guards; a task cannot be accepted on a
       property its own scope forbids it from implementing.
     - Split restructure (B4) — **OWNED BY 2b.2** (round-204 #2):
       `grouping_worst_case_cardinality_no_typed_clones()`
       (round-109 M6 — the INPUT-DERIVED
       maximal-cardinality fixture: a request at exactly
       `inbound_request_bytes` packed with ~13k minimal distinct
       participants, plus the full registry and a `max_aliases` host
       overlay — the same fixture the `grouping_factor_guard()`
       uses; the earlier "registry ∪ max_aliases" wording described
       a far smaller set and is retired),
       `fpd_validation_runs_for_survivors_only()`,
       `decide_materialize_scrub_matches_m3a_pipeline_snapshot()`
       (mixed allowed/scrubbed/dropped/sampled-out/cap-dropped
       fixture; per-bidder wire equality).
     - Final response + egress acquisition (B2):
       `final_response_serializer_capped_exact_cap_accepted()`,
       `final_response_over_cap_internal_500_releases_permit()`,
       `egress_permit_acquired_before_serialization()` (no serialized
       buffer exists while queued — allocator-observed),
       `egress_wait_cancellation_leaves_queue_no_permit()`,
       `egress_wait_deadline_expiry_returns_503()`,
       `response_write_deadline_distinct_from_tmax()` (round-108 H4 —
       an auction that exhausts its tmax budget still serializes and
       returns its timeout RESPONSE under a fresh write deadline,
       never a 503).
     - Round-109 B2 finalize cases:
       `reserved_header_rejected_case_insensitive()` (adapter pushes
       "content-LENGTH", "Content-encoding", "sec-gpc", and "tE"
       (round-111 H5) → `ReservedHeader` at push time, per-bidder
       code-5; canonical casing rejected identically),
       `uncompressed_body_over_encoded_cap_rejected()` (no endpoint
       compression: a raw body within `max_outbound_raw_bytes` but
       over `max_outbound_request_bytes` → `OversizeEncoded` on the
       uncompressed path — the encoded cap applies even without
       gzip), `gpc_appended_only_by_finalizer()` (with `gpc == "1"`
       the finalized request carries exactly one `Sec-GPC: 1` in the
       finalize reserve; an adapter-supplied one was rejected at
       push), `finalize_never_writes_content_length()` (round-147
       #2: with AND without gzip, the finalized header set is the
       adapter's headers plus at most `Content-Encoding` and
       `Sec-GPC`; a case-insensitive scan finds no Content-Length —
       framing belongs to EdgeZero's dispatch normalization and the
       provider transport),
       `outbound_request_body_cap_equals_envelope_cap()` (round-148
       #2: the converted `OutboundRequest`'s `max_request_body_bytes`
       equals the envelope's `max_outbound_request_bytes` for an
       envelope set ABOVE EdgeZero's 8 MiB default, and a finalized
       body at exactly that cap dispatches without a slot
       `BadRequest`). Exact-cap encoded output stays covered by
       `exact_cap_accepted()` in the outbound quartet.
     - Round-109 H3 deadline-dataflow cases:
       `response_write_deadline_codegen_default_10000()`,
       `response_write_deadline_range_rejected_pair()`,
       `response_write_deadline_not_overlayable()`,
       `envelope_propagates_response_write_deadline()`.
     - Round-110 H5 response-header cases:
       `response_headers_over_byte_cap_rejected()` (8_193 summed
       bytes → `OversizeResponse`),
       `response_headers_over_count_rejected()` (65 headers),
       `many_large_duplicate_headers_rejected()` (64 duplicates of a
       maximal-size header — byte cap binds first; allocator-observed
       peak stays within the charged transient bound),
       and the round-112 B2 DISTINCT wire tests (native-only, real
       local sockets through the CONFIGURED hyper-level client):
       `h1_parser_rejects_over_limit_headers()` (65th header and an
       over-`max_buf_size` header block both fail IN THE PARSER, not
       post-send), `h2_field_section_three_boundaries()` (round-113
       H2 / round-115 B1 — the round-112 "exactly 10_336 with
       policy-valid content" case was impossible: the LARGEST
       policy-valid response is 8_192 + 64 x 32 + 42 [:status =
       7 + 3 + 32] = 10_282. Three boundaries, phrased against the
       EXCLUSIVE knob (`H2_FIELD_SECTION_KNOB = 10_337`; h2 rejects
       at `headers_size >= max`, so 10_336 is the largest ADMITTED
       section): a 10_282 policy-max response PASSES parser AND
       Layer 2; a 10_336 response is parser-ADMITTED but fails the
       Layer-2 logical check; 10_337 is PARSER-rejected — together
       proving the knob admits every policy-valid response and binds
       exactly above 10_336), and
       `h2_tiny_field_flood_bounded_then_logically_rejected()` (300
       tiny fields pass the parser, fail the Layer-2 count check;
       allocator-observed peak stays within the transient bound
       inside `HEADER_STREAM_CHARGE`).
     - Round-114 B1 over-cap classification wire tests (the
       round-113 cases primarily proved AT-cap behavior; these prove
       the OVER-cap typed path end to end — each asserts the
       positional result is `OversizeResponse { source: Headers }`,
       code 4 / seatnonbid 100, never a transport error):
       `h1_initial_headers_over_cap_classified_oversize()`,
       `h2_informational_1xx_over_cap_classified_oversize()`,
       `h2_trailers_over_cap_classified_oversize()`.
     - Round-113 H1 trailer/informational cases (native wire tests):
       `h2_trailer_flood_dropped_and_bounded()` (a body followed by a
       trailer block at the parser ceiling: the trailer never
       surfaces on the `Bytes`-only stream, the call succeeds, and
       the allocator-observed peak stays within
       `HEADER_STREAM_CHARGE = 42_656` — retained initial map + the
       concurrent trailer transient),
       `h1_chunked_trailer_flood_dropped_and_bounded()` (h1 chunked
       trailers parse inside `max_buf_size`, are dropped, never
       surface), and
       `informational_1xx_blocks_bounded_and_invisible()` (a 103
       Early Hints block before the final response is bounded by the
       same parser ceiling, dropped, and invisible to the caller;
       peak within the transient bound — it precedes any retained
       map).
     - Round-110 H6: `deadline_expiry_before_first_byte_returns_503()`
       (expiry between permit grant and first byte — after
       serialization — yields 503 with the buffer dropped and the
       permit released; distinct from the queued-expiry and mid-write
       cases).
     - Boot order (B5): `bootstrap_initializes_gcl_before_budget_install()`
       (per adapter — the factory body initializes the GCL
       static ahead of `validate_isolate_limits`).

---

**Goal:** Ship the pieces of M3 that need upstream EdgeZero work:
- `ProductionFanout` mid-fanout deadline + per-call timeout +
  per-bidder cancellation on Cloudflare, Fastly, and Spin.
- Streaming `max_bidder_response_bytes` cap that aborts BEFORE the
  full response is buffered.
- `RuntimeConfigError::MalformedHost` variant + the fail-loud handler
  match that distinguishes `MissingKey` from `Parse` /
  `Validation` errors on `AppConfig::from_store`.
- Snapshot fixtures for the mid-fanout timeout and oversize response
  paths.

**Architecture:** All three tracks depend on EdgeZero prereqs (see
end of document). Ship in this order once prereqs land:

1. **Fail-loud config track.** Round-31 M2: the canonical
   error is the core-owned `RuntimeConfigError` (round-30 H2),
   NOT `AuctionError` variants. Config loading occurs BEFORE
   `run_auction`, so `AuctionError::MalformedHost` /
   `AuctionError::ConfigStoreUnavailable` were unreachable in
   practice and are removed. Rewrite the handler match from
   M3a's `.unwrap_or_default()` shape to the round-19-drafted
   distinguishing match, producing `RuntimeConfigError`
   variants. Add regression tests.
2. **Streaming body cap track.** Rewire the bounded read to abort
   before the full response is materialised. Add memory-bound
   regression test that measures observed bytes ≤ cap + MAX_CHUNK_SIZE.
3. **Multi-adapter production fanout track.** Land the real
   `ProductionFanout::send_all` mid-fanout deadline + per-call
   timeout on Cloudflare, Fastly, Spin per adapter capability.
   Unignore the Bucket B rows Task 10 already committed with
   `#[ignore]`.

## Global Constraints (inherited from M3a)

- **No review-lifecycle markers in `.rs` / `.toml` source.**
  Same rule as M3a's Global Constraints. Code snippets in
  the task bodies below carry `Round-XX` markers for the
  document reader; **strip them when transcribing into
  source files** (round-38 M1).
- **Env var names use `__` for section separation** —
  `AUCTIONEER__GVL__ARCHIVE__DIR`, not `AUCTIONEER_GVL_ARCHIVE_DIR`
  (round-38 M1).

## Tasks (design-normative; step expansion gated on the prereqs)

> **Round-112 H3 — what "skeletal" means NOW (the old blanket label
> under-described 100+ review rounds of hardening):** the DESIGN
> layer of this plan is normative and review-frozen — the "Admission
> + whole-isolate budget" section, the Task M3b-2 deliverable list
> (items 1–5), the test roster (+65 per round-115 M3, +12 more from
> rounds 122-124 as enumerated in M3b-8, +1 round-126
> config_parse_factor_guard, +97 rounds 127-181 = +175, of which
> 168 land with M3b-2 and seven (six inbound parser-boundary + the
> manifest-class semantic assertion) with
> M3b-10 — round-125 #14 / round-140), the
> outbound
> sink/finalize/CallBudget contracts, and
> the prereq gates are the acceptance contract an implementation
> will be reviewed against — with ONE exception (round-124 #2): the
> worked CF envelope TABLE is NOT frozen; it is explicitly marked
> invalid pending the post-measurement re-derivation, which is a
> listed pre-M3b-2a-FINAL-ACCEPTANCE gate (round-136 #9 — the
> M3b-2a algorithms start before it; only the envelope constants
> wait on it). The METHOD the table demonstrates is frozen;
> its numbers are not. What is DEFERRED is only the mechanical
> expansion of each task body into M3a-style bite-sized TDD steps
> (write-test/run/implement/run/commit). ROUND-143 #8: the reason
> for deferral has CHANGED — the outbound, config-extraction,
> ingress and clock signatures now EXIST at `120dd2aa`, so
> expansion is no longer waiting on upstream: it is per-task work
> that must quote the pinned signatures verbatim and be re-reviewed
> before that task's implementation starts. Only the raw-ingress
> and response-egress steps (M3b-10, Prereq 9) still lack real
> signatures to quote. **Expansion protocol:** when a
> prerequisite lands upstream, the affected task's body is expanded
> against the REAL pinned APIs and re-reviewed BEFORE implementation
> starts (same gate discipline as M3a); the design layer above is
> not reopened by expansion.

- [x] **Task M3b-1a — PURE MAPPING (free-standing; round-123 #11
  makes the round-121 split executable).** Scope (round-141 #2 —
  named against the REAL upstream type): the exhaustive, TOTAL
  match over `EdgeError::StoreExtraction { reason:
  StoreExtractionReason }` (the round-140 #4 sixteen-variant
  mapping + fail-closed wildcard), `RuntimeConfigError` variant
  population, wire shapes, and the `allow_missing_config` boolean
  rule — NO lifecycle facilities (no admission, no envelope, no
  static-egress dependence). Prerequisite: upstream's typed
  store-error API only. STATUS — COMPLETE (round-143; mapping core
  landed round-142 with the pin move): `classify_store_extraction`
  in config/runtime.rs is the total sixteen-variant map with the
  fail-closed wildcard; the `allow_missing_config` bootstrap is
  implemented as defined (`data/runtime-bootstrap.toml`,
  `RuntimeBootstrapRaw` with `deny_unknown_fields` + strict bool in
  build_codegen.rs, `emit_runtime_bootstrap` in build.rs →
  `auctioneer_config::RUNTIME_BOOTSTRAP`, default `false`; `MissingBlob`
  degrades only under `true`, otherwise 500); ROUND-144 #1/#2: the
  ABSENT-BINDING precheck takes the SAME immutable rule (one
  `missing_config` fn for both states — a manifest missing its store
  no longer degrades silently) and the `#[non_exhaustive]` remainder
  fails closed with fixed text (`classify_load` is the pure
  classification, unit-tested with constructed outer errors carrying
  a canary that must never reach `detail`); the residual
  `Internal` / `BadRequest` / `ServiceUnavailable` arms carry fixed
  text, never upstream strings; per-variant unit tests pin every
  reason's class, exact `detail`, and rendered status, both bootstrap
  branches, the baked default, and the named
  `config_error_logs_structured_allowlist_no_raw_text()`; the
  fail-closed integration rows assert the new default on the wire
  (`missing_blob_fails_closed_under_default_bootstrap`,
  `accepted_store_key_with_no_blob_fails_closed_under_default_bootstrap`)
  . NOT DONE: the mutation kill — `cargo-mutants` is not
  installed in this environment; recorded as an open acceptance
  item to run before M3b-8, not claimed. Completion: mapping unit tests green per
  variant (each variant → exact status/body) — the MUTATION KILL over
  config/runtime.rs runs under Task M3b-8's gate (round-144 #7 moves
  it there explicitly; it is not a 1a exit criterion) — ZERO
  references to M3b-2 types. The ledger may complete 1a with M3b-2
  unstarted.

- [ ] **Task M3b-1b — LIFECYCLE INTEGRATION (after the M3b-2
  foundations it consumes).** Scope: the config-fetch deadline
  (round-119 F1), admission-lease interaction, static-class 503
  (round-180 #1 — "503/408" here contradicted this task's own
  "503, never 408" below; config-fetch expiry is 503 throughout)
  responses, exactly-once release, late-result no-op. Prerequisites:
  M3b-1a + M3b-2's AdmissionGate/boot-envelope/static-egress.
  Completion: the round-119 F1 test roster green (never-completing
  lookups, timeout under both allow_missing_config values, capacity
  recovery, late-result no-op).
  **EXECUTABLE SHAPE (round-177 #1):**
  - **Blocked by (round-179 #2; round-181 #1 adds 2a.7):** **2a.7a —
    it CREATES the `configure` callback (per-`App` setters:
    extraction limits, egress policy, observer handle) and installs
    the gates and boot envelope in `bootstrap_adapter`, and it owns
    the static-egress class this task consumes. M3b-1b ADDS to that
    callback; it does not create it. Round-183 #7: this bullet
    previously said 2a.7 installs the envelope and gates IN
    `configure`, repeating the ownership error corrected in 2a.7a;
    process-global state lives in `bootstrap_adapter`.** Also 2a.1
    (`AdmissionGate`/`AdmissionPermit`), 2a.5 (which COMPUTES the
    envelope — round-180 #1: it does not INSTALL it),
    2b.0 (the `RequestClock` the deadline is measured on), and 2b.2.
    NOT blocked by the batch rebase — its surface is config +
    admission, not outbound. NOTE it is ALSO gated on Prereq 10 for
    the real injected-grant path; until that lands the dev-mode
    fallback applies and the tests exercise that shape.
  - **⚠ ROUND-180 #1 — THE DEADLINE IS INSTALLED UPSTREAM, NOT
    WRAPPED LOCALLY.** EdgeZero already applies ONE extraction
    deadline across the root config AND every secret
    (`ConfigExtractionLimits { timeout, .. }`, installed via
    `App::set_config_extraction_limits`, enforced by the shared
    extraction budget in `extractor.rs`). A local `tokio::time`-style
    wrapper around auctioneer's store read would (a) leave upstream's
    default in force for the provider reads auctioneer does not wrap,
    (b) let root and secret timing diverge, and (c) silently compete
    with a budget that already exists. **auctioneer's job is to INSTALL
    its value, not to re-implement the mechanism:** `app!` accepts a
    `configure` callback (verified in `edgezero-macros/src/app.rs`)
    and auctioneer's invocation currently passes NONE
    (`crates/auctioneer-core/src/lib.rs`: `app!("../../edgezero.toml")`),
    so today auctioneer silently inherits upstream's default rather than
    its own `max_config_fetch_ms`.
  - **⚠ HISTORICAL (ROUND-183 #1), SUPERSEDED BY ROUND-186 #7 —
    THIS TASK NO LONGER OWNS AN OUTER RACE.** `get_bounded` is
    mandatory upstream with no compatibility default, so the
    never-ready gap this bullet was written to cover is closed at the
    source. Retained because it records why a local timer was
    considered and why racing Fastly's immediately-resolving timer
    would have been a defect. The superseded reasoning:
    **ROUND-183 #1 — THIS TASK OWNS THE OUTER RACE, AND THE
    "NO LOCAL TIMER" INSTRUCTION ABOVE IS SUPERSEDED FOR THE
    NEVER-READY CASE ONLY.** Round-180 #1 remains correct that auctioneer
    must not re-implement upstream's extraction deadline: 2a.7a
    installs it, and every read upstream can bound stays bounded
    upstream. But upstream's DEFAULT `get_bounded` checks the clock
    only before and after an unbounded provider call, so a provider
    future that never resolves never reaches the second check. That
    ONE case needs an outer bound, it needs a clock and a timer, and
    this task runs after 2b.0, which is where both exist. Scope: the
    race wraps auctioneer's store read only, it is NOT a general wrapper
    around every config read, and it is deleted when G20 lands.
  - **⚠ ROUND-186 #7 — THE OUTER RACE IS LARGELY RETIRED. UPSTREAM
    NOW REQUIRES `get_bounded` AND HAS NO COMPATIBILITY DEFAULT.**
    At head `03cdba98` the trait says it outright: "Implementations
    must define their deadline and allocation behavior explicitly.
    There is no compatibility default because awaiting an unbounded
    provider call can hide a never-ready future." That is G20's
    failure mode, removed at the source, so the never-ready case is
    no longer auctioneer's to cover with a duplicate race.
    **WHAT SURVIVES, AND IT IS NOT WHAT THE PREVIOUS REVISION
    SAID.** Upstream's capability text (verified in
    `docs/guide/capabilities.md` at head): Cloudflare and Spin race
    guest futures against NATIVE TIMERS; **Axum and Fastly provider
    operations are synchronous and guest-nonpreemptible**; and "no
    provider operation is proved cancellable within a finite
    wall-clock interval, so **every adapter reports `BestEffort` for
    `config-read-deadlines`**". So the split is NOT Axum-with-the-
    racers and Fastly-alone: Axum sits with Fastly on
    preemptibility, and ALL FOUR are BestEffort.
    **CONSEQUENCE FOR M3b-1b:** auctioneer calls `get_bounded` and maps
    its expiry onto `ConfigStoreUnavailable`. It adds NO outer race
    on any target — duplicating a bound upstream now mandates would
    re-create the competing-deadline problem round-180 #1 rejected.
    Every adapter's `config-read-deadlines` row reads BestEffort,
    matching upstream rather than claiming more.
  - **PER-TARGET BEHAVIOUR — HISTORICAL (round-183 #1), SUPERSEDED
    BY ROUND-186 #7 ABOVE. Retained because it records why a local
    timer was ever considered, and why racing Fastly's immediately-
    resolving timer would have been a defect:**
    - **Axum, Cloudflare, Spin:** the timer sleeps, so the read races
      `Timer::sleep(max_config_fetch_ms)` against the store future
      and expiry yields `ConfigStoreUnavailable` (503) whether or not
      the inner future ever completes.
    - **Fastly: DO NOT RACE THE TIMER.** `FastlyTimer::sleep`
      resolves IMMEDIATELY by design (see M3b-4's wave-loop bullet),
      so racing it would fire a spurious 503 on every config read
      before the store had any chance to answer.
      **⚠ ROUND-201 #2 — THE ANCHOR IS EXTRACTION START, NOT
      REQUEST START. DECIDED, AND THE UPSTREAM ASK IS FILED
      SEPARATELY.** M3b-1b specified `request_start + baked
      max_config_fetch_ms`, which is NOT reachable through the typed
      extractor: `ExtractionBudget::start` computes
      `started_at = clock.now()` at extraction entry and adds
      `ConfigExtractionLimits.timeout`
      (`edgezero-core/src/extractor.rs`), and that struct exposes a
      relative `Duration` only. **auctioneer adopts EXTRACTION-START
      semantics** — the contract auctioneer can actually implement
      today, with no bypass of the typed extractor and no
      duplication of its validation.
      **WHAT THIS COSTS, stated rather than hidden:** the bound
      covers the EXTRACTION, not the whole request. Upload time,
      parsing and any pre-extraction work fall OUTSIDE it, so a slow
      client can push total request time past
      `max_config_fetch_ms`. That is acceptable because the value
      exists to stop a hung STORE from hanging a request, and it
      still does exactly that; it is not a request-latency SLO.
      **THE UPSTREAM ASK IS GENERAL, NOT A AUCTIONEER SPECIAL CASE
      (filed as G28).** EdgeZero already treats absolute `Deadline`
      as its universal time primitive — `ConfigStore::get_bounded`
      takes one, `DispatchBudget` carries one, and
      `ResponseEgressDeadline` shipped `after(Duration)` /
      `at(Deadline)` for precisely this reason. Config extraction is
      the ONLY bounded path that accepts a relative duration and
      re-anchors internally. Any application bounding a whole
      request rather than one operation hits this: relative timeouts
      do not compose, since N bounded operations of `t` each can
      consume `N×t`. The ask is therefore additive and
      idiom-consistent, and auctioneer does NOT block on it.
      Fastly instead
      re-reads the clock after the store future returns and maps a
      crossed deadline onto the same `ConfigStoreUnavailable`. The
      never-ready future is consequently NOT bounded on Fastly; that
      is a declared BestEffort gap in Fastly's capability row,
      pinned by
      `fastly_config_deadline_is_besteffort_and_documented()`, and it
      closes when G20 lands rather than by a local workaround.
  - Files: **NOT the `app!` invocation — 2a.7 creates that callback
    (round-181 #1); this task only extends its body.** Modify `crates/auctioneer-core/src/config/runtime.rs`
    to map the upstream timeout error onto
    `RuntimeConfigError::ConfigStoreUnavailable`, bounding the read
    with the
    **upstream `get_bounded` deadline — NO local race and NO local
    timer (round-187 #5: this line previously said "host the outer
    race described above … the ONLY local timer this task adds",
    contradicting round-186 #7 six lines up. `get_bounded` is now
    mandatory upstream with no compatibility default, so the race it
    duplicated no longer has a gap to cover);**
    Modify
    `crates/auctioneer-core/src/handlers/auction.rs` — **the handler
    VERIFIES the injected grant, it does NOT acquire (round-179 #2:
    this recipe said "admission lease acquired BEFORE the config
    read", which contradicts the normative contract: acquisition
    happens in EDGEZERO INGRESS via the Prereq-10 opaque-guard
    protocol and the handler debug-asserts the injected permit. The
    handler-acquires phrasing survives ONLY as the dev-mode fallback
    on adapters predating Prereq 10)** — and takes the grant via
    `ctx.take_ingress_grant()`, holding it across the config read so
    release remains exactly-once on every exit path; Create
    `crates/auctioneer-core/tests/config_deadline.rs`.
  - Interfaces (Produces): a config read bounded by
    `max_config_fetch_ms` from the BAKED bootstrap value —
    **round-197 #4: this said "from the resolved envelope", which is
    circular; the envelope is resolved BY the store read this
    deadline bounds. The deadline is always
    **`extraction_started_at + baked max_config_fetch_ms`
    (round-201 #2)** — returning
    `RuntimeConfigError::ConfigStoreUnavailable` on expiry — **503,
    never 408 (round-179 #2: an earlier line in this task's scope
    mentioned a static 408; config-fetch expiry is specified as 503
    throughout, and 408 belongs to a different surface)** — and an
    admission grant whose release is idempotent.
  - **⚠ ROUND-183 #4 — THE CAPACITY-RECOVERY TEST IS GATED ON G21,
    AND THIS TASK'S DEPENDENCY LIST MUST SAY SO.** "A timed-out
    request returns its admission capacity" is only demonstrable once
    the response that carries the timeout TERMINATES observably. A
    503 config-fetch expiry goes out as a REFUSAL/static response,
    and at the recorded head EdgeZero's completion path does not
    cover detached egress, so nothing signals that the refusal
    finished and the permit may be reclaimed. Until G21 lands, this
    task demonstrates capacity recovery against the HANDLER-FUTURE
    boundary (the permit releases when the handler's response future
    completes) and explicitly does NOT claim recovery-on-terminal-
    transmission. **G21 joins this task's blockers and its dispatch
    row.**
  - **⚠ ROUND-190 #4 — THE NEVER-COMPLETING TEST MUST NOT CLAIM
    CANCELLATION, AND ITS TARGET LIST WAS WRONG.** This step listed
    Axum among the targets where a never-completing lookup "expires
    at the deadline", with only Fastly excluded. Upstream's own
    capability text says the opposite: **Cloudflare and Spin race
    guest futures against native timers, while AXUM AND FASTLY
    provider operations are synchronous and guest-nonpreemptible**,
    and **every** adapter reports `BestEffort` for
    `config-read-deadlines` because no provider operation is proved
    cancellable in a finite interval. Requiring Axum to expire a
    never-ready read is therefore not implementable — except by the
    local outer race THIS SAME TASK prohibits (round-186 #7). Do not
    reintroduce the race to satisfy a test the platform cannot
    honour.
    **WHAT THE TEST ASSERTS INSTEAD:** that auctioneer CALLS
    `get_bounded` through the TYPED EXTRACTOR with the baked
    `max_config_fetch_ms` as `ConfigExtractionLimits.timeout`
    (round-197 #4 — NOT "the envelope's deadline": the envelope does
    not exist yet; round-201 #2 — and NOT `request_start + _`: see
    the anchor bullet below) and maps its expiry
    onto `ConfigStoreUnavailable` — a contract auctioneer owns and can
    prove on all four targets. Whether the underlying provider future
    is actually cancelled is upstream's BestEffort classification,
    which auctioneer records and does not claim to strengthen. No
    auctioneer test may assert that expiry CANCELS the operation.
  - Step 1: the four failing tests — a never-completing lookup is
    bounded by `get_bounded`'s deadline rather than hanging the
    handler indefinitely, asserted as "auctioneer passes the deadline and
    maps the expiry", NOT as "the provider read is cancelled"
    (round-190 #4; BestEffort on all four targets, Axum included); the timeout
    classifies identically under BOTH `allow_missing_config` values
    (the bootstrap flag governs ABSENCE, not slowness); a timed-out
    request returns its admission capacity so the next request is
    admitted (scoped to the handler-future boundary per round-183 #4
    until G21 lands).
    **⚠ ROUND-191 #3 — THE "LATE STORE RESULT" TEST IS REMOVED.**
    It asserted that a provider result arriving AFTER expiry is a
    no-op that cannot double-release. Without the outer race
    round-186 #7 deleted, auctioneer never observes that result at all:
    it awaits `get_bounded`, which returns ONCE, and a late provider
    completion is upstream-internal. A test cannot assert a
    no-op it has no way to trigger.
    **WHAT REPLACES IT — a FAKE `ConfigStore` auctioneer fully
    controls:** `config_read_passes_the_envelope_deadline()` installs
    a stub whose `get_bounded` RECORDS the `Deadline` it was handed
    and returns `Err(DeadlineExceeded)`. It asserts (a) the recorded
    deadline equals **`extraction_started_at + max_config_fetch_ms`**
    from the baked value (round-201 #2 — this asserted
    `request_start + _`, which the typed extractor cannot produce,
    so the test would have failed against a correct
    implementation), and (b) the handler answers 503 +
    `Retry-After`. That tests the contract auctioneer actually owns —
    supply the bound, map the typed expiry — with no dependency on
    provider cancellation. Step 2: compile failure. Step 3:
    implement. Step 4: PASS. Step 5: commit `auctioneer-core: config
    fetch deadline + admission lease lifecycle`.

  **Shared task body (1a owns the mapping items, 1b the lifecycle
  items — each bullet below is tagged where ambiguous).** Round-28 H5
  rewrite:
  - **Taxonomy, RE-BASED round-139 #4 onto the real upstream
    direction (the round-28 `StoreAppConfigError` variant roster is
    RETIRED as an upstream ask — `StoreAppConfigError` and the
    variant names below survive ONLY as auctioneer's internal
    mapping-table labels):** M3b-1a consumes
    `EdgeError::StoreExtraction { reason: StoreExtractionReason, .. }`
    and maps the REAL `#[non_exhaustive]` sixteen-variant enum
    TOTALLY (round-140 #4 — every current variant settled, plus the
    wildcard; this corrects round-139's blanket secret→500, which
    mis-mapped the transient secret-backend case):
    - `MissingBlob` → the `allow_missing_config` rule (round-117
      P2-11 — the `allow_missing` form WINS over the old
      silent-default shorthand);
    - `BackendUnavailable` | `SecretBackendUnavailable` |
      `DeadlineExceeded` → `ConfigStoreUnavailable` (503 +
      `Retry-After`) — ALL transient, the secret backend and the
      store deadline included;
    - `BackendFailure` | `Deserialization` | `IntegrityMismatch` |
      `InvalidKey` | `InvalidSecretValue` | `MalformedEnvelope` |
      `MissingRegistry` | `MissingSecret` | `UnknownStore` |
      `UnsupportedVersion` | `Validation` | `ValueTooLarge` →
      `MalformedHost` (500);
    - `_` (future variants — the enum is `#[non_exhaustive]`) →
      `MalformedHost` (500), FAIL-CLOSED by default, with the
      unknown discriminant in the structured log.
    LOGGING (round-138 #5 / round-140 #4): the reason discriminant,
    the field path, and APPROVED size metadata ONLY — never message
    text, never `err.to_string()`.
    The task's body EXPANDS against the real `StoreExtractionReason`
    enum after the atomic pin move — and only once EdgeZero closes
    the distinctions the G4 residue names (some categories are
    still indistinguishable at the candidate head); until then the
    mapping table is frozen at category level.
  - **Round-121 #10 — M3b-1 splits into 1a/1b; the config-first
    ordering hid internal dependencies.** As amended (rounds 117-121),
    "complete M3b-1" consumes admission leases, the boot envelope, and
    static-egress refusal — all owned by M3b-2. M3b-1a (PURE MAPPING:
    the StoreAppConfigError match, RuntimeConfigError population, wire
    shapes; no lifecycle facilities) is implementable first and its
    completion criteria say so; M3b-1b (LIFECYCLE INTEGRATION: the
    fetch deadline below, lease interaction, static-class responses)
    lands after the M3b-2 foundations it consumes. The task DAG note
    in the banner is updated to match.
  - **Round-119 F1 — the loader runs on borrowed admission time and
    must be deadline-bounded.** After upload completes, the request
    HOLDS its admission lease while config extraction (store fetch +
    any secret-store work the extractor performs) runs — a phase
    covered by neither the ingress-read deadline nor the egress write
    deadline, so enough stalled lookups exhaust auction/aux capacity
    with zero auctioneer allocation to blame. Contract: the COMPLETE
    extraction runs under a boot-owned absolute deadline
    (`max_config_fetch_ms`, a BAKED bootstrap constant — round-191
    #2); expiry is REPORTED by `get_bounded` as
    `DeadlineExceeded`, which auctioneer maps to fail-loud 503 +
    `Retry-After` through
    the static-egress class (a timeout NEVER becomes bootstrap
    fallback — `allow_missing_config` is about ABSENT keys, not slow
    stores). **⚠ ROUND-191 #3 — THIS SAID "expiry CANCELS the
    pending lookup", CONTRADICTING THE SAME TASK'S ROUND-190
    WITHDRAWAL.** Whether the provider operation is actually
    cancelled is EdgeZero's business and is `BestEffort` on all four
    adapters; auctioneer supplies the deadline and maps the typed
    expiry, and claims nothing further. The lease releases exactly
    once through that response's
    lifetime, and a lookup completing after expiry is dropped: it must
    not dispatch an auction, install defaults, or double-release.
    Tests: never-completing config and secret lookups; timeout with
    `allow_missing_config` true AND false (both → 503); capacity
    recovery after response termination; late-result no-op.
  - **Shared loader helper — TIGHTENS the M3a shell.** Round-30
    H2: `crates/auctioneer-core/src/config/runtime.rs::resolve_
    runtime_config` already exists (M3a **Task 15 Step 4b** lands
    it with silent-degrade — round-84 L1, moved from Task 18). M3b
    Task M3b-1 replaces its body
    with the exhaustive fail-loud match over the store-extraction
    taxonomy (ROUND-140 #3 — the CONSUMED type is
    `EdgeError::StoreExtraction { reason: StoreExtractionReason }`;
    `StoreAppConfigError` in the bullets below is the HISTORICAL
    internal label, the round-140 #4 total mapping above is
    authoritative, and the round-138 #5 log-hygiene rule bans any
    `err.to_string()` logging these bullets once implied),
    populating the M3a-declared
    `RuntimeConfigError` variants:
    - `StoreAppConfigError::MissingKey { .. }` — round-39
      H4 canonical shape (retires round-30 H2's
      unconditional silent default). Distinguish
      "intentionally unconfigured deploy" from
      "production key was accidentally deleted":
      * `Ok(AuctioneerAppConfig::default())` ONLY when the baked,
        immutable runtime bootstrap sets
        `allow_missing_config = true`. THE BOOTSTRAP IS DEFINED
        (round-143 #5 — every element named). **ROUND-191 #2: this
        file is UNCHANGED by M3b — an intermediate revision proposed
        relocating it into `auctioneer.toml` and having `build.rs` read
        that; withdrawn, because `auctioneer.toml` must never be baked
        into the binary.** As shipped: OWNING TYPE
        `auctioneer_config::RuntimeBootstrap { allow_missing_config:
        bool }`; SOURCE FILE
        `crates/auctioneer-config/data/runtime-bootstrap.toml` with
        `[runtime]\nallow_missing_config = false`; CODEGEN PATH:
        `auctioneer-config/build.rs` reads it exactly as it reads
        `host-defaults.toml` (same `resolve_defaults_path` helper —
        round-144 #4 picks ONE contract: SOURCE-CONTROLLED POLICY
        ONLY. The policy changes by editing
        `runtime-bootstrap.toml` in the repository; there is NO
        deploy-selectable override. The
        `AUCTIONEER__RUNTIME_BOOTSTRAP__PATH_OVERRIDE` variable exists
        solely for the build-failure harness under the
        `test-harness-override` feature, exactly like the host
        defaults' override, and an ordinary build ignores it) and
        emits
        `pub const RUNTIME_BOOTSTRAP: RuntimeBootstrap` into
        `generated.rs`; DEFAULT `false` (the safer default: a lost
        production key fails loud); BOOT VALIDATION happens at BUILD
        time — a missing key, a non-boolean, or an unknown key under
        `[runtime]` is a build failure, so a shipped binary carries
        exactly one validated bool and no runtime environment or
        store value can flip it. RUNTIME RULE:
        `classify_store_extraction`'s `MissingBlob` arm consults the
        constant — `true` ⇒ the documented WARN + baked default;
        `false` ⇒ `MalformedHost` 500 (a bound store with no blob is
        a misconfigured deploy, the same class as a rejected key).
        (round-118 F14 —
        this task's earlier `bootstrap = "allow_missing"` string
        form and its `AUCTIONEER__RUNTIME__BOOTSTRAP` env override are
        RETIRED: the spec's round-117 amendment is authoritative,
        the policy is a boolean baked at deploy, and NO environment
        or store value can enable fallback — tested by asserting a
        runtime env/store opt-in cannot resurrect it; a staging
        opt-in must not survive into production key-loss).
        Documented as the opt-in for fresh deploys /
        pre-configuration rollouts.
      * `Err(RuntimeConfigError::MalformedHost { detail:
        "config key missing and bootstrap mode is
        require_present" })` when the bootstrap marker is
        absent OR set to `"require_present"` (the safer
        default). Emits 500 with the standard
        `Malformed host configuration` body.
      Regression tests (round-39 H4):
      - `missing_key_with_bootstrap_allow_missing_yields_default_config()`.
      - `missing_key_with_bootstrap_require_present_returns_malformed_host()`.
      - `missing_key_default_bootstrap_require_present_returns_malformed_host()`
        (asserts the SAFER default when neither env var
        nor baked config specifies).
    - (Round-142 #6: the two mapping bullets that stood here —
      which spelled `detail: err.to_string()` and `format!("{:?}",
      source)` — are DELETED, not merely annotated: they contradicted
      the authoritative privacy-safe rule. The mapping is the
      round-140 #4 total table; `detail` carries ONLY the reason
      discriminant, the field path, and approved size metadata.
      M3b-1a also REPLACES the current M3a arms in runtime.rs that
      still preserve upstream strings — `detail: source.to_string()`
      and the raw `message` pass-through — with that typed,
      sanitized metadata.)

    Signature stays `pub async fn resolve_runtime_config(ctx:
    &RequestContext) -> Result<AuctioneerAppConfig,
    RuntimeConfigError>` (round-30 H2: **not**
    `EdgeError` — the pinned upstream serialises `EdgeError`
    as JSON and offers no `Retry-After` configuration, so a
    plain-text 503 with a configurable header can't ride on
    it). Callers explicitly build the response via
    `match … Err(err) => return err.into_response()` (round-31
    B1 fix — the mapper returns `Result<Response, EdgeError>`,
    NOT `EdgeError`, so a bare `.map_err(...)?` shortcut
    doesn't apply). No caller signatures change.

    Config-reading routes touched by this Task:
    - `POST /openrtb2/auction` (auction handler).
    - `GET /status` (round-28 H5: today
      [`status.rs:14`](../../../crates/auctioneer-core/src/handlers/status.rs)
      silently substitutes `AuctioneerAppConfig::default()` on
      error, contrary to the [spec `/status` malformed-host
      rule](../specs/2026-05-21-auctioneer-openrtb-server-design.md#L3830)
      that requires 500 `Malformed host configuration`. M3b
      switches `/status` to the shared helper so the error
      path 500s uniformly).
    - `GET /info/bidders` and `GET /info/bidders/<name>`
      (round-28 H6 — bidder-info handlers currently only read
      static registry + host aliases; they must consume the
      runtime overlay so disabled bidders and runtime aliases
      are reported correctly).
  - **Every mapping row covered by a regression test** — one
    per taxonomy variant × each config-consuming route (so
    `/status` gets its own `status_returns_500_on_malformed_host()`
    regression, `/info/bidders` gets one for
    `MissingSecretBinding`, etc.).

  Regression tests cover EACH mapping row × EACH route.

- [ ] **Task M3b-2 — Bounded body cap AND admission/budget
  implementation. SPLIT (round-134 #7) into two slices so parallel
  implementation is real, not implied: **M3b-2a — pure
  foundations** (admission/budget/gate types: AdmissionGate,
  StreamGate, `per_stream_bytes` + `validate_isolate_limits`,
  `bounded_json`, PrepareLimits, sink + CallBudget — runtime-neutral,
  NO EdgeZero dependency. Round-135 #3 scopes its start boundary
  precisely: the ALGORITHMS — types, helpers, validators, and their
  tests over ARBITRARY inputs — are startable immediately; the
  CONCRETE TARGET ENVELOPES are not part of that start: the axum
  row's three MEASURED-PENDING placeholders re-run under §8, and
  the CF/Fastly/Spin rows do not exist until the measurement
  re-derivation — those constants land as M3b-2a's FINAL
  acceptance step, after measurement, never as inputs to the
  algorithm work) and **M3b-2b —
  EdgeZero integration — STATUS: UNIMPLEMENTED (round-143 #2; the
  accurate baseline at `120dd2aa`: `EdgeZeroProxyAdapter` calls
  `HttpClient::send` once per call with NO caps
  (`into_bytes_bounded(usize::MAX)`) and `ProductionFanout`
  `join_all`s those independent calls — concurrent, but not one
  batch dispatch, no dual caps, no positional slot metadata, no
  bounded facade). 2b DELIVERS: `BoundedFetch` (one batch per
  wave lease, per-call caps from the facade `ResponseBodyPolicy`,
  `BoundedSlotResult` with G15 elapsed), the wave scheduler on the
  application clock, the handler's move to `ctx.request_start()` /
  `ctx.monotonic_clock()` (both PRECEDED by **M3b-2b.0 — the
  clock-domain migration**, round-148 #1, specified in full at the
  head of the 2b expansion below: auctioneer still represents monotonic
  time as an epoch-relative `Duration` in `auctioneer_runtime::Clock::now_monotonic`,
  `FakeClock`, `AuctionDeadline.deadline_at`,
  `SystemClock`, `CloudflareClock`, the strategy/fanout drive and
  every exchange test, and `Deadline::at_instant` takes a
  `MonotonicInstant`), ingress verification, and — round-145
  #2 — the 3xx REJECTION line item the master spec mandates: the
  M3a classifier (`fanout.rs::classify`) rejects only status
  >= 500, so a received 302/307/308 falls through as a bidder
  response today; 2b makes `classify` reject EVERY 3xx as
  `BadServerResponse` (code 4, seatnonbid 100) WITHOUT parsing the
  body, pinned by `redirect_3xx_is_bad_server_response_without_parsing()`
  (302, 307, 308; the 307 carries valid-looking bid JSON
  and a parser-hook counter proves it is never parsed) and the
  per-adapter `redirect_destination_never_contacted_<target>()`
  (the `Location` target's listener records zero connections).
  Its outbound inputs are LANDED; only the raw-ingress/egress rows
  M3b-10 owns stay upstream-gated.** Roster members follow their deliverable
  into the slice that builds it.

  - [ ] **2b.0 — clock-domain migration (round-148 #1; FIRST unit of
    2b, before any batch work)**
    - Files: Modify root `Cargo.toml` (`[workspace.dependencies]
      web-time = "1"` — the SAME major as EdgeZero's pin, so Cargo
      unifies both on one `web-time` crate and
      `auctioneer_runtime::MonotonicInstant` IS
      `edgezero_core::time::MonotonicInstant` by type identity;
      auctioneer-runtime gains NO edgezero dependency),
      `crates/auctioneer-runtime/Cargo.toml` (`web-time = { workspace =
      true }`), `crates/auctioneer-runtime/src/clock.rs` (`pub type
      MonotonicInstant = web_time::Instant;` and `fn
      now_monotonic(&self) -> MonotonicInstant`; `Timer::sleep`
      keeps its RELATIVE `Duration` — callers derive it as
      `deadline_at.saturating_duration_since(clock.now_monotonic())`
      at arming time, exactly the round-136 #6 wave rule),
      `crates/auctioneer-runtime/src/fake_clock.rs` (the axis becomes
      `origin: MonotonicInstant` captured in `new()` + `offset:
      Duration`; `now_monotonic() = origin + offset`; `advance(delta)`
      adds to the offset; `advance_to(target: MonotonicInstant)`
      never moves backwards; sleeper wake instants are
      `MonotonicInstant`s; `advance_to(origin + Duration::MAX)`
      is replaced by `drain()` which wakes every registered sleeper),
      `crates/auctioneer-runtime/src/lib.rs` (re-export
      `MonotonicInstant`), `crates/auctioneer-exchange/src/fanout.rs`
      (`AuctionDeadline { deadline_at: MonotonicInstant, source }`;
      `BidderCallResult.elapsed` stays `Duration`),
      `crates/auctioneer-exchange/src/tmax.rs` (deadline composition
      `request_start.checked_add(effective_budget)` — **AS BUILT: the
      auction deadline is composed in `tmax.rs`, in `resolve_tmax`,
      which now returns `Result<TmaxResolution, AuctionError>`; the
      earlier `run.rs` location in this line was wrong**;
      **ROUND-154 #9 —
      ONE OVERFLOW POLICY for BOTH the auction deadline here and the
      per-call bidder deadline in `compose_all`: `checked_add`
      overflow is an INTERNAL ERROR, never a saturation.** The
      auction path returns `AuctionError::Internal` (500) and the
      fanout path `FanoutInternalError::Internal` (500, before any
      dispatch). **SCOPE, AS BUILT: only the auction site existed at
      2b.0's base — `compose_all` lives in
      `crates/auctioneer-core/src/outbound/wave.rs`, which 2b.2 CREATES,
      and the per-call bidder deadline was a relative
      `timer.sleep(call.timeout)` with no addition to overflow. 2b.0
      therefore retired exactly ONE saturation and invented no
      second one. THE POLICY BINDS 2b.2: when it lands `compose_all`,
      the per-call deadline composes with `checked_add` and maps
      overflow to `FanoutInternalError::Internal` before any
      dispatch — never a saturation.** Saturating to
      `DEADLINE_FAR_FUTURE` /
      `Duration::from_hours(168)` is RETIRED at both sites (neither
      constant exists in auctioneer; both live in `edgezero_core::time`):
      an
      instant that cannot be represented means the inputs are
      corrupt, and silently substituting a week-long deadline would
      hold bidder calls open instead of failing loudly. Both
      validators already bound `tmax` and `bidder_timeout`, so the
      arm is unreachable in practice and cheap to assert. Pinned by
      `deadline_overflow_is_internal_error_not_saturation()`;
      `remaining =
      deadline_at.saturating_duration_since(now)`; elapsed =
      `now.saturating_duration_since(t0)`),
      `crates/auctioneer-core/src/host_primitives/clock.rs`
      (`SystemClock::now_monotonic = MonotonicInstant::now()`; the
      `OnceLock` epoch is deleted),
      `crates/auctioneer-core/src/host_primitives/{fanout.rs,
      strategy.rs}` (the drive's `deadline_at` arithmetic and the
      `EncodeCostClock` test clock),
      `crates/auctioneer-core/src/handlers/auction.rs` (`request_start`
      is `ctx.request_start()` — same type, read as-is; the
      `TestClockHandle` override is retired for MONOTONIC time in
      the auction handler as round-143 #3 states — it survives
      ONLY as the wall-clock test override), NEW
      `crates/auctioneer-core/src/host_primitives/request_clock.rs`
      (round-149 #1 — ONE CLOCK SOURCE, not merely one instant
      type: a request-scoped composite
      ```rust
      pub struct RequestClock {
          monotonic: edgezero_core::time::MonotonicClock, // ctx.monotonic_clock()
          wall: Arc<dyn Clock + Send + Sync>,             // boot-installed host clock
      }                                                    // (wall time only)
      impl Clock for RequestClock {
          fn now(&self) -> OffsetDateTime { self.wall.now() }
          fn now_monotonic(&self) -> MonotonicInstant { self.monotonic.now() }
      }
      ```
      built ONCE per request in `auction()` from `ctx.monotonic_clock()`
      + `clock_handle()`, and the SAME `Arc<RequestClock>` is handed
      to BOTH `AuctionContext.clock` and `ProductionFanout::new(..)`
      by `fanout_from_context` — so `run_auction`'s
      `ctx.clock.now_monotonic()`, the wave race, the inclusive
      recheck and `Deadline::at_instant` all read EdgeZero's injected
      clock; `MonotonicInstant::now()` is never called on the
      auction path (it remains only inside `SystemClock` /
      `CloudflareClock`, which are wall+monotonic HOST clocks whose
      monotonic half the request path no longer consults). TEST
      SEAM: a `RequestContext` built with
      `MonotonicClock::new(move || fake.now_monotonic())` makes one
      `FakeClock` drive EdgeZero's clock, `request_start`, the
      exchange and the fanout together),
      `crates/auctioneer-core/tests/conformance/mod.rs`,
      `crates/auctioneer-adapter-cloudflare/src/clock.rs`
      (`MonotonicInstant::now()` — `web-time` backs it with
      `performance.now()` on wasm32-unknown-unknown; the
      `Date`-based `epoch_ms` subtraction is deleted),
      `crates/auctioneer-adapter-axum/tests/cancellation.rs`
      (`TransportTestTimer`), and every `now_monotonic` call site in
      `crates/auctioneer-exchange/tests/*.rs` and
      `crates/auctioneer-core/tests/*.rs` (the migration is a
      `cargo build --workspace --all-targets` sweep: the return-type
      change makes every stale `Duration` arithmetic a compile
      error, so no site can be missed).
    - Interfaces (Produces): `auctioneer_runtime::MonotonicInstant`;
      `Clock::now_monotonic() -> MonotonicInstant`;
      `AuctionDeadline.deadline_at: MonotonicInstant`;
      `FakeClock::{advance, advance_to, drain}`.
    - Step 1: failing tests:
      `monotonic_instant_is_edgezero_type_identity()` (auctioneer-core
      test: `let _: edgezero_core::time::MonotonicInstant =
      auctioneer_runtime::MonotonicInstant::now();` and
      `Deadline::at_instant(clock.now_monotonic())` compile — the
      whole point of the migration, pinned at compile time),
      `handler_and_fanout_share_ctx_monotonic_clock()` (round-149
      #1: the `RequestContext` carries a `MonotonicClock` whose
      instants sit ten years past `MonotonicInstant::now()`; after
      one auction the composed `deadline_at` and the fanout's
      recorded wave-entry instant both lie in THAT domain — proving
      neither path sampled a different source — and the closure's
      call counter is > 0 for both the exchange and the fanout),
      `auction_deadline_composes_on_instant_without_projection()`
      (exchange: `deadline_at == request_start + effective_budget`
      on a `FakeClock`; after `advance(budget)` the expired-budget
      short-circuit fires; after `advance(budget - 1ms)` it does
      not — the inclusive `deadline_at <= now` rule);
      `fake_clock_drain_wakes_every_sleeper()` (replaces the
      `advance_to(Duration::MAX)` idiom).
    - Step 2: `cargo build --workspace --all-targets` — the
      return-type change fails every stale site; Step 3: migrate
      until the workspace builds; Step 4: `cargo test --workspace`
      PASS (the existing timing suites are the regression net);
      Step 5: commit `auctioneer-runtime/exchange/core: monotonic time
      is web_time::Instant — one clock domain with EdgeZero`.

  - [ ] **2b.1 — code-12 attribution by the auction clock alone
    (round-151 #1)**
    - Files: Modify `crates/auctioneer-exchange/src/run.rs` (the
      post-fanout `auction_deadline_elapsed` computation).
    - Rule: `auction_deadline_elapsed = ctx.clock.now_monotonic() >=
      deadline.deadline_at` — and NOTHING else. Today's `||` over
      per-slot `Err(Timeout(TimeoutCause::AuctionDeadline))` results
      is DELETED: that cause is provenance (which composed budget was
      smaller at composition), not proof the auction deadline
      elapsed — a BestEffort target (Fastly, Spin) can report it for
      an earlier platform-phase timeout while the auction clock still
      has budget, exactly as upstream's batch-cutoff provenance can.
      **ROUND-159 #5 — the provenance names changed with the batch
      API: `BudgetSource::BatchDeadline` is RETIRED, replaced by
      `BudgetSource::RequestDeadline` (the per-request
      `Deadline::at_instant` expired) and `BudgetSource::BatchCutoff`
      (the batch's observation window closed).** The mapping covers
      BOTH FORMS of a cutoff, because the new API can express it two
      ways: a terminal `GatewayTimeout { cause: BatchCutoff }` in a
      completed slot, AND — the normal case — a slot simply
      UNRESOLVED when `next()` reports the cutoff, which carries no
      `EdgeError` at all. Both map to `Timeout(stored
      selected_cause)`; neither is evidence the AUCTION deadline
      elapsed (round-151 #1's clock rule is unchanged). Per-bidder code-1 entries are unchanged.
    - Step 1: failing tests (auctioneer-exchange, `FakeClock`):
      `code_12_requires_clock_expiry_not_slot_cause()` — a stub
      fanout returns one `Err(Timeout(AuctionDeadline))` slot while
      the clock is 100 ms SHORT of `deadline_at`: the bidder gets
      code 1 / statuscode 101 and `ext.errors.prebid` carries NO
      code 12; the same fixture with the clock advanced past
      `deadline_at`: code 12 exactly once;
      `batch_deadline_report_with_live_auction_is_bidder_timeout_not_code_12()`
      — the round-123 #15 roster member lands HERE, not in M3b-8.
    - Step 2: compile/fail; Step 3: implement; Step 4: PASS;
      Step 5: commit `auctioneer-exchange: code 12 is attributed by the
      auction clock only`.


  **M3b-2a EXPANSION (round-144 #5) — the executable steps, against
  the pinned tree. Scope is the ALGORITHMS only (round-135 #3): every
  step below is runtime-neutral, imports nothing from EdgeZero, and
  tests over arbitrary inputs; the concrete target-envelope constants
  are 2a's FINAL acceptance step after the §8 measurements and appear
  in no step here. Each sub-task is one commit; each step is one
  action; every test named is a roster member (its exact assertion is
  the roster bullet's text). Run each `cargo test` with
  `--all-targets` clippy under `-D warnings` before its commit.**

  - [ ] **2b.2 — `BoundedFetch` + the wave scheduler (THE "M3b-2b
    core" UNIT). SELF-REVIEW FINDING 1: five places in this plan
    block on "M3b-2b core" as if it were a defined unit, but 2b
    stopped at 2b.1 — the thing everything waits on had no files,
    interfaces, steps or tests and could not be dispatched even once
    unblocked. This is that unit.**
    - **BLOCKED ON: M3b-0 (the batch-API rebase) and M3b-11 (the pin
      move).** It is written against the batch API from the start,
      because by the time it runs that is the only API present.
    - **ALSO CREATES AXUM'S PROFILE (round-178 #3 — the canonical
      section requires `limits.rs::OUTBOUND_PROFILE` in all FOUR
      adapters, but only the three wasm adapters had an owning task;
      Axum's was required by nobody).** Create
      `crates/auctioneer-adapter-axum/src/limits.rs` with
      `OUTBOUND_PROFILE` — `elapsed_semantics: CompletionTime`,
      `deadline_support: Native` (Axum is the ONE target Native for
      both deadlines and cancellation) — and install it through
      `bootstrap_adapter` in that crate. It belongs here rather than
      in an adapter task because 2b.2 introduces the profile seam
      itself and Axum has no M3b integration task of its own.
    - Files: Create `crates/auctioneer-core/src/outbound.rs`
      (`BoundedProxyClientApi`, `BoundedFetch`, `BoundedOutboundRequest`,
      `BoundedSlotResult`, `BoundedSlotError`, `BoundedProxyResponse`,
      `CapRow`, `ResponseBodyPolicy`, `DecodePolicy`, `ComposedDeadline`),
      `crates/auctioneer-core/src/outbound/wave.rs` (`compose_all`,
      `run_wave`, the driver loop); Modify
      `crates/auctioneer-core/src/host_primitives/edgezero_proxy.rs`
      (`ProductionFanout` drives the wave scheduler instead of
      `join_all`), `crates/auctioneer-core/src/lib.rs` (exports),
      `crates/auctioneer-core/Cargo.toml` (the `measure-decoder` feature +
      optional `async-compression`, per round-162 #3's split).
    - Interfaces (Produces): the §5.2 algorithm in the master spec,
      implemented verbatim — `compose_all(calls, auction,
      fanout_entry_at) -> Result<Vec<BoundedOutboundRequest>,
      FanoutInternalError>`; `run_wave(wave, out, batch_entry_at,
      clock, http, limits, yielder)`; the exhaustive
      `OutboundBatchNext` match (Item / Finished(Completed) /
      Finished(Cutoff) / Err / non_exhaustive wildcard); the
      `OutboundBatchFailure { error, slots, .. }` handling;
      `OutboundCachePolicy::Bypass` on every converted request.
      **THE VERIFIED UPSTREAM SHAPES (round-169, read from head
      `5eccff8e`; build against THESE, not against paraphrase):**
      ```rust
      pub struct OutboundBatchItem { pub index: usize, pub result: OutboundSlotResult }
      pub enum OutboundBatchNext { Item(OutboundBatchItem), Finished(OutboundBatchTermination) }
      pub enum OutboundBatchTermination { Completed, Cutoff }
      pub struct OutboundBatchResults { pub slots: Vec<Option<OutboundSlotResult>>,
                                        pub termination: OutboundBatchTermination }
      pub struct OutboundBatchFailure { pub error: EdgeError,
                                        pub slots: Vec<Option<OutboundSlotResult>> }
      impl OutboundBatch {
          pub fn cancel(self) -> Vec<usize>;                    // unresolved indices
          pub async fn next(&mut self) -> Result<OutboundBatchNext, EdgeError>;
          pub async fn collect(self) -> Result<OutboundBatchResults, OutboundBatchFailure>;
      }
      ```
      All five types are `#[non_exhaustive]`, so every destructure
      needs `..` and every match needs a wildcard arm. THREE
      BEHAVIOURS the signatures do not show, each with a test:
      `batch_item_index_drives_position_not_arrival_order()` (M1 —
      a driver delivering indices 2, 0, 1 still publishes in input
      order); `no_explicit_cancel_on_the_normal_fanout_path()`
      (round-170 #1 — a handle that panics on `cancel()` survives a
      full wave, proving auctioneer never calls it);
      `completed_is_observed_on_the_poll_after_the_last_item()` (M4 —
      the loop keeps polling once its tally is full and still sees
      `Finished(Completed)`).
      **ONE PRODUCTION PATH (round-165 #5): `run_wave` uses the
      completion-driven `next()` loop and NOTHING ELSE.** The two
      collected forms (`send_all_until(..)` and
      `start_batch_until(..).collect()`) are NOT alternative
      implementations to choose between — the earlier "both collected
      forms" wording invited an implementer to pick one. They exist
      in this plan as the CONTRACT for how a caller destructures
      `OutboundBatchFailure`, and they belong in contract tests and
      doc examples, never in `run_wave`. A second live path through
      the scheduler would double the surface where the
      terminal/cutoff distinction can drift.
      // `DecodePolicy` — the auctioneer-owned decode caps, carried WITH
      // each slot because `to_outbound` CONSUMES the call and the
      // decoder runs after the slot returns (master spec §5.2):
      //   #[derive(Clone, Copy, Debug)]
      //   pub struct DecodePolicy {
      //       pub max_decoded_bytes: u64,
      //       pub max_brotli_wbits: u8,
      //       pub max_brotli_layers: u8,
      //       pub max_brotli_decoder_bytes: u64,
      //       pub max_chunk_bytes: NonZeroU64,
      //   }
      //   impl From<&ResponseBodyPolicy> for DecodePolicy { .. }
      // Pinned by `decode_policy_travels_with_the_slot()`, which
      // fails to compile if the type is absent. ROUND-166 #6: this
      // shape lived inside 2a.4a's "all `pub` from `auctioneer_bidder`"
      // interface list while saying its owner was 2b.2 — a
      // contradiction on the page. It is now HERE, in the unit that
      // owns both its file and its construction, and 2a.4a says
      // nothing about it.
    - Step 1: failing tests — the roster members already written for
      this unit: `on_time_slot_is_not_rewritten_by_batch_or_publication_time()`, `decode_policy_travels_with_the_slot()`,
      `decode_expiry_under_frozen_cpu_clock_is_timeout()`,
      `collect_failure_preserves_completed_slots_is_documented()`,
      `preflight_compaction_restores_indices_and_skips_empty_upstream()`, `bidder_calls_set_cache_bypass()`,
      **plus the three batch-contract members this unit owns
      (round-170 #3 — they were added to the global roster but never
      to this step, which is exactly the per-unit reconciliation
      failure the dispatch rule exists to prevent):**
      `batch_item_index_drives_position_not_arrival_order()`,
      `completed_is_observed_on_the_poll_after_the_last_item()`, and
      `no_explicit_cancel_on_the_normal_fanout_path()` (round-170 #1
      — replaces the retired `cancel_unresolved_indices_become_positional_timeouts`,
      which specified an unreachable path).
      NINE tests total; the unit is not complete until all nine are
      green.
    - Step 2: `cargo test -p auctioneer-core outbound` — compile failure;
      Step 3: implement against the rebased §5.2 algorithm; Step 4:
      PASS; Step 5: commit `auctioneer-core: BoundedFetch + the
      completion-driven wave scheduler`.

  - [ ] **2a.1 — `AdmissionGate` + `AdmissionPermit`**
    - Files: Create `crates/auctioneer-runtime/src/admission.rs`; Modify
      `crates/auctioneer-runtime/src/lib.rs` (`pub mod admission;` +
      re-exports).
    - Interfaces (Produces): `AdmissionGate::new(capacity: u32)`,
      `AdmissionGate::try_acquire(&self) -> Option<AdmissionPermit>`
      (atomic, no queue), `AdmissionPermit: Drop` releases exactly
      once; `AdmissionGate::in_use(&self) -> u32` for tests.
    - Step 1: write the failing tests in `admission.rs`'s `mod tests`:
      `admission_permit_releases_on_drop_including_panic()` (a permit
      dropped during a `catch_unwind` releases) and
      `admission_full_returns_none_without_allocating()` (capacity 1,
      second `try_acquire` is `None`; a counting allocator wrapper
      records zero allocations across the call).
    - Step 2: `cargo test -p auctioneer-runtime admission::` — expect
      compile failure (no module).
    - Step 3: implement over `AtomicU32` with `fetch_update` (CAS
      loop; never over-decrements); `Drop` does `fetch_sub(1)`.
    - Step 4: `cargo test -p auctioneer-runtime admission::` — expect PASS.
    - Step 5: commit `auctioneer-runtime: AdmissionGate — atomic
      try-acquire lease with RAII release`.

  - [ ] **2a.2 — `StreamGate` v3, weighted (`acquire_wave(k)`)**
    - Files: Modify `crates/auctioneer-runtime/src/admission.rs` (same
      module; `std::sync::Mutex` + `Waker`, wasm-clean).
    - Interfaces (Produces): `StreamGate::new(capacity: u32)`,
      `acquire_wave(&self, k: u32) -> StreamAcquire<'_>` (a `Future<
      Output = Result<WavePermit, WaveError>>`; `WaveError::{ZeroWave,
      WaveExceedsCapacity { requested, capacity }}` returned on the
      FIRST poll), `WavePermit: Drop` releases all k at once,
      `StreamAcquire: Drop` removes its waiter node and re-runs the
      grant scan; internal `GateInner { free, granted, order:
      VecDeque<u64>, nodes: HashMap<u64, Waiter { requested, state }>,
      next_id }` with the debug-asserted invariant `free + held +
      granted == capacity`.
    - Step 1: write the failing tests, one per roster bullet:
      `stream_gate_zero_wave_and_over_capacity_are_typed_errors()`,
      `stream_gate_grant_is_atomic_no_partial_hold()`,
      `stream_gate_mixed_sizes_are_fifo_with_head_of_line_blocking()`,
      `stream_gate_cancel_before_grant_releases_nothing()`,
      `stream_gate_cancel_after_grant_releases_all_k()`,
      `stream_gate_waiting_head_cancel_unblocks_satisfiable_next()`
      (capacity 8, free 4, head wants 8, next wants 1),
      `stream_gate_granted_then_dropped_hands_weight_onward()`,
      `stream_gate_waker_replacement_repolls_same_node()`,
      `stream_gate_bounds_concurrent_live_bodies()` (100 waves, 8
      permits: high-water ≤ 8, positional results intact), plus the
      two WAKE-BEHAVIOR tests (round-145 #4 — a no-op waker with
      manual repolling masks a gate that never wakes, wakes a
      replaced waker, or wakes under the mutex):
      `stream_gate_notifies_only_the_latest_waker_exactly_once()`
      (register counting waker A, repoll with counting waker B, then
      release: B's count is exactly 1 and A's is 0) and
      `stream_gate_wakes_outside_the_lock_reentrant_waker()` (a
      waker whose `wake` re-enters the gate — polls a second
      acquisition — completes; a wake issued while the mutex is held
      would deadlock or poison it). Drive futures with COUNTING
      wakers (a `Wake` impl over an `AtomicUsize`) and manual
      `poll`; the no-op waker is acceptable only for the pure
      error-path cases.
    - Step 2: `cargo test -p auctioneer-runtime stream_gate` — expect
      compile failure.
    - Step 3: implement the four rules exactly as the §3 state
      machine states them (grant scan, no stealing, cancellation-safe
      with rescan on BOTH arms, waker replacement).
    - Step 4: `cargo test -p auctioneer-runtime stream_gate` — PASS.
    - Step 5: commit `auctioneer-runtime: weighted StreamGate — FIFO
      all-or-nothing wave leases`.

  - [ ] **2a.3 — `bounded_json`: parser-time retained-node budget**
    - Files: Create `crates/auctioneer-runtime/src/bounded_json.rs`;
      Modify `lib.rs`; `Cargo.toml` gains `serde` + `serde_json`
      (dev + normal; both already workspace deps).
    - Interfaces (Produces): `pub struct NodeBudgetExceeded { pub
      budget: u32 }`, `pub enum BoundedJsonError { Syntax(serde_json::
      Error), NodeBudget(NodeBudgetExceeded) }`,
      `pub fn from_slice_within<T: DeserializeOwned>(bytes: &[u8],
      max_nodes: u32) -> Result<T, BoundedJsonError>`,
      `pub fn value_within(bytes, max_nodes) -> Result<serde_json::
      Value, BoundedJsonError>` (= `from_slice_within::<Value>`), and
      `pub const NODE_RETAINED_BYTES: u64 = 384` (2026-09-29 measured
      correction, superseding 64). Counting is
      SYNTACTIC (every value and every object key, skipped subtrees
      included): the wrapper intercepts `deserialize_ignored_any` with
      its own counting walker — the pinned serde_json consumes a
      skipped subtree in one call, so delegation would undercount
      (round-129 #6).
    - Step 1: write the failing tests:
      `bounded_json_counts_nested_skipped_subtrees_at_exact_boundary()`
      (a typed target with one known field; unknown nested
      object/array sized to land AT the budget → Ok, ONE over → Err),
      `bounded_json_counts_keys_and_ignored_any_via_both_entry_points()`
      (typed and `Value` entry points agree on the count),
      `bounded_json_syntax_errors_are_syntax_not_budget()`.
    - Step 2: `cargo test -p auctioneer-runtime bounded_json` — compile
      failure.
    - Step 3: implement the `Deserializer` wrapper (`serde::de::Deserializer`
      forwarding every `deserialize_*` through a
      counting `Visitor` wrapper; `deserialize_ignored_any` walks the
      subtree with `deserialize_any` + a counting `IgnoredAny`
      visitor).
    - Step 4: PASS; Step 5: commit `auctioneer-runtime: bounded_json —
      syntactic retained-node budget at parse time`.
    - (The `parsed_node_charge_guard()` allocation measurement is a
      2a.5 test: it needs the budget helper AND the allocator harness.)

  - [ ] **2a.4a — sink builder primitives (round-147 #4: the types
    2a.4's signature names did not exist anywhere in the repository;
    this unit creates every one of them so 2a.4 is executable)**
    - **Also creates `outbound_meta_bytes()` (round-163 #3)** — see
      the meta-layout block below; it is exported from the crate
      root beside the layout constants and is the ONLY value
      `auctioneer-runtime`'s budget validator takes from this crate.
    - Files: Create `crates/auctioneer-bidder/src/capped_writer.rs`,
      `crates/auctioneer-bidder/src/meta.rs`,
      `crates/auctioneer-bidder/src/endpoint_validator.rs`; Modify
      `crates/auctioneer-bidder/Cargo.toml` (add `url = { workspace =
      true }` — the workspace pin at the root `Cargo.toml` already
      exists; auctioneer-bidder currently lacks it and
      `EndpointValidator::validate` takes `&url::Url`),
      `crates/auctioneer-bidder/src/lib.rs` (`pub mod` + re-exports of
      every name below).
    - Interfaces (Produces), all `pub` from `auctioneer_bidder`:
      ```rust
      // capped_writer.rs
      pub struct CappedWriter<'a> { buf: &'a mut Vec<u8>, cap: usize }
      impl<'a> CappedWriter<'a> {
          /// ROUND-155 #6 — the memory invariant is ENFORCED, not
          /// merely documented: a `buf` whose capacity is below `cap`
          /// would reallocate mid-write and break the allocation
          /// bound the certificate charges. The constructor is
          /// therefore FALLIBLE and checks it.
          ///
          /// `buf` is pre-allocated by the OWNER (the sink for the raw
          /// body, the finalizer for the encoded body) with
          /// `Vec::with_capacity(cap)`; the writer NEVER grows it.
          ///
          /// # Errors
          /// `BodyWriteError::UnderCapacity { capacity, cap }` when
          /// `buf.capacity() < cap` (an empty `buf` with exactly
          /// `cap` capacity is the normal call).
          pub fn new(buf: &'a mut Vec<u8>, cap: usize)
              -> Result<Self, BodyWriteError>;
          pub fn write_all(&mut self, bytes: &[u8]) -> Result<(), BodyWriteError>;
          /// serde_json::to_writer through the cap; an over-cap
          /// failure surfaces as `OverCap`, any other serializer
          /// failure as `Serialize` — adapters never touch std::io.
          pub fn write_json<T: serde::Serialize + ?Sized>(&mut self, value: &T)
              -> Result<(), BodyWriteError>;
          pub fn written(&self) -> usize;
      }
      impl std::io::Write for CappedWriter<'_> { /* over-cap ⇒
          ErrorKind::WriteZero and the writer latches
          `over_cap`; `write_json` reads the latch to classify */ }
      #[derive(Clone, Copy, Debug, PartialEq, Eq)]
      pub enum BodyWriteError {
          OverCap { limit: u64 },
          Serialize,
          /// round-155 #6: the backing buffer could not hold `cap`
          /// bytes without reallocating.
          UnderCapacity { capacity: usize, cap: usize },
      }
      // meta.rs
      /// ROUND-163 #3 — THE BUDGET'S ONE BIDDER-DERIVED INPUT,
      /// owned HERE because this unit owns the byte pool layout.
      /// `validate_isolate_limits` takes its RESULT as a scalar
      /// (round-162 #3), so no layout constant crosses the crate
      /// boundary and nothing is re-derived in `auctioneer-runtime`.
      ///   = META_BYTES_CAPACITY + offset_table_bytes(max_imps) + 96
      /// where `offset_table_bytes(n) = 2*4 + 36*8 + n*4` rounded
      /// up to 64. Exported from `auctioneer_bidder`'s crate root.
      /// **This line was stale until 2a.4a implemented it: it read
      /// `((2 + 36 + n) * 4)` with a 64-byte tail, which yields
      /// 4_224 / 7_808 and contradicts the 4_384 / 8_032 this task's
      /// own tests assert. The corrected form above is the one at
      /// lines 2299 and 3793-3852, and it is the authority — n=100
      /// gives 704 and 3584+704+96 = 4_384; n=1000 gives 4_352 and
      /// 3584+4352+96 = 8_032.**
      #[must_use]
      pub const fn outbound_meta_bytes(max_impressions_per_auction: u16) -> u32;
      pub const META_BYTES_CAPACITY: usize = 3_584;
      pub const META_BIDDER_REGION_BYTES: usize = 3_328;
      pub const META_FINALIZE_RESERVE_BYTES: usize = 256;
      pub const MAX_BIDDER_HEADERS: u16 = 32;
      pub const RESERVED_EXCHANGE_HEADER_SLOTS: u16 = 4; // 2 used
      // ROUND-157 #4 — EXACT accessor signatures. The earlier
      // "accessors ... return &str" summary was not implementable:
      // the master spec iterates `meta.headers()` as (name, value)
      // PAIRS, and `covered_imp_ids()` yields many values. All
      // borrows are tied to `&self` (the byte pool lives in the struct),
      // so no item outlives the meta.
      pub struct OutboundRequestMeta {
          // ROUND-204 — RENAMED FROM `arena`. It is a flat byte
          // pool: the concatenated method, URL, header names/values
          // and imp ids, indexed by the (start, len) tuples below.
          // "Arena" named an allocator this is not — no bump
          // pointer, no typed sub-allocation, no grouped free — and
          // the stale `// METAerena` comment it carried is what a
          // meaningless name costs: nothing reads wrong, so nobody
          // catches it.
          bytes: Vec<u8>,                 // never realloc'd
          method: (u16, u16),             // (start, len) into `bytes`
          url: (u16, u16),
          headers: Vec<(u16, u16, u16, u16)>, // (n_start, n_len, v_start, v_len)
          imp_ids: Vec<(u16, u16)>,
      }
      impl OutboundRequestMeta {
          pub fn method(&self) -> &str;
          pub fn url(&self) -> &str;
          /// Insertion order, duplicates preserved (the sink appends;
          /// reserved names were rejected at push). Exact type:
          /// `impl Iterator<Item = (&str, &str)> + '_`, so the spec's
          /// `for (k, v) in meta.headers()` compiles as written.
          pub fn headers(&self) -> impl Iterator<Item = (&str, &str)> + '_;
          /// The imp ids this call covers, in append order, for
          /// `seatnonbid` mapping. Empty when the adapter did not
          /// pre-correlate.
          pub fn covered_imp_ids(&self) -> impl Iterator<Item = &str> + '_;
          /// Count accessors the sink's offset-table caps use, so a
          /// caller never has to collect an iterator to size-check.
          pub fn header_count(&self) -> u16;
          pub fn imp_id_count(&self) -> u16;
      }
      pub struct OutboundRequestMetaBuilder<'a> { /* &'a mut byte pool
          region [0, META_BIDDER_REGION_BYTES) + the offset tables
          sized from `max_impressions_per_auction` */ }
      impl OutboundRequestMetaBuilder<'_> {
          pub fn set_method(&mut self, method: &str) -> Result<(), SinkAbort>;
          pub fn set_url(&mut self, url: &str) -> Result<(), SinkAbort>;
          pub fn header(&mut self, name: &str, value: &str) -> Result<(), SinkAbort>;
          pub fn imp_id(&mut self, id: &str) -> Result<(), SinkAbort>;
      }
      #[derive(Clone, Copy, Debug, PartialEq, Eq)]
      pub enum MetaField { Method, Url }
      #[derive(Clone, Debug, PartialEq, Eq)]
      pub enum SinkAbort {
          MetaOverCap { limit: u64 },
          ReservedHeader { name: String },
          TooManyHeaders { limit: u16 },   // the 33rd adapter header
          TooManyImpIds { limit: u16 },    // offset table full
          DuplicateMeta { field: MetaField }, // second set_method/set_url
          Body(BodyWriteError),
      }
      impl From<BodyWriteError> for SinkAbort { .. }
      // endpoint_validator.rs
      pub trait EndpointValidator {
          fn validate(&self, url: &url::Url, info: &BidderRequestInfo<'_>)
              -> Result<(), EndpointRejectReason>;
      }
      #[non_exhaustive]
      #[derive(Clone, Copy, Debug, PartialEq, Eq)]
      pub enum EndpointRejectReason {
          SchemeNotAllowed, HostNotAllowed, IpLiteralRejected,
          TemplateUnresolved, PortNotAllowed, MalformedUrl,
          UserinfoRejected, AuthorityMismatch,
      }
      ```
      `EndpointBuilder::expand_into(&mut OutboundRequestMetaBuilder<'_>)
      -> Result<(), SinkAbort>` is added to
      `crates/auctioneer-bidder/src/endpoint_builder.rs` in this unit.
      **OPEN, FOR 2a.4 TO DECIDE (raised by 2a.4a): `SinkAbort` has
      no variant for a TEMPLATE failure, so `EndpointBuilderError`'s
      `MissingMacro` / `MalformedTemplate` currently collapse onto
      `MetaOverCap` — a misleading classification, since nothing was
      over cap. The variant list is an enumerated contract, so 2a.4a
      correctly did NOT extend it unilaterally. 2a.4 owns the call
      site: if it needs a template failure distinguishable from an
      over-cap one, add `SinkAbort::Template(EndpointBuilderError)`
      there. If it does not, the collapse stays and the doc comment
      on `expand_into` records it.**
      The EXCHANGE's implementation of `EndpointValidator` (today's
      `validate_endpoint(url: &str, info: &BidderInfo)` in
      `crates/auctioneer-exchange/src/validate_endpoint.rs`, re-shaped
      to return the typed reason) lands in M3b-2b; 2a.4 tests use a
      test-local `RejectingValidator(EndpointRejectReason)`.
    - Step 1: failing tests (in each new module):
      `capped_writer_exact_cap_passes_cap_plus_one_rejects()` (the
      cap-th byte is written; byte cap+1 returns `OverCap { limit }`
      and the backing `Vec`'s capacity is unchanged afterwards),
      `capped_writer_rejects_under_capacity_buffer()` (round-155 #6:
      `CappedWriter::new(&mut Vec::with_capacity(cap - 1), cap)`
      returns `UnderCapacity`, so no writer can exist over a buffer
      that would reallocate; the exact-capacity buffer is accepted),
      `capped_writer_write_json_over_cap_is_over_cap_not_serialize()`,
      `meta_builder_admits_32_adapter_headers()` (32 distinct
      non-reserved headers accepted; `headers()` yields all 32 in
      order), `meta_builder_rejects_33rd_adapter_header_without_growing()`
      (the 33rd `header()` returns `TooManyHeaders { limit: 32 }`;
      the byte pool's length and capacity are unchanged by the call),
      `meta_builder_reserved_name_rejected_case_insensitively()`
      ("content-LENGTH", "hOsT", "transfer-encoding" ⇒
      `ReservedHeader`), `meta_builder_over_cap_errors_before_write()`
      (a value that would cross `META_BIDDER_REGION_BYTES` returns
      `MetaOverCap` with nothing partially appended),
      `endpoint_reject_reason_is_exhaustive_over_validate_failures()`
      (a `match` over all eight variants compiles without wildcard —
      the compile-time exhaustiveness pin),
      `outbound_meta_bytes_matches_worked_values()` (round-163 #3 —
      **ROUND-202 #1 — PIN THE LAYOUT AGAINST THE BUDGET WITH A
      COMPILED ASSERTION, NOT PROSE.** The earlier figures were
      hand-derived and wrong twice (a 4-byte header entry that is 8,
      a 64-byte handle that is 96), and no test would have caught
      either.
      **⚠ ROUND-203 #3 — THE ASSERTION I WROTE WOULD HAVE FAILED,
      AND IT WAS ASSIGNED TO THE WRONG UNIT.** `size_of::<Accepted
      Request>() <= 96` is false: the compiled type is **120
      bytes**. My round-202 probe modelled the meta and reservation
      but OMITTED the body, and `AcceptedRequest { request:
      BidderHttpRequest, reservation }` carries the body's 24-byte
      `Vec` handle inside `request`. 96 is the size WITHOUT it.
      **The 4_384 / 8_032 charges are unaffected** — they cover the
      meta byte pool, the offset table and the handle, while the body is
      charged separately under `max_outbound_raw_bytes`, so nothing
      is double-counted and nothing is missing. Only the assertion
      was wrong.
      **CORRECTED, and it asserts the WHOLE representation rather
      than a subtracted one** (a subtraction is exactly the kind of
      hand-arithmetic that produced two wrong numbers in two
      rounds):
      ```rust
      const _: () = {
          // Pointer-width-independent: (u16,u16,u16,u16) is 8 on
          // every target, which is the fact the offset-table charge
          // depends on.
          assert!(size_of::<(u16, u16, u16, u16)>() == 8);
          // ROUND-204 #1 — A CONSERVATIVE UPPER BOUND, NOT `== 120`.
          // 120 is the 64-bit figure; on the THREE wasm targets
          // auctioneer ships (wasm32-unknown-unknown, wasm32-wasip1,
          // wasm32-wasip2) a `Vec` handle is 12 bytes, not 24, so
          // the same struct measures 64. An equality assertion
          // would have failed every wasm build on first compile.
          assert!(size_of::<AcceptedRequest<'_>>() <= 136);
          // **AS BUILT — 136, NOT 120. THIS FIGURE HAS NOW BEEN WRONG
          // FOR THREE CONSECUTIVE ROUNDS (96 → 120 → 136), ALWAYS FOR
          // THE SAME REASON: the breakdown below describes a
          // `{ meta, body, reservation }` struct, but the DEFINITION
          // above holds a `BidderHttpRequest`, and those are not the
          // same size. 2a.4 probed it rather than deriving it:
          // `BidderHttpRequest` = 120 on 64-bit, because it carries
          // FIVE pointer-width handles (`method`, `url`, `headers`,
          // `body`, `covered_imp_ids`), not the three the 120
          // breakdown counts. Plus `CallReservation` 16 = 136.
          // Round-203 #3 moved 96 → 120 by adding the body handle and
          // missed `method`/`url`/`covered_imp_ids`.
          // Cost of accepting 136: +16 B per retained call, so 1.6 KB
          // per auction at 100 calls, against a 256 KiB body term —
          // negligible, and `OUTBOUND_META_BYTES`'s 96 charge is
          // unaffected.
          // OPEN, FOR M3b-2b: switching `AcceptedRequest` to
          // `{ meta, body, reservation }` would make the 120
          // arithmetic correct AND is where round-202 #2 already
          // heads (`make_bids` taking `&OutboundRequestMeta`). It is
          // not a 2a.4 change: it alters `SinkOutcome.accepted`'s
          // normative element type and strands 18 existing
          // `BidderHttpRequest` construction sites, so it belongs
          // with the consumer migration, atomically — the same
          // argument round-203 #4 makes about privatisation.**
          // The superseded breakdown, kept so the error is visible:
          // 120 = OutboundRequestMeta 80 + body Vec handle 24
          //     + CallReservation 16 (64-bit); 64 on wasm32. The
          //     body is charged under max_outbound_raw_bytes, NOT
          //     under OUTBOUND_META_BYTES.
      };
      ```
      **THE CHARGES STAY AT THEIR 64-BIT VALUES.** A wasm target
      uses LESS than the reservation, so the bound still holds there
      — and sizing the charge to the smaller target would under-
      reserve on Axum. `<=` is the correct direction for a budget
      assertion: it catches a struct that GROWS past the charge,
      which is the failure mode, and stays silent when a target is
      cheaper than budgeted.
      plus `outbound_meta_bytes(100) == 4_384`.
      **OWNED BY 2a.4, NOT 2a.4a** (round-203 #3): 2a.4 CREATES
      `OutboundRequestSink` and `AcceptedRequest`, so 2a.4a cannot
      test a type its predecessor builds. 2a.4a keeps
      `outbound_meta_bytes`, which is its own.
      the helper this unit now owns: 4_384 at the default 100 imps,
      8_032 at the 1_000-imp codegen maximum, and monotonic in
      between, so the budget cannot diverge from the byte pool),
      `meta_builder_duplicate_method_or_url_setter_is_rejected()`
      (round-148 #4: the second `set_method` returns `DuplicateMeta
      { field: Method }`, the second `set_url` returns `DuplicateMeta
      { field: Url }`, and the first values are unchanged).
    - Step 2: `cargo test -p auctioneer-bidder --lib` — compile failure
      (round-148 #7: `cargo test` takes ONE positional filter; the
      three new modules are exercised by the crate's lib target);
      Step 3: implement;
      Step 4: PASS; Step 5: commit `auctioneer-bidder: sink builder
      primitives — CappedWriter, OutboundRequestMetaBuilder,
      EndpointValidator`.

  - [ ] **2a.4 — `CallBudget` + `OutboundRequestSink` reservation**
    - Files: Create `crates/auctioneer-bidder/src/sink.rs` (`SinkLimits`,
      `CallBudget`, `CallReservation`, `AcceptedRequest`,
      `OutboundRequestSink`, `SinkOutcome`); Modify `crates/auctioneer-bidder/src/lib.rs`
      (re-exports), `types.rs` (`OutboundBuildError` — gains
      `TooManyHeaders { limit: u16 }` / `TooManyImpIds { limit: u16 }`
      mirroring `SinkAbort`; `BidderHttpRequest` fields become
      `pub(crate)` with the read-only accessors — **⚠ ROUND-203 #4:
      NOT IN THIS UNIT UNLESS THE CONSUMERS MOVE WITH IT.**
      `BidderHttpRequest`'s fields are WRITTEN outside
      `auctioneer-bidder` today: `auctioneer-core`'s
      `host_primitives/fanout.rs` assigns `req.body` and both
      retains and pushes onto `req.headers` for the gzip path, and
      the exchange appends GPC headers the same way. **Read-only
      accessors cannot replace a write**, so privatising the fields
      while modifying only `auctioneer-bidder` breaks the workspace
      build at the moment it lands.
      **THE RULE: privatisation is ATOMIC with the consumer
      migration, or it does not happen in this unit.** Those writes
      are exactly the ones `finalize_for_transport` subsumes
      (round-150 #6 already deletes the in-drive gzip), so the
      natural home is M3b-2b, where the finalizer replaces them.
      Whichever unit does it, its Step 4 runs
      **`cargo build --workspace --all-targets`** at that boundary —
      a crate-local `cargo test -p auctioneer-bidder` would stay green
      while `auctioneer-core` no longer compiles).
    - Depends on: 2a.4a (every builder-side type above).
    - Interfaces (Produces): `CallBudget::new(max_calls: u32)` shared
      per auction (`&CallBudget` handed to every sink);
      **ROUND-160 #5 — THE `u64` → `usize` BOUNDARY IS RESOLVED AT
      BOOT, NOT AT THE WRITER.** `max_outbound_raw_bytes` arrives
      from config as `u64`, while `CappedWriter` and
      `Vec::with_capacity` take `usize` — which is 32-bit on
      `wasm32-*`, so a configured value above `u32::MAX` is
      representable in config and unrepresentable at the allocation
      site. DECISION: `SinkLimits` stores **`usize`**, converted ONCE
      at boot with a checked conversion, so no per-call site can
      fail:
      ```rust
      pub struct SinkLimits {
          pub max_outbound_raw_bytes: usize,     // round-160 #5
          pub max_impressions_per_auction: u16,
      }
      impl SinkLimits {
          /// PRIMITIVE-ONLY constructor (round-161 #1). It takes no
          /// envelope type, so `auctioneer-bidder` needs NO dependency on
          /// `auctioneer-runtime`.
          #[must_use]
          pub const fn new(max_outbound_raw_bytes: usize,
                           max_impressions_per_auction: u16) -> Self;
      }
      ```
      **ROUND-161 #1 — THE CONVERSION HAPPENS IN `auctioneer-runtime`,
      NOT HERE. The `from_envelope(&ResolvedIsolateLimits) ->
      Result<_, BudgetError>` method round-160 #5 put on `SinkLimits`
      was unbuildable**: it would have needed `auctioneer-bidder →
      auctioneer-runtime` (for the envelope type and `BudgetError`) while
      `validate_isolate_limits` in `auctioneer-runtime` called back into
      `auctioneer-bidder` — a cycle Cargo rejects. Verified against the
      tree: `auctioneer-runtime` has NO auctioneer dependencies at all, and
      `auctioneer-bidder` depends only on `auctioneer-config` and
      `auctioneer-openrtb`. It also made 2a.4 depend on types 2a.5 does
      not create until later.
      THE SHAPE INSTEAD:
      - `auctioneer-runtime::validate_isolate_limits` performs the
        `usize::try_from(max_outbound_raw_bytes)` itself and stores
        the CONVERTED value in `Derived` (e.g.
        `Derived.max_outbound_raw_bytes_usize: usize`), failing with
        `BudgetError::LimitExceedsUsize { field, value }` when the
        target cannot represent it. The check stays at boot, where it
        belongs.
      - `SinkLimits::new(..)` is called by `auctioneer-core` (or
        `auctioneer-exchange`) — both already depend on `auctioneer-bidder`
        AND `auctioneer-runtime`, so the composition site sees both types
        and neither library crate learns about the other.
      - 2a.4 therefore needs nothing from 2a.5: it consumes two
        primitives.
      Test:
      `validate_isolate_limits_rejects_raw_bytes_above_usize_max()`
      — in `auctioneer-runtime` (round-161 #1: the owner of the
      conversion): a configured `u64::MAX` (and, under a 32-bit
      target, `u32::MAX as u64 + 1`) yields `LimitExceedsUsize`; the
      boundary value itself is accepted and appears converted in
      `Derived`.
      `SinkLimits { max_outbound_raw_bytes: usize,
      max_impressions_per_auction: u16 }` (bidder-owned, `Copy`; the
      exchange copies both values out of the resolved envelope —
      round-146 #1: there is no `OutboundLimits` type anywhere in
      this plan or the spec); `OutboundRequestSink::new(limits:
      SinkLimits, budget: &CallBudget, validator: &dyn
      EndpointValidator)`, and the TWO-STAGE `push_request(&mut self,
      info: &BidderRequestInfo<'_>, build_metadata: impl FnOnce(&mut
      OutboundRequestMetaBuilder<'_>) -> Result<(), SinkAbort>,
      build_body: impl FnOnce(&mut CappedWriter<'_>) -> Result<(),
      BodyWriteError>) -> Result<(), OutboundBuildError>` — identical
      to the §4.1 normative signature: the closures speak the builder
      contract (`SinkAbort` / `BodyWriteError`) and the SINK maps
      them onto `OutboundBuildError` — with RESERVE → METADATA →
      VALIDATE(finalized url, info) → BODY → COMMIT (round-145 #1;
      any failure at any stage rolls the reservation back, and a
      rejected URL means `build_body` is NEVER invoked),
      `into_outcome()` / `into_outcome_discarding_accepted()`, the
      typed error list capped at 16 with a `suppressed_error_count`,
      `rejected_call_count` for code 10999.
    - Step 1: failing tests: `call_budget_exhausted_returns_slot_on_build_failure()`, `sink_error_list_bounded_with_suppressed_count()`
      (17th typed error suppressed, counter increments, the
      list's capacity never grows), `budget_rejection_count_drives_10999_n()`, `endpoint_validator_rejects_before_any_body_is_built()`
      (the eight `EndpointRejectReason` variants; the body
      closure PANICS if invoked, proving it never runs, and the
      reservation is returned),
      `missing_method_refunds_reservation_and_never_invokes_body()`
      and `missing_url_refunds_reservation_and_never_invokes_body()`
      (round-148 #4: a `build_metadata` that returns Ok without the
      setter yields `MissingMethod` / `MissingUrl`; at capacity one
      the next push_request succeeds — the slot was refunded — and
      the body closure PANICS if invoked),
      `semantic_err_discard_refunds_every_retained_reservation()`
      (round-148 #3: two accepted requests at capacity two, then
      `into_outcome_discarding_accepted()`; `try_reserve` succeeds
      twice more — nothing was committed during push_request).
    - Step 2: compile failure; Step 3: implement; Step 4: PASS;
      Step 5: commit `auctioneer-bidder: CallBudget + OutboundRequestSink
      with reserve/commit/rollback`.

  - [ ] **2a.5 — `per_stream_bytes`, `validate_isolate_limits`, and
    the factor guards. PURE ARITHMETIC AND VALIDATION ONLY
    (round-160 #6).**
    - **DEPENDS ON 2a.3 AND 2a.4a (round-161 #6 — it is NOT
      independently startable, as an earlier note claimed).** The
      per-auction formula consumes `NODE_RETAINED_BYTES` (2a.3's
      bounded-JSON budget) and the bidder metadata layout
      (`META_BYTES_CAPACITY`, `MAX_BIDDER_HEADERS`,
      `RESERVED_EXCHANGE_HEADER_SLOTS`, the offset-table sizing rule
      — all 2a.4a's).
      **`auctioneer-runtime` must NOT re-declare the bidder's numbers** —
      a second copy of `META_BYTES_CAPACITY` would silently diverge from
      the byte pool the sink actually allocates. It also cannot import
      them: `auctioneer-runtime` has no auctioneer dependencies and must keep
      none (round-161 #1).
      **ROUND-162 #3 — THE `BidderLayoutConstants` STRUCT IS
      RETIRED; ONE `u32` REPLACES IT.** That struct was wrong twice:
      it omitted inputs the `OUTBOUND_META_BYTES` formula needs (the
      offset-table sizing from `max_impressions_per_auction`, the
      struct overhead), so the runtime could not have computed the
      charge from it anyway; and it exposed `node_retained_bytes`,
      which is 2a.3's RUNTIME-OWNED constant that the runtime crate
      must hold itself rather than trust from a caller. RESOLUTION:
      - `auctioneer-runtime` owns every runtime constant INTERNALLY —
        `NODE_RETAINED_BYTES`, the H1/H2 slot peaks, every factor.
        No caller supplies them.
      - `auctioneer-bidder` owns the byte pool layout and EXPORTS the
        finished charge:
        ```rust
        // auctioneer-bidder — the ONE authority for this number
        #[must_use]
        pub const fn outbound_meta_bytes(max_impressions_per_auction: u16) -> u32;
        //   = META_BYTES_CAPACITY + offset_table_bytes(max_imps) + 64
        ```
      - `auctioneer-core` calls it and passes the single `u32` into
        `validate_isolate_limits` (signature above). No layout
        constant crosses the boundary and nothing is re-derived.
      **CREATED AND TESTED IN 2a.4a** (round-163 #3 — it had no
      owning unit: the validator required it and a roster row named
      its test, but no file list created it). 2a.5 merely CONSUMES
      the scalar. Pinned by
      `outbound_meta_bytes_matches_worked_values()` in
      `auctioneer-bidder` (4_384 at the default 100 imps; 8_032 at the
      1_000-imp codegen maximum — the plan's worked table), so a
      change to the byte pool or the offset table moves the budget with
      it by construction.
    - **SCOPE SPLIT, because the two halves are mutually ordered:**
      round-157 #3 put the codec-pin work in this unit while
      round-158 #2 moved the dependency and the measurement to
      M3b-2b, after M3b-11 lands the target tuple — and a unit
      cannot both precede and follow the pin move. RESOLUTION:
      **2a.5 ships the arithmetic and the budget validator with NO
      codec dependency and NO lockfile work** (it is runtime-neutral
      and startable now). **OWNERSHIP, single statement (round-163
      #2 — this sentence previously assigned BOTH artifacts to
      M3b-2b and was contradicted three lines later):
      `codec_pins_match_lockfile()` is M3b-11's** (it writes the
      assertion right after moving the pins), **and
      `brotli_state_measured` is M3b-2b's** (it measures decoder
      state against those pins). Both validate the constant 2a.5
      consumes; neither is part of 2a.5. The codec-pin paragraph
      below is retained as the SPECIFICATION of that later work, not
      as part of this unit:
    - **[OWNERSHIP, corrected round-162 #2 — the lockfile PIN TEST
      belongs to M3b-11, the MEASUREMENT to M3b-2b.]** The round-161
      split left an impossible order: M3b-11's acceptance invoked
      `codec_pins_match_lockfile()` and demanded both decoder audits,
      while this unit was to CREATE that test afterwards. Since
      M3b-11 is the task that moves the tuple into the lockfile, it
      also creates and runs the test that asserts the tuple —
      `crates/auctioneer-core/tests/codec_pins.rs`, a pure lockfile
      parse with no codec dependency and no allocator. **M3b-2b owns
      only what needs a real decoder:** the `measure-decoder`
      feature, the optional `async-compression` dependency, the
      allocator harness, and the measured value that validates
      `brotli_state_bytes`. M3b-11's acceptance therefore requires
      the PIN TEST green and upstream's published charge recorded —
      NOT auctioneer's measured audit, which cannot exist until 2b.
      **ROUND-164 #3 — SPLIT BY OWNER, because the paragraph below
      previously told M3b-2b to create the pin test the line above
      had just assigned to M3b-11:**
      - **M3b-11 creates** `crates/auctioneer-core/tests/codec_pins.rs`
        with `codec_pins_match_lockfile()`, immediately after moving
        the pins. It is a pure `Cargo.lock` parse: no codec
        dependency, no feature, no allocator.
      - **M3b-2b creates** only the `measure-decoder` feature, the
        optional `async-compression` dependency, and the allocator
        harness — the things that need a real decoder.
      The specification that follows describes BOTH; each half is
      built by the owner named above.
      Modify `crates/auctioneer-core/Cargo.toml` (the `async-compression`
      dependency as the shared-tuple rule spells it — M3b-2b), refresh
      `Cargo.lock` (M3b-11), and create
      `crates/auctioneer-core/tests/codec_pins.rs` (M3b-11) with
      `codec_pins_match_lockfile()` — it parses `Cargo.lock` and
      asserts the six asserted versions in the table above. For the
      crates auctioneer does not name as direct dependencies
      (`brotli`, `brotli-decompressor`, `compression-codecs`,
      `compression-core`), the test is the ONLY witness, so it also
      asserts each appears EXACTLY ONCE in the lockfile — a second
      entry would mean the measured decoder is not the one that runs.
      **THE MEASUREMENT HARNESS — OWNERSHIP AND SHAPE (round-158 #2;
      the earlier one-line command named a feature, a test and a
      crate that do not exist, in a crate with no decoder dependency
      at all).** It does NOT live in `auctioneer-runtime`: that crate is
      runtime-neutral (`Clock`/`Rng`/`Timer`, three dependencies) and
      must stay so, while the decoder dependency belongs to
      `auctioneer-core`. SPLIT, therefore:
      - **`auctioneer-runtime` keeps the ARITHMETIC only** —
        `brotli_state_bytes(wbits) = BROTLI_DECODER_FIXED_CHARGE_BYTES
        + 2^wbits`, a pure function with no codec dependency.
        **INVALID INPUT (round-160 #6): the signature is
        `pub fn brotli_state_bytes(wbits: u8) -> Result<u64,
        BudgetError>`, because `1u64 << wbits` is undefined for
        `wbits >= 64` and meaningless outside the validated RFC 7932
        range.** It returns `BudgetError::BrotliWbitsOutOfRange
        { wbits }` for anything outside `10..=24` — the same range
        `validate_isolate_limits` enforces — so a caller that skips
        validation cannot shift its way to a bogus budget. Test:
        `brotli_state_bytes_rejects_out_of_range_wbits()` covers 9,
        25, 64 and 255, and accepts 10 and 24. Its
        unit tests assert the arithmetic at wbits ∈ {10, 22, 24}
        — **round-159 #4: `brotli_state_bytes(24) == 33_554_432`
        (16_777_216 + 2^24), NOT 34_365_440.** The 34_365_440 figure
        is the FULL `per_stream_bytes` value at wbits 24, which adds
        811_008 bytes of transport/decoded buffers and chunk staging
        on top of the decoder state. The two assertions stay
        SEPARATE, and 2a.5 does not start until both read correctly:
        `brotli_state_bytes(24) == 33_554_432` and
        `per_stream_bytes(.., wbits = 24, ..) == 34_365_440`.
        (`brotli_state_bytes(10) == 16_778_240`;
        `brotli_state_bytes(22) == 20_971_520`.) No harness needed.
      - **`auctioneer-core` owns the MEASUREMENT**, because it owns the
        decoder. New optional feature in
        `crates/auctioneer-core/Cargo.toml`:
        ```toml
        [features]
        measure-decoder = ["dep:async-compression"]
        [dependencies]
        async-compression = { workspace = true, optional = true }
        ```
        New test `crates/auctioneer-core/tests/brotli_state_measured.rs`,
        `#![cfg(feature = "measure-decoder")]`, containing:
        (i) a counting global allocator (`#[global_allocator]`,
        tracking live bytes with a high-water mark) — it is a
        SEPARATE TEST BINARY precisely so the allocator is isolated
        from every other test; (ii) for each wbits ∈ {10, 22, 24},
        construct `async_compression::futures::bufread::BrotliDecoder`
        over a fixture stream whose header declares that window,
        drive it to EOF, and read the high-water delta; (iii) assert
        `observed <= brotli_state_bytes(wbits)` and print both so the
        commit message can record them.
      - **CODEC INTEGRATION IS NOT PART OF THE 2a FOUNDATIONS.**
        2a is runtime-neutral algorithm work; adding a decoder
        dependency and an allocator harness to it would break that
        property. The feature, the test and the manifest edits above
        land in **M3b-2b**, alongside the passthrough decoder they
        measure. 2a.5 ships `brotli_state_bytes` and consumes its
        VALUE; the measurement that validates the constant is a 2b
        deliverable, gated on M3b-11 having moved the tuple.
      MEASUREMENT COMMAND, recorded with its result in the M3b-2b
      commit message: `cargo test -p auctioneer-core --features
      measure-decoder --test brotli_state_measured -- --nocapture`.
    - Files: Create `crates/auctioneer-runtime/src/budget.rs`
      (`ResolvedIsolateLimits`, `TargetPlatformConstants`,
      `BudgetError`, `brotli_state_bytes`, `per_stream_bytes`,
      `validate_isolate_limits`, `Derived`); Modify `lib.rs`;
      **plus the allocator harnesses in the crates that OWN their
      measured types (round-205 #2, completed round-206 #2): Create
      `crates/auctioneer-openrtb/tests/parse_factors.rs`
      (`inbound_parse_factor_guard` — the typed `BidRequest` parse)
      and `crates/auctioneer-core/tests/config_parse_factor.rs`
      (`config_parse_factor_guard`, over the real resolve path);
      Modify `crates/auctioneer-openrtb/Cargo.toml` to add
      `auctioneer-runtime` under `[dev-dependencies]` so the guards can
      import the factor constants (dev-only — the shipping graph is
      unchanged and `auctioneer-runtime` still depends on no auctioneer
      crate). **`parsed_node_charge_guard` is NOT here**: it
      measures retained `serde_json::Value` nodes and belongs to
      `crates/auctioneer-exchange/tests/node_charge.rs`, with
      `response_parse_factor_guard` and `assembly_factor_guard` in
      `crates/auctioneer-exchange/tests/response_factors.rs` — all
      three owned by 2b.2.**
      **AS BUILT — OWNERSHIP MOVED: 2a.5 already created
      `crates/auctioneer-exchange/tests/node_charge.rs` and landed
      `parsed_node_charge_guard` in it, because that guard needs
      BOTH `bounded_json`'s `NODE_RETAINED_BYTES` and the
      counting-allocator harness, which 2a.5 owns. 2b.2 therefore
      MODIFIES that file rather than creating it, and still owns
      `response_parse_factor_guard` and `assembly_factor_guard` in
      the separate `response_factors.rs`. 2b.2 must not recreate
      `node_charge.rs` or it will clobber a landed guard.**
      `auctioneer-runtime` names no auctioneer type, so the measurements
      cannot live beside the arithmetic.
    - **ONE error type (round-154 #8): `BudgetError`.** The name
      `AuctioneerBootError` is RETIRED from this plan — earlier text used
      the two names for the same boot-time failure set, with no
      conversion defined, which would have let two implementers
      invent incompatible APIs. `BudgetError` is the public, exported
      name; every fallible function in this module returns it
      (`thiserror`-derived, one variant per check: overflow,
      brotli-window out of range, guest-bound below minimum, budget
      over ceiling, and the knob/ceiling assertions).
    - Interfaces (Produces): the five-parameter max-rule
      `per_stream_bytes(max_transport, max_decoded, wbits, layers,
      guest_decoder_bound_bytes) -> Result<u64, BudgetError>`
      with `BROTLI_DECODER_FIXED_CHARGE_BYTES = 16_777_216`;
      `validate_isolate_limits(&ResolvedIsolateLimits,
      &TargetPlatformConstants, outbound_meta_bytes: u32)
      -> Result<Derived, BudgetError>` (round-162 #3 — the ONE
      canonical signature; every other spelling in this plan is
      superseded) computing every term of the equation — including
      `max_retained_nodes x NODE_RETAINED_BYTES`, `max(H2_SLOT_PEAK,
      H1_SLOT_PEAK)`, the knob == ceiling + 1 assertion, the
      pseudoheader-allowance check, and the static-egress term.
    - Step 1: failing tests: `brotli_state_bytes_is_fixed_charge_plus_window()`
      (round-159 #4 — the DECODER-STATE arithmetic
      alone: 16_778_240 at wbits 10, 20_971_520 at 22, **33_554_432
      at 24**; it is NOT 34_365_440, which is the per-stream total),
      `per_stream_bytes_matches_the_worked_values()`
      (wbits 22 → 21_782_528 at the shipped 256 KiB caps
      with guest bound 0; wbits 24 → 34_365_440 = the 33_554_432
      decoder state + 811_008 of transport/decoded buffers and chunk
      staging; guest bound above the chain wins the max),
      `validate_isolate_limits_rejects_overcommit_overflow_and_bad_defaults()`,
      `per_auction_formula_charges_clones_outbound_and_retained_responses()`,
      **⚠ ROUND-205 #2 — THE FACTOR GUARDS ARE NOT IN THIS UNIT'S
      CRATE, AND ROUND-204 #2 ONLY FIXED THE COUNT.** Round-204
      narrowed six guards to four but left all four in 2a.5, whose
      Files list is entirely `crates/auctioneer-runtime/`. Each guard
      ALLOCATOR-MEASURES A REAL TYPE:
      `config_parse_factor_guard` measures `AuctioneerAppConfig`
      (`auctioneer-core`), `inbound_parse_factor_guard` and
      `parsed_node_charge_guard` measure `BidRequest`
      (`auctioneer-openrtb`). **`auctioneer-runtime` depends on NO auctioneer
      crate** — verified: its `[dependencies]` are `async-trait`,
      `http` and `time` — so it cannot name any of them. Requiring
      the measurement there is the same cycle round-161 #6 rejected
      for `SinkLimits::from_envelope`.
      **⚠ ROUND-206 #2 — THREE GAPS IN THE ROUND-205 SPLIT, FIXED
      HERE.**
      - **`parsed_node_charge_guard` measures the WRONG TYPE.**
        Round-205 sent it to `auctioneer-openrtb` alongside
        `inbound_parse_factor_guard`, but `NODE_RETAINED_BYTES`
        charges RETAINED `serde_json::Value` nodes — the
        representation `auctioneer-exchange`'s `split.rs` holds
        (`//! M2 uses serde_json::Value`) — not a typed `BidRequest`.
        Its four adversarial shapes exercise Value trees, so
        measuring `BidRequest` would prove a different claim.
        **It goes to `auctioneer-exchange`**, whose `split.rs` retains
        them: `crates/auctioneer-exchange/tests/node_charge.rs`.
      - **`response_parse_factor_guard` and `assembly_factor_guard`
        had no files and no owning task at all.** Both measure
        RESPONSE-side allocations — the parsed bidder response and
        the assembled `BidResponse` — which live in
        `auctioneer-exchange`: `crates/auctioneer-exchange/tests/
        response_factors.rs`, owned by **2b.2** with the rest of the
        exchange-side work.
      - **`auctioneer-openrtb` needs a dev-dependency to see the
        constants.** Its guards assert against the factor constants
        2a.5 exports from `auctioneer-runtime`, which it does not
        currently depend on. **Add `auctioneer-runtime` under
        `[dev-dependencies]`** in `crates/auctioneer-openrtb/Cargo.toml`
        — dev-only, so the shipping dependency graph is unchanged
        and no cycle is created (`auctioneer-runtime` still depends on
        no auctioneer crate). The same applies to `auctioneer-exchange`,
        which already depends on it at runtime.
      **THE SPLIT, BY WHERE THE MEASURED TYPE LIVES:**
      - **2a.5 keeps the ARITHMETIC** in `auctioneer-runtime`:
        `brotli_state_bytes`, `per_stream_bytes`,
        `validate_isolate_limits`, and the factor CONSTANTS. Its
        tests take primitives only — no auctioneer type crosses the
        boundary. Acceptance: `cargo test -p auctioneer-runtime`.
      - **The allocator harnesses move to the crates that own their
        types.** `config_parse_factor_guard` → a `auctioneer-core`
        INTEGRATION test (`crates/auctioneer-core/tests/
        config_parse_factor.rs`), since it needs the real resolve
        path, not just the struct. `inbound_parse_factor_guard` and
        `parsed_node_charge_guard` → `auctioneer-openrtb`. Each asserts
        its measurement against the constant 2a.5 exports, so the
        two halves cannot drift.
      - **Acceptance runs BOTH:** `cargo test -p auctioneer-runtime -p
        auctioneer-core -p auctioneer-openrtb`. A runtime-only command would
        stay green while every guard was missing.
      The guards, with their new homes:
      `inbound_parse_factor_guard` and `parsed_node_charge_guard()`
      (auctioneer-openrtb), `config_parse_factor_guard` (auctioneer-core
      integration), and the remaining allocator-measured dimension,
      **⚠ ROUND-204 #2 — THE GROUPING AND TYPED-CLONE GUARDS ARE
      NOT THIS UNIT'S.** This step said SIX. Two of them
      (`grouping_factor_guard`, and the
      `grouping_worst_case_cardinality_no_typed_clones()` case that
      shares its fixture) depend on the group → decide → materialize
      restructuring of `split.rs`, which 2a.5's arithmetic-only
      scope forbids it from doing and which neither prerequisite
      performs. **They move to 2b.2 with the restructuring**; 2a.5
      cannot be accepted on a property its own scope excludes.
      `validator_regenerates_the_synthetic_worked_row()` — this last one
      asserts the FORMULA against fixed synthetic inputs, not against
      any adapter's shipped constants or measured reservations.
    - Step 2: compile failure; Step 3: implement (checked arithmetic
      everywhere; `BudgetArithmeticOverflow` on any overflow);
      Step 4: PASS on **`cargo test -p auctioneer-runtime -p auctioneer-core
      -p auctioneer-openrtb`** (round-205 #2 — a runtime-only command
      stays green while every allocator guard is missing;
      `auctioneer-exchange` is NOT in 2a.5's command because its three
      guards belong to 2b.2 and run with that unit, round-206 #2); Step 5:
      commit `auctioneer-runtime: isolate budget model —
      per_stream_bytes, validate_isolate_limits, factor guards`.

  - [ ] **2a.6 — `PrepareLimits` as `prepare()` step 0**
    - Files: Modify `crates/auctioneer-openrtb/src/prepare.rs` (signature
      gains `&PrepareLimits`), `crates/auctioneer-openrtb/src/lib.rs`
      (export), every in-tree caller (`crates/auctioneer-core/src/
      handlers/auction.rs` and the trusted-server path) — one
      mechanical sweep, same commit.
    - Interfaces (Produces): `pub struct PrepareLimits { pub
      max_impressions: u32 }`; step 0 rejects `imp.len() >
      max_impressions` with `ValidationError::TooManyImpressions {
      got, max }` BEFORE any per-imp allocation.
    - Step 1: failing tests: `prepare_rejects_over_limit_impressions_before_allocation()`
      (counting allocator: zero per-imp
      allocations on the reject path), `prepare_accepts_exactly_the_limit()`.
    - Step 2: compile failure at every caller (the sweep is the
      fix); Step 3: implement + migrate callers; Step 4: PASS across
      `cargo test --workspace`; Step 5: commit `auctioneer-openrtb:
      PrepareLimits — step-0 impression cap`.

  - [ ] **2a.7 — egress gates + boot lifecycle. SPLIT IN TWO
    (round-182 #3): 2a.7a is everything that needs no batch API and
    no shared clock (gate construction, bounded serialization, boot
    lifecycle) and runs at DAG row 4b; 2a.7b is the egress DEADLINE
    integration (the three checkpoints and the
    `ResponseEgressDeadline` attachment) and runs at row 8b, AFTER
    2b.0 and M3b-11. Each bullet below is marked (a) or (b).**
    (round-180 #2 — this
    is the REAL unit; round-179 #1 named it "provisionally 2a.7"
    with no files, interfaces, tests or DAG placement, which is not
    a task. It owns the six M3b-2 normative deliverables that no
    other unit claimed.)**
    - **Blocked by:** 2a.1 (`AdmissionGate`/`AdmissionPermit`, whose
      shape the aux and egress gates reuse) and 2a.5 (the envelope
      that supplies every capacity and byte cap it installs).
      **DAG placement: row 4b — after 2a.5, before 2a.FINAL.** Not
      blocked by the batch rebase: no outbound surface.
    - Files: Create `crates/auctioneer-runtime/src/egress.rs`
      (`EgressGate` + `PermitBody` — the permit rides the response
      body and releases on EOF/drop, never at header time, round-45
      B5); Create `crates/auctioneer-runtime/src/aux_gate.rs`
      (`AuxGate` — the aux-route class with its own capacity, so an
      aux flood cannot consume auction admission); Modify
      `crates/auctioneer-core/src/serialize.rs` (bounded final-response
      serialization against `final_response_bytes`, over-cap ⇒ 500
      through the static-egress class); Modify
      `crates/auctioneer-core/src/host_primitives/bootstrap.rs` (install
      the boot envelope and fix ADAPTER BOOT ORDERING: envelope
      validated → gates installed → routes served, so no request can
      reach a gate that does not exist yet); Modify
      `crates/auctioneer-core/src/lib.rs` (wire the installs into the
      `app!` `configure` callback). **ROUND-181 #1 — 2a.7 OWNS THAT
      CALLBACK.** The round-180 split had 2a.7 (row 4b) modifying a
      callback M3b-1b (row 11) was to create — a cycle, since 2a.7
      runs first. 2a.7 CREATES `configure` and fixes the boot order.
      **⚠ ROUND-183 #7 — IT DOES NOT INSTALL THE ENVELOPE OR THE
      GATES THERE.** An earlier revision of this bullet said 2a.7
      "installs the envelope, the gates and the extraction limits" in
      `configure`, which the round-182 #4 correction immediately
      below contradicts. The correction is right; that sentence was
      stale and is removed rather than left to be reconciled by the
      reader. No bullet in this unit may assign the gates or the boot
      envelope to `configure`.
      **⚠ ROUND-182 #4 — `configure` RUNS ON EVERY `App` BUILD, NOT
      ONCE PER PROCESS** (upstream's macro invokes it per build;
      on serverless adapters that can be per request). It therefore
      carries ONLY per-`App` setters — extraction limits, egress
      policy, observer handle — each idempotent by construction
      because it overwrites a field on the `App` being built.
      **PROCESS-GLOBAL state (the gates, the boot envelope) stays in
      auctioneer's own `bootstrap_adapter`,** already `OnceLock`-backed
      and idempotent; `configure` READS what is installed rather than
      installing it. Tests:
      `configure_is_idempotent_across_repeated_builds()` (REPEATED
      SEQUENTIAL builds — the per-`App` setter path) and
      `concurrent_app_builds_install_gates_once()` (CONCURRENT
      bootstrap — the `OnceLock` path). **Round-183 #7: these are
      two different properties and neither substitutes for the
      other** — a repeated-build test cannot observe a race, and a
      concurrency test cannot observe a non-idempotent setter.
      **THE EXTRACTION-LIMIT MAPPING, EXACTLY (round-181 #3 —
      upstream `ConfigExtractionLimits` requires FIVE fields while
      auctioneer's envelope defines only two, so "plus the byte caps the
      envelope already owns" was not implementable):**
      | upstream field | auctioneer source |
      |---|---|
      | `timeout` | `max_config_fetch_ms` (exists) |
      | `max_blob_bytes` | `max_runtime_config_bytes` (exists) |
      | `max_secret_bytes` | **NEW envelope field** — `AuctioneerAppConfig` declares no secret paths today, so a small validated floor, never upstream's default |
      | `max_backend_bytes` | **NEW envelope field** — **upstream defines this as ONE CUMULATIVE EXTRACTION-WIDE provider budget, NOT a per-read ceiling (round-182 #8 corrects the description)**; must be ≥ `max_blob_bytes` |
      | `max_total_bytes` | **NEW envelope field** — whole-extraction ceiling; must be ≥ `max_backend_bytes` |
      **2a.5 owns adding the three NEW fields** to
      `ResolvedIsolateLimits` with codegen defaults, range validation
      and the two ordering assertions. **⚠ ROUND-183 #5 — THE MEMORY
      FORMULA NEEDS TWO TERMS, NOT ONE. `max_total_bytes` ALONE
      UNDERCHARGES.** An earlier revision replaced the whole
      runtime-config term with `+ max_total_bytes`. That is wrong,
      because upstream's `max_total_bytes` is a RAW WIRE-BYTE budget
      charged BEFORE parsing: the extractor "separately charges each
      returned logical blob/secret length against `max_total_bytes`
      before retaining or inserting it" (blob-app-config spec, the
      extraction-accounting paragraph). It says nothing about what
      DESERIALIZING those bytes allocates, and a parsed
      `AuctioneerAppConfig` is several times its wire size. Dropping the
      parse factor therefore removed a real, previously accounted
      term (round-117 P0-2 added it precisely because the
      deserialized config was UNACCOUNTED) and silently shrank the
      reservation. The term is:

      ```
      runtime_config_term = max_total_bytes                       // raw extraction, upstream-bounded
                          + max_blob_bytes * CONFIG_PARSE_FACTOR  // deserialized AuctioneerAppConfig
      ```

      `max_total_bytes` covers every raw byte the extraction retains
      (root config + secrets); only the ROOT BLOB is deserialized
      into `AuctioneerAppConfig`, so the parse factor scales
      `max_blob_bytes`, not the total. `CONFIG_PARSE_FACTOR = 6`
      survives unchanged from round-126 #11 and stays guarded like
      every other factor — it is a MEASURED lifetime value, and
      2a.5's validation must re-measure it against the real parsed
      config rather than inheriting the number on faith. Charging
      `max_total_bytes` and its parts TOGETHER would double-count the
      raw bytes; charging raw and PARSE separately does not, because
      they are different allocations alive at the same instant.
      Pinned by `runtime_config_term_charges_raw_and_parsed()`. **ROUND-182 #8 — and
      `max_total_bytes >= max_backend_bytes` is a STRICTER AUCTIONEER
      POLICY, not an upstream requirement:** upstream treats the two
      as independent cumulative budgets, and auctioneer orders them so a
      single number bounds the whole extraction for the memory
      equation. **ROUND-184 #7 — THIS ORDERING CLAIM DOES NOT SET
      THE EQUATION TERM.** An earlier revision ended this paragraph
      with "the equation term is therefore exactly
      `+ max_total_bytes`", which contradicted the two-term formula
      above it; that sentence is REMOVED rather than left for the
      reader to reconcile. The ordering assertion bounds the RAW
      extraction in one number; it says nothing about parsed
      allocation, which is why the formula above carries a separate
      `max_blob_bytes * CONFIG_PARSE_FACTOR` term. Ordering is pinned
      by `extraction_limits_are_ordered_and_charged_once()`; the
      equation is pinned by
      `runtime_config_term_charges_raw_and_parsed()`.
      **⚠ ROUND-182 #2 — THE NEVER-READY CASE IS NOT COVERED BY
      UPSTREAM'S DEADLINE, AND AUCTIONEER MUST OWN THE OUTER TIMEOUT.**
      **⚠ ROUND-187 #5 — HISTORICAL: THE DEFAULT DESCRIBED BELOW NO
      LONGER EXISTS.** At head, `ConfigStore::get_bounded` is
      REQUIRED with no compatibility default, so this whole bullet is
      the record of a gap that upstream closed (G20), not an
      instruction. auctioneer installs the extraction limits and calls
      `get_bounded`; it adds no outer race. As of the superseded
      pin, `ConfigStore::get_bounded`'s DEFAULT was, in upstream's own
      words, "a cooperative compatibility implementation: it checks
      before and after the unbounded provider call" — so a provider
      future that NEVER resolves never reaches the second check and
      the deadline never fires. Upstream says providers "must
      override it before claiming native allocation or cancellation
      guarantees", and no auctioneer-facing provider does today.
      CONSEQUENCE: auctioneer's `max_config_fetch_ms` needs an OUTER
      bound that does not depend on the inner future ever resolving.
      **ROUND-183 #1 — THAT RACE IS NOT THIS UNIT'S. IT BELONGS TO
      M3b-1b, AFTER 2b.0.** A race needs a clock and a timer; 2a.7a
      is defined as the clock-INDEPENDENT half of the 2a.7 split and
      runs at DAG row 4b, BEFORE 2b.0 establishes `RequestClock`. An
      earlier revision of this bullet assigned the race here anyway,
      which contradicted both this unit's own scheduling premise and
      M3b-1b's standing "no local timer" instruction. 2a.7a therefore
      installs the extraction limits (`timeout` included) and stops
      there; M3b-1b owns the outer race, the per-target behaviour
      below, and the never-ready test. Filed upstream as feedback G20
      (a cancellable/raced provider read); when that lands the outer
      race becomes redundant and is removed.
    - Interfaces (Produces): `EgressGate::try_acquire() ->
      Option<EgressPermit>`, `PermitBody<B>` wrapping a response body
      and releasing exactly once. **ROUND-182 #1 — THESE ARE TWO
      DIFFERENT PERMITS AND THIS UNIT OWNS ONLY ONE OF THEM.** The
      normative contract says the EGRESS permit survives until
      EdgeZero reports terminal completion/disconnect; body EOF/drop
      is NOT that signal — a body can be fully read and dropped
      before a single byte reaches the client. So:
      - **ADMISSION-BODY permit (in scope):** rides `PermitBody`,
        releases on body EOF/drop. It bounds CONCURRENT LIVE BODIES,
        which is exactly what EOF/drop measures.
      - **RESPONSE-EGRESS permit (NOT fully in scope):** must bind to
        EdgeZero's terminal `ResponseEgressReport` for ITS OWN
        response. That binding is impossible today — the observer is
        global and the report carries no response identity (feedback
        G18, which upstream will fix). Until it lands this unit
        installs the gate and the queue/serialization checkpoints,
        and the release-on-terminal binding is DEFERRED with that
        dependency named, rather than approximated by EOF.
      The test `egress_permit_releases_on_body_eof_and_on_drop()`
      belongs to the ADMISSION-BODY permit and is renamed
      `admission_body_permit_releases_on_eof_and_on_drop()` so it
      cannot be read as satisfying the egress contract, `AuxGate` with the same
      try-acquire shape, `install_boot_envelope(&ResolvedIsolateLimits)`,
      **and the STATIC-EGRESS class — whose full contract is
      UPSTREAM-DEPENDENT (round-182 #5).** auctioneer requires every
      response, refusals included, to hold a static permit, and a
      transport reset/close when that pool is exhausted. Neither is
      expressible at head: admission refusals go out through DETACHED
      egress the completion path does not cover,
      `BufferedIngressResponse` carries no extensions to hang a
      permit on, and `AdmissionDecision` offers only admit /
      buffered-fallback / respond — no Abort or Reset. 2a.7a installs
      the pool and its accounting; RESET-ON-EXHAUSTION is DEFERRED on
      feedback G21 (a completion hook covering every converted
      response, refusals included) and G22 (a portable abort/reset
      admission outcome). **ROUND-184 #10: BOTH LANDED at `351698c9`, carried forward at `03cdba98`
      — `AdmissionDecision::Refuse` carries a `completion` and
      `AdmissionDecision::Abort` exists — so M3b-11 adopts them.**
      **⚠ ROUND-186 #2 — BUT `Abort` DOES NOT, BY ITSELF, MAKE
      "an exhausted static pool resets" TRUE.** `AdmissionDecision`
      is returned synchronously before the handler runs, so it cannot
      express a reset for a response DECIDED LATER (config-fetch
      503, serializer-overflow 500, egress-queue expiry). The
      invariant is satisfied instead by RESERVING one static permit
      per admitted request at admission — see the static-class
      acquisition bullet in the M3b-2 egress section. Exhaustion is
      then always detected at admission, which is exactly where
      `Abort` is expressible.
      **✅ ROUND-186 #1 — THE ROUND-184 #5 EXEMPTION BELOW IS
      WITHDRAWN: `03cdba98` CLOSED G23.**
      `App::set_detached_response_egress_decision_factory` returns
      `DetachedResponseEgressDecision::{Abort, Send { completion,
      deadline }}` (round-188 #7 — the round-186 spelling
      `..._completion_factory` named the SUPERSEDED shape, which
      could supply a completion but could not refuse). Pre-admission
      400/414/431 therefore hold a terminal-response permit, and an
      exhausted pool ABORTS rather than emitting ungated, so the
      guarantee is universal again. The text below is retained as the record of
      what was exempt and why, because the reasoning (an exemption
      must be declared, never silently assumed) is the part worth
      keeping. HISTORICAL:
      **⚠ ROUND-184 #5 — ONE CLASS OF RESPONSE WAS OUTSIDE THE
      STATIC-EGRESS GUARANTEE, AND THE GUARANTEE WAS WEAKENED
      RATHER THAN CLAIMED.** auctioneer's requirement that
      EVERY response hold a static permit CANNOT hold for
      PRE-ADMISSION normalized-head failures. Verified in the
      upstream tree: `App::detached_ingress_error_egress`
      (`edgezero-core/src/app.rs`) renders 400/414/431 for a head
      that fails normalization and hardcodes
      `ResponseEgressCompletion::empty()`, and its own doc states
      detached responses "do not invoke the application's
      response-egress policy or observer". These responses are
      emitted BEFORE application admission runs, so there is no
      admission decision to attach a permit to and no completion to
      release one.
      **THE WEAKER GUARANTEE, STATED NORMATIVELY:** auctioneer's
      static-egress pool bounds every response auctioneer ITSELF emits
      — auction results, refusals, aux routes, 500s. It does NOT
      bound EdgeZero's pre-admission normalized-head rejections,
      whose concurrency is bounded instead by the adapter's own
      connection limits and whose bodies are fixed-size constants
      rendered by upstream. This is a DECLARED exemption with a
      named boundary, not an oversight, and the capability row says
      so. Filed upstream as **feedback G23** (a completion hook for
      detached pre-admission responses); if it lands, the exemption
      is removed and these responses join the pool. Pinned by
      `pre_admission_rejections_are_outside_the_static_pool()`,
      which asserts the EXEMPTION — so if upstream later attaches a
      completion, the test fails and forces this text to be revisited
      rather than silently over-claiming.
      Until G23 lands,
      and a documented boot order the adapters call in sequence.
    - **(b) RESPONSE-WRITE DEADLINE — THREE OF FOUR CHECKPOINTS ARE
      IN SCOPE, IN 2a.7b (round-181 #2 corrected a blanket deferral;
      round-182 #3 places it after 2b.0 + M3b-11).** The
      round-180 entry called Prereq 9 "ABSENT" and moved all four to
      M3b-10. That was wrong on the facts: upstream head exposes
      `ResponseEgressDeadline`, `ResponseEgressPolicy`,
      `ResponseEgressAttempt` with abort, and terminal
      `ResponseEgressReport`s
      (`edgezero-core/src/response_egress.rs`), and the capability
      rows read BestEffort, not Unsupported. **IN SCOPE NOW:** (1)
      the QUEUE wait races the deadline; (2) recheck ON PERMIT GRANT
      before serialization; (3) recheck AFTER serialization, before
      the first byte — plus attaching the absolute
      `ResponseEgressDeadline` so upstream enforces the transmission
      leg. **NOT in scope:** certifying checkpoint (4), the
      write-complete/disconnect signal, as NATIVE — that is the
      BestEffort gap M3b-10 certifies, not a reason to skip the
      behaviour. Tests:
      `egress_deadline_expires_in_queue_before_serialization()`,
      `egress_deadline_rechecked_after_serialization_drops_buffer()`,
      `egress_deadline_attached_to_response_for_upstream()`, and
      **`egress_gate_full_returns_503_through_static_class()`
      (round-183 #3 — moved here from 2a.7a, which has no
      static-response completion API to satisfy it; it is gated on
      G21/G22 like the rest of this unit).**

    - **⚠ ROUND-184 #3/#6 — 2a.7b'S IMPLEMENTATION SHAPE. THE PERMIT
      OWNERSHIP MECHANISM, STATED CONCRETELY.** The unit previously
      listed tests but gave files and steps only for 2a.7a, and said
      "bind the permit" without naming a mechanism. Both are fixed
      here.

      **The ordering problem it must solve:** the egress permit is
      acquired AFTER `run_auction` returns, but
      `ResponseEgressCompletion` is constructed during SYNCHRONOUS
      admission, before the handler runs. The callback therefore
      cannot capture a permit that does not exist yet.

      **Mechanism — UPSTREAM'S `late_bound`, NOT A AUCTIONEER SLOT
      (round-186 #3).** An earlier revision specified a hand-rolled
      `EgressPermitSlot(Arc<Mutex<Option<EgressPermit>>>)`. Upstream
      head `03cdba98` ships exactly this machinery, typed and tested,
      so auctioneer MUST NOT duplicate it:
      ```rust
      // edgezero-core, verified at 03cdba98
      ResponseEgressCompletion::late_bound<T>()
          -> (ResponseEgressResource<T>, ResponseEgressCompletion)
      ResponseEgressResource::<T>::install(&self, resource: T)
          -> Result<(), ResponseEgressResourceInstallError>
      enum ResponseEgressResourceInstallError { AlreadyInstalled, Closed }
      ```
      The hand-rolled version could not distinguish "never
      installed" from "terminally closed" — the prose demanded that
      distinction and the type could not express it. Upstream's
      `InstallError` makes it a compile-time-visible two-variant
      result, and upstream carries NINE tests for the states auctioneer's
      prose merely described: install/close race, poison recovery,
      install-before-terminal, terminal-before-install, duplicate
      install, abandonment before and after install, competing
      terminal signals, and unbegun envelopes.
      **Wiring:** admission calls
      `ResponseEgressCompletion::late_bound::<RouteEgressPermit>()`
      (round-191 #4 — the route-class enum, not the auction permit),
      puts
      the `ResponseEgressCompletion` into the `AdmissionDecision` and
      the `ResponseEgressResource<RouteEgressPermit>` into the grant (round-193 #6)
      (below). The handler calls `install(permit)` after acquisition.
      Release is upstream's: the completion closes the resource at
      terminal egress and drops the permit exactly once.

      **⚠ ROUND-186 #4 — THE GRANT CARRIES ONE AUCTIONEER TYPE, NOT TWO
      INDEPENDENT INSERTS.** `IngressGrant` carries ONE type-erased
      value, so this plan's two separate claims on it — the
      admission permit (M3b-2 admission section) and the egress
      resource (here) — do not compose as written and would not
      type-check. auctioneer defines exactly one grant payload:
      ```rust
      // auctioneer-core: the ONLY type auctioneer puts in an IngressGrant.
      pub struct AuctioneerIngressLease {
          // ROUND-202 #3 — ROUTE-AWARE. An aux or fallback route is
          // admitted by `AuxGate`, NOT `AdmissionGate`, so a lease
          // hard-typed to `AdmissionPermit` cannot represent one.
          // ROUND-203 #1 — and it is a MARKER, not a permit:
          // EdgeZero drops the grant before building a fallback
          // response, so a releasable resource stored here would be
          // freed while that response is still unwritten. The
          // permit lives in the ResponseEgressCompletion from the
          // moment of admission.
          pub admission: RouteAdmissionMarker,
          // ROUND-190 #3: ONE late-bound slot, whose occupant is
          // decided by ROUTE CLASS. An auction route installs its
          // egress permit here; an aux route installs its aux
          // permit. A request is never both, so one slot suffices
          // and no request can hold two class permits at once.
          // ROUND-194 #7: named `route_egress_*` throughout, since
          // the slot serves auction AND aux routes. The earlier
          // `auction_*` spellings were stale after round-190 #3.
          pub route_egress_resource: ResponseEgressResource<RouteEgressPermit>,
      }

      pub enum RouteEgressPermit {
          Auction(EgressPermit),
          Aux(AuxPermit),
      }

      // ROUND-202 #3: acquired ONCE, at admission, by the gate that
      // owns the route class. ROUND-203 #1: the PERMIT moves into
      // the completion; only this zero-sized tag rides the grant,
      // so dropping the grant releases nothing.
      pub enum RouteAdmissionMarker {
          Auction,
          Aux,
      }
      ```
      **⚠ ROUND-202 #3 — AUX ROUTES ACQUIRED TWICE AND COULD
      DEADLOCK THEMSELVES.** Admission takes an `AuxPermit` from
      `AuxGate` before the body read; `with_aux_egress` then acquired
      ANOTHER aux permit inside the handler. With
      `aux_route_permits = 1`, a request that was already admitted
      blocks on a permit it is itself holding — **an admitted
      request rejecting itself**. The lease also demanded an
      `AdmissionPermit`, which an aux route never has.
      **⚠ ROUND-203 #1 — AND THE PERMIT MUST BE BOUND TO COMPLETION
      AT ADMISSION, NOT CARRIED IN THE GRANT. EdgeZero DROPS THE
      GRANT BEFORE IT BUILDS A FALLBACK RESPONSE.** Round-202 #3 put
      the permit in `AuctioneerIngressLease` and had `with_aux_egress`
      move it into completion ownership. That covers only routes
      whose HANDLER runs. Verified upstream
      (`edgezero-core/src/router.rs`): the fallback drain path calls
      **`drop(grant)` BEFORE constructing the response** —
      ```rust
      .await;
      drop(grant);
      match outcome? {
          FallbackDrainOutcome::Exceeded => Ok(Some(on_exceeded.into_response())),
          FallbackDrainOutcome::TimedOut => Ok(Some(on_timeout.into_response())),
      }
      ```
      so for canonical 404/405, body overflow, read timeout and
      every pre-dispatch error, the permit is released while the
      response is still UNWRITTEN. That is the under-hold that
      OVER-ADMITS — the same class of error round-184 #4 removed for
      the egress permit, reintroduced through a path whose handler
      never runs.
      **THE RULE, CORRECTED: BIND AT ADMISSION.** The admission
      callback moves the acquired permit **into the
      `ResponseEgressCompletion` it already constructs**, so release
      is terminal for EVERY response on that route — handler-served
      or framework-generated. `AuctioneerIngressLease` keeps only an
      **admission MARKER** (`RouteAdmissionMarker`, a zero-sized
      route-class tag) so a handler can still assert it was
      admitted; it carries no releasable resource, so dropping the
      grant releases nothing.
      **`with_aux_egress` therefore neither acquires NOR transfers**
      — it attaches the deadline and runs the body. Its round-202
      "takes the permit already in the lease" wording is WITHDRAWN
      in turn.
      **Tests, at capacity 1:**
      `fallback_404_holds_aux_capacity_until_terminal()` — a
      canonical 404 is in flight and a second aux request CANNOT
      acquire until that response reaches terminal egress; the same
      for body-overflow and read-timeout responses. A test that only
      exercises a handler-served aux route passes against the broken
      rule, which is why this survived round 202.
      **THE ORIGINAL RULE (round-202 #3), still binding for the
      ACQUISITION half: ACQUIRE ONCE, AT ADMISSION.**
      - Admission acquires from the gate owning the route class
        (`AdmissionGate` for auction, `AuxGate` for aux **and for
        the pre-resolved 404/405 fallback routes**) and puts the
        result in `AuctioneerIngressLease.admission` as
        the `ResponseEgressCompletion`, leaving a
        `RouteAdmissionMarker` in the lease (round-203 #1).
      - **`with_aux_egress` ACQUIRES NOTHING AND TRANSFERS NOTHING**
        (round-203 #1): the permit is already inside the completion.
        It attaches the deadline and runs the body.
      - The permit is retained through TERMINAL COMPLETION via the
        late-bound resource, exactly like the auction permit.
      Tests: `aux_route_acquires_exactly_one_permit()` (capacity 1,
      one in-flight aux request completes rather than deadlocking),
      and `fallback_route_carries_an_aux_permit_to_terminal()`.
      **⚠ ROUND-190 #8 — EVERY PERMIT IN THESE TYPES MUST BE OWNED,
      NOT BORROWED.** EdgeZero requires an ingress-grant value to be
      `Send + Sync + 'static` and a late-bound completion value to be
      `Send + 'static`. A borrowing permit — anything shaped like
      `StreamAcquire<'_>` holding a reference into gate state —
      satisfies NEITHER and will not compile into an `IngressGrant`
      or a `ResponseEgressResource<T>`. **Binding requirement:
      `AdmissionPermit`, `EgressPermit`, `AuxPermit` and
      `StaticEgressPermit` are OWNED types**, each holding an
      `Arc<GateInner>` (or equivalent) rather than a lifetime
      parameter, and each releasing on `Drop`. 2a.1 owns this shape
      for `AdmissionPermit` and the others follow it.
      **Pin it at COMPILE TIME, not in prose**, because a lifetime
      leaking back in is a type error that a prose rule will not
      catch:
      ```rust
      const _: fn() = || {
          fn assert_grant<T: Send + Sync + 'static>() {}
          fn assert_resource<T: Send + 'static>() {}
          assert_grant::<AuctioneerIngressLease>();
          assert_resource::<RouteEgressPermit>();
          assert_resource::<StaticEgressPermit>();
      };
      ```
      **⚠ ROUND-190 #3 — THE AUX PERMIT HAD NO COMPLETION OWNER.**
      §8's resource proof requires aux permits to be "held through
      egress exactly like auction permits (released on the Prereq-9
      signal)", but the lease carried only `admission_permit` and an
      AUCTION-typed resource. An aux route therefore had nowhere to
      put its permit, so either the proof was false or the aux permit
      leaked until the request future dropped — the same
      release-too-early/too-late problem round-184 #4 removed for
      egress. Making the slot's payload a ROUTE-CLASS enum fixes it
      without a second late-bound pair (which would need a second
      completion, and only one fits in an `AdmissionDecision`).
      Tests: `aux_permit_releases_on_terminal_egress()` and
      `aux_and_auction_permits_are_mutually_exclusive()`.
      **⚠ ROUND-188 #2 — THE ROUND-187 COMPOSITE `ResponsePermits`
      IS RETIRED. IT COULD NOT PRESERVE THE STATIC PERMIT, AND
      UPSTREAM NOW HAS `join`.** Two independent defects killed it:
      - **It had nowhere to live before the handler.** The composite
        was installed only AFTER the auction resolved, so during the
        body read, a pre-handler fallback, or a body timeout, the
        admission-reserved static permit was held in a local frame
        with no completion bound to it — exactly the abandonment case
        the late-bound mechanism exists to cover.
      - **`AdmissionDecision::Refuse` carries NO grant** (verified in
        `edgezero-core/src/ingress.rs`: it carries `completion` and
        `response` only). Round-187 promised the refusal path would
        "install `ResponsePermits { auction_permit: None }`". It
        cannot: there is no resource to install into.
      **THE CORRECT WIRING — TWO COMPLETIONS, JOINED.** `join`
      landed at `38f61a19`, after round-187 verified its absence, so
      the "no join/combine" premise no longer holds:
      ```rust
      // At ADMISSION, synchronously:
      // 1. The static permit is MOVED INTO its own completion. It is
      //    owned by EdgeZero from this instant, so every terminal
      //    path releases it — including Refuse, body timeout and
      //    abandonment, none of which have a grant.
      let static_completion =
          ResponseEgressCompletion::new(move |_report| drop(static_permit));
      // 2. The auction permit does not exist yet, so it gets a
      //    late-bound pair.
      let (route_egress_resource, route_egress_completion) =
          ResponseEgressCompletion::late_bound::<RouteEgressPermit>();
      // 3. EdgeZero receives ONE completion covering BOTH.
      let completion = static_completion.join(route_egress_completion);
      // 4. Only the grant-carried half goes in the lease.
      let lease = AuctioneerIngressLease { admission_permit, route_egress_resource };
      ```
      The handler installs into `route_egress_resource` IF it acquires a
      permit of its route's class; if it never does (refusal, config failure,
      timeout), the late-bound half simply closes empty and the
      static half still releases. **No auctioneer-owned lifecycle state
      machine, and no path where a permit outlives its response.**
      Each joined callback has its own panic boundary upstream, so a
      panic releasing one does not strand the other.

      **THE FIVE CASES — upstream's semantics, and THEY DESCRIBE THE
      LATE-BOUND (AUCTION) HALF ONLY.** The static permit is moved
      into its own completion at admission and has no install step,
      so none of these can strand it (round-188 #2):
      1. **COMPLETION BEFORE INSTALL** (terminal egress before the
         handler acquires): `install` returns `Err(Closed)` and the
         permit drops immediately. auctioneer logs and continues; it must
         NOT treat this as a handler bug.
         `install_after_close_drops_permit_immediately()`.
      2. **DUPLICATE INSTALL** (a handler bug): `Err(AlreadyInstalled)`,
         the second permit drops, no panic, logged at error.
         `duplicate_install_is_an_error_not_a_leak()`.
      3. **HANDLER FAILURE AFTER INSTALL:** the response still goes
         out — as a 500 — and still reaches terminal egress, so the
         completion closes normally. No special path.
         `handler_error_after_install_still_releases_on_terminal()`.
      4. **ABANDONMENT** (the response is dropped before terminal
         egress): upstream's owner closes on drop and releases.
         auctioneer asserts the upstream guarantee rather than
         reimplementing it.
         `abandoned_response_releases_the_installed_permit()`.
      5. **TERMINAL CALLBACK** (normal): the permit is dropped once,
         whatever `ResponseEgressOutcome` the report carries —
         success, fallback and abort all release.
         `every_terminal_outcome_releases_exactly_once()`.

    - **Files (2a.7b):** Modify `crates/auctioneer-core/src/lib.rs`
      (**install `set_error_response_renderer` in the `configure`
      callback 2a.7a created — round-207 #1: the API does not exist
      at the `120dd2aa` pin, so it cannot be installed by 2a.7a at
      DAG row 4b**) and Create
      `crates/auctioneer-core/src/host_primitives/error_renderer.rs`
      (the two-arm match: Appendix F's pinned bodies as
      `&'static str`, `error.kind()` fallback for the rest);
      Create
      `crates/auctioneer-core/src/host_primitives/ingress_lease.rs`
      (`AuctioneerIngressLease` — round-186 #3/#4: there is NO auctioneer
      slot type; upstream's `late_bound` owns the mechanism, and this
      file only composes the two per-request resources into the one
      value `IngressGrant` can carry); Modify
      `crates/auctioneer-core/src/host_primitives/bootstrap.rs` (build
      the slot during admission, move one handle into
      **the JOINED completion** —
      `static_completion.join(route_egress_completion)`, whose late-bound
      half is
      `ResponseEgressCompletion::late_bound::<RouteEgressPermit>()`
      (round-187 #5 corrected a bare `::new` here, which cannot hand
      back a resource; round-188 #2 replaces the composite `T` with
      the join) — attach the route-egress resource to the
      `IngressGrant`); Modify
      `crates/auctioneer-core/src/handlers/auction.rs` (acquire the
      egress permit after `run_auction`, INSTALL it as
      `RouteEgressPermit::Auction`, attach `ResponseEgressDeadline`,
      and run the three checkpoints);
      **⚠ ROUND-191 #4 — THE AUX ROUTES NEED THE SAME INSTALL, AND
      NAMING ONLY `auction.rs` LEFT THEM UNWIRED.** Round-190 #3
      gave the aux permit an owner (`RouteEgressPermit::Aux`) but
      this file list still touched the auction handler alone, so
      `status.rs`, `info_bidders.rs` and `health_gvl.rs` would
      acquire aux permits with nothing installing them — the exact
      leak the enum was added to prevent. Rather than repeat the
      install in three handlers: Create
      `crates/auctioneer-core/src/handlers/aux_egress.rs` — ONE wrapper
      (`with_aux_egress(ctx, f)`) that takes the permit ALREADY in
      the lease (round-202 #3 — it does NOT acquire), installs it as
      `RouteEgressPermit::Aux(permit)` into the lease's
      `route_egress_resource`, attaches the response deadline, and runs the
      handler body; Modify `crates/auctioneer-core/src/handlers/
      status.rs`, `info_bidders.rs` and `health_gvl.rs` to call it
      (each becomes a one-line wrap, so the three cannot drift);
      Create `crates/auctioneer-core/tests/egress_binding.rs`.
      Tests: `aux_routes_install_their_permit_through_the_wrapper()`
      and `every_aux_route_is_wrapped()` — the latter a source-level
      assertion that no config-consuming aux handler is registered
      without the wrapper.
    - **Interfaces (Produces):** `AuctioneerIngressLease
      { admission_permit, route_egress_resource }` — **round-189 #5
      normalised a name split (`response_resource` vs
      `auction_resource`); round-190 #3 renames it once more to
      `route_egress_resource` because its payload is now the route-class
      enum `RouteEgressPermit`, not an auction permit. The static
      permit never reaches the lease** — the single
      `IngressGrant` payload; an auction handler whose egress capacity is
      returned on terminal egress rather than on body EOF or handler
      return.
    - **Adapter integration:** the lease is portable — it touches no
      per-target API. Targets differ ONLY in whether upstream
      delivers a terminal report, which their capability rows already
      declare. A target that does not is BestEffort for egress
      bounding and falls to case 4; it does NOT get an earlier
      release signal (round-184 #4).
    - **Steps:** Step 1 — the five failing tests above plus the
      three deadline tests; Step 2 — compile failure; Step 3 —
      implement; Step 4 — PASS on `cargo test -p auctioneer-runtime -p
      auctioneer-core egress`; Step 5 — commit `auctioneer-core/runtime:
      response-terminal egress permit binding + write-deadline
      checkpoints`.
- **⚠ ROUND-183 #3 — 2a.7a IS GATE CONSTRUCTION ONLY; THE
      RESPONSE-INTEGRATION TESTS MOVE TO 2a.7b.** This unit defers
      static-response completion and reset behaviour on G21/G22 (see
      the STATIC-EGRESS bullet above), so a test that asserts a
      refusal travels OUT through the static class cannot pass here
      — the unit would ship declaring a test it has no API to
      satisfy. Split accordingly:
      `egress_gate_full_returns_503_through_static_class()` is
      RESPONSE INTEGRATION and belongs to 2a.7b, which runs after the
      upstream dependency lands. 2a.7a keeps only what it can prove
      against constructed gates and in-process state.
    - Step 1 (2a.7a): failing tests — `admission_body_permit_releases_on_eof_and_on_drop()`, `egress_gate_full_refuses_without_a_permit()`
      (pure gate construction: capacity+1 try-acquire
      returns `None` and allocates nothing — it asserts the GATE's
      decision, not the response that decision produces),
      `aux_flood_cannot_consume_auction_admission()`,
      `final_response_over_cap_is_500_not_truncation()`,
      `boot_order_rejects_a_request_before_gates_install()`.
      **Round-180 #2 note: these five are NEW roster members and
      count toward the reconciliation this unit owes before
      dispatch — they are named here WITH their assertions, so this
      unit's subset is already enumerated. Round-183 #3 substitutes
      one member for another (`egress_gate_full_refuses_without_a_permit()`
      replaces the static-class test, which moves to 2a.7b
      rather than being deleted), so the roster TOTAL is
      unchanged.**
    - Step 2: `cargo test -p auctioneer-runtime -p auctioneer-core egress` —
      compile failure; Step 3: implement; Step 4: PASS; Step 5:
      commit `auctioneer-runtime/core: egress + aux gates, bounded
      serialization, boot lifecycle`.

  - [ ] **2a.FINAL — target-envelope constants. GATE REWRITTEN: its
    original form demanded a measurement that does not exist on three
    of the four targets.** Both original blockers have resolved, and
    resolving them exposed that one third of the gate was
    unsatisfiable as written.

    **Blocker (b) — RESOLVED.** The Cloudflare table's round-118
    F02/F03 status is discharged: `INBOUND_PARSE_FACTOR = 2` was
    disproven and replaced by `ResolvedIsolateLimits::inbound_parse_factor`
    with a measured floor of `MIN_INBOUND_PARSE_FACTOR = 36`
    (`auctioneer-runtime/src/budget.rs:111`), derived from three
    adversarial fixtures under a counting allocator and enforced at
    boot by `BudgetError::LimitBelowMinimum`.

    **Blocker (a) — ONE THIRD RESOLVED, TWO THIRDS UNSATISFIABLE.**
    Of the axum row's three placeholders:
    - `BASELINE_BYTES` is **measured**: 27,328,512, replacing a
      96,000,000 placeholder that was 3.5x too large. Thirty no-op
      boots, RSS read from outside the process, max 22,773,760, margin
      `ceil(max * 6/5)`. Re-runnable via `scripts/measure-baseline.sh`.
      The touch pass is what makes it honest — without forcing every
      page of the embedded GVL archive resident, a native reading
      understates by the whole ~20 MB archive, because mmap'd pages
      fault lazily.
    - `TRANSMIT_STATE_BYTES` and `PARSER_TRANSIENT_BYTES` **cannot be
      measured on wasm, by construction.** Both are the HOST's h2/hpack
      table and write-path state. No h2 or hpack code runs in any wasm
      guest, so those bytes never enter guest linear memory and no
      guest-side allocator can observe them. This is not an
      unperformed measurement; it is an unperformable one, and no
      amount of harness work changes that.

    **What this gate now requires**, in place of "re-run the three
    measurements":
    1. `BASELINE_BYTES` measured per §8 — **done**.
    2. The two host-state constants **carry their provenance in the
       code, not only in a doc comment.** Today
       `auctioneer-adapter-axum/tests/boot_envelope.rs` pins both as bare
       literals, so a reader has no in-test signal that they are not
       measurements. Either name them as native-substitute constants
       or assert their provisional status where it is visible.
    3. A recorded statement of WHY they are not measured on wasm — the
       structural reason above — so a future reader does not re-open
       this as an oversight.
    4. The CF/Fastly/Spin rows derived, the `VALIDATED_TARGETS` table
       test, and the `defaults_validate_per_target()` roster row, as
       originally specified.

    **A gate that passed on an unmeasurable placeholder would be worth
    less than one that refuses and says why.** The rewrite keeps the
    refusal honest while removing a condition nothing could ever
    satisfy. NOT blocked, unchanged: the four `TargetOutboundProfile`
    literals (they carry no numbers) and every 2a.1-2a.6 algorithm.

  **⚠ Round-96 H5 / round-97 H6 / round-98 / round-100 B7 / round-103
  H7 / round-106 — NORMATIVE DELIVERABLE LIST (REPLACES the
  admission/budget instructions below; rewritten in place each
  revision because it says EXACTLY).** This task implements the
  "Admission + whole-isolate budget — the round-98 design" section
  (as amended through round-106) at the top of this plan, and its
  admission/budget deliverables are EXACTLY — additionally including
  (round-103, re-based round-134 #4): the FIVE-param max-rule
  `per_stream_bytes` (`DecodeMode` and the per-adapter
  `decode_mode_of` mappings are RETIRED from the envelope —
  `BodySource` stays the static capability description), the
  `max_aliases`
  reject-only
  validation contract, and the `OutboundRequestSink` /
  `OutboundBuildError` construction API (round-103 B6):
  1. `auctioneer_runtime::admission::{AdmissionGate, AdmissionPermit}` —
     the portable atomic try-acquire gate (immediate 503 `overloaded\n`
     on full; no queue; capacity from the §2 BAKED boot envelope).
     Round-126 #8: acquisition happens in EDGEZERO INGRESS via the
     Prereq-10 opaque-guard protocol — the handler VERIFIES the
     injected permit (debug assert), it does not acquire; the old
     "FIRST handler action" phrasing survives only as the dev-mode
     fallback on adapters predating Prereq 10 (§1 item 1).
  2. `auctioneer_runtime::admission::{StreamGate, StreamPermit,
     StreamAcquire}` — the cancellation-safe FIFO async semaphore with
     identified waiter nodes (reserve-for-head, no-stealing,
     cancel-handoff, waker replacement). Round-123 #4 — WAVE LEASES:
     because the upstream batch is one atomic dispatch that does NOT
     consult the gate, the fanout NEVER acquires permits one-by-one
     around it (hold-and-wait deadlocks at capacity) and NEVER passes
     a batch larger than its lease (bypasses the cap). StreamGate
     gains `acquire_wave(k)` — an ALL-OR-NOTHING FIFO acquisition of
     k permits (k = min(gate capacity, remaining calls)): the waiter
     holds ZERO permits until all k grant atomically, cancellation
     while waiting releases nothing because nothing is held, and the
     fanout dispatches exactly one batch of k per lease,
     releasing the whole wave on WAVE completion (the batch await
     PLUS auctioneer's passthrough decode — round-130 #2). Waves repeat until
     the call list drains; call deadlines are ABSOLUTE instants
     composed ONCE at FANOUT ENTRY (round-155 #3 — `compose_all(calls,
     auction, fanout_entry_at)` in the spec's §5.2 algorithm is the
     ONE anchor; the older "first-wave batch-entry" wording is
     retired as a second definition, though for wave one the two
     coincide). Round-121 #2: no re-anchoring ever, so later waves
     race the same instants (equality ⇒ `Bidder`, matching M3a). THE WAVE LOOP is explicit
     (round-129 #4 — heterogeneous deadlines inside one
     `acquire_wave(k)` future): with pending calls P, each carrying
     its composed absolute `deadline_at`:
     (i) `earliest = min(deadline_at over P)`;
     `k = min(gate capacity, |P|)`; the wave's MEMBERS are the FIRST
     k calls of P in ORIGINAL INPUT ORDER (round-131 #7 — the sink's
     registry-order positional sequence; explicitly NOT
     earliest-deadline-first, which would reorder bidders and change
     which calls expire while waiting; no tie-breaking is needed
     because positional order is total).
     (ii) RACE `acquire_wave(k)` against `earliest`. RACE MECHANICS
     (round-136 #6, re-based round-143 #3 onto the ONE application
     clock — the Duration-domain/`TimeDomain` wording that stood
     here is deleted, not layered over): `earliest` is a
     `MonotonicInstant` on the app clock handed to the fanout at
     construction (`ctx.monotonic_clock()`); the sleep is
     `timer.sleep(earliest.saturating_duration_since(clock.now_monotonic()))`
     (auctioneer-runtime's `Timer` takes a relative `Duration`, derived
     at the moment of arming), and the inclusive recheck reads the
     SAME `clock`. **ROUND-153 #3 — THE TIMER MUST ACTUALLY SLEEP.**
     All three WASM adapters install `ImmediateResolveTimer` today
     (`auctioneer-adapter-cloudflare/src/lib.rs:27`,
     `auctioneer-adapter-spin/src/lib.rs:28`,
     `auctioneer-adapter-fastly/src/main.rs:24`), whose own doc says a
     strategy that races timers must not be paired with it: under it
     the race resolves for the timer arm on the first poll, so a
     contended `acquire_wave(k)` expires INSTANTLY and every queued
     call becomes a spurious pre-dispatch timeout. Replacing them is
     owned by M3b-3 (Cloudflare), M3b-4 (Spin) and M3b-5 (Fastly) —
     each task lists its concrete timer and test below — and the
     shared gate is `wave_wait_does_not_expire_before_deadline_<target>()`
     : a contended acquisition under a deadline far in the
     future must NOT emit a timeout. The upstream `Deadline` is
     `Deadline::at_instant(composed_at)` — no projection, no bridge.
     In tests the app clock's controllable source drives both the
     race and the recheck.
     Acquisition
     wins → MANDATORY monotonic recheck BEFORE dispatch (round-130
     #4 — the race can resolve in acquisition's favor on the same
     poll where the timer is also due, and the INCLUSIVE
     `deadline_at <= now` rule must win): read `now`; if ANY pending
     call has `deadline_at <= now`, DROP the whole lease (RAII
     release), emit every due positional timeout (stored cause),
     and go to (i) with recomputed k — expired work is NEVER
     dispatched. Otherwise dispatch ONE `send_bounded_all` batch of
     those k calls, release the whole wave on wave completion
     (batch await + auctioneer passthrough decode — round-130 #2),
     remove them from P,
     go to (i). `earliest` expires first → DROP the pending
     `StreamAcquire` future (cancellation-while-waiting releases
     nothing, by the gate contract), emit `Timeout(<cause stored at
     composition>)` positional results for EVERY call in P with
     `deadline_at <= now` (not only the earliest — several may have
     expired during the wait), remove them, and go to (i) with the
     smaller P and recomputed k. NO deadline is ever re-anchored at
     any step.
     (iii) Loop until P is empty. An early bidder deadline therefore
     can never leave the acquisition pending past its instant — the
     race, not a permit release, bounds the wait.
     ROUND-124 #7 — how that
     survives upstream's API, which exposes ONE absolute
     `.deadline()` per request plus a relative `.timeout()` anchored
     at each batch entry: the facade sets
     `.deadline(min(bidder_absolute, auction_absolute))` per request
     (both computed once, at FANOUT ENTRY — round-156 #4: the retired
     "first-wave entry" wording is a second anchor that would grant
     extra bidder time after gate contention; `compose_all(calls,
     auction, fanout_entry_at)` in spec §5.2 is the ONE site)
     and does NOT use the relative `.timeout()` for correctness —
     later waves pass the SAME absolute instants, so nothing
     re-anchors. ATTRIBUTION (round-125 #8 corrects the
     compare-at-observation rule, which mis-reported a
     bidder-expired-first timeout as auction-caused whenever the
     result was observed after BOTH instants had passed): the cause is
     decided ONCE, at composition — `selected_cause = if
     bidder_absolute <= auction_absolute { Bidder } else { Auction }`
     (equality ⇒ Bidder per the tie rule) — stored on the call, and
     any upstream timeout reports THAT stored cause. Whether the
     auction-global code-12 condition is active is evaluated
     SEPARATELY against auctioneer's clock at response-assembly time;
     a call whose stored cause is Bidder never asserts code 12. A second absolute input upstream would make the
     min() composition unnecessary; asked as feedback G13. PLUS the `aux_gate`
     (§7b — `/status` + `/info/bidders` + `/info/bidders/{name}` +
     `/internal/health/gvl` — round-123 #13: the health route was
     required into the aux gate by round-117 P1-10 but omitted from
     THIS deliverable list) and
     the `egress_gate` (§5 — permits released on the Prereq-9
     write-complete/disconnect signal). Round-123 #13 acceptance
     roster, PER aux route (all four): success-path write-deadline
     anchored at egress entry; pre-first-byte expiry → static-class
     503; mid-write stall → abort at the write deadline; client
     disconnect → release; exactly-once permit release; capacity
     recovery after each.
  3. `auctioneer_runtime::budget::{ResolvedIsolateLimits,
     validate_isolate_limits, BudgetError}` with the FULL §7 field set
     (both response caps, dual outbound caps, aux + egress + decoder
     fields — round-100 B3) and the compositional §7 formula: the FIVE
     independent factors `INBOUND_PARSE_FACTOR` / `GROUPING_FACTOR`
     (round-108 B3) / `TYPED_CLONE_FACTOR` / `RESPONSE_PARSE_FACTOR` /
     `ASSEMBLY_FACTOR` (each with its own allocator guard test — the
     round-98 single `REQUEST_PIPELINE_EXPANSION_FACTOR` is RETIRED),
     `GVL_PARSE_EXPANSION_FACTOR` (measured 4× + documented 1.5×
     cross-target margin ⇒ budget 6×; GVL-only worst-case generator),
     transient dual outbound-cap charging (round-100 H6), the
     boot-initialized `Gcl` static inside `baseline_bytes` (no
     separate term), and the §8 two-run measurement procedure +
     per-target pinned table + `scripts/measure-baseline.sh`.
  4. `PermitBody` + the egress gate (round-100 B4 — admission rides
     the `Body::Stream`; the EGRESS permit releases only on Prereq 9's
     completion signal, and **the whole-isolate memory proof is
     certified ONLY once Prereq 9 lands** — M3b-8 records it as an
     open item until then; round-107 B2 — the permit is acquired
     BEFORE serialization, which runs through
     `serialize_response_bounded(&resp, final_response_bytes)`
     REPLACING serialize.rs's unbounded `serde_json::to_vec`, with
     the §5 exact-cap/over-cap/wait-cancel/wait-deadline behaviors),
     the §2 BOOT ENVELOPE lifecycle
     (request-independent inputs; once-per-isolate fail-loud install;
     overlays clamp DOWNWARD only), the round-100 H6 outbound dual-cap
     dataflow (`FanoutError::OversizeRequest` + code-5 wire behavior),
     handler wiring (**the Prereq-10 ingress seam already holds the
     admission lease — the handler TAKES the grant
     (`ctx.take_ingress_grant()`), downcast-verifies it, and never
     acquires; the former try_acquire-first ordering is retired** →
     config → bounded body
     read → … → drop GVL before fanout) + the §9 design tests (the 14
     round-98 cases PLUS: the six factor guards, the aux-gate pair,
     the outbound-cap quartet, and — Prereq-9-gated — the
     EOF/disconnect/slow-client/write-deadline egress cases).
  5. Round-106/107 additions, all specified in the design section
     above: `auctioneer_bidder::CallBudget` (the ONE auction-global
     shared call budget consumed in §5.9 registry order with
     RESERVE → BUILD → COMMIT/ROLLBACK semantics — slots return on
     build failure and on semantic-Err discard; `OutboundRequestSink::
     new(&limits, &call_budget, endpoint_validator)` is the sink's
     only constructor (round-125 #13 — the three-argument form IS the
     canonical one; the two-argument spelling predated the round-124
     #8 validator and is retired),
     `push_request` takes TWO closures after reservation — metadata,
     then (only past endpoint validation with the call's
     `BidderRequestInfo`) body (round-145 #1 retires the earlier
     single meta-AND-body closure, which validated too late) — the
     error list is bounded (16 +
     suppressed count) and the 10999 N = rejected_call_count, and the
     M2 post-hoc global-cap truncation in run.rs is RETIRED — round-106
     B6 / round-107 B3); the byte pool-backed `OutboundRequestMetaBuilder`
     (`META_BYTES_CAPACITY = 3_584` + derived offsets + 64 = the
     4_384-at-defaults `OUTBOUND_META_BYTES` charge — round-126 #7, corrected round-202 #1;
     sink-allocated post-reservation; allocation-bounded, never a
     post-hoc length tally — round-106 H7); the DECIDE → MATERIALIZE →
     SCRUB split restructure (`group_by_bidder` owns the single
     `request_value`; typed per-bidder clones exist ONLY for the
     post-cap survivor set; code-15 FPD validation at
     materialization, survivors-only; scrubs applied to materialized
     survivors per recorded decisions; worst-case-cardinality guard —
     round-106 B4 / round-107 B4); the B5 sizing-floor rule
     (`final_response_bytes >= max_calls x max_decoded_bytes x
     ASSEMBLY_FACTOR`, enforced by `validate_isolate_limits` — a
     floor, NOT an output-size proof, round-108 H4); the round-107
     B1 single-margin rule (`baseline_bytes` = ceil(max-of-30 x
     6/5), validation consumes it AS IS); the round-107 B5 boot
     order (`install_host_primitives` → `Gcl::baked()` →
     budget/admission install, in every adapter bootstrap and in
     `scripts/measure-baseline.sh`); and the round-108 additions:
     RAII `CallReservation` + complete `SinkOutcome`
     (accepted/errors/suppressed_error_count/rejected_call_count;
     `into_outcome` commits, `into_outcome_discarding_accepted`
     refunds by drop; the ONE 10999 message string — B1), the
     bounded finalization path (`SinkAbort` closure error, byte pool
     finalize reserve 256, `EndpointBuilder::expand_into`,
     `finalize_for_transport` for GPC/gzip/entity headers with its
     own `FinalizeError { OversizeEncoded, ReserveOverflow }` and
     case-insensitive `ReservedHeader` rejection at push time —
     round-108 B2 / round-109 B2; the M2 mutation sites in
     fanout.rs/run.rs deleted), the `GROUPING_FACTOR` term +
     input-derived worst-case guard (NO new request-alias cap;
     object-indexed param lookup — B3), and the
     `ResponseWriteDeadline` (distinct from tmax — round-108 H4;
     complete config-to-envelope dataflow incl.
     `ResolvedIsolateLimits.response_write_deadline_ms`, codegen
     range, and the not-overlayable rule — round-109 H3).
  **Every `AdmissionBootConfig` / `AdmissionSemaphore` /
  `install_admission` / two-budget-dataflow block that follows in
  this task is STRUCK — and (round-107 M8) each such block now opens
  with a visible "⚠ STRUCK — NON-NORMATIVE HISTORY" banner in place,
  so no reader can mistake it for live instructions: do NOT implement those shapes — they carry the
  four round-46 defects plus the round-96 false single-request
  premise. Round-97 H6: NOTHING in the struck blocks needs
  re-deriving — the design section's §2 boot lifecycle now owns
  adapter installation, idempotence, boot-envelope ownership, overlay
  clamping, and permit lifetime in full.**

  **ROUND-124 #6 as sharpened by ROUND-125 #6 — ONE canonical
  architecture, by IDENTITY.** The facade IS the `ProxyClientApi`
  trait: `ProxyClientApi::send_bounded`, the FACADE-owned dual-cap
  `ResponseBodyPolicy` (defined in the facade block below),
  `BoundedProxyResponse`, and `CapRow` are the facade's real,
  auctioneer-owned names — NOT expected upstream API (the round-120
  alignment retired them upstream in favor of
  `OutboundHttpClient::send/send_all`). Round-132 #3 reconciles this
  paragraph with round-131 #4: the facade's ERROR type is the
  adopted upstream `EdgeError`, and `TypedProxyError` /
  `TypedBodyError` are requirements-list names ONLY — nowhere real.
  `auctioneer_core::outbound::BoundedFetch` is the STRUCT implementing
  `BoundedProxyClientApi` over the upstream client (round-154 #1
  corrects this paragraph: the bounded methods live on THAT trait,
  never on `ProxyClientApi`, which keeps its single shipped
  `send_one`). Consequence: every existing `send_bounded` /
  `send_bounded_all` reference in this plan is a FACADE call on
  `BoundedProxyClientApi` and already consistent — no mass rename;
  only text claiming these names are upstream's, or that they extend
  `ProxyClientApi`, is wrong, and the master spec's pipeline mention
  carries the same note. Tasks M3b-2..M3b-7
  implement AGAINST THE FACADE only. The facade's first
  implementation wraps upstream's single-cap Buffered API under the
  round-124 #5 interim rule below; when upstream lands the G5
  dual-cap API, only the facade's internals change. This removes the
  two-architectures ambiguity: upstream vocabulary at the boundary,
  facade vocabulary inside auctioneer, and exactly one implementation
  target per task.

  **THE EXECUTABLE CONTRACT (round-139 #3 — one contract, five
  clauses; rewrites round-124 #5's interim rule against the
  candidate head, where independent encoded and decoded caps EXIST
  in code):**
  1. set BOTH EdgeZero caps independently (encoded and decoded —
     real, separate knobs at `120dd2aa`);
  2. the final BUFFERED memory cap is
     `max(max_transport_bytes, max_decoded_bytes)` (pre-dispatch,
     path-independent — the memory bound);
  3. a response with RETAINED non-identity `Content-Encoding` ⇒
     auctioneer decodes (RawPassthrough; the wave lease holds through
     that decode);
  4. absent/identity encoding ⇒ ready to parse;
  5. Identity-vs-upstream-decoded provenance is ANALYTICS-ONLY.
  The round-124 known asymmetry (upstream-decoded bare gzip/br hides
  the raw size, leaving the transport cap memory-bounded but not
  semantically enforced) is CLOSED at `120dd2aa`: the independent
  caps are LANDED (matrix row "Encoding-sensitive dual caps (G5)"),
  so this plan has ONE facade — the dual-cap one — and no interim
  single-slot facade or "swap" exists (round-149 #6; the earlier
  scaffolding/swap wording is in the historical appendix §5). The
  round-123 ownership table's OWNER columns are the shipped state;
  its tests run against that facade.

  Round-37 B5, RE-BASED round-130 #2 onto the pinned upstream
  BUFFERED model: `ProxyClientApi`'s methods return COLLECTED
  responses — `Ok` means the complete body was already buffered
  under `ResponseMode::Buffered { max_bytes }` per the alignment
  section's cap-ownership table, and per-slot failures are the
  slot's `Err` (upstream §3.1.3). There is NO streaming primitive
  in the facade (upstream §3.2 declares streamed-response fan-out
  non-portable; the round-36/37 stream-item vocabulary survives
  only as the requirements list in the alignment section). No
  `send_one_bounded` auctioneer-owned helper — strategies call the
  trait methods directly.

  **TWO traits, closed explicitly (ROUND-154 #1 — the round-153 #5
  note said "separate trait" while both bounded methods still sat
  inside the `ProxyClientApi` block; this is the corrected
  declaration).**

  `ProxyClientApi` is UNCHANGED from the shipped tree
  (`crates/auctioneer-core/src/host_primitives/fanout.rs:27`): exactly
  one method, `send_one`, and its six implementations
  (`EdgeZeroProxyAdapter`; `MockClient` in `fanout.rs` tests;
  `FailingClient`, `OkClient`, `PanicOnUse`, `NeverResponds` in
  `strategy.rs` tests) are untouched by M3b. M3a's single-call path
  keeps working beside the bounded one.

  ```rust
  // crates/auctioneer-core/src/host_primitives/fanout.rs — SHIPPED, M3b
  // adds NOTHING to it.
  #[async_trait(?Send)]
  pub trait ProxyClientApi: Send + Sync {
      // M3a — round-49 H5: the typed `auctioneer_bidder::ProxyError`
      // (M3a Task 9.5 Step 0), not the retired `Result<_, String>`.
      async fn send_one(&self, req: &BidderHttpRequest)
          -> Result<BidderHttpResponse, ProxyError>;
  }

  // crates/auctioneer-core/src/outbound.rs — NEW in M3b-2b. The bounded
  // seam. Object-safe (`&dyn BoundedProxyClientApi` is what the wave
  // scheduler holds); `?Send` because the pinned upstream is `?Send`
  // per §1.2. NO blanket impl in either direction and no supertrait
  // relationship: the two traits are INDEPENDENT, because a
  // single-call dev client has no meaningful bounded batch and
  // `BoundedFetch` has no use for `send_one`. `BoundedFetch` is the
  // ONLY implementor; test doubles for the wave scheduler implement
  // this trait directly.
  // Round-155 #4: `Send + Sync`, because `ProductionFanout` holds
  // this behind a `dyn` handle and must itself satisfy
  // `BidderFanout: Send + Sync`
  // (`crates/auctioneer-exchange/src/fanout.rs:90`). `?Send` on the
  // async methods (the pinned upstream futures are `?Send`, §1.2) is
  // independent of the TRAIT's auto-trait bounds and does not
  // conflict with them.
  #[async_trait(?Send)]
  pub trait BoundedProxyClientApi: Send + Sync {
      /// THE WAVE CALL — one per `acquire_wave` lease. Maps 1:1 onto
      /// the upstream batch handle (the batch entry
      /// point): positional results, out[i] answers calls[i], slots
      /// independent. ROUND-155 #4 — THE ERROR CHANNEL IS LOCAL, NOT
      /// A PLATFORM ONE: the upstream batch yields a plain
      /// `Vec<OutboundSlotResult>` and cannot refuse a batch, so the
      /// only `Err` this method ever produces is
      /// `BatchDispatchError::EmptyBatch`, raised by the FACADE when
      /// it is handed an empty wave — a auctioneer-side contract
      /// violation the wave scheduler never commits. Every transport
      /// failure is a per-slot `Err` element in the `Ok` vec. The
      /// earlier "the platform refused the whole batch before any
      /// slot ran" wording described a channel that does not exist.
      async fn send_bounded_all(
          &self,
          calls: Vec<BoundedOutboundRequest>,
      ) -> Result<Vec<BoundedSlotResult>, BatchDispatchError>;

      /// THE AUX ONE-OFF CALL — maps 1:1 onto upstream
      /// `OutboundHttpClient::send` (the single entry point), for
      /// non-fanout paths (e.g. the aux route). NEVER used by the
      /// wave scheduler, which always issues `send_bounded_all`
      /// even for a one-member wave, so the batch contract has one
      /// code path.
      async fn send_bounded(
          &self,
          call: BoundedOutboundRequest,
      ) -> Result<BoundedProxyResponse, BoundedSlotError>;
  }
  ```
  WHICH UPSTREAM CALL BACKS WHICH METHOD (round-154 #1):
  <!-- PIN-SCOPED naming history (batch-rebase-ok) -->
  `send_bounded_all` → `OutboundHttpClient::send_all(Vec<Outbound
  Request>) -> Vec<OutboundSlotResult>`; `send_bounded` →
  `OutboundHttpClient::send(OutboundRequest) -> Result<Outbound
  Response, EdgeError>`, lifted into `BoundedSlotError::Edge`.

  **Historical rationale for the batch shape** (round-37 B5 /
  round-126 #1, kept for the reasoning, not as a declaration):
  ```rust
  // — the shape notes below describe BoundedProxyClientApi above —
  #[expect(dead_code, reason = "explanatory block, not a declaration")]
  mod _shape_notes {
      /// Round-126 #1 — the wave-shaped batch call the fanout
      /// actually issues (one per acquire_wave lease; wraps upstream
      /// the batch). Object-safe; positional results — out[i] answers
      /// requests[i], slots independent (round-121 adoption). A
      /// BATCH-level failure (the platform refused the whole batch
      /// before any slot ran) is Err; per-slot outcomes are the Ok
      /// vec. Single-request send_bounded remains for aux/one-off
      /// paths and is specified below.
      /// Round-127 #1: PER-CALL everything — bidder calls carry
      /// their own transport/decoded caps, compression mode, and
      /// composed absolute deadline (cause stored at composition),
      /// so a batch-wide policy cannot express a wave. Each element
      /// OWNS its call:
      ///   pub struct BoundedOutboundRequest {
      ///       pub request: BidderHttpRequest,
      ///       pub policy: ResponseBodyPolicy,   // per-call caps + mode
      ///       pub deadline: ComposedDeadline,   // round-130 #3
      ///   }
      ///   pub struct ComposedDeadline {          // auctioneer-owned;
      ///       pub deadline_at: MonotonicInstant, // the EdgeZero app
      ///                                          // clock's instant
      ///                                          // (round-142 #3) —
      ///                                          // the composed
      ///                                          // min(bidder, auction)
      ///                                          // instant — the ONE
      ///                                          // stored value
      ///                                          // (round-131 #1: two
      ///                                          // fields required to
      ///                                          // stay equal invited
      ///                                          // drift; the upstream
      ///                                          // `Deadline` is DERIVED
      ///                                          // at dispatch from this
      ///                                          // instant — same clock
      ///                                          // domain, no
      ///                                          // conversion)
      ///       pub selected_cause: TimeoutCause,  // stored AT
      ///                                          // COMPOSITION —
      ///                                          // upstream Deadline
      ///                                          // carries ONLY an
      ///                                          // instant; the earlier
      ///                                          // "TimeoutCause
      ///                                          // inside" note was
      ///                                          // wrong (round-130 #3)
      ///   }
      ///   // COMPOSITION SITE (round-133 #4 / round-134 #2, re-based
      ///   // round-142 #3 onto EdgeZero's ONE application clock — no
      ///   // projection, no bridge): compose ABSOLUTE instants in the
      ///   // app clock's domain, once PER CALL, at one code location
      ///   // before wave one — spec §5.2 `compose_all(calls, auction,
      ///   // fanout_entry_at)`; each wave's `run_wave(.., batch_entry_at)`
      ///   // owns ONLY preflight elapsed and never re-composes
      ///   // (round-152 #2):
      ///   //   let bidder_at   = fanout_entry_at
      ///   //       .checked_add(call.timeout)
      ///   //       .ok_or_else(|| FanoutInternalError::Internal(..))?;
      ///   //   // inside `compose(..) -> Result<Vec<BoundedOutbound
      ///   //   // Request>, FanoutInternalError>` (round-151 #5; the
      ///   //   // spec §5.2 snippet is the normative signature)
      ///   //   let composed_at = bidder_at
      ///   //       .min(auction_deadline_at);
      ///   //   let deadline    = Deadline::at_instant(composed_at);
      ///   //   (round-143 #3: `Deadline::at` does not exist; the
      ///   //    API is `at_instant`)
      ///   // every operand comes from the same
      ///   // `ctx.monotonic_clock()` lineage. ACCESS: `BidderFanout::
      ///   // the batch call has NO RequestContext, so the
      ///   // ProductionFanout STRUCT holds the app clock handed to it
      ///   // at construction by the adapter (the context's clock) —
      ///   // the trait signature is unchanged. There is no
      ///   // TimeProjectionError any more: an unrepresentable
      ///   // deadline can only be checked_add overflow, which is the
      ///   // OUTER Err(FanoutInternalError::Internal) → 500 before
      ///   // any dispatch.
      ///   pub struct BoundedProxyResponse {      // BUFFERED
      ///       pub status: u16,                   // (round-130 #2):
      ///       pub headers: http::HeaderMap,      // Ok ⇒ the whole
      ///       pub body: Bytes,                   // body was collected;
      ///       pub cap_row: CapRow,               // which cap-ownership
      ///   }                                      // row produced `body`
      ///                                          // (ready-to-parse /
      ///                                          // raw passthrough —
      ///                                          // passthrough rows
      ///                                          // still owe auctioneer's
      ///                                          // decode; round-147 #5)
      ///   pub struct BoundedSlotResult {        // round-134 #3 —
      ///       pub elapsed: Duration,            // the G15 carrier:
      ///       pub outcome:                      // per-slot upstream
      ///           Result<BoundedProxyResponse,  // time, present on
      ///                  BoundedSlotError>,     // Ok AND Err
      ///   }
      ///   // round-150 #3 — ONE explicit local error channel: the
      ///   // facade's own preflight failure is not an EdgeError and
      ///   // must not masquerade as one.
      ///   pub enum BoundedSlotError {
      ///       Preflight,                        // auctioneer's conversion
      ///                                         // rejected the request
      ///                                         // (method/URI/header at
      ///                                         // the http layer);
      ///                                         // nothing was sent
      ///       Expired,                          // round-152 #1 as
      ///                                         // CORRECTED round-153 #2
      ///                                         // / round-155 #2:
      ///                                         // auctioneer's own absolute
      ///                                         // check over AUCTIONEER-OWNED
      ///                                         // WORK ONLY — the wave
      ///                                         // WAIT and the passthrough
      ///                                         // DECODE. There is NO
      ///                                         // post-batch recheck
      ///                                         // and NO publication
      ///                                         // recheck: an EdgeZero
      ///                                         // slot outcome is
      ///                                         // TERMINAL and is never
      ///                                         // reinterpreted (master
      ///                                         // spec §5.2), because the
      ///                                         // batch vector returns
      ///                                         // once and a slow sibling
      ///                                         // must never convert an
      ///                                         // on-time bidder into a
      ///                                         // Timeout
      ///       Edge(EdgeError),                  // the adopted upstream
      ///                                         // per-slot error
      ///   }
      ///   // The fanout's classifier is EXHAUSTIVE over all three arms:
      ///   // Expired → Timeout(selected_cause); Preflight →
      ///   // FanoutError::Transport(TransportError { kind: Preflight });
      ///   // Edge(e) → the round-130/147 mapping table (GatewayTimeout →
      ///   // Timeout(selected_cause) by auctioneer's stored cause). No
      ///   // wildcard over BoundedSlotError. The slot's COMPLETE
      ///   // `ComposedDeadline` (not only the cause) travels with it
      ///   // through compaction, dispatch, decode and publication —
      ///   // spec §5.2 `Slot { result, deadline }`.
      ///   // elapsed — UPSTREAM'S DEFINITION, ADOPTED VERBATIM
      ///   // (round-141 #4): every slot measures from ONE
      ///   // BATCH-ENTRY snapshot to that slot's
      ///   // completion/failure, INCLUDING upstream validation,
      ///   // preparation, and preflight (the earlier
      ///   // per-slot-dispatch anchor described an interval
      ///   // upstream does not measure). StreamGate wave wait is
      ///   // STILL excluded — it happens BEFORE the facade calls
      ///   // batch entry, so it precedes the entry snapshot by
      ///   // construction; auctioneer's passthrough decode is still
      ///   // added locally: responsetimemillis = elapsed +
      ///   // auctioneer's measured decode time for passthrough slots.
      ///   // ELAPSED PER OUTCOME CLASS (round-135 #8 — a call that
      ///   // never dispatches gets no BoundedSlotResult, so
      ///   // BidderCallResult.elapsed is defined for ALL of:
      ///   //  (a) dispatched Ok           → G15 elapsed; passthrough
      ///   //                                slots ADD auctioneer's local
      ///   //                                decode time;
      ///   //  (b) dispatched Err          → G15 elapsed, verbatim;
      ///   //  (c) PRE-DISPATCH expiry     → auctioneer-synthesized:
      ///   //      (wave-wait timeout)       now − fanout_entry_at at
      ///   //                                emission — it measures
      ///   //                                the wait, never zero,
      ///   //                                never fabricated
      ///   //                                upstream time;
      ///   //  (d) batch-fatal             → the OUTER Err carries no
      ///   //      (BatchContractViolation)  per-call results at all;
      ///   //  (e) PREFLIGHT failure       → auctioneer-measured from the
      ///   //      (local construction,      SAME wave-entry instant to
      ///   //      e.g. invalid header —     the preflight failure
      ///   //      round-140 #5; anchor      (round-141 #4 supersedes
      ///   //      re-based round-141 #4)    round-140's Duration::ZERO:
      ///   //                                upstream measures preflight
      ///   //                                inside elapsed, so auctioneer's
      ///   //                                local preflight uses the
      ///   //                                same convention: MEASURED,
      ///   //                                and ZERO IS LEGAL when the
      ///   //                                failure lands in the same
      ///   //                                clock tick as wave entry —
      ///   //                                EdgeZero's timing contract
      ///   //                                permits zero for same-tick
      ///   //                                start/completion; what is
      ///   //                                forbidden is a FABRICATED
      ///   //                                constant — round-151 #6).
      ///   // COMPACTION (round-140 #5): the facade dispatches ONLY
      ///   // the preflight-valid siblings to the upstream batch,
      ///   // restores results to their ORIGINAL indices afterwards
      ///   // (preflight-failed slots keep their positions), and
      ///   // when EVERY slot fails preflight it never calls
      ///   // upstream at all — an empty upstream batch is not
      ///   // constructed.
      /// CONTRACT: `Ok(out)` has `out.len() == calls.len()` and
      /// `out[i].outcome` answers `calls[i]` (positional).
      /// `BatchDispatchError` is FACADE-owned and names ONLY what
      /// the facade itself can refuse — round-128 #6: the gate is
      /// NOT consulted here (wave admission is the fanout's
      /// `acquire_wave(k)`, run BEFORE this method; its
      /// ZeroWave / WaveExceedsCapacity / closed-gate errors are
      /// StreamGate's error domain and never cross this boundary):
      ///   enum BatchDispatchError { EmptyBatch }
      /// FAILURE MAPPING (typed, never a runtime assertion;
      /// round-129 #2 — this must TYPE-CHECK against the spec's two
      /// error models): a `BatchDispatchError` returned for a batch
      /// the fanout built non-empty, or an `Ok(out)` whose length
      /// differs from `calls.len()`, surfaces as the OUTER
      /// `Err(FanoutInternalError::BatchContractViolation
      /// { expected, got })` from `BidderFanout::send_all` — the
      /// spec's batch-level channel, which the exchange already maps
      /// to `AuctionError::FanoutUnavailable` → standard 500.
      /// Per-call `FanoutError` gains NO variant. Whole-batch
      /// refusal beyond `EmptyBatch` DOES NOT EXIST: any
      /// transport-level failure is a per-call
      /// `Err(EdgeError)` element, and a correct fanout
      /// never builds an empty wave — so `EmptyBatch` appearing at
      /// all IS the contract violation. The wave lease releases by
      /// RAII; the contract is pinned by a test, not a panic.
      /// PREFLIGHT (round-138 #7): header conversion is
      /// FAIL-CLOSED — the M3a bridge (edgezero_proxy.rs) silently
      /// SKIPS invalid header names/values, dispatching a MODIFIED
      /// request; the M3b facade instead fails that slot with a
      /// typed pre-dispatch error (the slot's `outcome` is `Err`
      /// BEFORE anything is sent — never a silently altered
      /// request). Pinned by
      /// `invalid_header_fails_slot_preflight_not_silently_dropped()`.
      /// Per-call failures NEVER surface here — they are the inner
      /// `outcome: Err(BoundedSlotError)` slots (round-131 #4 / round-150
      /// #3: ONE error model — the facade's UPSTREAM error type IS the
      /// adopted `EdgeError`, wrapped as `BoundedSlotError::Edge`, beside
      /// the facade's one local arm, `Preflight`;
      /// `TypedProxyError`/`TypedBodyError` survive ONLY as
      /// requirements-list names in the alignment section, and the
      /// fanout maps EdgeError → FanoutError via the round-130
      /// table).
      ///   pub enum CapRow { ReadyToParse, RawPassthrough }
      ///   // round-147 #5 — BEHAVIOR-BEARING states only. Derived
      ///   // from the RESPONSE HEADERS under the @120dd2aa
      ///   // contract, which RETAINS non-identity Content-Encoding
      ///   // on passthrough and REMOVES it after a host-side
      ///   // decode: retained non-identity CE ⇒ RawPassthrough
      ///   // (auctioneer owes the decode; the wave lease holds until it
      ///   // completes); absent/identity CE ⇒ ReadyToParse. The
      ///   // former Identity-vs-UpstreamDecoded split asked for a
      ///   // "decoded upstream" provenance EdgeZero does not
      ///   // expose (it strips the header and reports nothing
      ///   // else), so that split is DELETED rather than kept as
      ///   // an unfillable analytics field (G14 stays DOWNGRADED:
      ///   // budgeting charges the max across dispositions, and
      ///   // overflow origin arrives via ResponseLimitReason — no
      ///   // accounting rule needs decoded-by-whom). Headers are plain
      ///   // `http::HeaderMap` (the undefined `ResponseHeaders`
      ///   // name is retired).
      ///   // (Round-132 #3's decode_disposition requirement was
      ///   // GAP G14, DOWNGRADED round-137 #4 and superseded by the
      ///   // header derivation above — round-139 #3 removes the
      ///   // contradiction; overflow origin arrives via
      ///   // ResponseLimitReason.)
      ///   pub struct ResponseBodyPolicy {   // FACADE-owned, DUAL-cap
      ///       pub max_transport_bytes: u64, // (round-132 #3: the
      ///       pub max_decoded_bytes: u64,   //  Prereq-2 streaming
      ///       pub max_brotli_wbits: u8,     //  block's single-cap
      ///       pub max_brotli_layers: u8,    //  seven-field struct is
      ///       pub max_brotli_decoder_bytes: u64, // round-140 #1 —
      ///                                     //  EdgeZero's INDEPENDENT
      ///                                     //  decoder limit (real
      ///                                     //  outbound.rs builder);
      ///                                     //  auctioneer SETS it to
      ///                                     //  brotli_state_bytes(wbits)
      ///                                     //  — the charged bound —
      ///                                     //  and validation asserts
      ///                                     //  policy == charge
      ///       pub max_header_bytes: u32,    //  requirements-record
      ///       pub max_header_count: u32,    //  only; THIS is the
      ///       pub max_chunk_bytes: NonZeroU64, // compilable shape —
      ///   }                                 //  BOTH caps present.
      ///   // round-149 #4: `max_chunk_bytes` is stored as the
      ///   // NonZeroU64 EdgeZero's builder takes. Round-150 #7 —
      ///   // LINT-COMPLIANT construction (the workspace denies
      ///   // clippy::unwrap_used / expect_used / panic / unreachable
      ///   // in non-test code and this plan adds no #[expect]):
      ///   //   pub const MAX_CHUNK_BYTES: NonZeroU64 =
      ///   //       match NonZeroU64::new(8 * 1024) {
      ///   //           Some(bytes) => bytes,
      ///   //           None => NonZeroU64::MIN,   // unreachable by
      ///   //       };                             // arithmetic; pinned:
      ///   //   const _: () = assert!(MAX_CHUNK_BYTES.get() == 8 * 1024);
      ///   // (a compile-time assertion, so the fallback arm can never
      ///   // silently ship a different value; the certificate's
      ///   // MAX_CHUNK_SIZE = 8 KiB staging term is the same value
      ///   // as usize). It is NOT envelope-configurable, so no
      ///   // zero-value configuration error exists; if it ever
      ///   // becomes configurable the envelope validator rejects 0
      ///   // at build.rs time like every other range violation.
      ///   // round-133 #6: NO body_source field — the disposition
      ///   // is per-RESPONSE metadata (G14), not a request-time
      ///   // policy; budgeting charges max() across dispositions.
      fn send_bounded_all_shape();

      // Round-37 B5 as re-based by round-131 #4: object-safe
      // bounded seam over the ADOPTED upstream client — NO default
      // impl. M3b's `BoundedFetch` implements it over
      // `OutboundHttpClient::send`, carrying BOTH caps (round-136
      // #2 — the row is response-dependent and UNKNOWABLE at
      // request build, so nothing "chooses per row" here: the
      // FINAL contract is G5's encoding-sensitive enforcement,
      // selected by EdgeZero after response headers — round-137
      // #2: the builder shape is upstream's — LANDED @120dd2aa as
      // `max_encoded_response_bytes` + `max_decoded_response_bytes`;
      // there is no single-slot interim, round-149 #6).
      // Uses `?Send` on `#[async_trait]` because the pinned
      // upstream is `?Send` per §1.2.
      fn send_bounded_shape();

      // ⚠ STRUCK — NON-NORMATIVE HISTORY (round-145 #5). The two
      // trait methods that stood here — `supported_body_sources()`
      // (round-41 B1, "every adapter MUST implement") and the
      // policy-aware `guest_decoder_memory_bound(&policy)` (round-45
      // B6 / round-132 #3) — are NOT facade methods: no such upstream
      // surface exists. Decoding follows the SDK response's retained
      // Content-Encoding/classification; decoder funding comes from
      // ResolvedIsolateLimits and is checked at boot. The adapter's
      // static capability declaration is not a runtime policy registry.
  }
  ```
  `BoundedProxyResponse` accessors — round-37 B5. **Round-110 H5 /
  round-111 H4 — response HEADERS are bounded via a PER-TARGET
  client contract (the round-110 "reject before constructing the
  HeaderMap" phrasing was not implementable at the pinned Axum
  boundary: reqwest's `builder.send()` returns a response whose
  headers are ALREADY materialized before EdgeZero can iterate them,
  so a post-send size check cannot bound the peak):** Prereq 2
  carries `MAX_RESPONSE_HEADER_BYTES = 8_192` (summed name + value)
  and `MAX_RESPONSE_HEADER_COUNT = 64` on `ResponseBodyPolicy`
  (round-111 B3 — real struct fields), enforced where each target
  can actually enforce them:
  **Round-112 B2 — PARSER CEILINGS and the LOGICAL policy are two
  DISTINCT layers (the round-111 draft passed the 8_192 logical
  byte cap directly into `http2_max_header_list_size`, which counts
  name + value + 32 PER FIELD incl. pseudo-fields per RFC 9113
  §6.5.2 — a policy-valid response could be parser-rejected; and
  hyper exposes NO h2 header-COUNT knob, so ~313 tiny fields can
  materialize under the derived ceiling while the logical count says
  64):**
  - **Layer 1 — parser ceilings (per protocol, derived so every
    policy-valid response passes):**
    - h2: `http2_max_header_list_size(H2_FIELD_SECTION_KNOB)` where
      `H2_MAX_FIELD_SECTION = MAX_RESPONSE_HEADER_BYTES +
      (MAX_RESPONSE_HEADER_COUNT + 1) x 32 + 64 = 8_192 + 65 x 32 +
      64 = 10_336` (the +1/+64 admit the `:status` pseudo-field's
      own name/value + overhead) and — round-115 B1 —
      `H2_FIELD_SECTION_KNOB = H2_MAX_FIELD_SECTION + 1 = 10_337`:
      h2 0.4.14 treats a block as oversized when `headers_size >=
      max` (it retains fields only while `headers_size < max`), so
      the knob is an EXCLUSIVE boundary and must be set ONE ABOVE
      the ceiling to admit exactly through 10_336. The transient
      memory bound is unchanged (admitted sections ≤ 10_336 ⇒ ≤ 313
      fields ⇒ the 30_368 term stands). No h2 count knob exists —
      the count is bounded only IMPLICITLY.
    - h1: `http1_max_headers(MAX_RESPONSE_HEADER_COUNT)` (h1 has no
      pseudo-fields, so the count maps directly) +
      `http1_max_buf_size(16_384)` — an explicit note that this
      bounds the ENTIRE read buffer (status line + framing + all
      header lines), not sum(name+value); 16_384 covers 8_192
      logical bytes + per-line `": " CRLF` overhead + the status
      line with slack.
  - **Layer 2 — the LOGICAL post-parse policy check:** sum(name +
    value) ≤ `max_header_bytes` AND count ≤ `max_header_count`,
    applied to the materialized map. On h2 this is the ONLY place
    the 64-count is enforced. The pinned reqwest path (proxy.rs:61)
    exposes neither knob → Prereq 2's client either constructs
    reqwest with the hyper-level configuration above or wraps hyper
    directly; without Layer 1 the post-send check alone bounds
    nothing (headers already materialized).
  - **Wasm targets (CF/Fastly/Spin): PRE-COPY.** The platform
    materializes response headers in HOST memory — and round-117
    P1-8 corrects the accounting story for Cloudflare, whose 128 MB
    limit covers JS-host and Wasm-guest heap TOGETHER (§2 baseline):
    "outside the guest budget" is not "outside the isolate". The
    guest-side Layer-2 check runs BEFORE copying into the guest
    HeaderMap, but the host-side header object already exists when
    it runs, so on Cloudflare the proof must charge a host-side
    header bound: `max_header_count x max_header_bytes` (the Layer-2
    constants) x the measured JS-object expansion factor, folded
    into the CF baseline margin as a per-in-flight-response term.
    If measurement shows the platform materializes unboundedly
    before the guest can refuse (headers larger than the Layer-2
    caps), the CF certificate must record the platform's own
    documented header limit as the bound and cite it. On
    Fastly/Spin the host heap is genuinely outside the guest's
    linear-memory ceiling and the existing text stands (no Layer-1
    knob exists or is needed there).
  - **TRAILERS and 1xx blocks (round-113 H1 — previously undefined;
    a native h2 trailer block parses while the retained initial map
    is live, so ignoring them under-charged the peak):** auctioneer
    NEVER consumes response trailers or informational blocks — the
    `BoundedProxyResponse` body stream is `Bytes`-only BY DESIGN, so
    trailers cannot surface and cannot receive the Layer-2 logical
    check. The contract is DROP-AT-THE-BOUNDED-PARSER: every
    non-initial field block (h2 trailers, h1 chunked trailers, 1xx
    informational headers) is (a) bounded by the SAME Layer-1
    ceilings (`SETTINGS_MAX_HEADER_LIST_SIZE` applies to every h2
    field block incl. trailers; h1 trailers parse inside
    `max_buf_size`), and (b) dropped immediately after parse, never
    materialized past the parser layer. 1xx blocks arrive BEFORE the
    final headers (nothing retained yet — inside the transient
    bound); a TRAILER block parses while the RETAINED initial map is
    live, so the per-stream charge is their SUM (below). On wasm
    targets the platform holds trailers host-side and auctioneer never
    requests them — no guest materialization.
  Either way, an over-limit response fails the call through the
  TYPED path (round-114 B1): pre-body breaches (initial or 1xx
  block) surface as `ResponseLimitReason::HeaderBytes` /
  `HeaderCount` (→ `OversizeResponse { source: Headers }`),
  trailer-block breaches through the same `ResponseLimitReason`
  family (round-151 #3: the `TypedProxyError` / `TypedBodyError`
  names that stood here are requirements-record only),
  and BOTH classifiers map them to `FanoutError::OversizeResponse
  { source: OversizeSource::Headers }` — the same code-4 /
  seatnonbid-100 wire family as an oversize body, never a generic
  transport error. **Charging (round-111 H4 /
  round-112 B2 — the PARSER-LEVEL worst case, not the logical
  one):** the transient native-h2 map that exists between parse and
  the Layer-2 check can hold up to 313 tiny fields:
  `HEADER_MAP_TRANSIENT_CHARGE = H2_MAX_FIELD_SECTION + 313 x
  HEADER_ENTRY_OVERHEAD_BYTES (64) = 10_336 + 20_032 = 30_368`.
  Round-113 H1: the PER-STREAM charge is `HEADER_STREAM_CHARGE =
  12_288 + 30_368 = 42_656` — the retained initial map PLUS a
  trailer block's concurrent parser-worst transient (a trailer
  parses while the initial map is live), which dominates both the
  initial-parse transient alone (30_368) and the post-pass peak
  (live logical map 12_288 + conversion copy 12_288 = 24_576). Maps that
  SURVIVE the Layer-2 check satisfy the logical policy, so each
  retained instance is charged
  `MAX_RESPONSE_HEADER_BYTES + MAX_RESPONSE_HEADER_COUNT x
  HEADER_ENTRY_OVERHEAD_BYTES (64)` = 12_288 (the formula and
  worked table use this figure).**
  **Round-150 #8: the rounds-37..46 admission drafts that were
  interleaved with the dataflow subtasks below (`AdmissionSemaphore`
  / `AdmissionBootConfig` / `install_admission` / per-adapter stream
  defaults / proof table / Fastly sandbox lifecycle, ~1_300 lines
  under three ⚠ STRUCK banners) are MOVED to the historical appendix
  §7; only the live two-budget dataflow subtasks remain here.**
  **⚠ STRUCK — NON-NORMATIVE HISTORY (round-134 #5).** The block
  below predates the buffered adoption and reuses the name
  `BoundedProxyResponse` for a STREAMING type that no longer
  exists. Read its shapes as upstream's `OutboundResponse` /
  `Body::Stream` vocabulary: the ONLY `BoundedProxyResponse` in
  auctioneer is the facade's buffered struct, and
  `TypedProxyError`/`TypedBodyError` are requirements-list names.
  Do not implement anything from this block.
  ```rust
  // crates/edgezero-core/src/proxy.rs (Prereq 2 landing).
  impl BoundedProxyResponse {
      pub fn status(&self) -> StatusCode;
      pub fn headers(&self) -> &HeaderMap;
      pub fn headers_mut(&mut self) -> &mut HeaderMap;
      /// Round-37 B5: `body` is consumed on take so the
      /// stream can be passed into the decoder pipeline;
      /// the method must NOT clone (a stream is not
      /// cloneable). Post-take, the struct still owns
      /// `status` + `headers` — those stay accessible.
      pub fn take_body(&mut self) -> BoundedBodyStream;
      /// Destructure. Preferred over `take_body` when
      /// callers don't need to retain a mutable
      /// `BoundedProxyResponse`. Round-37 B5: the plan's
      /// pseudocode reads `let (status, headers, body) =
      /// resp.into_parts();` so this method must exist.
      pub fn into_parts(self) -> (StatusCode, HeaderMap, BoundedBodyStream);
  }
  ```

  `ProxyClientApi::send_one_bounded` from every earlier
  round is DELETED — no such method has ever existed on
  the trait; residual references were carry-over from
  round-33's draft. Every strategy / classifier that
  pointed at `send_one_bounded` now calls
  `client.send_bounded(call)` directly (one
  `BoundedOutboundRequest` argument — round-131 #4).

  `max_transport_bytes`/`max_decoded_bytes` still flow from
  TOML through resolution to the outbound call — the
  dataflow subtasks below are unchanged. Only the trait
  method name is corrected.

  **Round-32 H3 dataflow subtasks against the ACTUAL overlay
  types.** Round-31 H1 correctly identified
  `HostConfig.resource_limits` as the build-baked host layer,
  but the RUNTIME overlay path lives in
  `crates/auctioneer-core/src/config.rs::AuctionConfig` +
  `BidderOverrides` (already carrying the historic
  `max_bidder_response_bytes` on `AuctionConfig`; see
  [config.rs:106](../../../crates/auctioneer-core/src/config.rs#L106)).
  Account-scoped runtime overrides land through
  M3a Task 14's planned `AccountOverride` type. Two-budget
  dataflow covers all three:
  - **Build-baked host defaults.** Add
    `max_transport_bytes: Option<usize>` and
    `max_decoded_bytes: Option<usize>` to
    `crates/auctioneer-config/src/types.rs::ResourceLimits`. Also
    add matching `Option<usize>` TOML fields on the
    build-time TOML struct + codegen renderer that emits
    `ResourceLimits`. Validator: **both in
    `1_024..=104_857_600` bytes** (round-33 H4 corrected
    interval — 1 KiB minimum, exactly 100 MiB maximum;
    round-32 stated "1..=100 MiB" in prose but the literal
    was 64 MiB and the minimum flipped between 1 byte and 1
    KiB across sites). 1 KiB = `0x0000_0400`; 100 MiB =
    `100 * 1024 * 1024 = 104_857_600 = 0x0640_0000`. Any
    ambiguity between decimal MiB and MB is resolved to
    binary MiB — deployers reading the fields expect
    memory-page multiples, not decimal marketing bytes.
  - **Runtime host overlay via `AuctionConfig`.** Round-32
    H3: the existing runtime overlay for
    `max_bidder_response_bytes` is at
    `crates/auctioneer-core/src/config.rs::AuctionConfig::
    max_bidder_response_bytes` (an `Option<usize>` under
    `#[validate(range(min = 0x0000_0400_usize, max =
    0x0100_0000_usize))]` — 1 KiB to 16 MiB). The two new
    budgets follow the SAME shape but use the wider 100 MiB
    ceiling per the round-33 H4 correction:
    ```rust
    #[serde(default)]
    #[validate(range(min = 0x0000_0400_usize, max = 0x0640_0000_usize))]
    pub max_transport_bytes: Option<usize>,
    #[serde(default)]
    #[validate(range(min = 0x0000_0400_usize, max = 0x0640_0000_usize))]
    pub max_decoded_bytes: Option<usize>,
    ```
    Runtime `AuctionConfig` wins over baked
    `HostConfig.resource_limits` when set. Boundary tests
    (round-33 H4):
    - `budget_exactly_1024_bytes_accepted()` (min).
    - `budget_exactly_100_mib_accepted()` (max =
      `0x0640_0000`).
    - `budget_1023_bytes_rejected()`.
    - `budget_100_mib_plus_one_byte_rejected()`.
    - `budget_80_mib_accepted()` (between the 64-MiB
      historic ceiling and the 100-MiB new ceiling —
      catches any lingering `0x0400_0000` literal).
  - **Account-scoped overlay via `AccountOverride`.**
    Round-32 H3: the two budgets get the SAME
    `Option<usize>` slots on M3a Task 14's planned
    `AccountOverride` struct
    (M3a plan §Task 14 defines it). Account overlay wins
    over runtime host overlay per §9.1 merge order:
    account → runtime host → baked host.
  - **`BidderOverrides` (per-bidder runtime overlay).** Same
    two `Option<usize>` slots on
    `crates/auctioneer-core/src/config.rs::BidderOverrides`
    ([config.rs:154](../../../crates/auctioneer-core/src/config.rs#L154)).
    Per-bidder overrides win over account, which wins over
    runtime host, which wins over baked host.
  - **`BidderBudgets` type + per-bidder map path** (round-33
    H1 — round-32 H4 named the type without defining it and
    put the map on the canonical `AuctionContext`, which
    doesn't carry budgets in the spec):
    ```rust
    // crates/auctioneer-exchange/src/budgets.rs — round-54 M1: NOT a
    // new module. M3a Task 13 already CREATED budgets.rs with an
    // empty `BidderBudgets` placeholder + the `ResolvedBudgets`
    // alias. M3b only REPLACES the placeholder struct body with
    // these fields; the module, the alias, and the `pub mod
    // budgets;` line already exist.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct BidderBudgets {
        pub max_transport_bytes: usize,
        pub max_decoded_bytes: usize,
    }

    pub type ResolvedBudgets = BTreeMap<BidderName, BidderBudgets>;
    ```
    Ownership path: the map lives on the auction-lifecycle
    scratch value that Task 13 already threads into the
    fanout — the **`AuctionPlan`** side-band, NOT
    `AuctionContext`. `AuctionContext` (spec §7.6) is a
    LONG-LIVED request/host/gvl_store handle and MUST NOT
    grow budgets or aliases per the reviewer's
    canonical-shape check. Round-38 B1: Task 13's handler
    entry constructs the plan via
    `AuctionPlan::new_m3b(aliases, disabled_bidders,
    budgets, bidder_sample_rates)` (round-45 H4 — the M3a
    constructor takes four args; the earlier three-arg call
    here dropped `bidder_sample_rates` and would not
    compile). This is NOT the round-37 draft's field-literal
    `AuctionPlan { budgets, aliases, ... }` — that shape is
    forbidden by `#[non_exhaustive]` from downstream crates.
    Hands `&plan` to fanout, split, and response assembly.
  - **`BidderCall` construction.** `auctioneer-exchange::split`
    reads the per-bidder resolved values via a safe helper
    that takes the resolved-aliases map + resolved-budgets
    map EXPLICITLY (not off `ctx`):
    ```rust
    /// Round-42 B7 canonical signature (retires round-41
    /// B3's `resolve_budget(budgets, aliases, name)` which
    /// had no boot envelope + no warning sink, so the
    /// promised overlay-clamp behaviour had no executable
    /// API).
    pub fn resolve_budget(
        budgets: &ResolvedBudgets,
        aliases: &ResolvedAliases,
        bidder_name: &BidderName,
        boot_envelope: BootEnvelope,
        // Round-43 B6: `ErrorEntry`, not `NonFatalWarning`
        // — this IS the target bidder's
        // `BidderResultBundle.warnings` vec. See the
        // taxonomy table below.
        warnings: &mut Vec<ErrorEntry>,
    ) -> Result<BidderBudgets, InternalInvariantError> {
        // 1. Direct entry.
        let mut b = if let Some(b) = budgets.get(bidder_name).copied() {
            b
        // 2. Alias fallback.
        } else if let Some(base) = aliases.map.get(bidder_name) {
            budgets.get(base).copied()
                .ok_or(InternalInvariantError::BudgetMissing {
                    bidder_name: bidder_name.clone(),
                })?
        } else {
            return Err(InternalInvariantError::BudgetMissing {
                bidder_name: bidder_name.clone(),
            });
        };

        // 3. Round-42 B7 executable clamp: cap EACH field
        // independently against the boot envelope's
        // per-field caps. A single aggregate cap couldn't
        // unambiguously split against two independent
        // fields.
        let mut clamped = false;
        if b.max_transport_bytes > boot_envelope.max_transport_bytes {
            b.max_transport_bytes = boot_envelope.max_transport_bytes;
            clamped = true;
        }
        if b.max_decoded_bytes > boot_envelope.max_decoded_bytes {
            b.max_decoded_bytes = boot_envelope.max_decoded_bytes;
            clamped = true;
        }
        if clamped {
            // Round-43 B6: push an `ErrorEntry` — the
            // exchange's own wire-row type — NOT a
            // `NonFatalWarning`. See the taxonomy note
            // below.
            warnings.push(ErrorEntry {
                code: 10_019,
                message: format!(
                    "per-bidder budget override exceeded the adapter's \
                     boot envelope and was clamped down",
                ),
            });
        }
        Ok(b)
    }

    /// Round-43 B3: stored on `InstalledAdmission` (see
    /// the registry block above), so split.rs reads it via
    /// `admission_handle().envelope`.
    #[derive(Clone, Copy, Debug)]
    pub struct BootEnvelope {
        pub max_transport_bytes: usize,
        pub max_decoded_bytes: usize,
    }
    ```

    **Warning taxonomy (round-43 B6 canonical).** Round-42
    B7 constructed a `NonFatalWarning { code: u32, message,
    bidder }` in a new
    `crates/auctioneer-exchange/src/warnings.rs`. That type
    does not exist. The real shapes are:

    | Type | Crate / file | Shape | Used for |
    |---|---|---|---|
    | `NonFatalWarning` | `auctioneer-openrtb`, [normalized.rs:43](../../../crates/auctioneer-openrtb/src/normalized.rs#L43) | `{ code: WarningCode, message: String, scope: Option<String> }` | prepare() + `PrivacyContext::take_warnings()` |
    | `ErrorEntry` | `auctioneer-exchange`, [response.rs:16](../../../crates/auctioneer-exchange/src/response.rs#L16) | `{ code: u32, message: String }` | the response wire row |

    Neither has a `bidder` field: bidder attribution comes
    from the MAP KEY (`BidderResultBundle` is keyed by
    `BidderName`; `prebid_warnings` is the `"prebid"` key).
    Round-43 B6 pins each new code to its scope and its
    exact conversion:

    - **10_019 `BidderBudgetClampedToBootEnvelope` —
      BIDDER-scoped.** Produced inside split.rs, which is
      already building per-bidder state. `resolve_budget`
      pushes an `ErrorEntry` straight into the
      `BidderResultBundle.warnings` vec for that bidder
      (the `warnings: &mut Vec<ErrorEntry>` arg above IS
      that bundle's vec). No `NonFatalWarning` involved —
      split.rs never crosses the openrtb warning type.
      Surfaces at `ext.warnings.<bidder>[]`.

    - **10_018 `GvlVersionUnavailable` — auction-scoped (`prebid` wire key).**
      Produced inside `auctioneer-privacy`, which only knows
      the openrtb type. `PrivacyContext` pushes
      `NonFatalWarning { code:
      WarningCode::GvlVersionUnavailable, message,
      scope: Some("regs.gdpr".into()) }`.
      `run_auction` drains it via
      `privacy.take_warnings()` and converts at the
      exchange boundary — the conversion that already
      exists for code 10001:
      ```rust
      // crates/auctioneer-exchange/src/run.rs
      input.prebid_warnings.extend(
          privacy.take_warnings().into_iter().map(|w| ErrorEntry {
              code: w.code.as_u32(),
              message: w.message,
          }),
      );
      ```
      Surfaces at `ext.warnings.prebid[]`.

    `WarningCode` (auctioneer-openrtb) gains one variant —
    round-43 B6: it is an ENUM with an `as_u32()` mapper
    ([normalized.rs:60](../../../crates/auctioneer-openrtb/src/normalized.rs#L60)),
    not a raw u32:
    ```rust
    pub enum WarningCode {
        InvalidPrivacyConsent,      // 10001 (existing)
        GvlVersionUnavailable,      // 10018 (round-43 B6)
        Unknown,                    // 10999 (existing)
    }
    // as_u32() gains the matching arm.
    ```
    10_019 needs NO `WarningCode` variant — split.rs emits
    the raw `ErrorEntry` directly, which is exactly why
    `ErrorEntry.code` is a raw `u32` (see its doc comment
    at response.rs:16: "so any code value can be emitted
    without adding a new variant").
    `InternalInvariantError` lives on `auctioneer-exchange`; the
    auction handler maps it to 500 with the generic
    "Internal auction error" body. This is distinct from the
    config-layer `RuntimeConfigError` — a missing budget
    entry post-resolution is a auctioneer bug, not a deployment
    error.
    - **Alias inheritance policy** (round-32 H4 answer to the
      reviewer's question): request+host aliases inherit
      their base bidder's budget row unchanged. Alias-
      specific overrides via `[bidders.<alias>]` are NOT
      accepted — the `BidderOverrides` map is keyed by base
      names in the registry; a `[bidders.<request-alias>]`
      block is a `MalformedHost` at config load. This
      matches the existing per-bidder-overlay semantics
      (Task 16's registry lookup).
    - Populates `BidderCall.max_transport_bytes` and
      `BidderCall.max_decoded_bytes`. Historic
      `max_response_bytes` field REMOVED — **but only in the
      same change that updates its sole reader.** Round-47 B2:
      `DevFanout::classify` currently compares
      `resp.body.len() > call.max_response_bytes`
      ([fanout.rs:81](../../../crates/auctioneer-core/src/host_primitives/fanout.rs#L81));
      removing the field without rewiring `classify` fails to
      compile. This step re-points `classify` at the decoded
      budget — `resp.body.len() > call.max_decoded_bytes` →
      `FanoutError::OversizeResponse` → `(4, 100)` — so the
      oversize wire shape is preserved across the rename. The
      per-slot `Err(EdgeError::ResponseTooLarge { reason:
      ResponseLimitReason })` (round-140 #3 — the real enum; no
      `origin` field exists) from
      Task M3b-2's buffered results (round-133 #1 — the stream-item
      `TypedBodyError::TooLarge` path is retired with the stream),
      mapped by the round-130 table to
      `FanoutError::OversizeResponse`, is what lets an over-cap
      upstream response reach the same `(4, 100)` instead of
      `classify`'s `Err(_) → Other → (999, 100)` path (round-47 B1;
      round-114 B1 fixes this note's earlier nonexistent
      `TypedProxyError::Oversize` name).
    - New tests: `alias_inherits_base_bidder_budget()`,
      `budget_lookup_missing_entry_returns_internal_invariant_error()`
      (round-37 M1 rename — the implementation returns
      `InternalInvariantError::BudgetMissing`; the earlier
      `_returns_malformed_host()` name pointed at a
      `RuntimeConfigError` variant that doesn't fire on this
      path).
  - **Bidder-call driver: ACQUIRE, then `send_bounded`,
    then hold the permit through body completion (round-45
    B5 canonical).** Round-40-44 defined
    `AdmissionSemaphore::acquire()` and said
    `ProductionFanout` "reads the installed semaphore", but
    no code path actually CALLED acquire — the gate was
    inert, so every stream ran unadmitted and the whole
    memory proof was vacuous. Round-45 B5 wires it into
    `RaceTimersStrategy::drive_one`:
    ```rust
    // Round-45 B5: the permit is taken BEFORE the outbound
    // so the semaphore bounds simultaneously-live streams;
    // it is a DEADLINE-AWARE acquire so a call that would
    // starve past the auction deadline drops out as a
    // timeout instead of blocking the whole fanout; and it
    // is MOVED INTO the AdmittedBody guard so it lives
    // exactly as long as the body stream.
    let admission = admission_handle();
    let permit = tokio::select! {
        p = admission.semaphore.acquire() => p,
        // Deadline reached while queued behind other
        // in-flight streams. This bidder gets the standard
        // deadline treatment, not a hang.
        () = deadline.sleep_until() => {
            return Err(FanoutError::Timeout(TimeoutCause::AuctionDeadline));
        }
    };
    let bounded = client.send_bounded(&prepared, policy).await
        .map_err(classify_send_error)?;
    let (status, headers, body) = bounded.into_parts();
    // The permit MOVES into the guard that wraps the body
    // stream, so it is released only when the body is fully
    // consumed or the response is dropped — never at
    // send_bounded return (headers arrive long before the
    // body drains). See `AdmittedBody` above.
    let admitted = AdmittedBody { inner: body, _permit: permit, _abort: abort_guard };
    let decoded = decoder_pipeline(headers.clone(), admitted).boxed_local();
    // decoded: Stream<Item = Result<Bytes, FanoutError>>
    ```
    Round-45 B5 acquisition contract, stated once so no
    strategy re-derives it:
    1. **Acquire before `send_bounded`** — the permit
       bounds streams that are about to become live, not
       ones already draining.
    2. **Deadline-aware** — `acquire` races the auction
       deadline; a queued call that can't get a permit in
       time surfaces as `Timeout(AuctionDeadline)`, the
       same shape a slow bidder produces, so the response
       assembler needs no new case.
    3. **Permit ownership rides the body** — moved into
       `AdmittedBody`, released on body EOF / drop, never
       at header arrival.
    `DelegateToDevStrategy` (the M3a default, no real
    outbound) does NOT acquire — there is no bounded body
    stream to admit. Only production strategies that call
    `send_bounded` acquire.

    Regression: `admission_permit_held_until_body_eof()`
    (drives 2 streams on a 1-permit adapter; asserts the
    second `acquire` blocks until the first body is fully
    read, NOT until the first headers arrive) and
    `admission_deadline_starved_call_times_out()` (fills the
    permits, asserts a queued call past the deadline returns
    `Timeout(AuctionDeadline)`).

    Over-cap collection (`ResponseLimitReason::EncodedBody`)
    classifies as `FanoutError::OversizeResponse { source:
    Transport }`; every other slot error classifies per the
    round-130/147 table (round-151 #3 — the stream-item vocabulary
    that stood here is retired).

    `response.body()` (round-36 B4's phrasing) referred to
    an accessor that didn't exist on `BoundedProxyResponse`;
    round-37 B5 pinned the accessor set as `status()`,
    `headers()`, `headers_mut()`, `take_body()`,
    `into_parts()` (see the trait extension block above).
    The pseudocode above uses `into_parts()`; a caller that
    needs to hold onto status + headers after taking the
    body uses `take_body()`.
  - **TOML schema + codegen renderer.** Round-32 H3: the
    two build-baked fields need matching entries in
    `crates/auctioneer-config/build_codegen.rs`'s host-TOML
    struct + the codegen renderer that emits
    `ResourceLimits`. Without these, TOML declarations of
    the new budgets would be silently dropped at build time.
  - **Validation coverage.** Add tests:
    - `both_budgets_default_to_max_bidder_response_bytes_when_unset()`.
    - `runtime_auction_config_wins_over_baked_resource_limits()`.
    - `account_override_wins_over_runtime_auction_config()`.
    - `bidder_override_wins_over_account_for_decoded_budget()`.
    - `budget_below_1_kib_rejected_by_validator()`.
    - `budget_over_100mib_rejected_by_validator()`.
    - `per_bidder_map_holds_distinct_values_per_bidder()`
      (round-31 H1: constructs a fixture with three
      bidders each getting a different resolved
      `max_decoded_bytes`; asserts the map preserves them).

  **Bound restated as `observed <= cap + MAX_CHUNK_SIZE`**
  (round-24 H7 correction: was `cap + 1`, unachievable while a
  chunked transport reader feeds auctioneer).

  **Typed error surface** (round-40 B10 canonical — round-39
  B7 removed `Debug` from `TransportError` to prevent leak
  via `format!("{fanout_err:?}")` at
  [run.rs:563](../../../crates/auctioneer-exchange/src/run.rs#L563),
  but `FanoutError` and `BidderCallResult` derive `Debug`
  at
  [fanout.rs:34](../../../crates/auctioneer-exchange/src/fanout.rs#L34)
  and would fail to compile with a non-Debug variant.
  Round-40 B10 provides a custom sanitised `Debug` impl on
  `TransportError` that emits ONLY the category — no
  fields, no derive-generated struct dump. `FanoutError`
  can keep its derive; the Debug output through it stays
  sanitised.
  ```rust
  // crates/auctioneer-exchange/src/fanout.rs additions.
  // FanoutError already derives Clone + Debug at
  // fanout.rs:34; the new variants must keep both.
  // Round-40 B10: TransportError has a HAND-WRITTEN
  // Debug impl that emits only the sanitised category
  // string. FanoutError's #[derive(Debug)] then emits
  // `Transport(<category>)` which is safe to serialise.
  #[derive(Clone, PartialEq, Eq)]
  pub struct TransportError {
      pub kind: TransportErrorKind,
      // NO String detail field, NO internal_details. The
      // wire-safe category is `kind.public_message()`.
  }

  impl std::fmt::Debug for TransportError {
      fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
          // Round-40 B10: emit the sanitised category
          // only. Do NOT use derive(Debug) — struct-field
          // dump would leak whatever fields we add later.
          f.debug_tuple("TransportError")
              .field(&self.kind.public_message())
              .finish()
      }
  }

  impl std::fmt::Display for TransportError {
      fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
          f.write_str(self.kind.public_message())
      }
  }

  impl TransportErrorKind {
      /// Round-38 H4 / round-40 B10: static, sanitised
      /// message per variant. Safe to serialise into
      /// `ext.errors.<bidder>[]`.
      pub const fn public_message(self) -> &'static str {
          match self {
              Self::Unreachable => "bidder host unreachable",
              Self::Initial => "outbound send failed",
              Self::MidStream { .. } => "connection terminated mid-response",
              Self::ChunkTooLarge { .. } => "adapter yielded oversize chunk",
              Self::Preflight => "bidder request could not be dispatched",
          }
      }
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum TransportErrorKind {
      /// Round-36 H2: RESTORED. Preserves the spec's
      /// Unreachable → wire code 999 + `seatnonbid.
      /// statuscode = 103` mapping (spec §5.2:1138 +
      /// `crates/auctioneer-exchange/src/run.rs:588`).
      /// Populated by the classifier from
      /// `BadGatewayReason::Unreachable` (round-151 #3).
      Unreachable,
      /// Initial send failed with any kind of non-timeout,
      /// non-unreachable error.
      Initial,
      /// Stream item error mid-body (after any bytes were
      /// received).
      MidStream { decoded_bytes: u64 },
      /// Adapter yielded a chunk larger than MAX_CHUNK_SIZE.
      ChunkTooLarge { observed: u64, limit: u64 },
      /// Round-149 #2: auctioneer's own transport-conversion preflight
      /// failed (method/URI/header rejected by the http layer);
      /// nothing was sent. Category text: "bidder request could
      /// not be dispatched".
      Preflight,
  }

  pub enum FanoutError {
      // — pre-existing variants — (Clone)
      Timeout(TimeoutCause),
      OversizeResponse { source: OversizeSource },
      BadServerResponse,
      Other(String),
      // — round-34 B4 unified transport variant —
      Transport(TransportError),
      // — round-31 M4 decode variants — (already Clone)
      UnsupportedContentEncoding { actual: String },
      TooManyEncodingLayers { actual: usize },
      DecoderFailure { layer: ContentCoding, source: String },
  }

  // AS SHIPPED this is `OversizeOrigin` with TEN variants, not the
  // three-way `OversizeSource` sketched here. The two-way
  // Transport|Decoded axis this plan argued from could not name
  // WHICH cap tripped, so an operator reading a refusal learned
  // nothing actionable. Each upstream `ResponseLimitReason` now maps
  // to its own origin, plus three auctioneer-owned ones:
  //   EncodedBody BufferedBody DecodedBody DecoderMemory BrotliWindow
  //   HeaderBytes HeaderCount        <- the eight upstream reasons
  //   RequestHeaders CallResponseCap <- auctioneer's own caps
  //   Unclassified                   <- upstream's `Unspecified` and
  //                                     any future variant, so a new
  //                                     reason cannot be silently
  //                                     folded into a named class
  // The WIRE contract is unchanged: every origin renders the single
  // category "bidder response too large" at code 4 / statuscode 100,
  // asserted by `every_origin_renders_the_same_wire_category()`.
  // See `auctioneer-exchange/src/fanout.rs` for the definition and
  // `outbound/wave.rs`'s `oversize_origin()` for the mapping.
  pub enum OversizeOrigin { EncodedBody, DecodedBody, BufferedBody,
                            DecoderMemory, BrotliWindow, HeaderBytes,
                            HeaderCount, RequestHeaders,
                            CallResponseCap, Unclassified }
  pub enum ContentCoding { Gzip, Brotli, Deflate, Identity }
  ```
  **Round-52 B4 — extend `FanoutError::public_message()` in the
  SAME change.** M3a (round-51 H4) added the inherent
  `public_message(&self) -> &'static str` over the M2 variant set
  and pointed `run.rs` at it. This M3b evolution restructures
  `OversizeResponse`, drops `Unreachable`, and adds four variants,
  so the existing `public_message` becomes non-exhaustive / names a
  removed variant and will NOT compile until it is updated. It must
  gain the new arms (fixed categories, no dynamic content — same
  wire-safety rule):
  ```rust
  impl FanoutError {
      pub fn public_message(&self) -> &'static str {
          match self {
              FanoutError::Timeout(_)                    => "bidder timeout",
              FanoutError::OversizeResponse { .. }       => "bidder response too large",
              FanoutError::BadServerResponse             => "bidder server error",
              FanoutError::Other(_)                      => "internal fanout error",
              FanoutError::Transport(t)                  => t.kind.public_message(),
              FanoutError::UnsupportedContentEncoding { .. } => "unsupported content encoding",
              FanoutError::TooManyEncodingLayers { .. }  => "too many content-encoding layers",
              FanoutError::DecoderFailure { .. }         => "response decode failed",
          }
      }
  }
  ```
  Round-34 B4 crate-layering note: `auctioneer-exchange` stays
  neutral of `EdgeError` — that type never appears in the
  `FanoutError` variant list. The classifier lives in `auctioneer-core`
  (`crates/auctioneer-core/src/host_primitives/fanout.rs`) and
  translates BEFORE crossing into exchange. Round-151 #3, CORRECTED
  round-156 #3: its ONE input is the facade's THREE-ARM
  `BoundedSlotError { Expired, Preflight, Edge(EdgeError) }` — the
  `Expired` arm was missing from this sentence, and an implementer
  following it would have fallen through to a generic transport
  failure for auctioneer's own deadline expiry instead of a timeout. Its
  ONE table is the round-130/147 mapping, exhaustive over all three
  arms: `Expired` → `Timeout(selected_cause)` (auctioneer's wave wait or
  passthrough decode ran past the composed instant); `Preflight` →
  `Transport(TransportError { kind: Preflight })`; `Edge(e)` → the
  upstream table (exhaustive over `BadGatewayReason`,
  `BadGatewayDecodeReason`, `ResponseLimitReason` and every other
  `EdgeError` variant, with `GatewayTimeout` → `Timeout(selected_
  cause)`). No wildcard over `BoundedSlotError`; the spec's `classify(outcome,
  selected_cause)` in §5.2 is the normative call site. The
  `classify_send_error(TypedProxyError)` /
  `classify_body_stream_error(TypedBodyError)` implementations that
  stood here targeted the retired streaming types and are in the
  historical appendix §10.

  **Companion — `run.rs` already uses `public_message()`
  (round-52 B4; supersedes the round-40 B10 "Shape A / Shape B"
  choice).** The `run.rs:563` serialiser was changed by M3a
  (round-51 H4) from `format!("{fanout_err:?}")` to
  `fanout_err.public_message().to_owned()`. So there is no
  remaining choice between a Debug impl and a helper — the inherent
  `public_message()` above is the single wire path, and M3b's only
  job here is to keep it exhaustive over the evolved variant set
  (done above). The stale "Shape A: keep `{:?}`" option is removed;
  Debug on `FanoutError` is no longer on the wire path, so
  `TransportError`'s hand-written Debug is a defence-in-depth extra
  (for logs), not the sanitiser of record.

  The round-40 B10 regression test asserts BOTH paths
  emit sanitised output:
  ```rust
  #[test]
  fn ext_errors_never_contains_transport_source_details() {
      let fanout_err = FanoutError::Transport(TransportError {
          kind: TransportErrorKind::Unreachable,
      });
      let serialised = serde_json::to_string(&build_response_ext_errors_from(&fanout_err))
          .unwrap();
      // Must not contain any URL/host/IP fragments a
      // wrapped anyhow::Error might have carried.
      assert!(!serialised.contains("http"));
      assert!(!serialised.contains("://"));
      assert_eq!(serialised.matches("\"bidder host unreachable\"").count(), 1);
  }
  ```
  Round-39 B7 concludes: `auctioneer-exchange`'s `Cargo.toml`
  has NO `anyhow` dependency; `auctioneer-core` retains it for
  the classifier + logger; the pathway from adapter →
  auctioneer-core → auctioneer-exchange narrows from a typed
  source-carrying struct to a bare category.
  Round-36 H2 note: **`TransportErrorKind::Unreachable` is
  RESTORED** (round-35 B9 retirement reversed). The spec at
  §5.2:1138 and current
  `crates/auctioneer-exchange/src/run.rs:588` both map
  unreachable to wire code 999 + `seatnonbid.statuscode =
  103`; collapsing that into code 4 (round-35 B9) was a wire
  regression against the current M2 behaviour. Round-151 #3: the
  discriminant is upstream's `BadGatewayReason::Unreachable`
  (LANDED @120dd2aa — EdgeZero's adapters classify connect/DNS
  failures); every other `BadGatewayReason` maps to
  `TransportErrorKind::Initial` (code 4) per the round-147 table.

  Wire mapping (round-37 H2 canonical — retires round-36
  H2's "everything non-timeout/non-unreachable → code 4"
  which contradicted the pinned spec + current
  `crates/auctioneer-exchange/src/run.rs:585-591` baseline
  where `FanoutError::Other` maps to `(999, 100)`):
  - `Timeout` → bidder code 1 + statuscode 101, plus global
    code 12 iff the exchange's post-fanout clock re-read shows
    `now_monotonic() >= auction deadline_at` (round-151 #1 — a
    slot's `AuctionDeadline` cause is provenance, never the
    trigger; see M3b-2b.1).
  - `TransportErrorKind::Unreachable` → bidder code 999 +
    statuscode 103.
  - `TransportErrorKind::Initial` / `MidStream` /
    `ChunkTooLarge` → bidder code 4 + statuscode 100.
  - `FanoutError::BadServerResponse` / `OversizeResponse` /
    `UnsupportedContentEncoding` / `TooManyEncodingLayers` /
    `DecoderFailure` → bidder code 4 + statuscode 100.
  - `FanoutError::Other(_)` → bidder code 999 + statuscode
    100 (round-37 H2: preserves the current M2 mapping so
    M3b is not a wire regression).

  Migration tests (round-37 H2 — asserted in Task M3b-2 as
  part of the wire-error regression suite):
  - `fanout_error_other_maps_to_bidder_code_999_statuscode_100()`.
  - `fanout_error_bad_server_response_maps_to_bidder_code_4_statuscode_100()`.
  - `fanout_error_transport_initial_maps_to_bidder_code_4_statuscode_100()`.
  - `fanout_error_transport_unreachable_maps_to_bidder_code_999_statuscode_103()`.
  - `fanout_error_timeout_bidder_maps_to_bidder_code_1_statuscode_101_no_global_code_12()`.
  - `fanout_error_timeout_deadline_maps_to_bidder_code_1_statuscode_101_and_global_code_12()`.
  - `transport_error_public_message_omits_source_details()`
    (round-38 H4, corrected round-50 M1). `TransportError` holds
    ONLY `kind` — there is NO `.message` and NO `.internal_details`
    field (see its definition above), so the test must NOT assert
    on them. It feeds `BoundedSlotError::Edge(EdgeError::BadGateway
    { reason: BadGatewayReason::Unreachable, .. })` whose message is
    `"resolve: bidder.example.internal:8080 -> 10.0.0.5:8080 (auth
    token=xyz)"` into the classifier (round-151 #3) and asserts:
    (a) the wire category — `kind.public_message() == "bidder host
    unreachable"` and the `run.rs` `ext.errors` message (now
    `fanout_err.public_message()`, round-52 B4) contains NO URL,
    IP, port, or token substring;
    (b) the log line carries NO detail at all (round-52 H1 — logs
    are an exported credential surface, and an allowlist beats a
    blocklist). The test installs `testing_logger` and asserts the
    single WARN line is exactly `"bidder unreachable"` — the
    `anyhow` `source` chain (`bidder.example.internal`, `10.0.0.5`,
    `auth token=xyz`) appears NOWHERE. The round-50/51 wordings —
    "carrying the full anyhow chain" and "redacted host/path" — are
    both retired: M3b logs the typed category only, never the
    source.

  **Round-34 B3 sweep of superseded variants:**
  `ProxyClientError` is REMOVED from the plan. Every earlier
  reference to it is dead — Prereq 2 (below) documents the
  pinned `ProxyClient::send -> Result<ProxyResponse,
  EdgeError>` shape as the canonical contract; no new proxy-
  side error type is introduced.

- [ ] **Task M3b-3 — Cloudflare `ProductionFanout` (ROUND-142 #2
  REWRITE: a auctioneer INTEGRATION task — cancellation and deadlines
  are EdgeZero-OWNED behaviors now; auctioneer wires nothing into the
  platform).** Auctioneer CONSUMES EdgeZero's **BestEffort** cancellation
  and deadline support on Cloudflare (round-174 #1 — both were
  described as "Native"; at head Cloudflare reports
  `outbound-batch-cancellation` AND `outbound-deadlines` as
  BestEffort, and only Axum is Native for cancellation): dropping the
  pending `send_bounded_all` future is the whole cancellation
  contract, and EdgeZero's adapter issues a concrete guest-side
  `AbortController::abort()` for each Fetch in that batch.
  **WHAT AUCTIONEER MAY AND MAY NOT GUARANTEE HERE:**
  - GUARANTEED, because they are auctioneer's own: no late slot is
    published as on-time, and the wave lease releases by RAII.
  - NOT GUARANTEED: that every origin-side Fetch has actually
    terminated. The abort is REQUESTED, not proven finite — upstream
    has no deployed evidence of bounded host teardown, which is why
    the row is BestEffort. No auctioneer test may assert origin-side
    termination (the round-133 drop-path tests that did are already
    retired). ROUND-147 #3 —
  WHEN the batch may be dropped: ONLY on auction/request cancellation
  (handler teardown — the client went away or the handler future was
  dropped) or once EVERY member of the batch is expired — i.e. at
  **ROUND-156 #1 — CORRECTED: deadline-driven batch cancellation is
  RETIRED; handler teardown is the ONLY drop trigger.** The round-155
  #7 timer race could not preserve already-terminal slot results:
  the removed one-shot call yielded its vector only when the whole batch
  future completes, so dropping it at `max(deadline_at)` would
  discard the slots that had already succeeded, the facade's recorded
  preflight failures, and every slot's own elapsed and error class —
  the precise harm round-153 #2 forbids, applied to a whole wave. The
  wave future is therefore AWAITED TO COMPLETION; per-request expiry
  is EdgeZero's (each request carries its own absolute deadline and
  the adapter expires that slot alone), and on BestEffort hosts an
  overrun is reported as the capability row says rather than cut
  short. The upstream ask that would close the gap — a batch handle
  that cancels pending slots while collecting completed ones — is
  feedback G17 and gates nothing. Round-154 #3's instant survives only
  as the description of when every member is expired: because each
  deadline is `min(bidder_at, auction_at)`,
  members are heterogeneous by construction and the EARLIEST
  deadline proves nothing about the others; only the LATEST one
  can. (The common case where every member shares one instant —
  every bidder's timeout exceeding the auction budget — is just
  `max == min` of that set, not a separate rule.) NEVER at the earliest member's deadline: members carry
  heterogeneous composed deadlines (§5.2 `min(bidder_at,
  auction_at)`), and dropping the batch when the first one expires
  would cancel siblings whose deadlines have not passed and lose
  their positional outcomes. Per-call expiry is EdgeZero's per-slot
  job — each `OutboundRequest` carries its own
  `Deadline::at_instant(composed_at)` and expires as
  `GatewayTimeout { cause }` INSIDE its own slot while the batch
  keeps running, and THAT outcome is terminal — round-154 #2: there
  is NO auctioneer post-return recheck over upstream's phase on ANY
  target. The batch vector returns once, so comparing the clock
  after the batch would convert a bidder that finished well inside
  its deadline into a Timeout merely because a sibling delayed the
  return. Auctioneer's absolute checks cover only auctioneer-owned work: the
  wave wait and auctioneer's own passthrough decode (spec §5.2). The
  earlier "wire
  AbortController through the proxy" instruction and the
  round-133 drop-path tests that asserted origin-side abort
  timing are RETIRED (they tested EdgeZero's behavior).
  Integration tests in Task M3b-3 — **EIGHT rows (round-181 #7: an
  earlier line said "six", which never matched the enumeration):**
  - `cf_besteffort_cancellation_suppresses_late_results_and_releases_lease()`
    (round-174 #1 — renamed from `cf_native_cancellation_…`: the
    capability is BestEffort, and the test asserts only what auctioneer
    owns — suppression and lease release — never origin-side abort)
    — drop the pending batch on auction teardown (the handler future
    is dropped mid-wave); assert no late slot is surfaced as on-time
    and the wave lease releases by RAII.
  - `cf_batch_is_never_dropped_on_a_deadline()` — round-156 #1
    (REPLACES round-155 #7's `cf_batch_dropped_only_at_latest_member_deadline`,
    whose drop destroyed completed results): with composed
    deadlines t+10, t+50, t+50, slot 0 expires as its OWN
    `GatewayTimeout` while slots 1 and 2 run on and return real
    responses; no auctioneer timer touches the batch, and the wave lease
    releases by RAII when the future completes. A second case drops
    the auction future mid-wave (handler teardown) and asserts ONLY
    AUCTIONEER-OWNED OUTCOMES (round-175 #1 — it previously asserted
    "every Fetch aborts", contradicting the BestEffort contract
    twenty lines above: origin-side termination is REQUESTED, never
    proven finite, so no auctioneer test may assert it): nothing is
    surfaced as on-time, the wave lease releases by RAII, and each
    unresolved slot keeps its stored `selected_cause` provenance.
  - `cf_batch_survives_earliest_member_deadline()` — round-147 #3:
    three calls with composed deadlines t+10, t+50, t+50 in one
    batch; the first expires as its own slot's `GatewayTimeout`
    while the batch is NOT dropped, and `out[1]`/`out[2]` deliver
    real responses in position (the earliest-member deadline never
    cancels valid siblings).
  - `cf_deadline_cause_matches_composed_selected_cause()` —
    (round-175 #1: renamed from `cf_native_deadline_…`; Cloudflare's
    `outbound-deadlines` is BestEffort at head, so a capability-
    neutral name is used — the assertion is about auctioneer's stored
    provenance, which holds at every support level)
    the `GatewayTimeout { cause }` reported upstream never overrides
    auctioneer's stored `selected_cause` (the BudgetSource provenance
    rule).
  - **Real host timer (round-153 #3).** Replace
    `ImmediateResolveTimer` in `auctioneer-adapter-cloudflare/src/lib.rs`
    with `CloudflareTimer`, a `Timer` whose `sleep(duration)` awaits
    `worker::Delay::from(duration)` (the Workers `setTimeout`-backed
    future) — a clock-refreshing host boundary, so the wave race and
    the post-acquisition recheck both read a fresh instant. Tests:
    `wave_wait_does_not_expire_before_deadline_cloudflare()` and
    `cloudflare_timer_sleep_advances_the_host_clock()` (the instant
    after `sleep(d)` is at least `d` later — the frozen-clock
    property the CPU-only path lacks).
  - **`DecodeCooperator` (round-153 #4, round-154 #7).** Cloudflare's
    impl awaits `worker::Delay::from(Duration::ZERO)` — a real
    host event that returns to the JS event loop and refreshes the
    frozen clock; an already-resolved promise or a bare `yield_now()`
    does NOT and must not be used here. Installed through the
    `bootstrap_adapter` installer argument
    (`make_decode_cooperator`), read via `decode_cooperator_handle()`,
    and threaded from `fanout_from_context` into every `run_wave`.
    **AS BUILT (M3b-3) — THREE CORRECTIONS. (1) It is the FIFTH of
    FIVE installer arguments, not the sixth: `bootstrap_adapter` took
    FOUR (clock, rng, timer, strategy) and the cooperator was inserted
    fourth, making five. Any text saying "sixth" is wrong.
    (2) NONE of this seam existed — no trait, no registry cell, no
    accessor, and `run_wave` took no such parameter. This bullet reads
    as though only the Cloudflare impl were missing; M3b-3 built the
    whole seam, which touches every adapter's boot call and is far
    larger than its own Files list implied. (3) The threading is NOT
    yet live on the handler path: `fanout_from_context` builds
    `ProductionFanout::new`, and `with_wave_scheduler` — which the
    cooperator is threaded through — had ZERO callers. Flipping that
    seam switches every target at once, so it is DEFERRED; this is why
    `decode_cooperator_handle()` is currently exported but uncalled,
    and a downstream unit owns the flip.**
    Test:
    `cloudflare_decode_expiry_under_frozen_cpu_clock()` — a decode whose CPU time exceeds the deadline returns
    `Timeout(selected_cause)` even though the clock only advances at
    `yield_to_host()`.
  - `capability_table_matches_cloudflare()` — round-147 #7: the
    per-target CI EQUALITY check (the round-143 #4 rule: there is
    no app-level boot assertion; optional degraded capabilities
    never fail startup) — the Cloudflare adapter's reported support
    equals the expected-support table row for row-by-row: Native for
    `outbound-http`, `outbound-batch-completion-order` and
    `outbound-batch-slot-isolation`; **BestEffort for
    `outbound-deadlines` AND `outbound-batch-cancellation`
    (round-178 #3 — this row said "Native for outbound-http and
    outbound-deadlines", contradicting its own task body and
    upstream)**; any drift fails CI.
  **EXECUTABLE SHAPE (round-177 #1 — this task named its tests but
  no files, interfaces or steps, so it could not be dispatched):**
  - **Blocked by:** 2b.2 (the wave scheduler it integrates).
  - Files: Modify `crates/auctioneer-adapter-cloudflare/src/lib.rs`
    (install `CloudflareTimer` and the `DecodeCooperator` through
    `bootstrap_adapter`, replacing `ImmediateResolveTimer`); Create
    `crates/auctioneer-adapter-cloudflare/src/timer.rs` (`CloudflareTimer`
    over `worker::Delay`) and
    `crates/auctioneer-adapter-cloudflare/src/cooperator.rs`
    (`DecodeCooperator` awaiting `Delay::from(Duration::ZERO)` — the
    real host event, round-154 #7); **Create** (round-178 #3 — it does
    not exist yet; every earlier "Modify" here was wrong)
    `crates/auctioneer-adapter-cloudflare/src/limits.rs`
    (`OUTBOUND_PROFILE`: `elapsed_semantics: CompletionTime`,
    `deadline_support: BestEffort` — round-174 #1); Create
    `crates/auctioneer-adapter-cloudflare/tests/fanout.rs` (the six
    integration rows below).
  - Interfaces (Produces): `CloudflareTimer: Timer`,
    `CloudflareDecodeCooperator: DecodeCooperator`, and the
    static `OUTBOUND_PROFILE` capability declaration.
  - **⚠ ROUND-179 #4 — WHAT IS AND IS NOT HOST-TESTABLE HERE, read
    from the manifests rather than assumed.** `auctioneer-adapter-
    cloudflare` declares `crate-type = ["cdylib", "rlib"]`, so a host
    test CAN import it — but `worker` is under
    `[target.'cfg(target_arch = "wasm32")'.dependencies]`, so
    `worker::Delay` DOES NOT EXIST on the host. A host `FakeClock`
    therefore cannot verify the concrete `Delay` binding, and
    `cloudflare_timer_sleep_advances_the_host_clock()` as written is
    unprovable on either target (host: no `Delay`; wasm: no runner).
    THE SEAM THIS TASK MUST ADD: `CloudflareTimer` is generic over a
    `DelaySource` trait (`async fn delay(&self, d: Duration)`), with
    the `worker::Delay` implementation `#[cfg(target_arch =
    "wasm32")]` and a host fake used by tests. The host test then
    proves the TIMER LOGIC — that `sleep` awaits the source for the
    requested duration and the wave race observes it — while the
    `Delay` binding itself is COMPILE-VERIFIED on wasm and exercised
    only by a deployed probe, which this repository cannot run. State
    that limit rather than implying the binding is tested.
  - **⚠ ROUND-178 #2 — THERE IS NO WASM RUNNER IN THIS REPOSITORY.**
    `cargo test --target wasm32-*` CANNOT be a passing command here:
    CI builds wasm targets with `--no-run` precisely because nothing
    can execute them (`.github/workflows/test.yml`). The recipes
    below therefore SPLIT: host-executable logic runs on the host;
    the wasm target is COMPILE-VERIFIED only. Adding a runner
    (wasmtime for wasip2, `wasm-bindgen-test` for
    wasm32-unknown-unknown) is a separate, optional follow-on — it is
    NOT a precondition for these tasks and must not be assumed.
  - Step 1: write the EIGHT failing tests named above (round-179 #7
    — round-178 #5 said seven and was itself miscounted; the set is
    the two cancellation/deadline rows, the two batch-lifetime rows,
    the timer row, the cooperator row, the wave-wait row and the
    capability row) — the two cancellation/deadline rows, the two
    batch-lifetime rows, the timer row, the wave-wait row and the
    capability row. Put every assertion that does not need the
    Workers runtime in `tests/fanout.rs` behind the HOST target, with
    the timer and cooperator exercised through their traits against a
    `FakeClock`; only the thin `worker::Delay` binding is
    wasm-specific. Step 2: `cargo test -p auctioneer-adapter-cloudflare`
    (HOST) — compile failure. Step 3: implement the timer, the
    cooperator and the profile. Step 4: PASS on the host, AND
    `cargo check -p auctioneer-adapter-cloudflare --target
    wasm32-unknown-unknown` plus `cargo test --no-run -p
    auctioneer-adapter-cloudflare --target wasm32-unknown-unknown` green
    — the same compile-only bar CI already enforces. Step 5: commit
    `auctioneer-adapter-cloudflare: production fanout — host timer,
    decode cooperator, BestEffort profile`.
  Unignore Bucket B rows on Cloudflare.

- [ ] **Task M3b-4 — Spin `ProductionFanout`.**
  **⚠ ROUND-200 #2 — THE wasip2 TARGET MOVE IS NOT THIS TASK'S.
  M3b-11 IS ITS SOLE OWNER.** Round-154 #4 made the
  `wasm32-wasip1` → `wasm32-wasip2` move this task's "FIRST UNIT,
  blocking", but M3b-4 is blocked by 2b.2, which is blocked by
  M3b-11 — and M3b-11 conditionally owned the SAME move over the
  SAME files. So the migration was scheduled to happen both before
  and after itself, and two tasks could each edit `edgezero.toml`'s
  target key. **M3b-11 carries it unconditionally** (it is already
  the atomic pin/compilation task, and the target move is a
  compilation concern), and M3b-4 is left owning **Spin fanout
  integration and its tests** — which is what its file list and
  test roster actually describe. HISTORICAL, for the reason the
  move was ever placed here: `crates/
  auctioneer-adapter-spin/spin.toml`, the two CI matrix rows, the master
  spec's build-matrix commands, and the README row; `cargo check -p
  auctioneer-adapter-spin --target wasm32-wasip2` green and the crate's
  `fanout_conformance` test still compiling, with
  `spin_builds_on_wasip2()` as the CI pin. Fastly stays on wasip1.
  THEN (ROUND-142 #2 REWRITE:
  integration under Spin's BestEffort deadlines — the round-112 H3
  drop-cancellation probe and the WASI-teardown design are RETIRED;
  whether the host tears the request down is EdgeZero's concern and
  is BestEffort by design).** Auctioneer verifies its OWN invariants
  hold under BestEffort host teardown:
  - `spin_upstream_expiry_is_reported_not_reinterpreted()` —
    round-154 #2 (REPLACES `spin_late_result_after_deadline_is_suppressed`,
    whose post-return clock compare could discard a
    fast sibling when another slot delayed the batch): a slot whose
    BestEffort host ran past the deadline arrives as
    `GatewayTimeout` and classifies as `Timeout(selected_cause)`,
    while a sibling that answered inside its own deadline in the
    SAME batch publishes `Ok` — auctioneer re-checks only its own decode.
  - `spin_lease_released_regardless_of_host_teardown()` — the wave
    lease releases by RAII whether or not the host honored the
    cancellation.
  - **Real host timer (round-153 #3).** Replace
    `ImmediateResolveTimer` in `auctioneer-adapter-spin/src/lib.rs` with
    `SpinTimer`, whose `sleep(duration)` awaits
    `spin_sdk::time::sleep` (ROUND-202 #6 — this said
      `spin_sdk::wasi::clocks::monotonic_clock::subscribe_duration`,
      which SDK 7 does not expose; `sleep` is what EdgeZero's own
      Spin adapter uses, verified in
      `edgezero-adapter-spin/src/config_store.rs`. The `DelaySource`
      wiring takes `sleep` too; the prescribed recipe would not have
      compiled)
    (the WASI monotonic-clock pollable). Test:
    `wave_wait_does_not_expire_before_deadline_spin()`.
  - **`DecodeCooperator` (round-153 #4).** Spin's impl is
    `yield_now()` — WASI does not freeze the monotonic clock across
    CPU work, so the yield only bounds poll monopolization; the
    clock read after it is already fresh.
  - `capability_table_matches_spin()` — round-147 #7: the per-target
    CI EQUALITY check (no boot assertion; a BestEffort row is an
    optional degraded capability and cannot fail startup) — Spin's
    reported support equals its expected-support column, including
    BestEffort for outbound-deadlines; any drift fails CI.
  **EXECUTABLE SHAPE (round-177 #1):**
  - **Blocked by:** 2b.2 **and M3b-11** (round-200 #2 — M3b-11 owns
    the wasip2 target move this task consumes; nothing here re-does
    it). This task has NO "first unit": it is Spin fanout
    integration, start to finish.
  - Files: Modify `crates/auctioneer-adapter-spin/src/lib.rs` (install
    `SpinTimer` + cooperator via `bootstrap_adapter`); Create
    `crates/auctioneer-adapter-spin/src/timer.rs` (`SpinTimer` over
    **`spin_sdk::time::sleep`** — round-202 #6: SDK 7 does not
    expose `subscribe_duration`, and `sleep` is what EdgeZero's own
    Spin adapter uses)
    and `crates/auctioneer-adapter-spin/src/cooperator.rs` (`yield_now`
    — WASI does not freeze the clock across CPU work, round-154 #7);
    **Create** (round-178 #3 — it does not exist yet)
    `crates/auctioneer-adapter-spin/src/limits.rs`
    (`OUTBOUND_PROFILE`: `CompletionTime`, `BestEffort`); Create
    `crates/auctioneer-adapter-spin/tests/fanout.rs`.
  - Interfaces (Produces): `SpinTimer: Timer`,
    `SpinDecodeCooperator: DecodeCooperator`, `OUTBOUND_PROFILE`.
  - **⚠ ROUND-179 #4 — SPIN IS `crate-type = ["cdylib"]` ONLY, so
    NOTHING can import it, host or otherwise, and `spin-sdk` is
    wasm-gated. The proposed `tests/fanout.rs` is unwritable as
    specified.** FIRST, add `"rlib"` to the crate type (as Cloudflare
    already has) — that is a Cargo.toml change this task owns — and
    give `SpinTimer` the same `DelaySource` seam described under
    M3b-3, with the WASI pollable implementation wasm-gated. Only
    then are the tests below writable.
  - Files: ALSO Modify `crates/auctioneer-adapter-spin/Cargo.toml`
    (`crate-type = ["cdylib", "rlib"]`).
  - Step 1: the four failing tests named above, host-executable
    against the `DelaySource` seam (round-178 #2 — no wasm runner
    exists; round-179 #4 — and no wasm SDK on the host).
    Step 2: `cargo test -p auctioneer-adapter-spin` (HOST) — compile
    failure. Step 3: implement. Step 4: PASS on the host, AND
    `cargo check -p auctioneer-adapter-spin --target wasm32-wasip2`
    plus `cargo test --no-run -p auctioneer-adapter-spin --target
    wasm32-wasip2`, with the crate's existing `fanout_conformance`
    still green. Step 5: commit `auctioneer-adapter-spin: production
    fanout on wasip2`.
  (Round-20 correction stands: Spin IS async.) Unignore Bucket B
  rows on Spin.

- [ ] **Task M3b-5 — Fastly `ProductionFanout` (ROUND-142 #2
  REWRITE: Fastly's batch dispatch LANDED at the candidate head —
  Prereq 4 is satisfied by dispatch-before-harvest, BestEffort by
  design; the "native fanout primitive" gate is retired).** Auctioneer
  verifies, without any Native cancellation claim:
  - `fastly_dispatches_every_slot_before_first_harvest()` — every
    successful dispatch call completes before the first pending
    result is harvested (round-152 #5: the asserted property is
    dispatch order relative to the first harvest, NOT simultaneous
    overlap, which sequential dispatch cannot guarantee).
  - `fastly_results_are_positional_under_harvest_order()` —
    harvest order never reorders `out[i]` relative to `calls[i]`.
  - `fastly_elapsed_is_observation_time_and_named_so()` — the
    per-slot metric carries the `elapsed_observed` semantic
    (round-138 #4), never presented as a completion timestamp.
  - **Timer + gate wait (round-153 #3, scoped round-154 #6) —
    Fastly has NO host sleep primitive available to the guest
    between dispatch and harvest. CONSEQUENCE, stated plainly: the
    wave-gate wait on Fastly is BestEffort and CAN overrun a slot
    deadline — a bare `acquire_wave(k).await` cannot re-read the
    clock until a permit wakes it, and no timer can wake it sooner.
    Auctioneer re-checks the clock on every permit wake and emits the
    due positional timeouts then; it does NOT claim the wait is
    deadline-bounded. This is declared in Fastly's capability row
    (`outbound-deadlines: BestEffort`) and its deployment docs, and
    pinned by `fastly_gate_wait_is_besteffort_and_documented()`.
    Concretely:** `FastlyTimer::sleep` resolves
    immediately, and Fastly's wave loop therefore does NOT race a
    sleeping timer — it recomputes `earliest` and re-checks the
    clock on every loop iteration instead, so an uncontended wave
    dispatches normally and a contended one degrades to a spin-free
    recheck at each acquisition rather than an instant spurious
    timeout. This is a BestEffort behavior, declared in Fastly's
    capability row like its deadlines. Tests:
    `wave_wait_does_not_expire_before_deadline_fastly()` (the
    BestEffort path still must not emit a pre-dispatch timeout while
    the deadline is in the future) and
    `fastly_wave_loop_does_not_busy_spin()` (bounded iteration
    count per acquisition).
  - **`DecodeCooperator` (round-153 #4).** Fastly's impl is
    `yield_now()`, as Spin's — the Compute clock is not frozen
    across CPU work.
  - `capability_table_matches_fastly()` — round-147 #7: the
    per-target CI EQUALITY check — Fastly's reported support equals
    its expected-support column (BestEffort for outbound-http,
    outbound-deadlines and the three `outbound-batch-*` rows); a
    Native claim
    on Fastly, or a BestEffort cell elsewhere turning Native, fails
    CI — never boot, which has no capability assertion API.
  **EXECUTABLE SHAPE (round-177 #1):**
  - **Blocked by:** 2b.2.
  - Files: Modify `crates/auctioneer-adapter-fastly/src/main.rs`
    (install `FastlyTimer` + cooperator); Create
    `crates/auctioneer-adapter-fastly/src/timer.rs` (the documented
    BestEffort immediate-resolve timer, round-154 #6 — Fastly has no
    guest-visible sleep between dispatch and harvest) and
    `crates/auctioneer-adapter-fastly/src/cooperator.rs` (`yield_now`);
    **Create** (round-178 #3 — it does not exist yet)
    `crates/auctioneer-adapter-fastly/src/limits.rs`
    (`OUTBOUND_PROFILE`: `elapsed_semantics: ObservationTime`,
    `deadline_support: BestEffort`); Create
    `crates/auctioneer-adapter-fastly/tests/fanout.rs`.
  - Interfaces (Produces): `FastlyTimer: Timer`,
    `FastlyDecodeCooperator: DecodeCooperator`, `OUTBOUND_PROFILE`
    (the ONLY profile with `ObservationTime`).
  - **⚠ ROUND-178 #2 — FASTLY IS BINARY-ONLY TODAY** (`Cargo.toml`
    declares `[[bin]]` and no lib target), so an integration test
    CANNOT import its timer or cooperator. THE FIRST STEP OF THIS
    TASK IS TO ADD A LIB TARGET: `[lib] name = "auctioneer_adapter_fastly"
    path = "src/lib.rs"`, with `src/main.rs` reduced to a thin
    entrypoint that calls into it. Without that the tests below are
    unwritable.
  - Files: ALSO Modify `crates/auctioneer-adapter-fastly/Cargo.toml` and
    Create `crates/auctioneer-adapter-fastly/src/lib.rs` (round-179 #4 —
    the step above named this work but the file list omitted both).
    `fastly` is likewise wasm-gated, so the timer takes the same
    `DelaySource` seam as the other two — **but with
    `ImmediateDelaySource` as its PRODUCTION implementation
    (round-180 #7: this task also says the Fastly timer resolves
    immediately and is never raced, which the bare phrase "the same
    seam" contradicted). The seam exists for TESTABILITY of the
    surrounding loop, not to give Fastly a real sleep it does not
    have. Host tests MUST NOT substitute a delaying source that would
    simulate deadline behaviour Fastly cannot deliver: the fixture
    asserts the BestEffort loop — bounded iterations, clock re-read
    per permit wake — against the immediate source, which is what
    ships.**
  - Step 1: add the lib target, then write the SEVEN failing tests
    named above (round-179 #7 — round-178 #5 said eight and was
    itself miscounted), including the
    gate-wait BestEffort row and the busy-spin bound — host-
    executable, per the Cloudflare note. Step 2: `cargo test -p
    auctioneer-adapter-fastly` (HOST) — compile failure. Step 3:
    implement. Step 4: PASS on the host, AND `cargo check -p
    auctioneer-adapter-fastly --target wasm32-wasip1` plus `cargo test
    --no-run -p auctioneer-adapter-fastly --target wasm32-wasip1`.
    Step 5: commit `auctioneer-adapter-fastly: production fanout —
    observation-time elapsed, BestEffort gate wait`.
  Unignore Bucket B rows on Fastly.

- [ ] **Task M3b-0 — batch-API documentation rebase (round-161 #3
  turns the P0 banner's requirement into an executable task).
  ROUND-162 #4: it is DOCUMENTATION PLUS ONE SCRIPT AND ONE CI WIRE
  — no library source changes, so it cannot break the build, but the
  "documentation only" label was wrong about its own deliverables.**
  - **Blocks:** M3b-11 and M3b-2b core. Blocked by: nothing.
  - **Files:**
    - `scripts/check-batch-rebase.sh` (NEW — the gate below).
    - `.github/workflows/test.yml` (wire the gate beside the
      existing doc checks).
    - `docs/superpowers/specs/2026-05-21-auctioneer-openrtb-server-design.md`
      — §5.2's algorithm: replace the `http.send_all(reqs).await`
      call and its exact-length vector validation with the batch
      handle (`start_batch_until` + the `OutboundBatchNext` loop),
      carrying the four-outcome mapping from the banner.
    - `docs/superpowers/plans/2026-09-06-edgezero-outbound-spec-feedback.md`
      — **ROUND-165 #1: it was NOT in this file list, yet it sits in
      `docs/superpowers/plans/` which the gate scans, and it still
      names the removed client method — so the gate would have stayed
      RED after every declared edit was done.** It is IN SCOPE, not
      excluded: its current-head summary is normative guidance
      (refreshed 2026-09-17), not a frozen historical record, so the
      removed-API mention there is rewritten like any other. Only the
      per-gap entries BELOW that summary are historical, and they
      describe upstream's design vocabulary rather than auctioneer's call
      sites, so they need no rewrite.
    - `docs/superpowers/plans/2026-07-10-m3b-multi-adapter-production-fanout.md`
      — every `BoundedFetch` / `run_wave` / facade paragraph that
      states the `send_all` contract normatively; the stale
      capability names (`send-all-slot-isolation` → the three
      `outbound-batch-*` rows) and `BudgetSource::BatchDeadline` →
      `RequestDeadline` / `BatchCutoff` wherever they still appear.
    - Both documents' pin references: record the EdgeZero head SHA
      the rebase targets: **`6904869bd0787ba8e6c36e732e3a447c45e99201`
      (round-188 #1 — live head on 2026-09-20, verified by
      `ls-remote`; `1e7e0562`, `03cdba98`, `351698c9` and `4790a140`
      are superseded in turn. Earlier supersessions, round-172: `fc8d7e9f`, `5eccff8e`,
      `3d87d5bc` and `543b782d` are superseded. Round-163 #1 first named the head
      this
      rebase is written against; `1d47bc0a` is superseded, and its
      `collect()` returning a bare `EdgeError` must not be
      implemented).** ROUND-162 #7 — THE SHA HANDOFF IS A
      PINNED ASSERTION, not a moving-branch note. `1d47bc0a` was
      head at round 160, but a branch-based `cargo update` in
      M3b-11 could resolve a different commit, silently rebasing the
      docs onto one API and the lockfile onto another. RULE: M3b-0
      writes the EXACT SHA it read; M3b-11's **Step 1** (round-168 #2
      — renumbered by the round-167 reorder) asserts, before
      touching the lockfile, that the branch still resolves to THAT
      SHA (`git ls-remote <branch>` compared against the recorded
      value). If it moved, M3b-11 STOPS and M3b-0 re-runs against
      the new head — the two tasks must agree on one commit, or the
      rebase is not a rebase of what actually lands.
  - **Out of scope, explicitly (round-160 #4):** auctioneer's own
    `BidderFanout::send_all` — a domain interface between
    auctioneer-exchange and auctioneer-core. It keeps its name.
  - **Step 1 — the stale-reference gate (write it first, watch it
    fail).** ROUND-162 #4 — TWO PROBLEMS WITH THE ROUND-161 DRAFT,
    both fixed here: it had no `set -eu` and no explicit search
    roots, and (the self-matching trap) the retired names appeared
    IN THE SCRIPT'S OWN DOCUMENTATION inside this plan, so the gate
    would have failed on the very file it was written to clean.
    FIX: the patterns live ONLY in the script — never spelled out
    again in prose — and the script skips its own source plus the
    historical appendices, which must keep their original wording.
    ```sh
    #!/usr/bin/env bash
    # scripts/check-batch-rebase.sh — M3b-0 gate.
    set -euo pipefail

    # ROUND-189 #1 — EXPLICIT TARGET FILES, NOT WHOLE DIRECTORIES.
    # The round-187/188 form scanned every plan and spec, excluding
    # only M3a and the appendix. That flagged
    # `ctx.fanout.send""_all(calls, deadline)` in the M2 plan — a
    # legitimate auctioneer DOMAIN call on a `BidderFanout`, reached
    # through a FIELD rather than a type path, which the
    # type-anchored allowlist could not accept. M3b-0 does not own
    # the M2 plan and must not edit it, so a gate that fails on it
    # could never go green. Name exactly the files M3b-0 rewrites;
    # the `fanout\.send""_all` allowlist arm above covers the
    # field-call form wherever it legitimately appears.
    # ROUND-190 #1 / ROUND-191 #1: all THREE documents M3b-0 owns.
    # `ledger` is named separately because its frozen per-gap region
    # is excluded by line range below.
    plan=docs/superpowers/plans/2026-07-10-m3b-multi-adapter-production-fanout.md
    spec=docs/superpowers/specs/2026-05-21-auctioneer-openrtb-server-design.md
    ledger=docs/superpowers/plans/2026-09-06-edgezero-outbound-spec-feedback.md
    roots=("$plan" "$spec" "$ledger")
    # Patterns are assembled, never written literally, so this file
    # does not match itself and neither does any prose quoting it.
    # ROUND-166 #4 — BOUNDED SO THE VALID NAME SURVIVES. Without a
    # boundary this also matched `send""_all_until`, the CURRENT API,
    # so the gate rejected correct post-rebase text. NOTE the form:
    # rg uses Rust's regex engine, which has NO LOOKAHEAD — a
    # `(?!_until)` pattern makes rg EXIT 2 (and the round-163 #5
    # exit-code handling would surface that as a hard failure, not a
    # false clean). The character-class boundary below is
    # rg-compatible and does the same job: `send""_all` followed by
    # anything that cannot continue the identifier, or end of line.
    # ROUND-169 — THE CLASS INCLUDES DIGITS (`[^_a-zA-Z0-9]`):
    # digits continue a Rust identifier, so the digitless form
    # wrongly rejected a distinct method such as `send""_all2`.
    # ROUND-188 #5 — TWO DEFECTS IN THE ROUND-187 INVERSION, BOTH
    # REPRODUCED AND BOTH FIXED HERE.
    # (a) MASKED SCANNER FAILURE. `rg ... | grep -vE` under pipefail
    #     leaves status 1 when rg EXITS 2, and the case arm read 1 as
    #     "no matches" — so a broken regex or unreadable root
    #     reported CLEAN. Measured: pipeline status was 1 on an rg
    #     failure. Same masked-failure class as round-163 #5's
    #     `|| true`, re-introduced by the pipe. FIX: capture the
    #     scanner's status ALONE, before any filtering.
    # (b) WHOLE-LINE SUPPRESSION. `grep -vE` drops the entire LINE,
    #     so `BidderFanout::send_all and http.send_all(reqs)` was
    #     silenced by its allowed neighbour. Measured: 1 forbidden
    #     occurrence, 0 reported. FIX: match per OCCURRENCE with
    #     enough left-context to classify it, then filter
    #     occurrences — never lines.
    # ROUND-187 #6 — the inversion itself stands: scan the BARE
    # token, then subtract what auctioneer legitimately keeps. The
    # qualified-only pattern missed the normative prose carrying the
    # retired contract ("submission is BATCH ENTRY"); 102 such
    # lines existed and the old gate passed on every one.
    # PORTABILITY (round-188 #5): use `grep -rEon`, NOT `rg`. On this
    # machine `rg` is a shell FUNCTION, not a binary, so a script
    # calling it exits 127 under `env -i`. The gate must run in CI.
    # ROUND-190 #1 — THE IDENTIFIER BOUNDARY IS BACK. The
    # round-188 form `[A-Za-z_]*` after the token matched
    # `send""_all` INSIDE `send""_all2` (digits are not in the
    # class, so the scan stopped early and reported a bare hit —
    # the exact false positive round-169 fixed), and captured
    # `send""_all_until_removed` whole, which the unanchored
    # allowlist arm then ACCEPTED — a false NEGATIVE on a name
    # that is not the current API. Both reproduced. The trailing
    # class below requires a character that cannot continue a
    # Rust identifier, or end of line.
    occurrence="[A-Za-z_:.]*send""_all([^_A-Za-z0-9]|$)"
    # auctioneer's OWN exchange-facing trait method (master spec §5.2),
    # its three impl types, and upstream's CURRENT convenience.
    # Anchored with `$` so `::send_all` does not also allow
    # `::send_all_something_else`.
    allow_own="(BidderFanout|ProductionFanout|DevFanout|NoOutboundCapabilityFanout)::send""_all([^_A-Za-z0-9]|$)|fanout\.send""_all([^_A-Za-z0-9]|$)"
    # ROUND-190 #1 — `send""_all_until` NO LONGER NEEDS AN
    # ALLOWLIST ARM: the bounded `occurrence` never matches it,
    # so it cannot reach the filter. Keeping an unanchored arm
    # was what let `send""_all_until_removed` through.

    # ROUND-189 #1 — THE THREE PATTERNS BELOW WERE DELETED BY THE
    # ROUND-188 REWRITE WHILE THE SCANNER STILL REFERENCED THEM.
    # Under `set -u` that is an UNBOUND VARIABLE and the script
    # exits 1 before scanning anything — reproduced verbatim:
    # "retired_names: unbound variable". A gate that cannot RUN is
    # worse than the false-clean it replaced, because its exit 1
    # reads as a legitimate finding. Restored:
    # ROUND-176 #2 — the retired BudgetSource name, caught BARE.
    retired_names="send-all-""slot-isolation|(BudgetSource::)?\bBatch""Deadline\b"
    # ROUND-163 #1 — the SUPERSEDED collect signature.
    obsolete_collect="Result<Outbound""BatchResults, Edge""Error>"
    # ROUND-165 #1 — the wording this plan actually used.
    slot_invalidating="no per-slot outcome is trust""worthy|driver fail""ure.*invalidat"

    # ROUND-163 #5: `|| true` masked BOTH "no match" (exit 1) and a
    # real rg failure (exit >1) — a broken regex or an unreadable
    # root would have read as CLEAN. Handle the three cases apart.
    # ROUND-188 #5 — THE SCANNER RUNS ALONE AND ITS STATUS IS
    # CHECKED BEFORE ANY FILTERING. Never pipe the scanner into the
    # allowlist: that is exactly what let an exit-2 read as "clean".
    raw=$(mktemp)
    set +e
    # No --exclude needed: `roots` names two files (round-189 #1).
    grep -rEon -e "$occurrence" -e "$retired_names" \
               -e "$obsolete_collect" -e "$slot_invalidating" \
               "${roots[@]}" >"$raw" 2>/dev/null
    scan_status=$?
    set -e
    # 0 = matches, 1 = none, >1 = the scanner itself failed.
    if [ "$scan_status" -gt 1 ]; then
      echo "scanner failed (exit $scan_status)" >&2; exit "$scan_status"
    fi
    # ROUND-191 #1 — THE LEDGER'S PER-GAP ENTRIES ARE FROZEN AND
    # MUST BE EXCLUDED BY LINE RANGE, NOT BY FILE. The ledger is IN
    # SCOPE for its current-head summary (round-165 #1) but its
    # per-gap entries from `### G1 ` onward are declared HISTORICAL
    # by the file's own header, and M3b-0 is forbidden to rewrite
    # them. Scanning the whole file made the gate unsatisfiable:
    # MEASURED — 104 hits, of which 6 were frozen ledger entries no
    # declared edit may touch. A --exclude cannot express this,
    # because the boundary is a SECTION, not a path.
    # ROUND-197 #6 — THE `-z` CHECK WAS DEAD CODE. Under `set -e`
    # a failing pipeline in an ASSIGNMENT aborts the script before
    # the guard runs, so a missing marker exited 1 (which the caller
    # reads as "stale references found") instead of the documented
    # 3. REPRODUCED, then fixed: put the assignment inside the `if`
    # so its failure is TESTED rather than fatal.
    b=""
    if ! b=$(grep -n '^### G1 ' "$ledger" | head -1 | cut -d: -f1) \
       || [ -z "$b" ]; then
      echo "ledger boundary marker '### G1 ' not found in $ledger" >&2
      exit 3
    fi
    kept=$(mktemp)
    awk -F: -v L="$ledger" -v b="$b" '!($1==L && $2>=b)' "$raw" >"$kept"
    # Filter OCCURRENCES, not lines (round-188 #5b).
    hits=$(grep -vE "$allow_own" "$kept" || true)
    if [ -n "$hits" ]; then
      printf 'stale batch-API references:\n%s\n' "$hits" >&2; exit 1
    fi
    ```
    ITS OWN TESTS — **TEN fixtures, (a) through (j), and this
    ordered list is EXHAUSTIVE (round-198 #6: the header said SIX
    while the tally said EIGHT, the cases ran out of order, two more
    were stranded AFTER the tally, and a duplicate "(c)" named the
    obsolete appendix fixture that (i) replaced)**. Every fixture
    whose defect was REPRODUCED says so and gives the measurement,
    in
    `scripts/tests/check-batch-rebase.bats` or an equivalent shell
    test: (a) NEGATIVE — a temporary fixture containing one removed
    API name makes the script exit 1 and print the path; (b)
    POSITIVE — the same fixture with the name removed exits 0;
    **(c) SCANNER-ERROR — point the script at an unreadable root and
    assert it exits with the scanner's status (>1), NOT 0 and NOT 1.
    Measured before the fix: the pipeline returned 1 and the gate
    reported clean. (d) MIXED-LINE — a fixture line carrying BOTH an
    allowed and a forbidden occurrence, e.g.
    `BidderFanout::send""_all and also http.send""_all(reqs)`, must
    exit 1 and name the forbidden occurrence only. Measured before
    the fix: 1 forbidden occurrence present, 0 reported;
    (e) DOMAIN-CALL — a fixture containing
    `ctx.fanout.send""_all(calls, deadline)` must exit 0. This is
    auctioneer's OWN `BidderFanout` reached through a field, and the M2
    plan contains it; a gate that fails on it can never go green.
    (f) UNRELATED-PLAN — the gate's `roots` must name only the
    THREE documents M3b-0 rewrites (round-193 #8: this said "two",
    written before round-190 #1 added the feedback ledger), asserted
    by checking that a `send""_all` planted in the M2 plan does NOT
    fail the gate.
    (g) MISSING-MARKER — round-197 #6: point the gate at a ledger
    with no `### G1 ` heading and assert it exits **3**, not 1 and
    not 0. Measured before the fix: exit 1, because the `-z` guard
    was unreachable under `set -e`.
    (h) FROZEN-SECTION — round-191 #1: plant a removed-API name in
    the ledger BELOW its `### G1 ` boundary and assert the gate stays
    GREEN (that region is frozen history M3b-0 may not rewrite);
    plant one ABOVE the boundary, in the current-head summary, and
    assert it goes RED. This replaces the "appendix-exclusion"
    fixture, which named a file the gate no longer scans and so
    tested nothing.
    (i) RETIRED-NAME — round-176 #2: a fixture containing the
    retired name UNQUALIFIED (`Batch""Deadline` with no
    `BudgetSource::` prefix) exits 1, proving the bare spelling is
    caught; a POSITIVE companion containing only `BatchCutoff`
    exits 0, proving the current name is not rejected.
    (j) CURRENT-API — round-166 #4: a fixture containing
    `HttpClient::send""_all_until(reqs, cutoff)` exits 0, proving the
    gate accepts the VALID current API and rejects only the removed
    one.
    All TEN were RUN before this text was written: (a)-(b), (d)-(f)
    and (h)-(j) pass, (g) exits 3, and (c) exits 2 rather than 0
    or 1.
    **MEASURED on the live tree with the FINAL script (round-193 #8
    re-ran it): 104 hits before the frozen-section exclusion, 98
    after — exactly the 6 frozen ledger entries removed, which
    reconciles.** The earlier text paired 104 with "99", and that 99
    was the error, not the six: it came from an intermediate pattern
    revision. The ledger holds 11 matches in total — 5 above the
    `### G1 ` boundary (IN SCOPE, still flagged, and M3b-0 must fix
    them) and 6 below it (frozen, excluded). Re-run the pair when
    the script changes; the exclusion's correctness is pinned by
    fixture **(h)**, not by these figures (round-199 #4 — it was
    (g) before the round-198 #6 renumber);**
    Expected at Step 1: FAIL on the real tree, listing every site
    the rebase must touch.
  - **Step 2:** apply the rewrites above, recording the asserted
    head SHA in both documents.
  - **Step 3:** the gate passes; additionally
    `rg -n 'BidderFanout::send_all' docs/superpowers/specs
    docs/superpowers/plans` still MATCHES (round-163 #5 — with
    EXPLICIT ROOTS; a rootless `rg` would search the whole tree and
    could pass on an unrelated hit), proving the narrow scope was
    respected (round-160 #4).
  - **Step 4 — acceptance:** (a) the gate script is green and wired
    into CI beside the existing doc checks; (b) §5.2's algorithm
    names only batch-API calls; (c) no non-appendix document
    contains a retired capability or `BudgetSource` name; (d) the
    recorded head SHA matches the revision the rebase was written
    against.
  - **Step 5: commit** `M3b plan/spec: rebase the outbound batch
    contract onto start_batch_until` — the COMPLETION MARKER for
    this task is that commit plus a green gate; M3b-11 and M3b-2b
    may be dispatched only after it.

- [ ] **Task M3b-11 — EdgeZero PR-head pin move (round-154 #5
  created this task; round-157 #5 gives it a body).** ONE atomic
  commit per the pin-move rule: lockfile + every code and doc change
  the move forces, so no intermediate commit fails to build.
  - **⚠ ROUND-184 #1 — THIS TASK ABSORBS M3b-12. THERE IS ONE PIN
    MOVE, NOT TWO, AND ITS TARGET IS `46f85958` (round-195 #7; retargeted through `03cdba98`, `0d84247e`).** Round-183 #6 split
    the migration in two on the premise that the batch API and the
    lifecycle contracts would land at DIFFERENT upstream revisions,
    letting auctioneer take the batch pin early and adopt G18–G22 later.
    That premise is now FALSE: upstream landed the lifecycle work on
    the SAME branch, eight commits past `4790a140`, and head is
    `351698c9`, then `03cdba98`, `1e7e0562` `0d84247e` `46f85958` `325134e4` and now `6904869b`
    (round-189 #6 — this sentence named an exact pin target a second
    time, so one active task carried TWO; the reference-state block
    at the top of this plan is the single source, and it says
    `6904869b`). Pinning `4790a140` today would mean pinning a
    revision that is no longer head purely to preserve a two-step
    sequence whose reason has evaporated — and then breaking the
    build a second time. Worse, the lifecycle change is BREAKING
    (`build_app()` returns `Result`, `AdmissionDecision` gains a
    variant and a field on three others), so the second move could
    never be additive. **The batch migration and the G18–G22
    adoption are therefore ONE atomic commit against `6904869b`,**
    which is also the only sequence that satisfies the pin-move rule
    (no intermediate commit fails to build). The former M3b-12 body
    survives below as this task's ADOPTION half; M3b-12 is RETIRED as
    a separate task and its DAG row is removed.
  - **CONSEQUENCE FOR THE DAG (round-184 #2):** with one pin, the
    ordering inversion disappears. 2a.7b and M3b-1b consume the
    lifecycle APIs and are scheduled AFTER this task, which is the
    only place those APIs enter the tree. A task that needed
    `ResponseEgressCompletion` while scheduled before the pin that
    introduces it could not have compiled.
  - **⚠ BLOCKED on M3b-0 (the documentation rebase).** Head
    hard-removes `OutboundHttpClient::send_all`, which auctioneer
    implements in **exactly three places** (round-161 #2 — the
    "four times" count and the two misidentified files below are
    corrected; see the banner's verified list), so the move breaks
    the build on its first compile. M3b-0 must land first; this task
    then ALSO covers, beyond the SDK/manifest/profile/codec/
    entrypoint work already listed:
    - **THE THREE `OutboundHttpClient` IMPLEMENTATIONS, by file and
      type** — `TestProxyClient`
      (`crates/auctioneer-core/tests/conformance/mod.rs:237`), `NeverResponds`
      (`crates/auctioneer-core/tests/axum_production_strategy.rs:35`)
      and `MockProxyClient`
      (`crates/auctioneer-core/tests/production_fanout.rs:65`) — each
      migrated from `send_all` to `start_batch_until`, RETURNING an
      `OutboundBatch` (round-164 #4: the earlier "+ the
      `OutboundBatchNext` loop" clause is DELETED — it contradicted
      the producer/consumer rule four bullets below and would have
      put 2b's scheduler inside three test doubles). **NOT in scope, and NOT outbound
      clients:** `EdgeZeroProxyAdapter`
      (`host_primitives/edgezero_proxy.rs`, which implements
      `ProxyClientApi` over the single-request `HttpClient::send`)
      and `NoOutboundCapabilityFanout` (`conformance.rs:317`, an
      impl of auctioneer's own `BidderFanout`). Neither implements
      `OutboundHttpClient`, so neither needs a BATCH migration.
      **BUT ROUND-162 #1 — `edgezero_proxy.rs` IS IN THIS TASK FOR A
      DIFFERENT REASON: cache policy.** `EdgeZeroProxyAdapter::send_one`
      builds its `OutboundRequest` with
      `OutboundRequest::new(..).body(..)` plus headers and sets NO
      cache policy (`crates/auctioneer-core/src/host_primitives/
      edgezero_proxy.rs:43-56`), so at head every bidder call it
      sends inherits `OutboundCachePolicy::PlatformDefault` — the
      exact defect round-159 #3 made required-capability work. It
      therefore gains `.cache_policy(OutboundCachePolicy::Bypass)`
      in this task, with
      `proxy_adapter_sets_cache_bypass()` asserting the built
      request carries Bypass. "Excluded from the batch migration" is
      not "excluded from the task".
    - **WHAT THE THREE CLIENT IMPLEMENTATIONS ACTUALLY DO
      (round-162 #6 — they are BATCH PRODUCERS, not batch
      consumers):** each implements the new trait method by
      CONSTRUCTING and returning an `OutboundBatch` over its own
      driver. **The `OutboundBatchNext` loop is NOT theirs** — that
      is `BoundedFetch`'s consumption code and belongs to M3b-2b.
      Mixing the two would duplicate 2b's scheduler inside three
      test doubles.
      **ROUND-163 #4 — THEY ARE NOT INTERCHANGEABLE CANNED-SLOT
      PRODUCERS, and migrating them to a static batch would silently
      weaken every fixture that depends on them.** Each keeps its
      distinguishing behaviour, restated as a batch driver:
      - `TestProxyClient` (`conformance.rs:237`) RECORDS every
        dispatched request (url + body) and models PER-SLOT DELAYS,
        including slots that never complete. Its batch driver must
        yield `Item`s in COMPLETION ORDER (not input order), leave
        never-completing slots unresolved, and emit
        `Finished(Cutoff)` when the cutoff instant is reached with
        slots still pending — precisely what the conformance suite's
        timeout rows exercise. Recording still happens at DISPATCH,
        before any slot resolves.
      - `NeverResponds` (`axum_production_strategy.rs:35`) must stay
        PERMANENTLY PENDING. **ROUND-164 #4 — the round-163 wording
        ("never yields `Finished` unless the caller's own cutoff
        fires") assumed the batch object arms the cutoff itself. It
        does not: the DRIVER emits the cutoff event, so a driver that
        never resolves will never produce `Finished(Cutoff)`
        either.** Pick ONE of these, explicitly, when migrating:
        (a) **PRESERVE THE PENDING DRIVER** — `next()` returns a
            future that never resolves, and the EXISTING test keeps
            driving expiry from its own `FakeClock` timer, exactly as
            it does today. Smaller change, and it keeps the test's
            meaning (the STRATEGY, not the batch, must give up).
            PREFERRED.
        (b) **INJECT CLOCK/TIMER INTO THE DRIVER** — the fixture takes
            the test clock and the driver itself emits
            `Finished(Cutoff)` when the cutoff instant passes. Choose
            this only if a test must exercise the batch's own cutoff
            path; it then belongs beside the conformance fixtures.
        Under either choice it must NOT become a batch that
        immediately reports `Finished(Completed)` with zero items —
        that would make the strategy test pass for the wrong reason.
      - `MockProxyClient` (`production_fanout.rs:65`) RECORDS headers
        and bodies per request and returns canned responses; its
        driver may resolve in input order, but the recording
        assertions must see the same requests, in the same order, as
        before.
      Each migration keeps its crate's existing tests green; where a
      test asserted single-`send` behaviour, the batch form asserts
      the same observable outcome. Per-driver acceptance: completion
      order, cutoff behaviour, cancellation/drop behaviour, delay
      modelling, and the pending case are each stated in the
      migrated doc comment.
    - **NOT `BoundedFetch` / `run_wave` (round-161 #2): those are
      M3b-2b's core deliverables** and were double-assigned by
      round-159 #6. M3b-11 moves the pin and repairs what the pin
      breaks; 2b then BUILDS the scheduler against the batch API,
      which by then is the only API present.
    - `OutboundCachePolicy::Bypass` on every bidder call, with the
      authority-override note;
    - the capability-name migration (`send-all-slot-isolation` →
      the three `outbound-batch-*` rows, plus required
      `outbound-cache-bypass`);
    - the `BudgetSource` migration (the retired name →
      `RequestDeadline` / `BatchCutoff`);
    - conformance-suite and per-adapter batch tests, including a
      cutoff case per target;
    - removal of every remaining normative reference to **EdgeZero's**
      <!-- M3b-0's own file list (batch-rebase-ok) -->
      `OutboundHttpClient::send_all` / `HttpClient::send_all` from
      this plan and the master spec (the pin-scoped notes become
      history). **ROUND-160 #4 — THIS DOES NOT TOUCH
      `BidderFanout::send_all`**, auctioneer's OWN exchange-facing trait
      method (master spec §5.2, `crates/auctioneer-exchange/src/
      fanout.rs`). That is a domain interface between auctioneer-exchange
      and auctioneer-core, it names a batch of BIDDER calls rather than
      an upstream HTTP batch, and it is unaffected by the upstream
      change. An implementer who rewrites it has exceeded this
      task's scope.
  - **Depends on:** nothing upstream — but it RUNS BEFORE the
    decoder measurement (round-158 #1): head promotes the brotli
    crates from dev-dependencies to normal dependencies of
    `edgezero-core`, so the head tuple only enters auctioneer's lockfile
    here, and `brotli_state_bytes` must be measured against THAT
    tuple, not today's. Also depends on M3b-4 having landed the Spin
    wasip2 move — **round-200 #2: it always carries it; the
    conditional is removed** (file list below).
  - **Files (round-162 #6 — the three client files and the proxy
    were named in the scope bullets but missing from this list):**
    `crates/auctioneer-core/tests/conformance/mod.rs` (`TestProxyClient`,
    batch producer), `crates/auctioneer-core/tests/axum_production_
    strategy.rs` (`NeverResponds`), `crates/auctioneer-core/tests/
    production_fanout.rs` (`MockProxyClient`),
    `crates/auctioneer-core/src/host_primitives/edgezero_proxy.rs`
    (cache policy only — round-162 #1), plus
    `Cargo.toml` (the three SDK bumps: `fastly` 0.12 →
    0.13, `worker` 0.8 → 0.8.5, `spin-sdk` ~6.0 → 7.0),
    `Cargo.lock` (the `cargo update -p edgezero-core` pin move —
    which also brings the head CODEC TUPLE, round-158 #1, since head
    promotes the brotli crates to normal dependencies),
    `crates/auctioneer-adapter-fastly/src/main.rs` (the entrypoint
    rewrite: upstream's `run_app` changes from taking a request to a
    no-argument runtime entry), whichever adapter files the SDK
    bumps break, this plan's capability table + the four
    `TargetOutboundProfile` rows, and M3b-3's acceptance wording.
    **PLUS (round-158 #3 — files its own acceptance criteria require
    but the list omitted):**
    - `edgezero.toml` — it carries NO `[capabilities]` section
      today, while this plan specifies a concrete one (see the
      manifest block near the top). M3b-11 WRITES that section:
      `required = ["ingress-admission", "outbound-cache-bypass"]`
      (round-160 #3 — BOTH, matching the manifest block above) plus
      every degraded row as
      optional, generated from the same adapter dump that
      regenerates the table, and any per-target fixture/manifest the
      CLI validates against it.
    - The Spin wasip2 set — **UNCONDITIONAL as of round-200 #2:
      M3b-11 is the sole owner, and the earlier "IF M3b-4 has not
      already landed it" made ownership depend on a task that runs
      AFTER this one:**
      `edgezero.toml` (the `target` key and the build command),
      `crates/auctioneer-adapter-spin/spin.toml`, both
      `.github/workflows/test.yml` matrix rows, the master spec's
      build-matrix command, and the README row. Carrying that move
      means carrying ALL of these files, not the pin alone.
  - **⚠ ROUND-167 #2 — STEP ORDER CORRECTED. The dump CANNOT come
    first.** Step 1 used to say "run each adapter's `capability()` at
    the new pin", but the new pin was only installed in the step
    AFTER it, and a
    later line (correctly) requires the dump to be taken from the
    resolved checkout. As written the task was impossible: it read a
    revision that was not yet present. THE ORDER IS:
    verify head → set full rev → update lockfile → assert lockfile →
    THEN dump and diff capabilities.
  - **Step 1: pin to the exact revision. PIN BY FULL SHA, NEVER BY BRANCH
    (round-165 #3 — the round-162 #7 pre-check closed only half the
    race: asserting the branch head and then running a BRANCH-based
    `cargo update` leaves a window in which the branch moves and the
    lockfile resolves a different revision than the docs were rebased
    onto).** Sequence, mechanically:
    1. `git ls-remote <repo> refs/heads/docs/outbound-http-spec` and
       compare against the SHA M3b-0 recorded. Mismatch ⇒ STOP.
    2. Change `Cargo.toml`'s `edgezero-*` entries from
       `branch = "docs/outbound-http-spec"` to `rev = "<full SHA>"`,
       then `cargo update -p edgezero-core`. The revision is then
       fixed by the manifest, not by whatever the branch points at
       when Cargo runs.
    3. ASSERT before building — **STRUCTURALLY, NOT BY BARE grep
       (round-166 #3: the earlier
       `grep "edgezero" Cargo.lock | grep -v "#<sha>"` ALWAYS FAILS.
       Verified against the current lockfile: it emits 44 lines,
       because `name = "edgezero-core"` and dependency-list entries
       like ` "edgezero-core",` contain no SHA and so survive the
       inverse filter. The check must look at `source =` lines
       only).** Restrict to EdgeZero source lines, then verify BOTH
       their count and their revision:
       ```sh
       want="<full SHA>"
       # every edgezero source line, and how many carry the wanted rev
       total=$(grep -cE '^source = "git\+.*edgezero' Cargo.lock)
       pinned=$(grep -cE "^source = \"git\+.*edgezero.*#${want}\"" Cargo.lock)
       [ "$total" -gt 0 ] && [ "$total" -eq "$pinned" ] \
         || { echo "edgezero rev mismatch: $pinned/$total at $want" >&2; exit 1; }
       ```
       A zero `total` also fails, so a renamed dependency cannot make
       the assertion vacuously true.
    4. The capability dump (now **Step 2**, round-168 #2) is taken
       FROM THAT SAME resolved
       checkout, not a fresh branch clone, or the regenerated table
       can describe a revision the lockfile does not carry.
    Only now is the target revision actually present in the tree.
  - **Step 2: capability dump + diff — FROM THE CHECKOUT STEP 1
    RESOLVED** (round-167 #2; this is the step the "must come from
    that same resolved checkout" rule was always describing). Run
    each adapter's `capability()`, regenerate the expected-support
    table and the profile rows from that dump, and diff against the
    committed ones. **ACCEPTANCE IS THE DIFF ITSELF (round-170 #2):
    the regenerated table must equal the dump for EVERY variant of
    upstream's `Capability` enum — **23 at head `6904869b`**
    (round-195 #7: this said 22, written before
    `ingress-admission-abort` was counted in round-190 #5) — not
    merely for the
    rows this plan happens to name. A capability present in the dump
    and absent from the table fails this step.** Review EVERY changed
    cell for its consumer
    (round-156 #5): Cloudflare's `outbound-deadlines` and
    `streamed-upload-deadlines` demote to `BestEffort`, and several
    `response-egress-*` rows move from `Unsupported` to `BestEffort`,
    which may move them between the declared and not-declared lists.
  - **Step 2b: failing state.** `cargo build --workspace
    --all-targets` — expect the Fastly entrypoint and SDK breaks;
    record them.
  - **Step 3: migrate.** Rewrite the Fastly entrypoint, apply the SDK
    bumps, write the regenerated capability table and profile rows,
    reword M3b-3's Cloudflare acceptance to the BestEffort form
    M3b-4/M3b-5 already use — **BOTH the deadline AND the
    CANCELLATION wording (round-174 #1: this step named only the
    deadline demotion, but `outbound-batch-cancellation` is equally
    BestEffort on Cloudflare; migrating one and not the other would
    leave the plan asserting a Native cancellation guarantee that no
    adapter but Axum makes).** The renamed
    `cf_besteffort_cancellation_…` test travels with it.
  - **Step 4: PASS.** The full gate chain green: `cargo fmt --check`,
    `cargo clippy --workspace --all-targets -- -D warnings`,
    `cargo test --workspace`, plus
    `cargo check -p auctioneer-adapter-fastly --target wasm32-wasip1`,
    `cargo check -p auctioneer-adapter-spin --target wasm32-wasip2`,
    `cargo check -p auctioneer-adapter-cloudflare --target
    wasm32-unknown-unknown`, and `codec_pins_match_lockfile()` —
    which THIS task creates (round-162 #2) — against the NEW tuple.
  - **Step 5: commit** `auctioneer: move the EdgeZero pin to <sha> —
    SDK bumps, Fastly entrypoint, regenerated capability matrix`
    (ONE commit; the atomic pin-move rule forbids splitting).
  - **Acceptance:** every capability row matches a fresh adapter
    dump; no task's acceptance still claims a guarantee the new pin
    demotes; `codec_pins_match_lockfile()` is green against the head
    tuple and upstream's published decoder charge is recorded beside
    it. **NOT auctioneer's measured audit** (round-162 #2): that needs
    the allocator harness M3b-2b builds, and demanding it here was
    the circular requirement.

- [ ] **M3b-11, ADOPTION HALF — the G18–G25 lifecycle migration
  (round-183 #6 created this as Task M3b-12; round-184 #1 MERGES it
  into M3b-11, because upstream landed the batch API and the
  lifecycle contracts on the same branch — `351698c9`, extended at `03cdba98`).**
  - **WHY THIS IS ONE COMMIT WITH THE BATCH MIGRATION.** Every
    deferral in this plan that reads "when that lands" — 2a.7a's
    static-egress reset, 2a.7b's response-terminal egress binding,
    M3b-1b's terminal capacity recovery and its Fastly never-ready
    gap, the fallible-`configure` question — resolves at the SAME
    SHA as the batch rebase. Adopting them separately would require
    pinning a stale revision first and breaking the build twice.
  - **VERIFIED AT HEAD (round-184 #10 — read from the upstream tree,
    not inferred from the feedback ledger; a landed feedback item is
    not evidence, since G17 was closed by an API differing from the
    one requested):** `ResponseEgressCompletion::new(FnOnce(&Response
    EgressReport))` in `response_egress.rs`; `AdmissionDecision::Abort`
    plus a `completion` field on `Admit`,
    `ReadBodyBeforeFallback` and `Refuse` in `ingress.rs`;
    `configure` and `build_app()` returning `Result<_, EdgeError>`
    in `edgezero-macros/src/app.rs`.
  - **⚠ ROUND-187 #9 — WHICH OF THESE VALUES ARE OPERATOR-TUNABLE,
    AND WHICH ARE NOT.** Asked directly whether these belong in
    `auctioneer.toml`. The rule this plan already follows, stated once so
    later rounds stop re-deciding it per value:
    **⚠ ROUND-188 #3 — THE ROUND-187 ANSWER WAS WRONG FOR THE
    ADMISSION-PATH VALUES, AND IT CONTRADICTED THE BOOT ENVELOPE.**
    Round-187 said `static_egress_permits` and
    `refusal_write_deadline_ms` are "push-tunable through
    `auctioneer.toml`". They cannot be. The boot envelope is built from
    REQUEST-INDEPENDENT inputs only, BEFORE any store access, because
    admission must precede the config read (round-98 H6: the
    "budget from resolved host config" model was circular on wasm and
    unimplementable on Axum process boot). And the pre-admission
    detached factory is SYNCHRONOUS — it cannot await a store fetch
    to learn a request overlay. A value consumed at or before
    admission therefore CANNOT come from the runtime overlay, however
    desirable that would be.
    **CORRECTED SPLIT, by WHEN the value is read — not by whether it
    looks deployment-shaped:**
    - **BAKED BOOT-ENVELOPE values** (build-time TOML, changed by
      rebuild+redeploy, NOT by `auctioneer-cli config push`):
      `terminal_response_permits`, `refusal_write_deadline_ms`,
      `max_concurrent_auctions`, `aux_route_permits` — every value
      the admission path or the detached factory reads. The master
      spec already makes response deadlines host-baked; this aligns
      the plan to it rather than the reverse.
    - **RUNTIME-OVERLAY values** (`auctioneer.toml`, push-tunable):
      values read INSIDE a handler, after the config resolve —
      `final_response_bytes` and the `[auction]` knobs that already
      work this way.
      **⚠ ROUND-191 #2 — `max_config_fetch_ms` IS NOT IN THIS LIST.
      IT IS BAKED, AND CLASSIFYING IT AS OVERLAY WAS CIRCULAR.** It
      bounds THE OPERATION THAT LOADS THE OVERLAY: it is installed
      into `ConfigExtractionLimits` in `configure`, before any store
      access, so reading it from the store would require the fetch it
      is meant to bound. Elsewhere this plan already declares it a
      boot-envelope constant; this list was the outlier.
      **CONFIG OWNERSHIP, CORRECTED (round-191 #2). THREE RULES THE
      USER SET, AND M3b OBEYS ALL THREE:**
      1. **One operator config: `auctioneer.toml`.** Every M3b knob is
         declared there with the `[auction]` knobs. M3b adds no
         second operator-facing file.
      2. **`auctioneer.toml` is NEVER read at runtime.** It is the
         SOURCE artifact. auctioneer projects `[runtime]` from it and
         delegates to EdgeZero's config-push pipeline, which
         **hashes, envelopes and writes** it to `[stores.config]`
         (round-200 #1 — NOT "signs": see below); the runtime reads
         the STORE BLOB. Nothing in M3b may read the file at request time.
      3. **`auctioneer.toml` is NEVER baked into the binary.** An
         earlier revision of this block proposed having `build.rs`
         read it so one file could serve boot-critical keys. **That
         is WITHDRAWN.** Baking it would put a second, invisible
         copy of operator config inside the binary, able to disagree
         with the pushed blob — exactly what the push flow exists to
         prevent.
      **WHAT IS COMPILED IN IS A DEFAULT, NOT CONFIG.** The pushed
      blob overrides the baked defaults per request. **ROUND-192 —
      AND THOSE DEFAULTS LIVE IN `auctioneer.toml` TOO: M3b-1d FOLDS
      `host-defaults.toml` AND `account-defaults.toml` INTO IT.**
      The split was pure duplication — five keys appeared in both
      files with identical values, and the `[gdpr]` / `[ccpa]` /
      `[lmt]` sections existed ONLY in the baked file, so
      `auctioneer.toml` did not show an operator the settings that most
      affect privacy behaviour. After M3b-1d there is ONE file that
      declares every knob AND its default; `build.rs` bakes the
      defaults from it (the file is still never read at runtime and
      still never baked wholesale into the binary — `build.rs`
      extracts DEFAULT VALUES, exactly as it does today from
      `host-defaults.toml`). M3b's new knobs are declared there like
      any other.
      **THE TIMING CONSTRAINT, AND WHAT IT ACTUALLY COSTS.** Three
      values are consumed before any store read can happen —
      `max_config_fetch_ms` (it BOUNDS the fetch, so reading it from
      the store is circular) and the two process-globals installed
      at boot, `terminal_response_permits` and
      `refusal_write_deadline_ms`. The signatures make this
      unavoidable: `configure(&mut App) -> Result<(), EdgeError>`
      and `build_app()` are SYNCHRONOUS and receive no store handle
      (verified in `edgezero-macros/src/app.rs`), and the admission
      policy is `Arc<dyn Fn(&IngressHead) -> AdmissionDecision + Send
      + Sync>`. EdgeZero's boot config supplies SETTERS, not VALUES.
      **⚠ ROUND-193 #2 — CONSEQUENCE, STATED CORRECTLY. THE
      EARLIER WORDING WAS FALSE.** It said a pushed change to these
      three "takes effect on the next PROCESS START". That is
      impossible: restarting the SAME binary re-reads the SAME
      compiled-in constant. A `config push` can never change them.
      **What is actually true:**
      - `max_config_fetch_ms`, `terminal_response_permits` and
        `refusal_write_deadline_ms` are consumed before any store
        read exists, so their effective value is the COMPILED-IN one.
      - Changing any of them requires **rebuild + redeploy**. There
        is no push path, and no restart shortcut.
      - They are still DECLARED in `auctioneer.toml` with the other host
        settings (round-193 #1), so an operator reads every
        knob in one file — but that table is baked by `build.rs`, not
        pushed, and its doc comment says REBUILD REQUIRED.
      - The only alternative would be a real synchronous bootstrap
        source readable before `build_app()` — EdgeZero exposes none
        (`configure` and `build_app` are sync and take no store
        handle), so M3b does not pretend otherwise.
      **Do NOT present these three as push-tunable.** A knob that
      `config push` appears to accept but cannot actually change is
      worse than one honestly marked build-time.
      **`runtime-bootstrap.toml` stays exactly as it is** — shipped
      M3a code, one key, no M3b change. M3b adds no keys to it.
      `edgezero.toml` is untouched: it is EdgeZero's manifest
      (routes, store bindings, adapter wiring), not auctioneer policy.
      **`response_write_deadline_ms` is NOT in this list
      (round-189 #8): it is BAKED.** An earlier revision put it here
      "as it applies to auction egress", contradicting both this
      plan's own dataflow and the master spec, which makes response
      deadlines host-baked and rejects a runtime overlay for them.
    **ROUND-187 #3's interdependence still holds and now cuts the
    other way:** pool drainage is `terminal_response_permits ×
    refusal_write_deadline_ms`, and since BOTH are baked, the
    relationship is checked by a BOOT ASSERTION rather than left to
    an operator to maintain across two pushes. **Making either one
    runtime-tunable would require an explicit atomic pool-resizing
    mechanism — resizing a live semaphore while permits are held —
    which M3b does NOT specify and MUST NOT imply.** If a deployment
    genuinely needs it, that is a new unit with its own proof, not a
    config key.
    **NOT CONFIG (a compile-time constant) when the value is fixed by
    a CONTRACT rather than by a deployment**: `ConfigStatusCode`'s
    wire strings and `STATUS_DIAGNOSTIC_LINE_BYTES` (64) are
    determined by a closed enum this build defines — an operator who
    changed them would break the contract, not tune it. Likewise
    `refusal_body_bytes` (256) and `CONFIG_PARSE_FACTOR` (6): both
    are MEASURED properties of this build's code, not preferences,
    and both are guarded by boot assertions that would reject an
    operator's value anyway.
    **THE TEST FOR NEW VALUES:** if two sensible deployments would
    want different numbers, it is envelope-owned; if a different
    number would make the build WRONG, it is a constant. No value
    added by M3b may be left hardcoded without answering this.
  - **⚠ ROUND-187 #4 — SCOPE: THIS TASK OWNS THE PIN, NOT THE
    BEHAVIOUR THE PIN ENABLES.** The bullets below name auctioneer
    behaviour tests (`egress_permit_releases_on_terminal_report_not_on_eof()`, `static_pool_exhaustion_aborts_without_a_response()`, and the rest). Those belong to the units that own
    the surfaces — chiefly **2a.7b**, which the DAG places AFTER
    this task. As written, M3b-11's acceptance could not be met
    without implementing 2a.7b inside it, which is both a scope
    inversion and a second implementer in the same files.
    **THE SPLIT, BINDING:**
    - **M3b-11 owns:** the exact pin SHA, the lockfile, the
      compilation migration (every call site the breaking changes
      force — `build_app()` returning `Result`, the
      `AdmissionDecision` variant and fields, `get_bounded` becoming
      required), the three adapter SDK bumps, capability-table
      regeneration, and SMOKE TESTS that the upstream primitives
      exist and are callable. Its acceptance is: **it compiles, the
      capability table regenerates exactly, and nothing references a
      removed API.**
    - **The consuming units own:** every assertion about auctioneer
      BEHAVIOUR through those primitives. Each bullet below therefore
      names its OWNING UNIT, and the test lands there, after the pin.
    A test named below is a REQUIREMENT ON THE OWNER, not a
    deliverable of this task.
  - **Adopt, one bullet per gap — each names its owner:**
    - **G18/G21 (response-scoped completion) — LANDED.** 2a.7b binds
      the egress permit release to the terminal `ResponseEgressReport`
      for the response that owns it, via
      `ResponseEgressCompletion::late_bound`, replacing (NOT
      supplementing) the unsound handler-future release that
      round-184 #4 removed. **OWNER: 2a.7b.** Test:
      `egress_permit_releases_on_terminal_report_not_on_eof()`.
    - **G21 (detached/refusal completion) — LANDED, WITH ONE GAP.**
      `AdmissionDecision::Refuse` carries a `completion`, which is
      STRONGER than the filed ask, so M3b-1b's capacity recovery is
      re-scoped from the handler-future boundary to terminal
      transmission and the static-class test
      `egress_gate_full_returns_503_through_static_class()` gains its
      completion signal. **GAP (round-184 #5): pre-admission
      normalized-head failures are NOT covered** — see the
      static-egress weakening bullet in 2a.7a and feedback G23.
    - **G22 (abort/reset disposition) — LANDED.**
      `AdmissionDecision::Abort` closes or resets the native request
      with NO response, so the static-egress pool's
      RESET-ON-EXHAUSTION becomes expressible and 2a.7a's deferred
      half lands. **OWNER: 2a.7a.** Test:
      `static_pool_exhaustion_aborts_without_a_response()`.
    - **G19 (fallible `configure`) — LANDED, AND BREAKING.**
      `configure` returns `Result<(), EdgeError>` and `build_app()`
      returns `Result<App, EdgeError>`, so EVERY adapter entry point
      must handle the error rather than unwrapping an `App`. auctioneer's
      boot-time validation failures (a rejected envelope, an
      out-of-range limit) propagate as a refused build instead of a
      panic or a silently skipped install. Test:
      `invalid_boot_envelope_refuses_app_build()`. **This is the
      single largest mechanical surface of the migration — four
      adapter entry points plus the `app!` invocation.**
    - **G20 (bounded provider read) — LANDED AT `03cdba98`
      (round-186 #7; it was NOT landed at `351698c9`, which is why
      round-184 kept the outer race).** `ConfigStore::get_bounded`
      is now REQUIRED with no compatibility default, on upstream's
      stated ground that "awaiting an unbounded provider call can
      hide a never-ready future". auctioneer calls it, maps expiry onto
      `ConfigStoreUnavailable`, and **DELETES its outer race on every
      target** — keeping it would duplicate a bound upstream now
      mandates. Every adapter's `config-read-deadlines` row reads
      BestEffort, per upstream's own capability text. Test:
      `config_read_uses_upstream_bound_not_a_local_race()`.
    - **G29 (bounded framework error bodies) — LANDED AT
      `325134e4`, ONE ROUND AFTER AUCTIONEER FILED IT.**
      `App::set_error_response_renderer(Fn(EdgeError) -> Response)`
      gives the application the whole error body. **auctioneer installs
      a renderer in `configure` that maps Appendix F's PINNED
      variants to their exact wire bodies and falls back to
      `error.kind()` for the rest — NOTHING caller-supplied, no
      path, no allowed-set (round-207 #2: a blanket `kind()`
      renderer would have broken the 414 and 431 contracts).** Measured: 9 bytes, against ~840 for the default
      renderer on a 768-byte path and 256 charged per permit, so
      the per-permit byte charge becomes a true ceiling for every
      terminal response and the round-205 exemption is withdrawn.
      **OWNER: 2a.7b, NOT 2a.7a (round-207 #1).** 2a.7a creates
      `configure`, but it is the **API-INDEPENDENT** half at DAG row
      4b — it runs BEFORE M3b-11, and
      `set_error_response_renderer` does not exist at auctioneer's
      `120dd2aa` pin (verified: zero occurrences in `app.rs` at that
      revision). Assigning the installation there could not compile.
      2a.7a keeps gate construction and the empty `configure`;
      **2a.7b installs the renderer and owns its tests**, at row 8b
      after the pin lands — the same placement as every other
      post-pin lifecycle setter. Tests:
      `framework_404_body_is_bounded_by_the_renderer()` and
      `error_renderer_never_echoes_caller_input()`.
    - **G23 (detached pre-admission completion) — LANDED AT
      `03cdba98`, ONE ROUND AFTER AUCTIONEER FILED IT.**
      `App::set_detached_response_egress_decision_factory` lets
      auctioneer return `Abort` OR `Send { completion, deadline }` for
      pre-admission 400/414/431 responses (round-188 #7 — updated
      from the superseded completion-factory spelling). **The static-egress exemption round-184 #5 declared
      is REMOVED** — those responses now join the pool, and the test
      that asserted the exemption
      (`pre_admission_rejections_are_outside_the_static_pool()`) is
      REPLACED by `pre_admission_rejections_hold_a_static_permit()`.
      This is exactly the "if upstream later attaches a completion,
      the test fails and forces this text to be revisited" case
      round-184 wrote the exemption test for.
  - **Acceptance (round-200 #3 — RESTRICTED TO WHAT THIS TASK CAN
    ACTUALLY DO).** The earlier wording required every deferral to be
    "adopted with a green test", which this task cannot satisfy: its
    own scope bullet assigns those behaviour tests to downstream
    owners (2a.7a, 2a.7b, M3b-1b), and several of them run only
    AFTER M3b-11. An acceptance that depends on later tasks can
    never be met at the time it is checked. **M3b-11 is accepted
    on, and only on:**
    1. **It compiles** — `cargo check` across the workspace and all
       four target triples, after the breaking migration
       (`build_app()` returning `Result`, the `AdmissionDecision`
       variant and fields, `get_bounded` becoming required, the
       Spin wasip2 move).
    2. **Primitive smoke tests** — each adopted upstream API exists
       and is callable: `late_bound`, `join`,
       `ResponseEgressDeadline::after`, `AdmissionDecision::Abort`,
       `DetachedResponseEgressDecision`. Existence and
       callability, NOT auctioneer behaviour through them.
    3. **Exact capability regeneration** — the regenerated table
       equals a fresh adapter dump for EVERY variant (23 at head).
    4. **Lockfile + codec pins** — the structural `source =` count
       assertion at the recorded SHA, and
       `codec_pins_match_lockfile()`.
    5. **No reference to a removed API remains** — the M3b-0 gate
       is green.
    **Every behaviour test named in the adoption bullets belongs
    EXCLUSIVELY to its named owner and is that owner's acceptance,
    not this task's.** A deferral that names no owning task is still
    a defect — but the fix is to name the owner, not to test it
    here. **M3b-8's gate runs AFTER this task and after those
    owners**, so `BehaviorComplete` certifies behaviour the pin can
    actually express.

- [ ] **Task M3b-2a.0 — ROSTER RECONCILIATION. The prerequisite
  every roster-owning unit waits on (round-200 #1).**
  - **WHY IT IS A TASK, NOT A NOTE.** §9 declares 175 tests; 80 are
    named with assertions; ~95 exist only as GROUP COUNTS in §9's
    head ("7 weighted-gate", "3 elapsed-timing", "3 time-domain",
    the `<target>` templates counted once per target, and the rest).
    The plan has recorded that gap for thirty-odd rounds and blocked
    2a.1–2a.7b, 2b.0–2b.2 and M3b-10 on it — but a blocker with no
    OWNER, no steps and no acceptance is a standing excuse, not a
    plan. **This task owns it.** It is the last thing between the
    foundation lane and the central fanout wave.
  - **Blocked by:** nothing. It reads and edits ONE section of this
    plan. No code, no clock, no batch API, no pin. **Dispatchable in
    parallel with M3b-0, M3b-1c and M3b-1d.**
  - **Files:** Modify `docs/superpowers/plans/2026-07-10-m3b-multi-
    adapter-production-fanout.md` — §9 (the roster) and the
    dispatch/expansion tables that cite its counts. **No source file
    changes**; this task produces the enumeration the implementation
    units consume.
  - **METHOD, and it is RECOVERY not invention (round-171 #3's rule
    stands): every group label cites the round that introduced it,**
    so each member is a LOOKUP in this plan's history, not a guess.
    For each of the ~95, in §9 order:
    1. Follow the group label to its originating round.
    2. If the member is recoverable, write its **exact test name and
       its assertion** into §9 inline, and assign it to **exactly
       one** owning unit.
    3. If it is NOT recoverable, **STRIKE it and record why** in one
       line. The count is not sacred — round-168 #1 already
       established that the correspondence between count and
       enumeration is what matters. A struck member lowers the
       total.
    4. A `<target>` template row expands to FOUR concrete tests
       (axum / cloudflare / spin / fastly) and counts four times
       (round-146 #3).
  - **Interfaces (Produces):** a §9 in which **every member is named
    with an assertion and an owning unit**, and a total that equals
    the enumeration. Each roster-owning unit can then answer the
    gating question — *can I list this unit's tests by name and
    assertion?* — with yes.
  - Step 1: produce the reconciliation table — one row per member:
    group label → originating round → recovered name + assertion →
    owning unit, or STRUCK + reason. Step 2: fold the recovered
    names into §9 inline and strike the rest. Step 3: correct every
    count that cites the roster (§9's head, the dispatch table, the
    expansion table, the "80 named" line). Step 4: run the
    arithmetic check below. Step 5: commit `M3b plan: reconcile the
    §9 test roster`.
  - **ACCEPTANCE — A SCRIPT, NOT MANUAL COUNTING (round-200 #4).**
    Hand-counting is what produced the 104/99-vs-six arithmetic
    error in round-193 and the 71-vs-80 drift before it. This task
    ships `scripts/check-roster.sh`, run in CI beside the M3b-0
    gate, which:
    1. extracts every enumerated member from §9 (a backticked
       `name()` in a roster bullet), expanding a `<target>` template
       to its four concrete targets;
    2. extracts the declared TOTAL from §9's head;
    3. **exits 1 unless `named == total`**, printing both numbers
       and the first ten missing names;
    4. **exits 1 if any surviving group label** ("weighted-gate",
       "elapsed-timing", "time-domain", …) still appears as a
       COUNT rather than prose;
    5. **exits 1 if any member is claimed by two owning units, or
       by none**;
    6. exits >1 on its own failure, never 0 or 1 — the
       masked-failure lesson from round-188 #5.
    Its own fixtures, both directions: a roster with a deliberate
    off-by-one fails; the reconciled roster passes.
    - `named == total` in §9, enforced by that script rather than
      asserted in prose, and the total restated in EVERY place
      that cites it.
    - **Zero members carry a group label with no name.** A grep for
      the surviving label strings ("weighted-gate", "elapsed-timing",
      "time-domain") returns only PROSE, never a count standing in
      for members.
    - Every named member has exactly ONE owning unit; a member owned
      by two units, or by none, fails.
    - Struck members each carry a one-line reason.
  - **NOT IN SCOPE:** writing the tests. This task produces NAMES
    and ASSERTIONS; the owning units implement them. It also must
    not invent a member to make arithmetic reconcile — round-171 #3
    rejected exactly that, and striking is always available.

- [ ] **Task M3b-1d — `host-defaults.toml` and
  `account-defaults.toml` FOLD INTO `auctioneer.toml` (round-192).**
  - **WHY.** Today the same five keys exist TWICE with identical
    values: commented-out in `auctioneer.toml` under `[auction]`
    (`default_tmax_ms = 1000`, `max_tmax_ms = 2000`,
    `max_bidders_per_auction = 50`, `max_outbound_requests = 100`,
    `max_bidder_response_bytes = 262144`) and live in
    `host-defaults.toml` under `[resource_limits]`. An operator
    uncommenting a line has to know the real default lives in
    another file under a different section name, and a reviewer
    checking "what is the default tmax" has two places to look that
    can silently disagree. The rest of the split is worse, not
    better: `[gdpr]`, `[ccpa]`, `[lmt]`, `[host_cookie]` exist ONLY
    in the baked file, so `auctioneer.toml` does not show an operator
    the settings that most affect privacy behaviour.
    **ONE FILE SHOWS EVERY KNOB AND ITS DEFAULT.**
  - **WHAT THIS IS NOT.** It does NOT bake `auctioneer.toml` into the
    binary (round-191 #2 — that remains prohibited) and it does NOT
    make the runtime read the file. `auctioneer.toml` stays the SOURCE
    artifact: **auctioneer PROJECTS `[runtime]` from `auctioneer.toml` and
    delegates it to EdgeZero's existing config-push pipeline, which
    HASHES, ENVELOPES and WRITES it to the store** — auctioneer
    implements no hashing, envelope or store write of its own;
    `run_config_push_typed` owns all of it, and auctioneer's only
    contribution is the `Serialize` impl deciding WHICH subtree it
    receives.
    **⚠ ROUND-200 #1 — IT IS NOT "SIGNING", AND CALLING IT THAT
    OVERSTATED A SECURITY PROPERTY AUCTIONEER DOES NOT HAVE.**
    Round-199 #2 wrote that the pipeline "performs the standard
    signing/envelope" and that auctioneer implements no "signing,
    hashing, key management". EdgeZero produces a **version-1
    envelope carrying a canonical SHA-256**
    (`edgezero-core/src/blob_envelope.rs`: `BlobEnvelope { data,
    generated_at, sha256, version }`), and upstream's own design
    says what that is NOT: *"The sha is a drift / integrity signal,
    not a signature. It proves 'this `data` matches that `sha256`'
    — nothing about WHO wrote it. An attacker with write access to
    the store can compute a new sha for arbitrary content and the
    runtime accepts it."*
    **CONSEQUENCES, stated so no later round re-reads this as
    authentication:**
    - The envelope detects **drift and corruption**, not tampering
      by anyone holding store-write access. Store-write access IS
      the trust boundary.
    - **auctioneer must NOT add a second signing layer.** A auctioneer-side
      signature would duplicate ownership and create two
      incompatible trust semantics over one blob.
    - If authenticated authorship is ever required, it is an
      **EdgeZero v2 signed envelope plus runtime verification**, not
      a auctioneer feature. Not filed as a gap: M3b does not need it,
      and the current model is documented rather than assumed.
    The runtime reads the STORE BLOB. What changes is where
    `build.rs` reads the DEFAULTS from — one file instead of three.
  - **⚠ ROUND-193 #1 — THE FIRST SHAPE WAS UNIMPLEMENTABLE. THIS
    IS THE CORRECTED ONE.** The round-192 text said "point `build.rs`
    at `auctioneer.toml` and merge the section names". That cannot work,
    for four verified reasons:
    - `HostDefaultRaw`, `AccountRaw` and `AuctioneerAppConfig` each carry
      `#[serde(deny_unknown_fields)]` with REQUIRED fields, so no
      flat document satisfies all three — each would reject the
      others' sections.
    - **A real type collision:** host `GdprY.enabled` is `bool`
      (`build_codegen.rs:334`); account `AccountGdprConfigRaw.enabled`
      is a tri-state `String` (`:488`). One TOML path, two types.
    - `[ccpa]`, `[lmt]` and `[host_cookie]` do not exist in
      `AuctioneerAppConfig`, so `config push` would REJECT a file
      carrying them.
    - `resolve_defaults_path` anchors to `CARGO_MANIFEST_DIR` +
      `data/`, so changing the filename does not reach repo-root
      `auctioneer.toml`.
  - **⚠ ROUND-194 #1 — THE ROUND-193 LAYOUT IS WITHDRAWN. IT
    COLLIDED WITH THE RUNTIME AT FIVE POINTS.** Putting baked
    defaults at `auctioneer.toml`'s top level and account defaults under
    `[accounts.default]`, while leaving `auctioneer-cli` pushing the
    whole document, breaks in ways verified in the tree:
    - **The pushed blob becomes TOTAL.** Every live baked default
      would push as `Some(...)`, so the blob stops inheriting future
      compiled defaults — the opposite of the partial-overlay
      behaviour the same task promises.
    - **`[accounts.default]` is a REAL ACCOUNT at runtime.**
      `resolve_account` does `app.accounts.get(&id)`
      (`config/resolve.rs:213`), so a request whose publisher id is
      literally `"default"` selects the defaults entry and reports
      `AccountSource::Configured`. **A build-time assertion cannot
      constrain a runtime request id.**
    - **`resolve.rs` would have to change.** It DELIBERATELY passes
      `ccpa`, `lmt`, `host_cookie` and the status body through
      untouched (`:76-84`); adding them as overlay fields is a merge
      change, and round-193 both omitted `resolve.rs` from the file
      list and claimed the merge was unchanged.
    - **`HostDefaultRaw` cannot deserialize the whole document**
      under `deny_unknown_fields` — it would need a new combined
      root parser or an explicit projection step, neither specified.
    - **It IS a blob schema change**, additive but real, so the "no
      blob schema change" claim was false.
  - **THE SHAPE, TAKE THREE — ONE SOURCE DOCUMENT, TWO EXPLICIT
    NAMESPACES, CLI PUSHES THE RUNTIME PROJECTION.** This is the
    reviewer's second option, and it is the only one that satisfies
    "one config file" without fighting the runtime:
    ```toml
    # auctioneer.toml — the single source an operator edits.
    [baked]              # compiled in by build.rs; REBUILD REQUIRED
    [baked.host]         # was data/host-defaults.toml
    [baked.account]      # was data/account-defaults.toml
    [runtime]            # pushed by auctioneer-cli; effective next request
    ```
    - **`build.rs` reads `[baked.host]` and `[baked.account]`**,
      each into its EXISTING `HostDefaultRaw` / `AccountRaw` struct
      with `deny_unknown_fields` intact. No type collision — host
      `gdpr.enabled` stays `bool` under `[baked.host]`, account
      `gdpr.enabled` stays a tri-state `String` under
      `[baked.account]`.
      **⚠ ROUND-195 #4 — THERE IS A STRICT COMBINED ROOT, AND AN
      EARLIER REVISION WRONGLY SAID THERE WAS NOT.** Strict SUBTREE
      parsing cannot reject an extra ROOT namespace: `[runtim]`
      (typo) or `[baked.other]` would simply never be read, so the
      claim that "every unknown key fails the build" was false.
      `build.rs` therefore parses the document into
      `AuctioneerConfigSource` (the same strict root type
      `auctioneer-cli` uses, `deny_unknown_fields`), then hands each
      `[baked.*]` subtree to its existing Raw struct. One root type,
      two consumers, no drift. Tests:
      `build_rejects_unknown_root_key()` (`[runtim]` fails),
      `build_rejects_unknown_baked_namespace()` (`[baked.other]`
      fails), `build_rejects_missing_runtime()`, and
      `build_rejects_missing_baked()`.
    - **`auctioneer-cli config push` pushes `[runtime]` ONLY — THROUGH
      A AUCTIONEER WRAPPER TYPE, NOT A PROJECTION ARGUMENT
      (round-195 #1).** `run_config_push_typed::<C>(args)` takes
      ONLY command args and deserializes the file root itself
      (`edgezero-cli/src/config.rs`), so there is no projection
      parameter to pass. But `C` is a TYPE parameter and the pushed
      payload is `build_config_envelope::<C>(&typed)`, i.e.
      `serde_json::to_value(typed)` — **whatever `Serialize`
      emits**. So auctioneer supplies a wrapper with ASYMMETRIC derives:
      ```rust
      // crates/auctioneer-config-schema/src/lib.rs — a NEW
      // dependency-neutral crate (round-196 #2). It depends on
      // serde + validator ONLY, never on auctioneer-core or
      // auctioneer-config, so auctioneer-config's BUILD SCRIPT can use it
      // with no cycle. Exact precedent: `auctioneer-gvl-schema`, whose
      // own header explains the pattern ("a build-dependency on a
      // sibling path crate is legal and has no cycle: the sibling
      // has no build.rs of its own").
      #[derive(Deserialize)]
      #[serde(deny_unknown_fields)]   // strict combined ROOT
      pub struct AuctioneerConfigSource<Host, Account, Runtime> {
          pub baked: BakedSection<Host, Account>,  // REQUIRED
          pub runtime: Runtime,                    // REQUIRED
      }

      #[derive(Deserialize)]
      #[serde(deny_unknown_fields)]   // strict BAKED namespace
      pub struct BakedSection<Host, Account> {
          pub host: Host,
          pub account: Account,
      }
      ```
      **⚠ ROUND-197 #2 — `BakedSection` IS EXPLICIT, AND
      `BakedIgnored` IS GONE.** The round-196 sketch had ONE `Baked`
      parameter while `build.rs` must carry BOTH `HostDefaultRaw`
      and `AccountRaw`, and it named a `BakedIgnored` type it never
      defined while claiming that type validated both nested tables
      strictly. Two parameters fix the first; the second is settled
      by a rule:
      **STRICT BAKED DTOs STAY BUILD-ONLY.** The schema crate owns
      the two ROOT shapes above and nothing else — it never sees
      `HostDefaultRaw` or `AccountRaw`, which remain
      build-script-only types in `auctioneer-config`. Each consumer
      instantiates the generics with what it already has:
      - **`build.rs`:** `AuctioneerConfigSource<HostDefaultRaw,
        AccountRaw, toml::Value>` — full strict validation of both
        baked tables, and the runtime subtree parsed only loosely
        enough to prove it EXISTS.
      - **`auctioneer-cli`:** `AuctioneerConfigSource<toml::Value,
        toml::Value, AuctioneerAppConfig>` — the mirror image. The
        `[baked]` tables are accepted as opaque values, so a push
        does NOT re-validate them (the build already did, and the
        CLI cannot see their types without the cycle round-196 #2
        rejected). **A typo inside `[baked.host]` is caught at
        BUILD, not at push** — stated plainly rather than implied,
        because round-196 wrongly claimed the CLI caught it too.
        Both namespaces must still be PRESENT, which the strict
        root enforces.
      Parity test: `build_and_cli_agree_on_the_root_shape()` — one
      fixture document deserializes under both instantiations.
      **⚠ ROUND-196 #2 — THE TYPE CANNOT LIVE IN `auctioneer-core`.**
      Round-195 put it there while also having `auctioneer-config`'s
      build script parse through it. `auctioneer-core` DEPENDS ON
      `auctioneer-config` (`Cargo.toml:34`), so that is a cycle. The
      generic parameters let each consumer instantiate it with the
      structs it already owns — `build.rs` with `HostDefaultRaw` /
      `AccountRaw` (build-script-only types), `auctioneer-cli` with the
      runtime pair — so ONE root definition serves both without
      either crate seeing the other's types.
      **⚠ ROUND-196 #3 — AND THE ROUND-195 PSEUDOCODE WAS
      INTERNALLY CONTRADICTORY.** Three defects, all corrected
      above or below:
      - `#[serde(default)] baked` contradicted
        `build_rejects_missing_baked()`. **Removed** — both fields
        are required, which is what makes that test pass.
      - It derived `AppConfig` (which GENERATES `AppConfigMeta`)
        while also hand-writing `secret_fields()`. Those conflict.
      - Without `#[app_config(nested)]` the derive finds no runtime
        secrets; with it, every secret path gains an unwanted
        `runtime.` prefix that would not match the deployed blob.
      **The CLI-side wrapper is therefore HAND-WRITTEN, not
      derived:**
      ```rust
      // crates/auctioneer-cli: the type passed as `C`.
      pub struct AuctioneerPushSource(
          AuctioneerConfigSource<toml::Value, toml::Value, AuctioneerAppConfig>,
      );

      impl<'de> Deserialize<'de> for AuctioneerPushSource { /* delegates */ }

      // Serialize emits ONLY the runtime subtree, so the envelope's
      // `data` is what today's auctioneer.toml produces.
      impl Serialize for AuctioneerPushSource {
          fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
              self.0.runtime.serialize(s)
          }
      }
      impl Validate for AuctioneerPushSource { /* delegates to runtime */ }
      // HAND-WRITTEN, never derived: paths must match the deployed
      // blob exactly, with NO `runtime.` prefix.
      impl AppConfigMeta for AuctioneerPushSource {
          fn secret_fields() -> Vec<SecretField> {
              AuctioneerAppConfig::secret_fields()
          }
      }
      ```
      The `[baked]` tables ride as `toml::Value` and are dropped by
      `Serialize` (round-197 #2).
      **No EdgeZero change is required**, and no `[accounts.default]`
      reaches the runtime accounts map, no new overlay fields exist,
      and `resolve.rs` is untouched. Test:
      `pushed_envelope_data_matches_todays()` (round-196 #1 — compare the envelope's `data` and its SHA, NOT literal envelope bytes: `generated_at` changes on every push).
    - **The operator still sees one file** with every knob and its
      default — which was the whole point — and the namespaces make
      the rebuild/push boundary VISIBLE rather than implied.
    - **The boot-only values** (`max_config_fetch_ms`,
      `terminal_response_permits`, `refusal_write_deadline_ms`) live
      under `[baked.host]`, where the section name itself says
      REBUILD REQUIRED. This also resolves the §6.1 contradiction
      (round-194 #5): `[runtime]` is "tunable without a rebuild",
      `[baked]` is not, and no value has to satisfy both.
  - **⚠ ROUND-202 #8 — "A BUILD STILL FAILS ON AN UNKNOWN KEY" IS
    ONLY TRUE AT THE TOP LEVEL TODAY. THE NESTED DTOs MUST GAIN THE
    GUARD.** `HostDefaultRaw`, `AccountRaw` and `GdprY` carry
    `#[serde(deny_unknown_fields)]`, but **`CcpaY`, `LmtY`,
    `HostCookieY` and `ResourceLimitsY` do not** — verified in
    `build_codegen.rs`. So a misspelled key INSIDE `[ccpa]`,
    `[lmt]`, `[host_cookie]` or `[resource_limits]` is silently
    ignored and the operator keeps the default, which is precisely
    the failure this task claims to prevent. Today the blast radius
    is small because the files are short and rarely edited; after
    the fold the same tables sit in the file operators edit
    routinely, so the gap gets more exposure, not less.
    **This task adds `deny_unknown_fields` to all four**, and the
    regression test is per-struct, not just per-document:
    `build_rejects_unknown_key_in_each_nested_baked_table()` plants
    a misspelling in each of the four in turn and asserts the build
    fails each time — a single top-level fixture passes while three
    of the four stay broken.
  - Every guard `build.rs` runs today stays: `ResourceLimitsY::validate()`, `validate_host_aliases_against_registry`, the typed
    `RenderError` checks, and the harness override (renamed
    `AUCTIONEER__DEFAULTS__PATH_OVERRIDE`). **A build still fails on a
    missing key, a bad type or an unknown key** — the property that
    keeps defaults as data rather than
    `impl Default for HostConfig`.
    **Every value is byte-identical to what ships today**; this task
    changes no behaviour and no blob schema.
  - **THE OVERLAY SEMANTIC IS UNCHANGED AND IS WHY THIS IS SAFE.**
    `AuctioneerAppConfig`'s 54 fields stay `Option<_>`, so a pushed blob
    remains a PARTIAL overlay and `resolved_host` keeps merging
    overlay-over-baked (`config/resolve.rs`: "Absent → the baked
    gdpr passes through whole"). A deploy that pushes a blob setting
    only `default_tmax_ms` still gets every other value from the
    baked layer. **Do NOT make the blob total** — that would make a
    deploy with no blob have no `gdpr.enabled` at all.
  - **Files:** Modify `auctioneer.toml` (gains `[baked.host]`,
    `[baked.account]`; its current contents move under `[runtime]`);
    Modify `crates/auctioneer-config/build.rs` (`resolve_defaults_path`
    gains the repo-root anchor; each emitter selects its own
    namespace — NO change to `HostDefaultRaw`, `AccountRaw`,
    validation or codegen output); Create
    `crates/auctioneer-config-schema/` (the dependency-neutral root DTO
    — round-196 #2); **Modify `Cargo.toml` (the workspace uses
    EXPLICIT `members`, so a new crate is invisible until listed),
    `crates/auctioneer-config/Cargo.toml` (a `[build-dependencies]`
    entry, the `auctioneer-gvl-schema` pattern),
    `crates/auctioneer-cli/Cargo.toml`, and `Cargo.lock` — round-197
    #3: all four belong to this task's ACCEPTANCE, not to incidental
    churn; a plan that creates a crate without them does not
    build**; Modify `crates/auctioneer-cli/src/` (define
    `AuctioneerPushSource` and pass it as `C` to `run_config_push_typed`
    / `run_config_diff_typed`, **and migrate `Validate` to
    `run_config_validate_typed::<AuctioneerPushSource>`** — round-196
    #5: it currently calls the UNTYPED `run_config_validate`, which
    skips typed deserialization, validators and secret checks, so an
    operator can be told "valid" for a document `push` would
    reject); Delete
    `crates/auctioneer-config/data/host-defaults.toml` and
    `data/account-defaults.toml`; Modify the build-failure harness
    fixtures that name the old paths.
    **`crates/auctioneer-core/src/config.rs` and `config/resolve.rs` are
    UNCHANGED** (round-194 #1; round-197 #7 — the wrapper type lives
    in `auctioneer-cli` and the root DTO in `auctioneer-config-schema`, so
    neither this claim nor the no-cycle rule is violated) — `AuctioneerAppConfig` gains no fields
    and the merge is untouched, which is what makes every deployed
    blob still valid.
  - **Interfaces (Produces):** `auctioneer_config::host_default()` and
    `account_default()` UNCHANGED in name, type and value, so the
    four call sites (`handlers/auction.rs:402`, `conformance.rs`,
    and two tests) need no edit. That invariance is the acceptance
    test.
  - Step 1: the failing tests (round-195 #6 — the count is stated
    nowhere, because an earlier revision said "four" while listing
    six; the list below is authoritative) —
    `baked_defaults_match_the_pre_fold_values()` (a snapshot assertion
    that every field of `host_default()` and `account_default()` is
    byte-identical to today's, so the fold cannot silently change a
    default), `pushed_blob_is_the_runtime_table_only()` (round-194
    #1 — the pushed bytes must be byte-identical to what today's
    `auctioneer.toml` pushes, and must carry NO `[baked.*]`, so the blob
    schema is provably unchanged and no deployed blob is
    invalidated), `build_rejects_unknown_key_in_baked_host()` and
    `build_rejects_unknown_key_in_baked_account()` (each namespace
    keeps `deny_unknown_fields` independently),
    `build_fails_on_missing_required_default()`,
    `partial_overlay_still_falls_through_to_baked()`,
    `build_rejects_unknown_root_key()`,
    `build_rejects_unknown_baked_namespace()`,
    `build_rejects_missing_runtime()`,
    `build_rejects_missing_baked()`,
    `env_overlay_reaches_a_live_runtime_key()`,
    **`build_and_cli_agree_on_the_root_shape()`** (round-198 #4 —
    required by the schema-crate bullet but missing here: one
    fixture document must deserialize under BOTH generic
    instantiations, which is the only thing stopping the two
    consumers drifting), **`absent_runtime_key_is_not_env_overridable()`
    ** (round-198 #4 — required by the overlay bullet
    and missing here: it pins the DOCUMENTED LIMITATION, so a later
    round cannot quietly "fix" it by shipping live runtime
    defaults), and
    **`pushed_envelope_data_matches_todays()`** (compare the
    envelope's `data` and SHA, never literal bytes — `generated_at`
    changes on every push).
    **Round-198 #4: this list is the ONLY authoritative one. A test
    named in a body bullet but absent here is a defect in this
    list, not an optional extra.** Step 2:
    compile failure. Step 3: implement. Step 4: PASS on
    `cargo test --workspace`.
    **⚠ ROUND-195 #6 — ONE EXISTING TEST MUST CHANGE, AND THE
    "NO EDITS" ACCEPTANCE WAS FALSE.**
    `crates/auctioneer-core/tests/config_reference_examples.rs`
    `include_str!`s the WHOLE `auctioneer.toml` and parses it directly
    as `AuctioneerAppConfig`; after the fold the document root is
    `AuctioneerConfigSource`, so that test cannot pass unedited. It is
    SPLIT: one case validates the `[runtime]` projection as
    `AuctioneerAppConfig` (preserving exactly what it asserts today),
    and a second validates the `[baked.*]` examples against their
    Raw structs. **Every OTHER test stays green with no edits**, and
    that — plus the byte-identical envelope — is the proof this is
    a pure relocation. Step 5: commit `auctioneer-config: fold host/account
    defaults into auctioneer.toml`.
  - **⚠ ROUND-195 #2 — THIS TASK ALSO FIXES A SHIPPED DOC DEFECT:
    THE ENV OVERLAY DOES NOT WORK FOR COMMENTED-OUT KEYS.** The spec
    and `auctioneer.toml` both advertise
    `AUCTIONEER__AUCTION__DEFAULT_TMAX_MS=1500 auctioneer-cli config push`.
    **That is silently a no-op today.** Upstream's loader states it
    plainly: "The overlay only overrides keys that ALREADY EXIST in
    the parsed tree" (`edgezero-core/src/app_config.rs`), and
    `auctioneer.toml` has **ZERO uncommented keys** — verified by
    `grep -cE '^[a-z_]+ *=' auctioneer.toml` returning 0. So every
    documented example targets a key the overlay cannot reach. This
    predates M3b-1d; the task inherits it because it rewrites the
    file.
    **⚠ ROUND-196 #1 — THE ROUND-195 FIX IS WITHDRAWN: SHIPPING
    LIVE `[runtime]` DEFAULTS WOULD BREAK THE OVERLAY CONTRACT.**
    Round-195 proposed shipping `[runtime]`'s most-tuned keys at
    their default values so the env overlay could reach them. That
    is self-defeating: `AuctionConfig.default_tmax_ms` is
    `Option<i64>`, so a present `default_tmax_ms = 1000`
    deserializes as `Some(1000)` and **permanently pins** the value
    — the blob stops inheriting future baked defaults, which is the
    exact property this task promises two bullets earlier, and the
    file regains two active sources of truth that can diverge.
    **RUNTIME DEFAULTS STAY ABSENT.**
    **THE CONTRACT, STATED HONESTLY INSTEAD:** an env overlay can
    only override a key the file ALREADY SETS. A commented-out or
    omitted `[runtime]` key is **not** env-overridable, and no
    wording in this plan, in `auctioneer.toml` or in the master spec may
    imply otherwise. An operator who wants
    `AUCTIONEER__RUNTIME__AUCTION__DEFAULT_TMAX_MS` to work must first
    uncomment `default_tmax_ms` — at which point they have made a
    deliberate, visible choice to pin it, which is the correct
    semantics rather than a workaround.
    **The `[baked.*]` keys are live by nature** (they are the
    defaults), so they are always present — but see round-196 #6:
    `build.rs` reads them with `fs::read_to_string` + `toml::from_str`
    and applies NO env overlay, so `AUCTIONEER__BAKED__...` affects
    nothing. Baked values change by **editing the file and
    rebuilding**, full stop.
    **Filed upstream as G27** (populate absent typed `Option` fields
    from the env overlay); if it lands, absent runtime keys become
    overridable without being pinned, and this bullet is revisited.
    Test: `absent_runtime_key_is_not_env_overridable()` — asserts
    the documented limitation rather than a workaround.
  - **Blocked by:** nothing. It touches `auctioneer-config`,
    `auctioneer-cli` (which owns `AuctioneerPushSource`),
    `auctioneer-config-schema` (the generic root) and one TOML
    — round-199 #3: this said "`auctioneer-core`'s source-wrapper
    type", contradicting the ownership fixed in round-197 #7 and the
    no-cycle rule; **`auctioneer-core` is NOT touched** —
    file; no clock, no batch API and no pin. **Dispatchable in
    parallel with M3b-0 and M3b-1c.**
  - **NOT IN SCOPE:** `runtime-bootstrap.toml`. Its one key
    (`allow_missing_config`) governs what to do when the store has
    NO blob, so folding it into the pushed source would be circular
    — the same self-reference rule that keeps `max_config_fetch_ms`
    boot-owned (round-191 #2). It stays a separate 20-line file for
    that reason alone, and M3b adds no keys to it.

- [ ] **Task M3b-1c — `/status` reports what is misconfigured
  (D-1, resolved round 185).**
  - **CONTRACT.** `/status` continues to answer from baked defaults
    when runtime config cannot be loaded — it does NOT 500 — and it
    NAMES THE FAULT in its response. A probe that degrades silently
    is indistinguishable from a healthy one, which is the actual
    operational failure: an operator sees green while GDPR
    enforcement runs off a default nobody vouched for.
  - **Files:** Modify `crates/auctioneer-core/src/handlers/status.rs`
    (stop discarding the error); Modify
    `crates/auctioneer-core/src/config/runtime.rs` — **MANDATORY, not
    conditional (round-190 #7): it adds
    `RuntimeConfigError::StoreKeyRejected` and retargets the
    `EdgeError::BadRequest` arm at it. Without that variant the
    third status code cannot be derived from the error kind, and
    deriving it from `detail` is forbidden**; Modify
    `crates/auctioneer-core/tests/runtime_config_failclosed.rs` (extend,
    do not weaken, the existing assertions).
  - **THE SHAPE, EXACTLY.** Today the handler writes
    `resolve_runtime_config(&ctx).await.unwrap_or_else(
    RuntimeConfigError::degrade_to_default)`, which drops the error
    on the floor. Replace it with a match that KEEPS the error
    alongside the degraded config, then:
    - **A dedicated response header on every answer,**
      `x-auctioneer-config-status: ok | degraded`. This is the part a
      machine reads, and it works on BOTH branches — including the
      204, which has no body to carry anything. A monitoring rule
      alerts on `degraded` without parsing a body.
    - **The fault named in the body** on the 200 branch, appended to
      the configured `status_response` as a final line:
      `config: degraded (<code>)`.
      **⚠ ROUND-186 #5 — `<code>` IS A NEW CLOSED ENUM, NOT
      `RuntimeConfigError::detail`. THE EARLIER TEXT HERE WAS WRONG
      AND WOULD HAVE LEAKED CONFIG DATA ON AN UNAUTHENTICATED
      ROUTE.** Round-185 claimed `detail` was "a curated, fixed
      string … built from a closed set", generalising from
      `outer_edge_errors_fail_closed_without_upstream_text()`. That
      test covers ONLY `classify_load`'s handling of upstream
      `EdgeError` text. It does not reach `validate_alias_bases`
      (`crates/auctioneer-core/src/config/runtime.rs`), which builds
      `detail: format!("host alias {alias:?} references unknown base
      {base:?}")` from STORE-CONTROLLED strings. Four of the seven
      `detail` construction sites are fixed literals; three are
      `format!`, and that one interpolates config values. Publishing
      `detail` would hand an unauthenticated caller fragments of the
      deploy's configuration, chosen by whoever can write the config
      store.
      **THE FIX — TWO CHANNELS, NOT ONE:**
      ```rust
      // auctioneer-core: the PUBLIC vocabulary. Closed, exhaustive,
      // and carrying no value-derived text by construction.
      // ROUND-187 #7: THREE codes, not two. The wire string is the
      // snake_case variant name, fixed here so the contract does not
      // depend on a Debug impl.
      pub enum ConfigStatusCode {
          MalformedConfig,   // "malformed_config"  — blob unparseable/untrusted/invalid
          StoreKeyRejected,  // "store_key_rejected" — the bound key was refused
          StoreUnavailable,  // "store_unavailable"  — transient backend failure
      }
      ```
      **⚠ ROUND-189 #2 — THE THREE CODES ARE NOT DERIVABLE FROM
      TODAY'S ERROR TYPE, AND THIS UNIT MUST ADD THE VARIANT
      FIRST.** `RuntimeConfigError` has exactly TWO variants,
      `MalformedHost` and `ConfigStoreUnavailable`, and the
      rejected-key case is ALREADY COLLAPSED into `MalformedHost`
      (`config/runtime.rs`, the `Err(EdgeError::BadRequest { .. })`
      arm: `MalformedHost { detail: "config store rejected the bound
      key" }`). So a corrupt blob and a refused key are
      STRUCTURALLY IDENTICAL today, and the only thing distinguishing
      them is the `detail` STRING — which round-186 #5 forbids this
      route from inspecting, precisely because `detail` can carry
      store-controlled text.
      **THE FIX, AND IT IS A CODE CHANGE THIS UNIT OWNS:** add
      `RuntimeConfigError::StoreKeyRejected { detail: String }` and
      re-target the rejected-key path at it.
      **⚠ ROUND-202 #7 — RE-TARGETING `EdgeError::BadRequest` ALONE
      MISSES THE TYPED EXTRACTOR, WHICH IS THE PATH M3b USES.**
      Through the typed extractor a rejected key arrives as
      `StoreExtractionReason::InvalidKey`, and
      `classify_store_extraction` currently groups it with the
      eleven malformed-family reasons — so `/status` would report
      `malformed_config`, NOT `store_key_rejected`, and the
      distinction this unit exists to provide would silently not
      appear. **BOTH paths re-target:** the `EdgeError::BadRequest`
      arm AND `Reason::InvalidKey`, which moves OUT of the
      malformed-family arm into its own. The HTTP disposition is
      unchanged — 500 fail-closed on every route reaching a privacy
      or auction decision — so this stays a reporting split.
      **Test through the REAL extractor**, not a hand-built
      `EdgeError`:
      `invalid_key_reason_reports_store_key_rejected()` drives
      `classify_store_extraction(Reason::InvalidKey, ..)` and
      asserts both the variant and the `/status` code — a test
      constructing `BadRequest` directly would pass while the
      shipped path stayed wrong. `ConfigStatusCode` is
      then a TOTAL match on the error KIND — three variants, three
      codes, no string inspection anywhere:
      ```
      MalformedHost        -> malformed_config
      StoreKeyRejected     -> store_key_rejected
      ConfigStoreUnavailable -> store_unavailable
      ```
      **The master spec's error-mapping table gains the same row**
      (§ the `EdgeError` → `RuntimeConfigError` mapping, where
      `EdgeError::BadRequest` currently points at `MalformedHost`).
      `StoreKeyRejected` keeps `MalformedHost`'s fail-closed
      disposition on every OTHER route — this is a reporting split,
      not a policy change, and a test asserts the two classify
      identically outside `/status`.
      **⚠ ROUND-187 #7 — TWO CODES COULD NOT SATISFY THIS UNIT'S
      OWN ACCEPTANCE.** The step list requires each of the three
      shipped degradation cases to be named DISTINCTLY, but
      `corrupt_blob_keeps_status_answering()` and
      `rejected_store_key_keeps_status_answering()` both mapped to
      `MalformedConfig`, so an operator could not tell "the blob is
      corrupt" from "the key is refused" — two faults with two
      different fixes. `StoreKeyRejected` closes that.
      **THE WIRE FORMAT, EXACTLY** (round-187 #7 — previously
      unspecified, which would have let two implementers disagree):
      - Healthy, empty `status_response`: `204`, no body.
      - Healthy, configured: `200`, body = `status_response` bytes
        VERBATIM, unchanged from today.
      - Degraded: `200`, body = the **BAKED** `status_response`
        bytes, then a single `\n` separator IF and only if those
        bytes are non-empty and do not already end in `\n`, then
        `config: degraded (<code>)\n`.
        **⚠ ROUND-209 — "the CONFIGURED `status_response`" WAS
        WRONG, AND PUBLISHING IT WOULD HAVE REOPENED THE LEAK THIS
        UNIT CLOSES.** On a degraded answer the blob was REFUSED:
        `degrade_to_default` returns `AuctioneerAppConfig::default()`,
        so the store's banner is gone by construction. Emitting it
        would mean echoing bytes out of the very blob
        `ConfigStatusCode` exists to stop us trusting. The baked
        value is the only one available and the only one safe.
        Since the shipped baked `status_response` is `""`, every
        degraded body today is the diagnostic line alone; the
        separator rule is pinned directly on `status_inner`, where
        the baked banner is an input rather than a rebuild.
      - `<code>` is the fixed snake_case string above. Nothing else
        is ever appended.
      So an empty `status_response` yields exactly
      `config: degraded (store_unavailable)\n` with no leading
      blank line. Test: `degraded_body_format_is_exact()`.
      `/status` publishes ONLY `ConfigStatusCode` (a fixed
      `&'static str` per variant). The full `detail` goes to the LOG
      line the handler already has, where the operator reads it with
      deploy credentials. The variant is derived from the error
      KIND, never from its message.
      **Why THREE variants and not more (round-189 #3 — this
      sentence said "two", left over from the superseded pair):**
      each maps to a DIFFERENT operator action — fix the blob, fix
      the key binding, or wait for the backend. Finer granularity
      would push toward encoding WHICH FIELD was wrong, which is the
      leak in another form.
    - **The 204 branch becomes 200 WHEN DEGRADED.** An empty
      `status_response` means "nothing to say"; a degraded deploy
      has something to say. Healthy + empty stays 204.
  - **⚠ WHAT THIS DELIBERATELY DOES NOT DO.** It never publishes
    `RuntimeConfigError::detail`, any alias or base name, any store
    key, or upstream error text. `/status` is unauthenticated, so the
    public channel carries a CLOSED enum and nothing else — the
    separation is structural, not a promise about current string
    contents. **A test asserts the boundary directly** (round-186
    #5): `status_never_leaks_config_values()` pushes an overlay whose
    alias and base names are hostile canaries, reads `/status`, and
    asserts neither canary appears in the body OR in any header. It
    fails if someone later routes `detail` to the public channel.
    The fail-closed 500 rule on privacy- and auction-deciding routes
    is untouched.
  - **Interfaces (Produces):** a `/status` answer that is 200/204 on
    a healthy deploy and 200-with-a-named-fault on a degraded one,
    plus `x-auctioneer-config-status` on every answer.
  - Step 1: the five failing tests —
    `status_names_the_fault_when_config_is_malformed()` (the body
    carries the CODE; the status is NOT 500),
    `status_never_leaks_config_values()` (round-186 #5 — hostile
    alias/base canaries appear in neither body nor headers),
    `status_marks_degraded_in_a_header_on_both_branches()` (the
    204-shaped deploy still reports degraded),
    `status_is_ok_and_unmarked_on_a_healthy_deploy()` (no false
    positives), `status_detail_never_echoes_upstream_text()` (the
    leak guard, asserted at the ROUTE this time, not only at
    classification — and now at the ROUTE, which is the boundary
    that actually matters).
    **⚠ ROUND-189 #3 — THE THREE EXISTING DEGRADATION TESTS MUST
    CHANGE, AND THE ROUND-185/186 INSTRUCTION THAT THEY "KEEP
    PASSING UNCHANGED" WAS WRONG.** Those tests assert
    `StatusCode::NO_CONTENT` — verified in
    `crates/auctioneer-core/tests/runtime_config_failclosed.rs`:
    `corrupt_blob_keeps_status_answering()` (line ~501),
    `store_outage_keeps_status_answering()` (~514) and
    `rejected_store_key_keeps_status_answering()` (~558) each assert
    204. The degraded contract this unit introduces says a degraded
    deploy answers **200 with a diagnostic body**, never 204,
    because 204 has no body to carry the fault. The two cannot both
    hold. An implementer following the old instruction would either
    leave the contract unimplemented or delete the assertion to make
    it pass.
    **WHAT EACH TEST BECOMES** — they are UPDATED, not deleted, and
    each keeps its original job (the route still answers, still does
    not 500) while gaining the new one:
    - `corrupt_blob_keeps_status_answering()` → asserts **200**,
      body ends `config: degraded (malformed_config)\n`, header
      `x-auctioneer-config-status: degraded`.
    - `rejected_store_key_keeps_status_answering()` → **200** and
      `store_key_rejected` — the case that REQUIRES round-189 #2's
      new variant, and the one that proves the three codes are
      distinct rather than decorative.
    - `store_outage_keeps_status_answering()` → **200** and
      `store_unavailable`.
    - `degraded_status_still_carries_milestone_header()` keeps
      passing unchanged: the milestone stamp is on every answer.
    **THE REGRESSION GUARD MOVES WITH THEM.** The property these
    tests exist to pin is "a degraded deploy still ANSWERS, and does
    NOT 500" — that survives verbatim. What changes is the shape of
    the answer. If an implementer finds themselves asserting 500,
    the change has drifted into fail-closed and D-1 is being
    re-litigated in code.

  - **Blocked by:** nothing. It touches one handler and its tests,
    needs no clock, no batch API and no new pin, so it is
    **dispatchable immediately** — it is not gated behind M3b-0 or
    the pin move.

- [ ] **Task M3b-6 — Snapshot fixture: mid-flight timeout.** Assert
  three surfaces per spec §5.2 line 1309 (round-22 C2
  correction — earlier draft conflated code 12 with `nbr`):
  - Per-bidder `ext.errors[<bidder>].code == 1` (`Timeout`) for
    each bidder whose per-bidder timeout / deadline elapsed.
  - Global `ext.errors.prebid[0].code == 12` (`TmaxTimeout`)
    once, when the whole-auction deadline elapsed. Uses the
    existing `ResponseInput.prebid_errors` field at
    `auctioneer-exchange/src/response.rs:47`.
  - `seatnonbid[<bidder>].statuscode == 101` for each timed-out
    bidder.
  - `nbr` is `None` in this fixture (nbr taxonomy is separate;
    only the "no live bidders" case sets it — spec Appendix E.5).
  Lives in `crates/auctioneer-core/tests/` (round-20 H4).

  **EXECUTABLE SHAPE (round-177 #1):**
  - **Blocked by:** 2b.2 (it asserts against real fanout output).
  - Files: Create `crates/auctioneer-core/tests/response_mid_flight_timeout.rs`
    and its fixture pair under
    `crates/auctioneer-core/tests/snapshot/mid_flight_timeout.{request,response}.json`.
  - Step 1: the fixture drives a three-bidder auction on a
    `FakeClock` where one bidder answers inside its deadline, one
    exceeds its per-bidder timeout, and the auction deadline elapses
    before the third resolves; assert all four surfaces listed above
    (code 1 per timed-out bidder, ONE code 12, statuscode 101 per
    timed-out bidder, `nbr` absent) and that the on-time bidder's bid
    is present and unmodified. Step 2: compile/assert failure.
    Step 3: no product code — this task is fixtures; any failure is a
    defect in 2b.2 or the exchange and is fixed there. Step 4: PASS.
    Step 5: commit `auctioneer-core: snapshot fixture — mid-flight
    timeout`.

- [ ] **Task M3b-7 — Snapshot fixture: oversize response.**
  Memory-bound assertion. Round-37 H3 canonical formula
  (retires round-35 H5's shape which had the baseline
  concealing an additional decoded-buffer worth of allocation
  by subtracting a max_decoded_bytes-sized baseline from the
  peak and simultaneously bounding by a term that already
  included max_decoded_bytes — net effect: the bound tolerated
  ~2× the intended decoded footprint):

  ```rust
  // Round-37 H3 canonical: EMPTY baseline (the harness
  // captures allocator peak against a NO-response run, not
  // a max_decoded_bytes identity run). Absolute cap on
  // peak. Checked u64 arithmetic — an overflow is a bug in
  // the test setup, not a passing result.
  let baseline_empty = allocator_peak_with_no_response();
  let peak = allocator_peak_after_send_bounded(...);
  let observed_over_baseline = peak
      .checked_sub(baseline_empty)
      .expect("post-run peak must be >= baseline");
  // Round-45 H3: the bound IS the canonical helper — no
  // hand-assembled sum. The round-44 shape added
  // `LAYERS_MAX × MAX_CHUNK_SIZE` on top of
  // `per_stream_bytes(0, 0, ..)`, but per_stream_bytes
  // ALREADY includes that chunk-staging term (see its
  // `chunk_staging` local), so the old `bound` double-
  // counted 3 × MAX_CHUNK_SIZE. It also called the helper
  // in a `.checked_add(..)` position, which no longer
  // compiles now that the helper returns `Result`.
  let bound = per_stream_bytes(
      max_transport_bytes as u64,
      max_decoded_bytes as u64,
      max_brotli_wbits,
      max_brotli_layers,
      0,                           // round-134 #4: the five-param
                                   // max-rule ABI; guest bound 0 —
                                   // this harness measures auctioneer's
                                   // OWN pipeline, so the chain arm
                                   // is the bound
  ).expect("bootstrap wbits already validated at install");
  assert!(observed_over_baseline <= bound,
      "peak allocation exceeds absolute bound: \
       observed={observed_over_baseline}, bound={bound}");
  ```
  Key differences from round-35 H5:
  1. **Empty baseline.** The harness reads allocator peak
     against a run that performs NO outbound request. The
     old shape used a `max_decoded_bytes`-sized identity
     response as baseline — but that means the harness
     already spent one full decoded buffer BEFORE the
     "real" test, and the bound then added the SAME term
     back on top. Net: the test tolerated 2×
     `max_decoded_bytes` before failing.
  2. **Absolute cap.** Alongside the delta-vs-baseline
     check, the harness asserts `peak <= ADAPTER_MEMORY_BUDGET
     * 3 / 4` (the same 75%-of-adapter-memory that B6's
     admission gate enforces). This catches the case
     where baseline itself drifted (memory noise, background
     tasks) — the absolute cap is the platform-independent
     ceiling.
  3. **`checked_sub` + `checked_mul` + `checked_add` throughout.**
     Silent saturation in the old shape masked a case where
     `peak < baseline` (allocator race after the "no
     response" run) produced 0 as the delta and the bound
     passed trivially. `checked_sub` panics loudly on that
     invariant break.
  4. **Adapter transport instrumentation.** The harness ALSO
     asserts adapter-side counters (Task M3b-2's
     `transport_bytes_pulled()` / `decoded_bytes_emitted()`
     test hooks) — Rust-side `#[global_allocator]` cannot
     see Cloudflare V8 buffers or Fastly host-process
     buffers. See "separate proof surfaces" below.

  This is the ONE canonical formula referenced by every
  M3b-7 test case below; no reformulation elsewhere in the
  plan.

  **Round-32 H5 — Brotli standard window is 16 MiB per RFC
  7932, not 8 MiB.** Round-31 H4 said "brotli-decompressor
  ~800 KiB, capped at 8 MiB via `--large-window=false`". The
  `--large-window=false` flag disables Brotli's EXTENSION
  beyond RFC 7932 (windows up to 30 bits); it does NOT
  restrict the STANDARD Brotli maximum, which is `2^24 - 16
  = 16_777_200` bytes (nearly 16 MiB). auctioneer's constant
  budgets for full standard Brotli:
  - Codec crates: EXACT versions pinned in
    `[workspace.dependencies]` under round-38 H3 (retires
    the round-32 H5 claim of `flate2 = "=1.0.30"` — the
    workspace at [Cargo.toml:48](../../../Cargo.toml#L48)
    actually declares `flate2 = { version = "1", ... }`
    which resolved to 1.1.9 at Cargo.lock time. The
    decoder-state bound is only true for a MEASURED codec
    version; an open range means the admission gate can
    shift under the plan silently (round-44 B4: the bound
    now comes from `brotli_state_bytes(wbits)`, measured
    on the pinned decompressor). M3b
    lands the exact-pin changes below):
    - `flate2 = "=1.1.9"` (round-38 H3 — matches the
      currently-locked minor and enables the workspace
      lint that CI codec-pin check verifies). `gzip +
      zlib + raw-deflate paths via `rust_backend`; measured
      state ≤ **128 KiB** on this version.
    - `brotli-decompressor` — **round-158 #1: auctioneer writes NO pin
      for this crate; upstream owns it (`=5.0.1` at PR head, where
      it becomes a normal dependency). The M3b target is 5.0.1, and
      the `=5.0.3` this bullet used to demand is retired.** History:
      round-38 H3 pinned `=4.0.1`; round-39 H2 corrected that to the
      then-resolved chain
      `async-compression 0.4.43 → brotli 8.0.4 →
      brotli-decompressor 5.0.3` as recorded at
      [Cargo.lock:291](../../../Cargo.lock#L291); a direct
      `=4.0.1` dep would silently add a second unused
      version and the RUNTIME still calls 5.0.3). Pin the
      whole resolved decoder chain so a Cargo.lock refresh
      that shifts any layer fails the CI codec-pin check:
      **ROUND-156 #2 — the pin list was WRONG AND INCOMPLETE, and
      the decoder-state proof depends on it being neither.** It named
      `async-compression = "=0.4.44"` while the workspace actually
      resolves **0.4.43** (`Cargo.lock`), and it omitted
      `compression-codecs` (resolved **0.4.39**) and
      `compression-core` (resolved **0.4.33**) — the crates that
      contain the decoder wiring the charge is measured against —
      while claiming the whole chain was pinned. It also diverges
      from the pinned EdgeZero, which pins
      `async-compression = "=0.4.43"` and
      `brotli-decompressor = "=5.0.1"` and audited its own fixed
      16 MiB charge against THAT pair plus `compression-codecs
      0.4.38`.
      **ROUND-158 #1 — THE RESOLUTION FACTS, CORRECTED. The round-157
      wording was wrong twice: an exact requirement is NEVER
      "caret-compatible", and the same paragraph then claimed two
      decoder paths resolve to different versions while demanding one
      shared tuple.** What is actually true, verified at `120dd2aa`:
      - Upstream's `brotli` / `brotli-decompressor` / `flate2` exact
        pins live in `edgezero-core`'s **`[dev-dependencies]`**
        (`crates/edgezero-core/Cargo.toml:50-54`), NOT its
        `[dependencies]`. Dev-dependencies of a git dependency are
        not part of a downstream build graph, so auctioneer NEVER
        inherited that tuple — which is why auctioneer resolves
        `brotli-decompressor 5.0.3` today with no conflict, and why
        no "caret compatibility" was ever involved.
      - Cargo resolves ONE version of each crate per lockfile. Two
        exact requirements that disagree are unsatisfiable, full
        stop.
      - PR-275 head PROMOTES those crates to normal dependencies of
        `edgezero-core`. From that moment auctioneer DOES inherit them,
        and the head tuple becomes binding on auctioneer whether or not
        auctioneer names the crates.
      **THE M3b TARGET TUPLE IS THE PR-HEAD TUPLE:**
      | crate | M3b target (PR head) | auctioneer today |
      |---|---|---|
      | `async-compression` | 0.4.43 | 0.4.43 |
      | `compression-codecs` | 0.4.38 | 0.4.39 |
      | `compression-core` | 0.4.32 | 0.4.33 |
      | `brotli` | 8.0.4 | 8.0.4 |
      | `brotli-decompressor` | 5.0.1 | 5.0.3 |
      Three rows differ, so a measurement taken today is NOT a
      measurement of what M3b ships.
      **ORDERING CONSEQUENCE (binding): M3b-11 RUNS BEFORE M3b-2b'S
      DECODER MEASUREMENT** (round-163 #2 — it is 2b's, not 2a.5's;
      2a.5 ships pure arithmetic and measures nothing). The pin move
      brings the head tuple into the lockfile; M3b-11 then writes
      `codec_pins_match_lockfile()` against it, and M3b-2b measures
      `brotli_state_bytes` against the same tuple. Measuring first
      would pin the
      budget to `5.0.3`/`0.4.39`/`0.4.33`, which M3b-11 then
      invalidates. Both decoder audits — upstream's fixed charge and
      auctioneer's `brotli_state_bytes` — are re-run against the ONE
      resolved head tuple; there is no second tuple and no
      "different codec versions" case to account for.
      THE RULE, binding on 2a.5, M3b-2b and certification: auctioneer
      declares only what it directly uses, never an exact version
      that contradicts upstream's, and the asserted tuple below is
      the HEAD tuple, checked against `Cargo.lock`:
      ```toml
      # crates/auctioneer-core/Cargo.toml — round-157 #1: auctioneer declares
      # only what it directly uses, and NEVER an exact version that
      # contradicts an upstream exact pin (see the shared-tuple rule
      # above). The decoder-state proof is anchored by the LOCKFILE
      # assertion, not by duplicating upstream's pins.
      [dependencies]
      async-compression       = { version = "0.4", features = ["futures-io", "gzip", "brotli", "zlib"] }
      ```
      and the ASSERTED tuple — the PR-head versions the measurement
      is taken against AFTER M3b-11, checked against `Cargo.lock` by
      `codec_pins_match_lockfile()`. Every exact pin is UPSTREAM'S
      (they become normal dependencies of `edgezero-core` at head);
      auctioneer asserts, it does not duplicate them:
      | crate | asserted version | pinned by |
      |---|---|---|
      | `async-compression` | 0.4.43 | EdgeZero (`=0.4.43`) |
      | `brotli` | 8.0.4 | EdgeZero (`=8.0.4`) |
      | `brotli-decompressor` | 5.0.1 | EdgeZero (`=5.0.1`) |
      | `compression-codecs` | 0.4.38 | EdgeZero |
      | `compression-core` | 0.4.32 | EdgeZero |
      | `flate2` | 1.1.9 | EdgeZero (caret); auctioneer asserts the resolved version |
      The CI codec-pin verification test cross-references
      `Cargo.lock` for all six and fails when any drifts, and asserts
      each appears EXACTLY ONCE — a second resolved copy would mean
      the measured decoder is not the one that runs.
      `codec_pins_match_lockfile()` is its roster row.
      ONE TUPLE, TWO CALLERS (round-158 #1 replaces round-156 #2's
      "separate accounting", which assumed two version sets): both
      decoders run on the SAME resolved crates. The certificate still
      charges two ARMS — auctioneer's passthrough chain from auctioneer's
      `brotli_state_bytes` measurement, and the adapter's in-guest
      decoder from the reported `guest_decoder_bound_bytes` — because
      they are different DECODER INSTANCES with different lifetimes,
      not because they are different versions. Both measurements are
      re-run whenever that one tuple moves.
      **⚠ ROUND-157 #2 — THE "state ≤ 24 MiB" FIGURE THAT STOOD HERE
      IS DELETED AS A CONTRACT.** It contradicted both the executable
      unit (2a.5 expects `brotli_state_bytes(24) = 33_554_432`, i.e.
      the 16 MiB fixed charge + the 2^24 ring — round-159 #4
      corrects the 34_365_440 that stood here, which is the
      per-stream total, not the decoder state) and the master spec,
      which retires the flat `MAX_DECODER_STATE = 24 MiB` constant
      outright (§5.3). NOTHING derives an admission decision from a
      24 MiB figure. The ONLY decoder-state authority is
      `brotli_state_bytes(wbits) = BROTLI_DECODER_FIXED_CHARGE_BYTES
      (16_777_216) + 2^wbits`, measured on the asserted codec tuple
      above; any older empirical observation is history, not a bound.
      auctioneer REJECTS "large-window" extension window sizes
      (WBITS > 24) BEFORE decoder allocation. **Round-33 M1
      correction — `brotli-decompressor` has no high-level
      "builder" API**; the round-32 draft's `via builder`
      phrasing referenced something that doesn't exist. The
      real WBITS check happens by decoding the stream's
      Brotli header prefix into a scratch buffer (the first
      byte or two carry the WBITS field per RFC 7932 §9.1),
      inspecting the value directly, and rejecting >24
      before instantiating the full `DecompressorReader`.
      Concrete shape:
      ```rust
      use brotli_decompressor::HuffmanCode;
      // Round-33 M1: peel the first 6 bytes off the raw
      // compressed stream; parse WBITS via the strict-state
      // API. If WBITS > 24 → error before touching the
      // decoder. Otherwise feed the peeled bytes back into
      // a `DecompressorReader::new(chained_reader, buf_size)`.
      let wbits = parse_brotli_wbits(&prefix)?;
      if wbits > 24 {
          return Err(FanoutError::UnsupportedContentEncoding {
              actual: format!("br;large-window;wbits={wbits}"),
          });
      }
      ```
      `parse_brotli_wbits` is a small (~30-line) helper in
      `crates/auctioneer-core/src/host_primitives/brotli.rs`
      documented against RFC 7932 §9.1. This bounds the
      STANDARD path at 16 MiB without depending on a
      library API that doesn't exist.
    - No zstd (round-30 M4 explicitly rejects `zstd`).
    Any bump to any of these crates without a matching
    re-measurement of `brotli_state_bytes` /
    `FLATE2_STATE_BYTES` fails the CI codec pin check
    (round-44 B4).

  **Round-34 H8 — async stream → sync decoder bridge.** The
  `flate2::read::ZlibDecoder` and `brotli_decompressor::DecompressorReader`
  APIs are `std::io::Read`-based
  (synchronous). Auctioneer's raw body is an ASYNC
  `Stream<Item = Result<Bytes, anyhow::Error>>`. The plan
  must define the bridge, not assume one:
  **ROUND-158 #4 — THE PATH IS `async-compression`, DECIDED. The
  "OR write a small bridge module" alternative below is RETIRED**
  (every later section already assumes `async-compression`, and two
  live options in an executable plan is an invitation to implement
  the wrong one). The bridge description is kept only to explain why
  the async decoder is needed at all.
  - Use **`async_compression`** — NOT the alternative bridge
    module that this sentence used to offer: `crates/auctioneer-core/src/host_primitives/
    body_reader.rs` that wraps the `Stream` in a
    `futures::io::AsyncRead` (via
    `tokio_util::io::StreamReader` if the async runtime is
    Tokio; via `futures::stream::TryStreamExt::into_async_read`
    otherwise), then feeds that reader through a
    **synchronous** decoder by pumping chunks via
    `poll_read` in an async wrapper.
  - Concrete recommendation: adopt
    `async-compression = "0.4"` with the `futures-io` +
    `gzip` + `brotli` + **`zlib`** features. `Content-
    Encoding: deflate` on the wire is a zlib-framed DEFLATE
    stream per RFC 9110 §8.4.1.2 (round-35 H4 correction —
    the round-34 draft named `DeflateDecoder`, which
    `async-compression` provides as RAW-deflate; the
    standards-compliant HTTP decoder is `ZlibDecoder`).
    Auctioneer's mapping:
    ```rust
    fn decoder_for(coding: ContentCoding) -> Decoder {
        match coding {
            ContentCoding::Gzip     => Decoder::Gzip,   // GzipDecoder<AsyncRead>
            ContentCoding::Deflate  => Decoder::Zlib,   // ZlibDecoder<AsyncRead>
                                                        //   (round-35 H4)
            ContentCoding::Brotli   => Decoder::Brotli, // BrotliDecoder<AsyncRead>
            ContentCoding::Identity => Decoder::Passthrough,
        }
    }
    ```
    `async-compression` also ships `DeflateDecoder` for
    raw-deflate; auctioneer does NOT wire that on the main
    decode path. Compatibility fallback: if a bidder is
    known to emit raw-deflate under the `deflate` label
    (some legacy endpoints), operators can flip a
    per-`BidderInfo` opt-in field
    (`accept_raw_deflate: bool`) — Task M3b-2 declares this
    as a follow-on knob; the M3b default is zlib-only.
    **ROUND-158 #4 — THE "EdgeZero must enable zlib" PREREQUISITE IS
    RETIRED.** Cargo UNIFIES features across the graph: when auctioneer
    declares `async-compression` with `zlib`, the single resolved
    copy is built with `zlib` enabled for every consumer, upstream
    included. auctioneer needs nothing from EdgeZero here — it simply
    declares the feature it uses. (The observation that upstream's
    own declaration lists `gzip` + `brotli` only remains true and
    remains harmless.)
  - Round-34 H8 rejects the round-33 M1 shape which
    pointed at `brotli_decompressor::DecompressorReader`
    (a synchronous `std::io::Read` reader) inside the
    async decode path. The WBITS pre-check helper from
    round-33 M1 STAYS — it parses raw header bytes, not
    the decoder state, so it doesn't care whether the
    following decoder is sync or async. But the actual
    decode step calls `async_compression::futures::
    bufread::BrotliDecoder`, not the sync
    `brotli_decompressor::DecompressorReader`.

  **Round-34 H8 — separate proof surfaces.** The M3b-7
  isolated allocator harness observes RUST-side allocation
  via `#[global_allocator]`. It CANNOT observe:
  - Cloudflare's V8/Workers-runtime buffers (managed by the
    JS side; no Rust allocation event).
  - Fastly's Compute@Edge platform buffers (managed inside
    the host process, not the WASM guest).
  - Spin's HTTP-body reader buffers.

  M3b-7 therefore SPLITS the memory proof into two:
  1. **Rust-side allocation ceiling** — the pinned formula
     from H4/H5 above. Asserted by `#[global_allocator]`
     on every M3b-shipped adapter's test binary. This proves
     auctioneer's own decoder pipeline stays inside the
     documented bounds.
  2. **Host-runtime transport bound** — asserted via the
     adapter's COLLECT-LOOP counters (round-135 #1: the pull
     primitives named below are each loop's INTERNALS — chunked
     reading inside the adapter — not a auctioneer-visible stream)
     plus a
     PLATFORM-SPECIFIC assertion delegated to the adapter's
     integration test:
     - Axum: assert `bytes_stream()` yields ≤ MAX_CHUNK_SIZE
       per item under the EdgeZero-side wrap.
     - Cloudflare: **there is no deterministic per-request
       Worker heap peak endpoint.** Round-35 H5 removal of
       the round-34 H8 "Workers debug endpoint" claim —
       Cloudflare documents interactive heap snapshots and
       aggregate isolate-wide metrics, not a per-request
       peak deterministic enough for an assertion. The
       Cloudflare platform proof therefore reduces to the
       runtime-delivered-bytes bound (round-35 H3, re-based
       round-135 #1 buffered): the integration test drives
       `send_bounded(call)` with the transport cap governing,
       then asserts the COMPLETED slot's `body.len() ==
       min(max_transport_bytes, wire_len)` and that an
       over-cap upstream response surfaces as
       `ResponseTooLarge` with the adapter's collect-loop
       counter showing no read past the cap. No heap-peak
       assertion.
     - Fastly: assert `Body::read_chunks(8 KiB)` yields ≤
       8 KiB per item.
     - Spin: assert `read_chunk(MAX_CHUNK_SIZE)` yields ≤
       MAX_CHUNK_SIZE.
     These are platform-observability assertions, not
     Rust-allocator assertions; the plan keeps them
     separate so it never conflates the two proofs.
  - Constants: see the **canonical decoder-memory model**
    block near the top of Task M3b-2. Round-44 B4 DELETED
    the duplicate `MAX_DECODER_STATE = 24 MiB` definition
    that used to sit here — it was the second of three
    conflicting figures the same document quoted. The
    only decoder-memory API is:
    ```rust
    // crates/auctioneer-core/src/host_primitives/proxy_bounds.rs
    pub const LAYERS_MAX: usize = 3;
    pub const MAX_CHUNK_SIZE: usize = 8 * 1024;   // 8 KiB
    pub const MAX_BROTLI_LAYERS: usize = 1;       // round-44 B2
    pub const FLATE2_STATE_BYTES: u64 = 128 * 1024;
    pub const DEFAULT_MAX_BROTLI_WBITS: u8 = 22;  // round-44 B2
    // Round-46 H2: NOT `const fn`, and `per_stream_bytes`
    // returns `Result` — the canonical signatures are the
    // checked-arithmetic ones defined near the top of Task
    // M3b-2 (round-45 H2). This summary must match them.
    pub fn brotli_state_bytes(wbits: u8) -> Result<u64, BudgetError>;
    //   round-160 #6: fallible — out-of-range wbits (outside
    //   10..=24) is BudgetError::BrotliWbitsOutOfRange, never an
    //   undefined shift.
    pub fn per_stream_bytes(
        max_transport_bytes: u64,
        max_decoded_bytes: u64,
        max_brotli_wbits: u8,
        max_brotli_layers: u8,
        guest_decoder_bound_bytes: u64,  // round-135 #2: this summary
                                 // now matches the canonical
                                 // five-parameter max-rule form
                                 // (round-134 #4 — DecodeMode is
                                 // retired)
    ) -> Result<u64, BudgetError>;   // round-154 #8: ONE name
    ```
    The allocator harness asserts against
    `per_stream_bytes(...)` — never a hand-written sum.
    Round-37 H3: `BASELINE_ALLOC_TARGET` REMOVED. Baseline
    is captured by the harness against a NO-response run
    (the "empty baseline" per H3 above) — no hard-coded
    per-machine target. The formula is the canonical one
    from Task M3b-7 above (round-37 H3): `checked_sub` on
    the peak-vs-empty-baseline delta, absolute cap on peak
    against the adapter memory budget, and `checked_*`
    arithmetic throughout.

  Counting observed stream bytes at the auctioneer
  layer alone is NOT sufficient proof of that bound because the
  adapter may have already allocated the full response before
  auctioneer reads any byte (pinned Axum uses
  `response.bytes().await` at
  [`edgezero-adapter-axum/src/proxy.rs:61`](https://github.com/IABTechLab/edgezero/blob/9e661ae/crates/edgezero-adapter-axum/src/proxy.rs#L61)
  — NOT `body.collect()` as earlier drafts said — but either.
  ROUND-118 R01 as corrected by ROUND-119 F7 (decoder EOF ≠ HTTP EOF,
  and member multiplicity ≠ malformedness): RFC 1952 gzip is a
  SEQUENCE of members — a JSON document split across consecutive
  members is valid input, so the round-118 "second member → malformed"
  rule is retired as wrong. Contract: decode ALL gzip members under
  the CUMULATIVE decoded cap and the original deadline, concatenating
  output (member-agnostic for brotli/deflate, which define no member
  sequence — trailing bytes there remain junk); after the FINAL
  member, drain the transport to ITS EOF under the same budgets;
  ROUND-121 #7 — deadline precedence through every layer: expiry of
  the call's deadline DURING decode, between members, or during the
  post-decode drain is ALWAYS Timeout(cause), never decoded-side
  corruption or overflow — a stall is a time failure even when it
  happens to sit mid-member;
  corrupt or truncated members, non-member trailing junk, cumulative
  overflow fail decoded-side (deadline exhaustion does NOT — per the
  precedence rule above it is ALWAYS Timeout(cause); the earlier
  wording of this list contradicted that rule and is corrected); a
  transport
  error arriving after valid decoded JSON still FAILS the call (the
  bytes were never proven complete). **Member mode is NOT the
  default and must be enabled explicitly.** The workspace resolves
  async-compression **0.4.43** (round-158 #4 — the "0.4.44" figure
  that stood here was never the resolved version; `Cargo.lock`
  carries 0.4.43, matching upstream's `=0.4.43` pin, and the M3b
  target tuple keeps it there). Verified in
  that release's source: `generic/bufread/decoder.rs` constructs
  `Decoder` with `multiple_members: false`, and on `State::Flushing`
  a finished member goes straight to `State::Done` unless the flag
  is set — so with the default, bytes after the FIRST member are
  never decoded and a split JSON document would silently truncate,
  violating this contract. The decoders expose
  `pub fn multiple_members(&mut self, enabled: bool)` (on the
  `futures::bufread` decoder types, delegating to the generic
  decoder); it takes `&mut self` and returns `()`, so it is a
  post-construction setter, NOT chainable off the constructor —
  bind the decoder `mut` and call
  `decoder.multiple_members(true);` before the first poll. This
  applies to GZIP only; brotli and deflate define no member
  sequence and stay member-agnostic per the contract above. The
  contract is unchanged — decode ALL members — and enabling the
  flag is what makes the implementation meet it; the member
  semantics are still confirmed against the resolved version as a
  task step rather than assumed — and that step is NOT a formality:
  the multi-member re-init path itself changed between 0.4.42 and
  0.4.43 (after `decoder.reinit()` the loop now sets
  `first = input.unwritten().is_empty()` where 0.4.42 set
  `first = true` unconditionally), so the empty-second-member and
  split-document fixtures below must be run against the version the
  lockfile actually resolves, not reasoned about from an older
  reading. Fixtures (as corrected by
  round-119 F7): valid-JSON + delayed transport error (fail); a JSON
  document SPLIT ACROSS two gzip members (SUCCEED) and an empty second
  member (succeed); corrupt and truncated second members (fail);
  non-member trailing junk (fail); cumulative decoded overflow across
  members (fail).
  (A passing status alone cannot prove bounded collection — the
  platform may have materialised the whole body before auctioneer's
  bounded checks run.) Real proof requires ONE of:
  1. **Adapter-level pull instrumentation.** Task M3b-2b's
     per-adapter bounded-collect impl exposes two test-only counters
     `crate::proxy::test_hooks::transport_bytes_pulled()` and
     `crate::proxy::test_hooks::decoded_bytes_emitted()`. Each
     regression case asserts:
     - `FanoutError::OversizeResponse { source: Transport | Decoded }` fires,
     - `transport_bytes_pulled() <= max_transport_bytes + MAX_CHUNK_SIZE`,
     - `decoded_bytes_emitted() <= max_decoded_bytes + MAX_CHUNK_SIZE`.
  2. **Isolated peak-allocation measurement WITH worst-case
     3-layer decoder pipeline.** Round-30 H6: byte counters
     (option 1) prove pull limits but do not prove peak
     allocation.

     Round-45 H3 canonical (retires the contradictory
     round-38/39 prose that both said "Axum only" AND
     "Axum/Fastly/Spin", and mislabelled CF `HostDecoded`).
     The allocator peak proof observes **auctioneer's own
     decoder pipeline** through a counting
     `#[global_allocator]`. auctioneer runs that decoder ONLY on
     the `RawCompressed` path; on `GuestDecoded` the decoder
     runs inside the adapter / EdgeZero, OUTSIDE auctioneer's
     allocator (the pinned CF adapter runs `flate2` +
     `brotli_decompressor` at `proxy.rs`, which auctioneer's
     allocator cannot see). So the scope is:
     - **Axum (native) — runs the allocator harness.** It is
       the one target where a process-global counting
       allocator is reliable, and it ships `RawCompressed`,
       so auctioneer's own 3-layer decoder executes under
       observation. The harness proves auctioneer-core's decoder
       peak ≤ `per_stream_bytes(..)` for the configured
       `max_brotli_wbits`. This is a property of
       **auctioneer-core's decoder**, independent of which
       adapter later selects `RawCompressed` at runtime.
     - **Cloudflare — GuestDecoded, no allocator harness.**
       EdgeZero decodes in-guest but outside auctioneer's
       allocator, so `#[global_allocator]` counting is
       blind to it. Its decoder bound is the Prereq 8
       `guest_decoder_bound_bytes` the adapter reports to
       `install_admission`, floor-checked there against
       `per_stream_bytes(0, 0, wbits, layers, 0)
       (round-134 #4 — the pure auctioneer-chain floor)`.
     - **Fastly / Spin — GuestDecoded at runtime, but ship
       `RawCompressed` too.** They rely on the Axum
       allocator proof for auctioneer's decoder (same
       auctioneer-core code) plus their own Prereq 8 bound when
       they negotiate `GuestDecoded`. No separate wasm
       allocator harness — V8 / host-process / Spin-body
       buffers are invisible to `#[global_allocator]`.
     - **Every adapter — transport proof (round-133 #1,
       BUFFERED).** Assert a completed `send_bounded(call)`
       delivered `body.len()` within the slot's governing cap
       (transport or decoded, per its disposition row), and that
       an over-cap upstream response surfaces as
       `ResponseTooLarge` WITHOUT the collect loop materializing
       bytes past the cap — observed via the adapter's
       collect-loop counters (the bounded-buffering contract:
       enforcement DURING the read, never collect-then-check).
       The former first-yield streaming observation is retired
       with the stream.

     Run the Axum allocator test in a dedicated `#[test]`
     binary with a counting global allocator installed for
     the process; drive a WORST-CASE 3-layer response
     (`Content-Encoding: br, gzip, deflate`) with each layer
     nearly full; use the canonical formula from Task M3b-7
     above (round-37 H3). Constants imported from
     `auctioneer_core::host_primitives::proxy_bounds`. Baseline
     is captured against a NO-response run — the earlier
     "identity response of `max_decoded_bytes`" shape
     conflated the baseline with a full decoded allocation,
     which the observation delta then re-added.

  Wire-format assertion (round-133 #1 REPLACES the round-37 B5
  stream-item shape): cap breaches arrive as the slot's
  `Err(EdgeError::ResponseTooLarge { reason: ResponseLimitReason })`
  (round-140 #3 — the real field) in the buffered
  results — never as stream items — and the round-130 mapping
  table classifies them to `FanoutError::OversizeResponse
  (OversizeOrigin)` from the error's explicit origin field (GAP
  G14).
  **G14 IS NOW IMPLEMENTED.** `FanoutError::OversizeResponse`
  carries `OversizeOrigin`
  (`auctioneer-exchange/src/fanout.rs:71`), a ten-variant enum, and
  `oversize_origin()` (`auctioneer-core/src/outbound/wave.rs:598`)
  has one explicit arm per upstream `ResponseLimitReason` rather
  than discarding it. `Unspecified` and any future upstream
  variant route to `Unclassified`, so a new reason cannot be
  folded into a named class it has no claim to. M3b-7's
  `CAP_CLASSES`
  (`auctioneer-core/tests/response_oversize_response.rs:127`) is now
  a discrimination table rather than a collapse-pinning fixture,
  and `every_origin_renders_the_same_wire_category()` (line 444)
  holds the wire contract fixed while the internal axis
  discriminates. `public_message()` reads no payload on any arm,
  so an origin cannot leak untrusted transport text to the wire.
  This paragraph previously recorded G14 as an unmade code
  change; that is no longer the case.
  natural place. DECISION NEEDED: propagate
  `ResponseLimitReason` into auctioneer's taxonomy, or restate the
  certification to claim only the collapsed classification.**
  There is no auctioneer-owned
  `send_one_bounded` helper and no `classify_body_stream_error` —
  the classification site is the per-slot result. Round-34
  B3's obsolete `ProxyClientError` type stays retired.

  Round-38 H3: the round-31 H4 "option (2) runs on EVERY
  adapter" claim is RETIRED. The allocator-based proof only
  makes sense on Axum (see the round-38 H3 note in option
  (2) above); Cloudflare / Fastly / Spin get the
  platform-transport proof instead. Option (1)
  (adapter-level pull counters) still runs on every
  adapter as supplemental evidence.

  Additionally, **`bounded_collect_stops_at_cap()`** (round-133
  #1; observation model corrected round-136 #7): the UNIVERSAL
  assertion is the adapter's own collect-loop counters — no read
  proceeds past the governing cap plus one chunk. The
  ORIGIN-OBSERVED teardown assertion (the upstream server sees
  the abort within one chunk of the cap crossing) runs **ONLY ON
  AXUM — round-176 #1 removes Cloudflare from this list.** Verified
  per adapter at head: `outbound-batch-cancellation` is Native ONLY
  on Axum; Cloudflare, Fastly and Spin are all BestEffort.
  Cloudflare DOES issue `AbortController::abort()`, but upstream has
  no deployed proof of finite host teardown, so demanding an
  origin-observed abort there asks for a guarantee EdgeZero does not
  make — the same error round-174/175 corrected in M3b-3 and the
  master spec, surviving here.
  **WHAT EACH TARGET ASSERTS INSTEAD:**
  - **Axum (Native):** the full origin-observed teardown assertion.
  - **Cloudflare (BestEffort):** that cancellation was REQUESTED
    (the abort call is made), plus the auctioneer-owned outcomes — no
    late publication, wave lease released, timeout provenance
    retained. Never origin-side termination.
  - **Fastly and Spin (BestEffort):** characterization only, exactly
    as before.
  It is explicitly SKIPPED on Fastly, whose documented
  cancellation is best-effort (requiring every origin to observe
  teardown was unprovable there). On Axum the test additionally
  reads the allocator peak to bound the Rust side.

  **EXECUTABLE SHAPE (round-177 #1):**
  - **Blocked by:** 2b.2 and M3b-2b's decoder integration.
  - Files: Create `crates/auctioneer-core/tests/response_oversize_response.rs`
    plus fixtures for the four cap classes — encoded over cap,
    decoded over cap, header bytes over cap, header count over cap.
  - Step 1: each fixture asserts the mapped `FanoutError` (per the
    round-130/147 table), the per-bidder code-4 entry, seatnonbid
    100, and that the oversize bidder is dropped while its siblings
    publish normally. Step 2: failure. Step 3: fixtures only, as
    M3b-6. Step 4: PASS. Step 5: commit `auctioneer-core: snapshot fixture
    — oversize response, four cap classes`.

- [ ] **Task M3b-8 — Behavior-complete workspace gate +
  adapter-level integration matrix (round-41 H4).**
  Extend Task 23's clippy/test matrix so Bucket B is no
  longer `#[ignore]`d on the adapters M3b activates.
  Passing this gate is M3 "behavior-complete" per spec
  §14; M3a was "dev-complete".
  **Round-95 H2 — EXECUTION ORDER: this gate task runs LAST among the
  behavior-complete tasks, AFTER Task M3b-9.** Behavior-complete =
  M3a + M3b-1..9 **+ M3b-0 + M3b-11 (SELF-REVIEW FINDING 3 — those
  two were added in rounds 161 and 154, after this formula was
  written, and appeared nowhere in this gate. They are NOT optional:
  M3b-0 rebases the outbound contract off a REMOVED upstream method
  and M3b-11 moves the pin that makes the batch API exist, so a gate
  run without them could certify "behavior-complete" while the
  outbound path still targets `OutboundHttpClient::send_all`, which
  does not exist at head. M3b-10 remains excluded, as every
  "M3b-1..9 stay behavior-complete without it" statement in the
  master spec says — that exclusion is unaffected.)** Step 0 of this
  gate therefore also asserts: the M3b-0 drift gate
  (`scripts/check-batch-rebase.sh`) is green, `Cargo.lock` pins
  the revision M3b-11 moved to, and **the test roster's arithmetic
  matches its enumeration** (self-review 2026-09-17: the head derives
  +175 while the bullets enumerate 80 with templates expanded — this
  gate counts the roster, so it cannot certify a total it cannot
  enumerate. Either the missing bullets are recovered from the rounds
  that introduced them or the total is corrected downward; a
  mismatch fails Step 0). A gate executed before M3b-9's runtime
  host-alias validation lands would certify an incomplete milestone
  (M3b-9's +1 test is even counted in this gate's row). Task NUMBERS
  are stable identifiers, not execution order: implement M3b-9 first,
  then run this gate; the gate's Step-0 checklist asserts M3b-9's
  `runtime_host_alias_unknown_base_returns_malformed_host` test is
  present (round-125 #14: M3b-9's actual name; the stale spelling
  this line carried is retired everywhere, not only in the roster
  note) and
  green before the milestone flip (this is also the commit that flips
  `auctioneer_core::milestone::MILESTONE` to `BehaviorComplete` + updates
  its pinning test, per M3a Task 23 Step 0).

  **Round-41 H4 adapter CI gates.** Ordinary
  `cargo test --workspace` cannot exercise CF/Fastly/Spin
  streaming, cancellation, compressed-body limits, or
  final artifact size. Task M3b-8 lands these adapter-
  level integration gates in `.github/workflows/`:

  - **`ci-cloudflare.yml`** — installs `wrangler`, runs
    `wrangler deploy --dry-run --outdir out/cf` against
    `crates/auctioneer-adapter-cloudflare`. Asserts (round-43
    H4 canonical — retires round-42's `du -sb out/cf`,
    which measured an uncompressed directory that may
    include files wrangler never uploads and could not
    see the gzip figure the 10 MB limit actually applies
    to):
    - **Parse wrangler's own report.** `wrangler deploy
      --dry-run` prints
      `Total Upload: <X> KiB / gzip: <Y> KiB`. The gate
      extracts both numbers from stdout.
    - **Compressed:** `Y` ≤ **9,000,000 bytes**, under
      CF's documented 10 MB *after compression* ceiling
      (3 MB on free plan — the target plan comes from a
      CI variable).
    - **Uncompressed:** `X` ≤ **60,000,000 bytes**,
      under CF's documented 64 MB *before compression*
      ceiling, which covers the wasm module + data
      segments where the GVL blobs live.
    - Round-43 H4: both are DECIMAL MB per the
      Cloudflare docs. The plan's earlier binary-MiB
      constants (`10 * 1024 * 1024 = 10_485_760`)
      exceeded the real 10,000,000-byte limit and would
      have passed CI while failing deploy.
    - Spawns `wrangler dev` with a test worker script
      that drives a bounded fixture through the compiled
      artifact; asserts the response body-cap fires at
      `max_transport_bytes + MAX_CHUNK_SIZE`.
    - **Cancellation:** the CF cancellation gate is
      REPLACED (round-42 H4). Killing the wrangler dev
      process doesn't exercise `AbortController` — it
      just terminates the whole runtime. Instead the test
      drives a specifically-crafted fixture that returns
      a body slower than `deadline`; asserts the guest
      never reads past the deadline (measures observed
      byte count vs elapsed time) as an indirect signal
      that `AbortController.abort()` was invoked. This is
      still not a direct `AbortController` unit test —
      the true test lives inside the pinned EdgeZero
      adapter's cancellation regression, not this CI
      gate.

  - **`ci-fastly.yml`** — installs the `fastly` CLI,
    runs `fastly compute build --package-name auctioneer` on
    `crates/auctioneer-adapter-fastly`. Asserts (round-42 H4
    canonical, retires round-41's incorrect 50 MB and
    `always_on_state` claims):
    - Built package fits under Fastly's **100 MB**
      documented compiled-package ceiling.
    - Spawns `fastly compute serve` with a fixture body
      that exceeds `max_transport_bytes`; asserts the
      1-permit admission and body-cap firing.
    - Fresh-sandbox lifecycle is the default; deployers
      who need reusable sandboxes switch entrypoint from
      `#[fastly::main]` (fresh) to the Rust Serve API
      (`fastly::main::serve`). No `fastly.toml` flag
      exists.

  - **`ci-spin.yml`** — installs `spin`, runs
    `spin build` on `crates/auctioneer-adapter-spin`.
    **Round-95 M3 — the size assertion is exact, not "asserts
    artifact size":** measure the built component the manifest
    points at —
    `wc -c < target/wasm32-wasip2/release/auctioneer_adapter_spin.wasm`
    (round-155 #8: `wasm32-wasip2` — the M3b-4 target move; this
    measurement runs AFTER that unit lands)
    — and assert the byte count is **≤ 50,000,000 bytes (50 MB
    decimal)**. Rationale: Fermyon Cloud (the assumed managed Spin
    target) documents a **100 MB application-package quota**, and the
    uncompressed component wasm dominates the package; a 50 MB
    project budget leaves 2× headroom the same way the CF gate's
    9,000,000-byte figure does against the 10 MB ceiling. The gate
    prints the measured figure either way. Then the `spin up`
    bounded-body drive matching the CF/Fastly shapes.

  Each workflow uploads its adapter's build-artifact
  size to the PR check runs so an unexpected artifact
  bloat is visible in review. The workflows use
  vendor-supplied test tokens where possible; when they
  need actual deploys (rare, gated by a `full-deploy`
  label on the PR), they use ephemeral test accounts.

  **Round-41 H4 test count:** +3 CI workflow files, +1
  workspace-gate test that verifies all three CI
  workflows exist (`ci_adapter_workflows_present()` —
  reads `.github/workflows/` and asserts the three
  filenames). Round-124 #12 — roster synchronization: M3b-8's
  expected gate-test name is
  `runtime_host_alias_unknown_base_returns_malformed_host` (M3b-9's
  actual name; the stale `runtime_alias_unknown_base_fails_validation`
  spelling is retired). The roster additionally absorbs the rounds
  122-124 tests: `acquire_wave_all_or_nothing_no_partial_hold()`,
  `acquire_wave_cancellation_releases_nothing()`,
  `ingress_header_bytes_cap_431()`, `ingress_header_count_cap_431()`,
  `impression_limit_boundary_at_and_over()`,
  `offset_table_fits_all_limit_imps()`,
  `static_egress_charged_in_total()`,
  `aux_route_write_deadline_per_route()` (x4 routes), and
  `batch_deadline_report_with_live_auction_is_bidder_timeout_not_code_12()`.

  **⚠ ROSTER RECONCILED AGAINST THE TREE. Eight of these nine names do
  not exist as written — the BEHAVIOURS do, under different names. A
  gate asserting these literally fails on all eight, which is what
  M3b-8 found. The mapping, verified by grep:**

  | roster name | as built |
  |---|---|
  | `acquire_wave_all_or_nothing_no_partial_hold` | `stream_gate_grant_is_atomic_no_partial_hold` (`auctioneer-runtime/src/admission.rs`) |
  | `acquire_wave_cancellation_releases_nothing` | `stream_gate_cancel_before_grant_releases_nothing`, plus `stream_gate_cancel_after_grant_releases_all_k` |
  | `impression_limit_boundary_at_and_over` | `prepare_rejects_over_limit_impressions_before_allocation` (`auctioneer-openrtb`) |
  | `static_egress_charged_in_total` | the term is `terminal_term`, summed into the total in `validate_isolate_limits`; no separately-named test |
  | `offset_table_fits_all_limit_imps` | covered by the `auctioneer-bidder` meta/offset-table tests; no test of this name |
  | `aux_route_write_deadline_per_route` (x4) | the deadline lives in `handlers/aux_egress.rs`; no per-route test of this name |
  | `ingress_header_bytes_cap_431`, `ingress_header_count_cap_431` | the pinned bodies are asserted by `the_app_renders_the_pinned_414_and_431_bodies` |
  | `batch_deadline_report_with_live_auction_is_bidder_timeout_not_code_12` | **PRESENT** — the one name that resolves |

  **This is naming drift, not missing coverage, EXCEPT for two real
  gaps: `static_egress_charged_in_total` and
  `aux_route_write_deadline_per_route` name behaviours that exist in the
  code but have no test asserting them by any name. Those two are
  coverage findings; the other six are renames. M3b-8 must assert the
  as-built names, and the two gaps need tests or an explicit waiver
  before the gate can certify this roster.**

  **EXECUTABLE SHAPE (round-177 #1 — the gate's own steps, distinct
  from the criteria it enforces):**
  - **Blocked by:** everything above it in the dispatch table.
  - Files: **Create the three per-platform workflows this task's own
    requirements specify — BY THEIR REQUIRED NAMES AND WITH THEIR
    REQUIRED CONTENT (round-179 #3: my round-178 entry invented
    `adapter-*.yml` names running only `cargo check` and
    `--no-run`, which is strictly weaker than the requirements above
    and could have flipped `BehaviorComplete` without a single vendor
    build ever running):** `.github/workflows/ci-cloudflare.yml`
    (installs `wrangler`; vendor build; artifact-size gate; runtime
    body-cap check; cancellation/deadline probe),
    `.github/workflows/ci-fastly.yml` (installs the `fastly` CLI;
    same four classes), `.github/workflows/ci-spin.yml` (installs
    `spin`; same four classes) — each exactly as the requirements
    section above enumerates, PLUS the compile-only rows this
    repository can run today (`cargo check` and `cargo test --no-run`
    for the target triple, round-178 #2) and the target's
    `capability_table_matches_<target>()` row.
    **⚠ ROUND-180 #5 — TWO TIERS, NOT ONE. My round-179 entry said
    every unrunnable probe must "FAIL LOUDLY", which would make
    ordinary PR CI structurally unable to reach
    `BehaviorComplete`: the requirements themselves say actual
    deployment is rare and full-deploy gated. Split them:**
    - **REQUIRED FOR BEHAVIOR-COMPLETE (every PR):** vendor toolchain
      build (`wrangler`/`fastly`/`spin` build succeeds), the
      artifact-size gate, `cargo check` + `cargo test --no-run` for
      the target triple, and the capability-equality row. All run
      without deploying anything.
    - **ALSO REQUIRED PER PR — THE LOCAL VENDOR RUNTIMES
      (round-181 #5 corrects an over-broad deferral).** My round-180
      split moved EVERY runtime probe to promotion-only, which would
      let `BehaviorComplete` certify code that merely BUILDS. The
      requirements name locally runnable runtimes — `wrangler dev`,
      `fastly compute serve`, `spin up` — and those need no
      credentials and no deployment. The body-cap check and the
      cancellation/deadline probes RUN THERE, in ordinary CI.
    - **PROMOTION-ONLY (not a behavior-complete gate):** only the
      probes that genuinely require a CREDENTIALED DEPLOYED
      environment — a real edge POP, real origin observation. These
      are SKIPPED with an explicit recorded reason in ordinary CI and
      REQUIRED in the promotion job. A skip must be visible in the
      run summary — silent absence is what this split exists to
      prevent;
    Create `crates/auctioneer-core/tests/ci_workflows.rs` with
    `ci_adapter_workflows_present()`, which the requirements already
    name and which asserts all three files exist and reference their
    target triple; Modify `.github/workflows/test.yml` (un-ignore the
    Bucket B rows on all four adapters); Modify
    `crates/auctioneer-core/src/milestone.rs` (flip `MILESTONE` to
    `BehaviorComplete`) and its pinning test.
  - **⚠ ROUND-184 #1/#8 — THIS GATE RUNS AFTER M3b-11, WHICH NOW
    CARRIES THE LIFECYCLE ADOPTION.** Round-183 #6 ordered this gate
    after a separate M3b-12 because M3b-11's pin (`4790a140`)
    implemented none of G18–G22. Upstream then landed them on the
    same branch, so there is ONE pin (`6904869b`) and one task; the
    ordering requirement is unchanged in substance — the gate cannot
    flip `BehaviorComplete` until the contracts it certifies exist in
    the pinned revision — but it is satisfied by M3b-11 alone.
  - **✅ `/status` — THE TWO-CONTRACT CONFLICT IS RESOLVED (D-1,
    round 185; this block is retained because it records WHY, and the
    history is what stops the conflict being reintroduced).** The
    master spec and this plan both required malformed runtime config
    to answer 500. The SHIPPED handler deliberately does the opposite
    (`crates/auctioneer-core/src/handlers/status.rs`): it resolves the
    config on its PERMISSIVE disposition and degrades to defaults,
    and the shipped tests PIN that —
    `corrupt_blob_keeps_status_answering()` asserts the route does
    NOT return 500. So a gate run today would either fail a green
    suite or certify a documented requirement the code does not meet.
    **THE RESOLUTION TAKEN: the code's DISPOSITION is right, but
    neither side's REPORTING was.** `/status` is a liveness probe that reaches no privacy
    decision; failing it closed converts a config-store outage into a
    fleet-wide restart loop that cannot fix the store, which is
    strictly worse than answering from defaults. The fail-closed 500
    rule belongs to the routes that make privacy or auction
    decisions — where it already applies and must not weaken. If that
    is accepted, the spec's `/status` row and this plan's repetition
    of it are amended to "PERMISSIVE: degrades to baked defaults,
    never 500", with the rationale above recorded beside it. **This
    is a decision for the spec owner, not something this task may
    settle by editing one side to match the other.**
    **✅ ROUND-185 — D-1 IS RESOLVED, AND THE ANSWER IS NEITHER
    BRANCH.** The spec owner's decision (2026-09-19): **`/status`
    keeps answering and REPORTS WHAT IS WRONG.** The framing above
    treated the status code as the only channel, so it offered a
    false choice — either 500 through an outage, or a green probe
    that hides a broken deploy. Both sacrifice something avoidable.
    The resolution: `/status` stays 200 (availability preserved, no
    restart loop) and its BODY names the fault (signal restored).
    The permissive disposition in `handlers/status.rs` is CORRECT and
    stays; what changes is that the error is no longer DISCARDED.
    Today `resolve_runtime_config(&ctx).await.unwrap_or_else(
    RuntimeConfigError::degrade_to_default)` throws away a
    `RuntimeConfigError` that already carries a curated `detail`
    string. **M3b-1c owns the change** (task body below). The spec's
    `/status` row is amended to the reporting contract, and
    `corrupt_blob_keeps_status_answering()` KEEPS passing — the route
    still does not 500 — which is why this resolution needs no
    change to the fail-closed rule on privacy- and auction-deciding
    routes. **M3b-8 Step 0 no longer waits on a decision;** it waits
    on M3b-1c landing, like any other task.
  - Step 0 (the preconditions, each a hard stop): **M3b-1c has
    landed, so the `/status` reporting contract, the spec row and the
    code agree (D-1, resolved round 185 — this precondition was "the
    contract conflict is RESOLVED", a decision; it is now a task);** M3b-9's
    `runtime_host_alias_unknown_base_returns_malformed_host` green;
    the M3b-0 drift gate green; `Cargo.lock` pinned at or after the
    revision M3b-11 moved to (`6904869b`) — the tree is currently at
    the later descendant `ab333f4e`, which carries M3b-11's contracts
    plus the published platform ceilings, so a pin PAST `6904869b`
    satisfies this precondition rather than failing it; and the
    roster's arithmetic matching its enumeration (round-167 #1).
  - Step 1: run the full matrix with Bucket B un-ignored — expect
    failures wherever an adapter task left a row red. Step 2: fix
    NOTHING here; a red row is a defect in its owning task and goes
    back there. Step 3: with the matrix green, flip the milestone
    constant. Step 4: `cargo test --workspace` plus all four target
    checks green. Step 5: commit `auctioneer: behavior-complete — M3b
    gate green on all four adapters`.

- [x] **Task M3b-9 — Runtime host-alias registry validation.
  (COMPLETE — commit e414a1e; TDD red→green, both tests +
  fmt/clippy/full auctioneer-core suite green; ledger entry in
  docs/superpowers/sdd/. Round-141 #5: checkbox ticked at the
  reviewer's direction — for THIS plan, completion marks live in
  both the ledger and here.)**
  Round-30 M4: M3a's Task 18 Step 1 moved unknown-alias-base
  rejection to `build.rs` on the baked
  `data/host-defaults.toml`, but the runtime-overlay path
  (`[aliases]` under a config-store `AuctioneerAppConfig`) has no
  equivalent check. M3a intentionally skipped it — the
  round-29 H4 fix expected M3b's `StoreAppConfigError::Validation`
  to cover unknown runtime bases through the
  generic validator layer. That doesn't work: the generic
  validator can't know about the bidder registry, and
  auctioneer's `[aliases]` field on `AuctioneerAppConfig` is a plain
  `BTreeMap<String, String>` with no `validator::Validate`
  attribute checking the base against a registry.

  Round-31 M3 correction: the runtime overlay field lives at
  the ROOT of `AuctioneerAppConfig`
  (`crates/auctioneer-core/src/config/mod.rs::AuctioneerAppConfig::
  aliases: BTreeMap<String, String>`), NOT under
  `host.aliases`. The earlier draft's `cfg.host.aliases`
  reference didn't exist. Also, calling
  `crate::handlers::auction::registry_bidder_list()` from
  inside `crates/auctioneer-core/src/config/runtime.rs` reverses
  the layer topology (config-layer code should not depend on
  handler-layer symbols). Correct shape uses a shared
  `crate::bidder_registry::live_names() -> impl
  Iterator<Item = &BidderName>` service that both the
  handler and the config helper import.

  Files:
  - Modify: `crates/auctioneer-core/src/bidder_registry.rs` (new
    module) — extract the registry singleton + `live_names()`
    accessor from `handlers::auction`. Both the auction
    handler and the runtime-config helper import from here.
  - Modify: `crates/auctioneer-core/src/config/runtime.rs::
    resolve_runtime_config` — after `AppConfig::from_store`
    returns the parsed `AuctioneerAppConfig`, iterate
    `cfg.aliases` (round-31 M3 — root-level field on
    `AuctioneerAppConfig`, not `host.aliases`) and check every
    base against `crate::bidder_registry::live_names()`. A
    base absent from the registry (even if disabled —
    disabled bases stay valid per Task 18) →
    `Err(RuntimeConfigError::MalformedHost { detail:
    format!("host alias {alias:?} references unknown base
    {base:?}") })`.
  - Add: `runtime_host_alias_unknown_base_returns_malformed_host()`
    regression in `crates/auctioneer-core/tests/
    resolve_runtime_config.rs`. Simulates a store returning a
    valid-shape TOML whose root `[aliases]` references
    `nonexistent-bidder`; asserts the helper returns
    `Err(RuntimeConfigError::MalformedHost { .. })` and the
    shared response mapper emits 500 `Malformed host
    configuration`.

  Test count: +1 (adds to Task M3b-8's gate row).

- [ ] **Task M3b-10 — Per-route EdgeZero ingress hook + adapter wiring
  (PRODUCTION-COMPLETION owner; round-142 #4 re-verification
  REPLACES round-139 #5's: the candidate head `120dd2aa` PROVIDES
  string route classes, `RouteResolution`, `IngressHead`,
  `AdmissionDecision`, `IngressGrant`, and pre-body admission
  wiring in ALL FOUR adapters — the `Decision` / `FramingState` /
  numeric-class vocabulary in this task's body is design-era and
  must be rewritten against those real types before expansion.
  What REMAINS upstream-blocked is exactly the explicitly
  unsupported RAW-INGRESS capabilities (strict raw-boundary framing
  rejection, parser header/request-target caps) and RESPONSE-EGRESS
  completion (Prereq 9) — so this task's PRODUCTION gate stands on
  those two, not on admission wiring).** Round-70 H1 / round-71: the spec's
  production-complete boundary (§14) + open item 14 require the
  per-route ingress policy hook so each adapter enforces the route
  table BEFORE `into_core_request` materializes the body. Nothing owned
  it, so as-scoped M3a + M3b-1..9 reach only behavior-complete. **This
  task is the CANONICAL owner (round-71 H2 — it supersedes the spec's
  stale "registration ships in M2" note at §13 and M3a's "M4/M5 owns
  wiring" deferral; both are corrected to point here).** It is
  **upstream-gated** on EdgeZero Prereqs 5 AND 10 for IMPLEMENTATION
  (round-125 #10 — the ingress contract spans both: framing/hook from
  5, admission lease + start capture + read deadline from 10),
  **plus Prereq 9 for PRODUCTION CERTIFICATION** (round-165 #6 — the
  status table said "5 + 9 + 10" while this body said "5 and 10";
  both are right about different things, and the distinction is now
  explicit: 5 + 10 must land before this task can be IMPLEMENTED at
  all; 9's response-egress completion must land before the result can
  be CERTIFIED for production. Neither set blocks behavior-complete
  fanout) and is a
  PRODUCTION-release gate, NOT part of the behavior-complete gate
  (Task M3b-8). TEST OWNERSHIP (round-129 #7): the authoritative
  roster's two inbound parser-cap cases —
  `inbound_h1_header_overflow_writes_431_and_closes()` and
  `inbound_h2_field_section_breach_resets_stream()` — land HERE,
  since they exercise Prereq 10 and the connection-driver
  interception seam — joined (round-130 #6) by the two exact-boundary
  knob cases,
  `inbound_h1_header_count_boundary_admits_128_rejects_129()` and
  `inbound_h2_field_section_boundary_admits_21632_rejects_21633()`,
  and (round-131 #6) by
  `inbound_h1_buf_size_boundary_admits_cap_rejects_over()` and
  `inbound_h2_pseudoheader_bounds_method_scheme_connect_rejected()`,
  and (round-136 #4) by
  `manifest_class_semantics_match_expected_map()`
  (seven M3b-10 roster members in all); and this task's own deferred
  test roster (+N)
  MUST be expanded and re-reviewed against the real EdgeZero
  signatures once Prereqs 5/10 land, per the expansion protocol at
  the top of the Tasks section.

  **Hook shape (spec open item 14). Round-75 H1 — the protocol types
  (`Decision`, `FramingState`, `Rejection`, `BoundedReadPolicy`,
  `OnSuccess`) are EdgeZero-OWNED, defined in `edgezero-core`, NOT in
  auctioneer.** The adapter must interpret the returned `Decision` (and
  produce the `FramingState`) BEFORE `into_core_request` / router
  dispatch, and EdgeZero's `Hooks`/`App` API is what invokes the hook —
  so these are EdgeZero's protocol vocabulary. auctioneer IMPLEMENTS only
  the callback body (`decide`) against those types; it does not define
  them. **Round-77 H1 — the EdgeZero callback signature must not name a
  auctioneer-owned type either:** `RouteTable` lives in
  `auctioneer-runtime` — and `RegisteredRoutes` is auctioneer-core-owned,
  adapter-CONSTRUCTED (round-134 #1) — so NEITHER can appear in the
  EdgeZero-accepted closure type; auctioneer's `decide` takes the
  `RegisteredRoutes` as its extra param and the
  adapter passes it via a capturing closure (see the two-contract split
  below). The shapes below are what auctioneer ASKS Prereq 5 to expose
  (shown here so the callback body is unambiguous), not auctioneer-owned
  declarations:
  ```rust
  // TWO SEPARATE contracts (round-77 H1 — the EdgeZero callback type must
  // NOT name any auctioneer-owned type; `RouteTable` lives in `auctioneer-runtime`
  // and EdgeZero cannot depend on it).
  //
  // (1) EdgeZero-OWNED callback signature (edgezero-core, Prereq 5). This
  //     is the type EdgeZero's `Hooks`/`App` API accepts — a plain
  //     closure over EdgeZero/`http` types only, no auctioneer vocabulary:
  //         Fn(&Method, &str, &HeaderMap, &FramingState,
  //            &RouteResolution) -> Decision
  //     (round-132 #2 / round-134 #1 — the FIFTH parameter is
  //      EdgeZero-owned; a bare Option<RouteId> conflated 404 with
  //      405 and forced decide to re-match, so the resolution is
  //      explicit:
  //          enum RouteResolution {
  //              Matched(RouteId),
  //              MethodNotAllowed { allowed: SmallVec<[Method; 4]> },
  //              NotFound,
  //          }
  //      `RouteId` is issued at registration; `allowed` is what
  //      decide's 405 arm names in the RFC 9110 `Allow` header —
  //      the routing knowledge arrives IN the resolution, so the
  //      no-auctioneer-types rule holds and nothing re-matches.)
  //
  // (2) auctioneer-OWNED implementation (auctioneer-core::ingress). It needs the
  //     milestone's route table, which is auctioneer's, so it takes it as an
  //     extra param:
  fn decide(
      method: &Method,
      path: &str,
      headers: &HeaderMap,
      framing: &FramingState,   // round-74 H1 — normalized by the adapter
      resolution: &RouteResolution, // round-134 #1 — Matched(id) is
                                // resolved through routes.by_id;
                                // MethodNotAllowed { allowed } feeds
                                // the 405 arm's Allow header;
                                // NotFound feeds the 404 arm — no
                                // re-matching anywhere
      routes: &RegisteredRoutes,   // round-133 #2 — the boot-built zip
                                   // of edgezero.toml handler wiring +
                                   // RouteTable policy rows, carrying
                                   // the issued RouteId → RouteMatch
                                   // map `decide` resolves `matched`
                                   // through; EdgeZero never sees the
                                   // type
  ) -> Decision;
  //
  // The adapter BRIDGES the two: it registers an EdgeZero closure of
  // shape (1) that CAPTURES the `RegisteredRoutes` and calls `decide` of shape
  // (2). EdgeZero only ever sees the shape-(1) closure; `RouteTable`
  // stays entirely on auctioneer's side of the boundary:
  //     let routes = registered_routes.clone(); // boot-built zip (auctioneer side)
  //     edgezero_hooks.ingress(move |method, path, headers, framing, resolution| {
  //         auctioneer_core::ingress::decide(method, path, headers, framing,
  //                                      resolution, &routes)
  //     });

  // Round-74 H1 — the adapter (which owns the HTTP parser) hands the
  // hook a NORMALIZED framing determination, because Hyper/the runtime
  // has already consumed the raw framing headers before the hook runs
  // (see step 1b). The `u64` Content-Length parse lives on the adapter
  // side, so it is portable across 32-bit wasm.
  enum FramingState { Length(u64), Chunked, None, Malformed }

  // Round-72 B2 — the round-71 `Continue { bounded_read: Option<usize> }`
  // shape could NOT implement the contract: when the adapter's bounded
  // read crosses the cap it must know the ROUTE-SPECIFIC status+body
  // (the auction / cookie_sync / 4 KiB bodies differ), and for a
  // chunked/lengthless request on the MethodNotAllowed / NotFound arms
  // it must return 405 / 404 AFTER a successful (within-cap) read, which
  // a bare `Option<usize>` cannot carry. The richer result:
  // Round-127 #2 — Decision is UNCHANGED by the admission lease:
  // admission is the SEPARATE Prereq-10 pre-decide callback, its
  // grant travels in the EdgeZero-owned move-safe `IngressGrant`
  // container (`ctx.take_ingress_grant()`), and no Decision variant
  // carries or names a guard. The earlier `IngressDecision::Admit
  // { guard }`-in-Extensions sketch is retired.
  enum Decision {
      /// Reject BEFORE reading any body: bad Content-Encoding, OR a
      /// known Content-Length over cap, OR a wrong-method/unmatched arm
      /// whose Content-Length is already within cap (so no read is
      /// needed → an immediate 405 / 404).
      Reject(Rejection),
      /// Dispatch to the handler immediately (a Matched arm whose
      /// Content-Length is present and <= cap; the body read is bounded
      /// by Content-Length, and the handler's layer-3 read backstops it).
      /// Round-118 F08: this branch materializes a body too — the
      /// Prereq-10 lease + start capture precede EVERY materialization
      /// branch, Dispatch included, not only BoundedRead; gate
      /// selection (auction vs aux) is by route class before `decide`
      /// runs, and aux-overload still precedes every other rejection.
      /// Round-119 F3: the FALLBACK arms are a gate class too —
      /// unmatched-route and wrong-method requests whose rejection
      /// needs a bounded read (chunked/lengthless) run that read under
      /// the AUX gate (they are small, capped reads; auction capacity
      /// is never spent on a 404). A saturated aux gate refuses them
      /// via the static class with ZERO body bytes read, same
      /// precedence as every other aux refusal. Fixtures: concurrent
      /// unmatched + wrong-method chunked requests at aux saturation
      /// (zero reads observed); admitted ones keep the 4 KiB cap and
      /// exact 404/405 semantics; timeout/disconnect recover capacity.
      /// Saturated-gate fixtures cover known-length, chunked, AND
      /// lengthless paths, plus saturated aux routes with bad encoding
      /// or oversize bodies.
      Dispatch,
      /// Content-Length is MISSING or `Transfer-Encoding: chunked`:
      /// EdgeZero performs a bounded read and applies this policy.
      /// Round-117 P0-1: the bounded read runs ONLY under an
      /// already-held admission lease with `request_start` already
      /// captured (Prereq 10) — a gate-full request is rejected
      /// before this read buffers a single byte, and upload time
      /// spends the request's own tmax, not unaccounted wall time.
      BoundedRead(BoundedReadPolicy),
  }
  // Round-118 F09: rejection bodies are BOUNDED-DYNAMIC, not the
  // static-class 'static slices — a request-derived token (e.g. the
  // first Content-Encoding value) is clipped at 64 bytes with a `…`
  // marker BEFORE rendering, the whole body renders into the fixed
  // 256-byte buffer a static-egress permit reserves, and the wire
  // contracts that quoted request-derived tokens are so clipped (an
  // "exact wire shape" containing attacker-length tokens was
  // unsatisfiable). Round-136 #5 REPLACES round-118 F16's 'static
  // headers list: the 405 Allow set is RUNTIME data
  // (RouteResolution::MethodNotAllowed carries it), so a 'static
  // slice could never name it — the field is typed and bounded, and
  // EDGEZERO renders the header from it.
  struct Rejection {
      status: u16,
      body: BoundedRejectionBody,      // renders into the 256 B permit buffer
      allow: Option<SmallVec<[Method; 4]>>, // Some ⇒ EdgeZero emits
                                       // `Allow:` from it (405 arm:
                                       // moved straight from the
                                       // resolution's allowed set);
                                       // None ⇒ no Allow header
  }
  // Round-118 F07 — framing failures are TWO classes with distinct
  // owners: (a) header-time ambiguity (CL+TE together, unknown TE,
  // method/route) rejects BEFORE the hook per the decision table;
  // (b) read-time framing failures (malformed second chunk, missing
  // terminator, bad trailer — knowable only mid-body, RFC 9112 §7.1)
  // occur INSIDE the admitted bounded read: the adapter answers 400
  // through the static-egress class, never dispatches the handler,
  // and tears the connection down (framing state is unrecoverable).
  // Precedence on simultaneous failure: first condition HIT wins
  // (overflow / ingress deadline / framing, in observation order).
  // Fixtures must include LATE-malformed chunks, not only malformed
  // headers.
  struct BoundedReadPolicy {
      /// The route cap. The read returns `on_exceeded` the instant it
      /// crosses this many bytes, instead of `to_bytes(_, usize::MAX)`.
      max_bytes: usize,
      /// Exact rejection when the read exceeds `max_bytes` — always a
      /// 400 with the ROUTE-SPECIFIC oversize body.
      on_exceeded: Rejection,
      /// What to do once the body is read WITHIN the cap.
      on_success: OnSuccess,
  }
  enum OnSuccess {
      /// Matched arm — hand the buffered body to the handler.
      Dispatch,
      /// MethodNotAllowed / NotFound arm — the read was only to enforce
      /// the cap; return this 405 / 404 instead of dispatching.
      /// Round-118 F16: `Rejection` carries `headers` so this arm can
      /// satisfy RFC 9110 §15.5.6 — a 405 MUST name `Allow` (e.g.
      /// `Allow: POST` for GET /openrtb2/auction) — without routing
      /// knowledge leaking into the adapter.
      Reject(Rejection),
  }
  ```
  The hook OWNS route classification (`body_caps::match_route`
  internally), Content-Encoding rejection, the Content-Length
  pre-check, the bounded-read policy, and the unmatched/wrong-method
  status — and every rejection it can produce carries its EXACT
  status+body, so no downstream layer has to re-derive them.

  **Complete decision table (round-71 H1 — spec §12/13 lines 715-801 +
  open item 14). Evaluate in THIS order; the FIRST firing arm
  short-circuits the rest:**
  1. **Content-Encoding rejection** — uniform across ALL three
     `match_route` arms (Matched, MethodNotAllowed, NotFound). A
     non-empty `Content-Encoding` header → `Reject(400, "Invalid
     request: Content-Encoding of type <encoding> is not supported\n")`
     (Appendix F canonical body, trailing `\n`). This WINS over the
     route/method status mapping: `POST /missing` with
     `Content-Encoding: gzip` is 400 with THIS body, never 404.
     **Executable extraction rule (round-78 M1 — the earlier "non-empty
     → reject" under-specified repeated / multi-token / empty-first /
     malformed headers):**
     - **Iterate `headers.get_all("content-encoding")`** (ALL header
       lines, not `.get()` which returns only the first), and within
       each line split on `,` into tokens. This covers both a repeated
       header (`Content-Encoding: gzip` twice) and a single comma-joined
       line (`Content-Encoding: gzip, br`).
     - **`.to_str()` failure (non-UTF-8 / malformed header bytes) → REJECT**
       — a `Content-Encoding` value that is not valid header text is
       treated as an unsupported encoding, not ignored. Use the literal
       `"(unparsable)"` as `<encoding>` in the body for this case.
     - **Trim ASCII whitespace from each token. Every NON-EMPTY coding
       rejects — INCLUDING `identity` (round-79 H1 — `identity` is not
       special-cased; the canonical spec rejects every non-empty value).**
       Only genuinely empty/whitespace-only tokens are skipped. The
       `<encoding>` in the body is the FIRST non-empty token, post-trim,
       original case (no normalization / joining) — first coding wins,
       matching upstream's "first offending coding" message shape.
     - **Delimiter-only vs truly-empty (round-79 H1):** a header whose
       value is entirely empty or whitespace (no coding, no `,`) — e.g.
       `Content-Encoding: ` alone — does NOT reject (falls through to the
       route/framing arms). But a value that is present-yet-coding-less
       because it is delimiter-only, e.g. `Content-Encoding: ,` or
       `Content-Encoding: , ` — has a `,` but yields no non-empty token —
       is MALFORMED and rejects as `<encoding> = (unparsable)`. Rule: after
       scanning all lines/tokens, if any non-empty token was seen → reject
       on the first; else if any line contained a `,` (delimiter present,
       zero codings) → reject `(unparsable)`; else (no coding, no
       delimiter) → no rejection.
     - Tests (Axum E2E + `decide` contract): (i) single `gzip` →
       `<encoding> = gzip`; (ii) single `identity` → reject
       `<encoding> = identity`; (iii) REPEATED `Content-Encoding: identity`
       then `gzip` → reject `identity` (first coding wins); (iv) EMPTY /
       whitespace-only `Content-Encoding:` with no `,` → NO reject;
       (v) `Content-Encoding: , gzip` → reject `gzip`; (vi) delimiter-only
       `Content-Encoding: ,` → reject `(unparsable)`; (vii) non-UTF-8 bytes
       → reject `(unparsable)`.
  1b. **HTTP framing — a RAW-BOUNDARY guarantee, not a post-parse check
     (round-75 B2 — corrects round-74's "post-parse `FramingState` is
     enough").** The hook cannot inspect raw framing headers: in the
     pinned Axum path Hyper parses framing DURING request parsing,
     BEFORE `into_core_request` (and thus before the hook), and it does
     NOT uniformly reject smuggling-shaped inputs. Concretely, per
     `hyper-1.10.1` `proto/h1/role.rs`: it rejects *conflicting*
     `Content-Length` values, folds byte-identical duplicates (round-76
     H1 — but auctioneer's policy REJECTS duplicates even when identical, so
     the raw-boundary guarantee below must override this fold), **and
     for `Content-Length` + `Transfer-Encoding` it does NOT reject — it
     uses chunked and SUPPRESSES the `Content-Length` (when
     `Transfer-Encoding` is seen first) before constructing the
     `HeaderMap`.** So a
     classic CL+TE smuggling request is ACCEPTED as chunked and the
     ambiguity is INVISIBLE downstream — an adapter that derives
     `FramingState` from the parsed request would report `Chunked`,
     losing all evidence. **A `FramingState` computed from the parsed
     request is therefore insufficient for the anti-smuggling policy.**
     Two layers, with the anti-smuggling guarantee owned UPSTREAM:
     - **Layer A — RAW-boundary STRICT REJECTION, the SOLE EdgeZero
       Prereq 5 anti-smuggling obligation (round-77 B2/B3 — the round-76
       "provenance" alternative is REMOVED).** auctioneer CANNOT enforce
       anti-smuggling from the normalized request, and a small provenance
       struct handed to the hook CANNOT classify the full failure set
       (repeated `Transfer-Encoding`, non-`chunked` TE, illegal chunked
       framing, multiple comma-separated `Content-Length` values) without
       re-implementing the parser inside auctioneer — which auctioneer, holding
       only a normalized `HeaderMap`, cannot do. So EdgeZero MUST
       guarantee, at the raw HTTP-parser boundary (before normalization),
       **strict rejection**: the parser is configured/replaced so that
       CL+TE, conflicting-OR-duplicate `Content-Length` (even
       byte-identical), non-`chunked` or repeated `Transfer-Encoding`,
       and illegal chunked framing are REJECTED with a fail-closed 400
       **before the ingress hook is invoked** (the default Hyper "accept
       CL+TE as chunked" behaviour is NOT acceptable). Because EdgeZero
       rejects these BEFORE calling `decide`, auctioneer's hook never sees a
       raw-boundary failure and there is no rejection-body-ownership
       question: the 400 body is entirely the parser's/adapter's own, and
       the E2E matrix asserts STATUS 400 ONLY, never auctioneer's
       `FRAMING_ERROR_BODY`. **Strict rejection is a PER-TARGET
       conformance capability, not one global parser (round-78 H1).** A
       managed wasm runtime may not expose or permit replacing its raw
       HTTP parser, so a single global "replace the parser" obligation
       would make the wasm targets IMPOSSIBLE rather than deferred. The
       scoping is therefore: the ingress hook API + normalized
       `FramingState` is the GLOBAL Prereq 5 blocker (all of M3b-10);
       **Axum strict raw-boundary conformance is the M3b-10/Axum
       production gate** (the Axum E2E anti-smuggling cells below are
       blocked on it); **Cloudflare/Fastly/Spin strict framing
       conformance is each target's M6 production gate.** So the
       anti-smuggling cells are claimed for Axum at M3b-10 and for each
       wasm target only when that target's raw-boundary conformance
       lands in M6 (see Prereq 5 and the Acceptance block).
     - **Layer B — the hook's framing check (Prereq 5).** For requests
       that REACH the hook, EdgeZero exposes a NORMALIZED framing signal
       so the hook does not re-parse raw headers: **Prereq 5 gains a
       `FramingState` alongside the `HeaderMap`** — `enum FramingState {
       Length(u64), Chunked, None, Malformed }`, where the adapter
       parsed any `Content-Length` into `u64` (portable — a `usize`
       parse would wrap above 4 GiB on 32-bit `wasm32`; the parse is the
       ADAPTER's responsibility, surfacing `Malformed` for a
       non-numeric / negative / `u64`-overflowing value the parser
       tolerated). The hook maps it:
       - `FramingState::Malformed` → `Decision::Reject(Rejection {
         status: 400, body: FRAMING_ERROR_BODY })`, where
         **`FRAMING_ERROR_BODY = "Invalid request: malformed or
         ambiguous request framing\n"`** (the ONE canonical body). This
         is the ONLY case auctioneer's exact body is asserted.
       - `FramingState::Length(n)` → step 2's Content-Length pre-check
         with the `u64` `n` (compare `n > cap_bytes as u64`).
       - `FramingState::Chunked | None` → step 3's bounded read.
  2. **Content-Length pre-check** — classify via the
     `RouteResolution` (round-134 #1: Matched resolves through
     `routes.by_id`; MethodNotAllowed/NotFound feed the 405/404 arms
     directly, `allowed` naming the Allow header;
     `match_route(method, path, &routes.table)` remains only the
     M3a/dev-path fallback where no registration ran). Let `oversize(cap)` be the `Rejection {
     status: 400, body: <per-route oversize body> }` where the body is:
     - `auction` (256 KiB): `Invalid request: request size exceeded
       max size of 262144 bytes.\n`
     - `cookie_sync` (32 KiB, M5+ only): `cookie_sync request size
       exceeded max size of 32768 bytes.\n`
     - every 4 KiB route (non-body routes `status` / `info_bidders` /
       `info_bidder`, AND the `MethodNotAllowed` / `NotFound` arms):
       `request size exceeded max size of 4096 bytes.\n`

     Then, per arm — with a KNOWN `Content-Length`:
     - `Matched { cap_bytes }`: CL > cap → `Decision::Reject(oversize
       (cap))`; CL <= cap → `Decision::Dispatch`.
     - `MethodNotAllowed { cap_bytes: 4096 }`: CL > 4096 →
       `Reject(oversize(4096))`; CL <= 4096 → `Reject(Rejection {
       status: 405, body: "" })` (no read needed).
     - `NotFound { cap_bytes: 4096 }`: CL > 4096 → `Reject(oversize
       (4096))`; CL <= 4096 → `Reject(Rejection { status: 404, body:
       "" })`.
     **The cap is enforced BEFORE the 404/405** (round-71 H1 — closes
     the "POST huge body to /missing" gap: CL over 4 KiB is 400, not
     404).
  3. **Bounded-read fallback** — Content-Length MISSING or
     `Transfer-Encoding: chunked`: return `Decision::BoundedRead(
     BoundedReadPolicy { max_bytes: cap_bytes, on_exceeded:
     oversize(cap_bytes), on_success })` where `on_success` is:
     - `OnSuccess::Dispatch` for a `Matched` arm, OR
     - `OnSuccess::Reject(Rejection { status: 405|404, body: "" })` for
       the `MethodNotAllowed` / `NotFound` arms.
     EdgeZero's `into_core_request` performs the bounded read: it emits
     `on_exceeded` the instant the body crosses `max_bytes`, and
     otherwise applies `on_success`. This is what the round-71
     `Option<usize>` could not express (round-72 B2).
  4. **Dispatch** — covered by the `Dispatch` variant (Matched, CL
     known and within cap) and by `OnSuccess::Dispatch` (Matched,
     bounded read within cap).

  **Acceptance matrix (round-71 H1 + round-73 H2 — the Axum E2E suite,
  spec §11.2, asserts each cell):**
  - **Well-formed:** { non-empty CE → 400 CE-body (all 3 arms) } ×
    { CL > cap → 400 route-body, no body read } × { CL missing/chunked
    → bounded read, 400 at cap } × { `POST /missing` huge body → 400
    (cap before 404) } × { wrong method → 405 empty } × { exact bodies
    + CE-over-status precedence }.
  - **`FramingState::Malformed` → 400 exact `FRAMING_ERROR_BODY` — a
    SYNTHETIC hook contract test, NOT an Axum raw-wire E2E cell
    (round-75 M1).** On Axum, Hyper parses a single `Content-Length`
    strictly as `u64` and rejects malformed syntax / overflow BEFORE the
    application, so a `Malformed` state is not reachable by feeding raw
    bytes through the Axum server — the round-74 "Axum E2E" placement was
    unreachable. Test it instead by calling `ingress::decide(...,
    &FramingState::Malformed, ...)` directly and asserting the returned
    `Decision::Reject(Rejection { status: 400, body: FRAMING_ERROR_BODY
    })`. (Which raw inputs actually PRODUCE `Malformed` is
    per-adapter/per-runtime and belongs to each adapter's conformance
    suite — round-75 H2 — not the shared hook contract.)
  - **Raw-boundary framing failures → 400 STATUS ONLY (round-77 B2/B3 —
    the single policy is EdgeZero STRICT REJECTION; the round-76 "option
    ii provenance" alternative is removed). Corrects the round-74 matrix's
    "Hyper rejects these" claim: default Hyper does NOT reject CL+TE, it
    ACCEPTS it as chunked, so EdgeZero's Prereq 5 raw-boundary parser must
    reject it instead.** The following are ALL fail-closed 400s —
    { `Content-Length` + `Transfer-Encoding` together } × { conflicting
    `Content-Length` } × { **duplicate `Content-Length`, EVEN
    byte-identical** — round-76 H1: the policy is REJECT; the round-74
    "fold identical to one, accept" is dropped, because a consumer cannot
    distinguish a benign fold from a smuggling attempt and must fail
    closed } × { `Transfer-Encoding: gzip` / non-`chunked` } × { repeated
    `Transfer-Encoding` } × { illegal chunked framing }. EdgeZero's
    raw-boundary parser produces the 400 with its OWN body **before the
    ingress hook runs** (assert **status 400 only**, never
    `FRAMING_ERROR_BODY`); auctioneer's `decide` never sees these inputs, so
    there is no rejection-body-ownership question. **Test owner:**
    EdgeZero's per-adapter conformance suite (round-75 H2) — NOT the
    shared `decide` contract, which only sees the post-guarantee
    inputs.

  Files (when EdgeZero Prereq 5 lands):
  - Create: `crates/auctioneer-core/src/ingress.rs` — a SHARED
    `decide(method, path, headers, framing, resolution, routes) -> Decision`
    fn implementing the decision table, consuming the
    `RouteResolution` (round-134 #1; `body_caps::match_route` is the
    dev-path fallback only).
    **Round-75 H1 — `decide` IMPORTS the protocol types (`Decision`,
    `FramingState`, `Rejection`, `BoundedReadPolicy`, `OnSuccess`) from
    `edgezero-core`; it does NOT define them.** They are EdgeZero's hook
    vocabulary (the adapter interprets the returned `Decision` before
    body conversion / router dispatch, and produces the `FramingState`),
    so they must be EdgeZero-owned — auctioneer provides only the callback.
    `decide` never re-parses raw framing headers; it consumes the
    adapter-supplied `framing: &FramingState`. All four adapters call
    this one implementation.
  - Modify: each `crates/auctioneer-adapter-*/src/{main,lib}.rs` — install
    the `ingress` hook, delegating to `auctioneer_core::ingress::decide`
    with the boot-built `RegisteredRoutes` and the adapter-normalized
    `FramingState`.
  - Modify: `edgezero.toml` — the per-route STRING class keys
    (`class = "auction"` on the auction trigger, `class = "aux"` on
    the others; round-142 #4 — EdgeZero's route classes are
    strings, and today's triggers carry no class at all).
  - Create: the `crates/auctioneer-runtime/build.rs` manifest consumer —
    generates the `RouteTable` module (replacing the hand-maintained
    body_caps.rs list) and the `EXPECTED_CLASSES` build assertion.
  - Modify: `crates/auctioneer-runtime/Cargo.toml` — the
    build-dependency for the manifest parser.
  - Delete: `crates/auctioneer-runtime/tests/route_table_parity.rs` —
    replaced by the build/boot validation (round-133 #2).
  - **DO NOT remove the handler-local bounded read (round-71 H3 —
    defense in depth).** `auction_inner` KEEPS its
    `Body::into_bytes_bounded(MAX_AUCTION_BODY)`
    ([auction.rs:82](../../../crates/auctioneer-core/src/handlers/auction.rs#L82))
    as spec §13 layer 3 — it catches a buggy adapter that skipped the
    hook, a future streaming `Body`, or a platform-limit change. The
    ingress hook is an ADDITIONAL earlier layer, never a replacement
    for the handler read.

  **Tests (round-71 M1 / round-75 H2).**
  - Axum: the ingress E2E acceptance matrix above (raw wire) + the
    synthetic `decide` contract tests (the `Malformed` cell).
  - Cloudflare / Fastly / Spin: `cargo check` / `cargo build` that the
    hook installs and `auctioneer_core::ingress::decide` compiles for the
    target. **Round-75 H2 — compilation is NOT sufficient to call these
    targets production-complete.** A build proves nothing about the
    managed runtime's actual parser behaviour, pre-allocation capping,
    request cancellation, or framing normalization — all of which differ
    per platform. Establishing those requires **(a) EdgeZero-side
    per-adapter conformance tests** (the adapter's own harness asserting
    it produces the right `FramingState` / enforces the raw-boundary
    guarantee / bounds the read) **and (b) deployed runtime probes**
    against each managed platform. Those land with M6's per-adapter
    coverage, not here.

  Test count: +N Axum E2E + synthetic `decide` contract tests (deferred;
  sized when the upstream hook API is known) + the four build checks.
  **Acceptance (round-75 H2, EXTENDED by round-117 P1-4/P1-5 —
  per-target certification is machine-readable and the single global
  `MILESTONE` cannot represent it):**
  - A **target-indexed certification manifest** ships in-repo
    (`crates/auctioneer-core/src/milestone.rs` grows
    `pub fn target_certification(adapter: &str) -> Milestone`, a
    match over the four adapters, plus a serialized copy the deploy
    tooling reads). The global `MILESTONE` const becomes the FLOOR
    (minimum across targets) so every existing gate stays sound.
  - `scripts/promote-production.sh` validates the SELECTED
    `--adapter` against the manifest (a new
    `auctioneer-cli assert-milestone --adapter <a> <label>` form), not
    the global floor alone — promoting axum must not be blocked by
    wasm targets remaining lower, and promoting a wasm target must
    not ride axum's certification.
  - Flipping any target to `ProductionComplete` requires, in the SAME
    commit: (a) the updated manifest entry + its pinning test, (b) a
    checked-in, machine-readable MEMORY CERTIFICATE for that target
    (the §8 measured baseline + factor table + the whole-isolate
    inequality inputs, with the measuring commit sha), and (c) every
    prerequisite the target depends on marked landed. M3b-10's
    ingress tests alone do NOT flip anything.
  M3b-10 is complete for the AXUM target when its Axum E2E
  suite + synthetic contract tests are green, the handler-local read
  is retained, AND the axum memory certificate above exists. **The
  production label for Cloudflare / Fastly / Spin
  requires EdgeZero adapter conformance + deployed runtime probes on
  each, NOT just a passing build** — until those land (M6), the
  production GDPR-gate label applies to **Axum only**, and the wasm
  adapters are build-verified but NOT production-claimed. This corrects
  the round-71 "all four adapters build ⇒ production-complete" wording,
  which conflated compilation with runtime conformance.

  **Per-target raw-framing capability (round-78 H1 — resolves the
  "global vs per-target" ambiguity between this Acceptance block and
  Prereq 5).** The raw-boundary anti-smuggling guarantee is scoped per
  target, because a managed wasm runtime may not expose or permit
  replacing its raw HTTP parser — a single global raw-parser obligation
  would make the wasm targets impossible, not deferred:
  - **EdgeZero ingress hook API + normalized `FramingState`** — a GLOBAL
    prerequisite (Prereq 5). Blocks all of M3b-10 until it lands in the
    pinned SHA.
  - **Axum strict raw-boundary conformance** — the M3b-10 / **Axum**
    production gate. The Axum E2E anti-smuggling cells are green only
    when this lands; it is what M3b-10 claims at production for Axum.
  - **Cloudflare / Fastly / Spin strict framing conformance** — **each
    target's M6 production gate**, established by that adapter's EdgeZero
    conformance suite + deployed runtime probes. Until M6, each wasm
    target is build-verified only and its anti-smuggling cells are
    unclaimed.

## EdgeZero prerequisites (each gates only its consuming tasks)

> **THE ONE EXECUTABLE CHECKLIST (round-152 #4 — authoritative over
> every status sentence below it):**
>
> **TWO COLUMNS (round-161 #5): the PINNED state is what auctioneer
> compiles against today; the PR-HEAD state is what M3b-11 will
> bring. Mixing them caused prerequisites to read as absent when
> the surface already exists at head, inviting duplicate upstream
> asks.** "IMPLEMENTED, NOT CERTIFIABLE" means the surface exists
> and auctioneer can consume it, but the guarantee a certificate row
> needs is Unsupported or BestEffort.
>
> | Prereq | Status @120dd2aa (pinned) | Status @PR head | Blocks |
> |---|---|---|---|
> | 1 store-config errors | LANDED, consumed by M3b-1a | LANDED | nothing |
> | 2 bounded outbound collection (dual caps, elapsed, reasons; former 4 folded in) | LANDED | LANDED (batch API reshaped — M3b-0/M3b-11) | nothing — G6 bounds certificate scope |
> | 3 cancellation (CF abort, Spin BestEffort) | LANDED | LANDED + batch `cancel()`/drop | nothing |
> | 4 Fastly nonblocking outbound | SATISFIED (dispatch-before-harvest), retired | SATISFIED | nothing |
> | 5 per-route ingress hook | ABSENT | **IMPLEMENTED, NOT CERTIFIABLE** — route metadata, classification and admission surfaces exist; RAW INGRESS FRAMING/head guarantees remain Unsupported | M3b-10's certification only |
> | 8 Brotli window gate + decoder bound | LANDED | LANDED | nothing |
> | 9 egress completion + write deadline | ABSENT | **IMPLEMENTED, NOT CERTIFIABLE** — response-egress surfaces exist but report BestEffort (round-161 #5: the egress limitation belongs HERE, not under Prereq 10) | production certification only (never M3b-2's start) |
> | 10 ingress seam (consolidated) | ABSENT (partial surfaces) | **IMPLEMENTED, NOT CERTIFIABLE** — route metadata/classes, `RouteResolution`, transferable `IngressGrant` + `take_ingress_grant`, `request_start`, the application monotonic clock and bounded fallback reads all exist; what remains absent is the raw parser-boundary head/framing guarantee (round-161 #5 moves the response-egress limitation to Prereq 9, where it belongs) | M3b-10's CERTIFICATION only — not core fanout |
>
> **OPEN DECISIONS — AUCTIONEER-OWNED, NOT UPSTREAM (round-183 #9).**
> The table above tracks what EdgeZero owes auctioneer. These are what
> auctioneer owes itself; each blocks the milestone gate exactly as an
> absent prerequisite does, and each needs a person, not a landing.
>
> | # | Decision | Owner | Blocks | Open since |
> |---|---|---|---|---|
> | **D-1** | `/status` on malformed runtime config | **spec owner** | **M3b-8 Step 0** | round 178 — **RESOLVED round 185** |
>
> **✅ D-1 IS DECIDED (2026-09-19, spec owner): `/status` KEEPS
> ANSWERING AND REPORTS WHAT IS WRONG.** Neither branch the review
> offered was taken. PERMISSIVE was right that a liveness probe must
> not 500 through a config-store outage; FAIL-CLOSED was right that
> silently serving a green probe off baked defaults hides a broken
> deploy. Both were wrong to treat the STATUS CODE as the only
> channel. The resolution keeps availability AND restores the signal:
> `/status` answers 200 with a body naming the fault. An operator
> reading the probe learns the deploy is degraded and WHY; a
> restart-on-500 supervisor is not triggered into a loop that cannot
> fix the store. **A probe that cannot say what is wrong is the
> failure mode, not the status code.** M3b-1c owns the change; D-1 no
> longer blocks M3b-8.
>
> **G18–G22 ADOPTION (round-183 #6).** The feedback ledger's G18–G22
> are filed upstream and confirmed as "will fix", but the pin M3b-11
> targeted (`4790a140`) implemented none of them — SUPERSEDED by round-184 #10: head `351698c9` implements G18/G19/G21/G22 and M3b-11 adopts them. They are therefore
> not prerequisites of M3b's START — every deferral has a declared
> approximation — but they ARE prerequisites of its CERTIFICATION.
> **Round-184 #10 supersedes this: G18, G19, G21 and G22 are LANDED
> at `03cdba98` — with G20 and G23 — verified in the upstream tree, and M3b-11 adopts
> them in the same atomic commit as the batch migration.
> **ROUND-187 #5: an earlier version of this line ended "G20 is NOT
> landed and its deferrals survive", directly contradicting the
> sentence before it. G20 IS landed (`03cdba98`), and its deferrals
> — the outer race, the Fastly never-ready gap — are RETIRED.** M3b-8 runs after M3b-11.**
>
> **THREE STATES, not two (round-160 #7):** this ledger distinguishes
> ABSENT (no upstream surface exists), IMPLEMENTED-BUT-NOT-CERTIFIABLE
> (the surface exists and auctioneer can consume it, but the guarantee
> needed for a certificate row is Unsupported or BestEffort), and
> LANDED (surface plus guarantee). Prereq 10's ingress surfaces are
> the second state at head; treating them as ABSENT overstates the
> block, and none of it gates core M3b fanout.
>
> M3b-2b..M3b-7 therefore wait on NO upstream item.

Round-113 M5 (replaces the all-or-nothing sentence that stood
here): each prerequisite gates ONLY the tasks that consume it — a
task starts when its own prerequisites have shipped in the pinned
release and its body has been expanded + re-reviewed (Tasks-section
protocol). Track as issues on the EdgeZero repo; auctioneer's
Cargo.lock bump that picks up each fix is the green-light for the
corresponding M3b task. Cross-task gates that are NOT start gates
stay as stated where they stand (e.g. Prereq 9 blocks the MEMORY
CERTIFICATION, not M3b-2's start).

**Round-112 H4 prerequisite audit — MOVED to the historical appendix
(round-148 #5).** That audit recorded every prerequisite as OPEN at
`fcefdde3`; at the current pin `120dd2aa` the matrix above is the
authoritative status, and this section's remaining text records
requirements and vocabulary, not availability.

**Round-120 — upstream published its own design for this surface; auctioneer's
invented API names are re-expressed in upstream vocabulary.** The pinned
branch (audited at `fcefdde3`; pin now `12965da2`) carries four design
docs: `2026-05-21-outbound-http-design.md` (the master outbound design —
section references below cite it unless another file is named),
`2026-08-22-inbound-body-design.md`, `2026-07-10-outbound-http-implementation.md`,
and `2026-07-10-outbound-http-phase1a-error-time.md`. NONE of auctioneer's
invented names (`send_bounded`, `BoundedProxyResponse`, `ResponseBodyPolicy`,
`TypedBodyError`, `TypedProxyError`, `BodySource`, `StoreAppConfigError`)
appear upstream; where upstream defines the same capability under its own
names, the prerequisite is AMENDED to that vocabulary (adopt/match), and
where upstream's design is silent or conflicting the requirement stands
UNCHANGED and is tagged `GAP vs upstream outbound spec (round-120)`. The
full gap list, written as citable upstream feedback, lives in
`2026-09-06-edgezero-outbound-spec-feedback.md` (gap ids G1..G16 below
reference it). Canonical vocabulary map (adoptions):
- `ProxyClient::send_bounded(req, policy) -> BoundedProxyResponse` →
  upstream `OutboundHttpClient::send` / the batch handle with
  `ResponseMode::Buffered { max_bytes }` on `OutboundRequest` (outbound
  design §3.1.1, §3.1.3). The batch is index-carrying, preflights per
  slot, and is buffered-only by design; streamed-response fan-out is
  explicitly non-portable (§3.2). ROUND-123 #5 (supersedes round-121
  #1's incorrect "always raw" claim): upstream Buffered `max_bytes`
  caps the DECODED, app-visible output for bare gzip/br (upstream
  decodes), and RAW bytes for identity and passthrough
  deflate/stacked/unknown encodings. **ROUND-153 #6 — the
  `Buffered max_bytes` column below is REWRITTEN: it used to name a
  DIFFERENT value per encoding, which is unknowable at request
  build. The ONE value auctioneer sets, on every row, is
  `max(max_transport_bytes, max_decoded_bytes)` — a pre-dispatch,
  path-independent MEMORY bound (the executable contract's clause 2).
  Semantic enforcement is the two INDEPENDENT caps
  (`max_encoded_response_bytes` / `max_decoded_response_bytes`,
  landed at `120dd2aa`), which EdgeZero selects after response
  headers.** The rows below now record only cap OWNERSHIP —
  every response path takes exactly one:
  | path | Buffered max_bytes = | transport cap owner | decoded cap owner |
  |---|---|---|---|
  | identity | max(transport, decoded) | upstream | same bytes (== raw) |
  | bare gzip/br (upstream-decoded) | max(transport, decoded) | upstream (its own read loop; ceiling named+cited per adapter) | upstream |
  | deflate / stacked / unknown (auctioneer-decoded) | max(transport, decoded) | upstream | AUCTIONEER, during its own decode, enforcing THIS SLOT'S `DecodePolicy` (round-153 #1: `max_decoded_bytes`, `max_brotli_layers`, `max_brotli_wbits`, `max_brotli_decoder_bytes`, `max_chunk_bytes` — copied out of the call's `ResponseBodyPolicy` at composition and carried WITH the slot, because the transport conversion consumes the call), under the ORIGINAL per-call deadline, while the upstream raw buffer is retained (both charged for the overlap) |
  Tests: unequal caps in BOTH directions per row, exact-cap success,
  stacked intermediate overflow, terminal-EOF cases (the round-121
  reviewer's 32,779-byte zlib-of-32,768 fixture is the canonical
  passthrough case). Buffered batches ride the round-123 #4
  wave lease for admission; retained raw+decoded overlap is charged in
  the wave's memory term; the WHOLE WAVE releases at WAVE COMPLETION —
  which is round-130 #2 LATER than the batch await: the lease is
  held until auctioneer's OWN decode of every passthrough
  (deflate/stacked/unknown) slot finishes and the charged raw AND
  decoded buffers are dropped, because releasing at the batch await
  would let the next wave's buffers overlap this wave's decoding and
  exceed the equation. For a wave with no passthrough slot the two
  instants coincide. (Round-125 #9 stands: the batch is one atomic
  await with no per-slot completion surface, so per-slot release is
  unimplementable; the memory term charges full-wave lifetime
  accordingly.) The upstream batch does
  NOT implement auctioneer's StreamGate and must
  not be assumed to.
- `TypedProxyError` / `TypedBodyError` → typed `EdgeError` variants
  (`BadGateway` 502, `GatewayTimeout { cause: BudgetSource }` 504,
  `ResponseTooLarge` 502-distinct-kind; §3.4.3, phase1a Task 1).
  ROUND-130 #2 — under the adopted BUFFERED `send_all` these arrive
  as each slot's `Err(EdgeError)` in the index-aligned results (`Ok`
  means the complete body was already collected — there is no
  mid-body error surface for fan-out); the `Body::Stream` chunk-error
  migration (`anyhow::Error` → `EdgeError`, §6) matters only for
  non-fanout raw-body paths. Auctioneer's classifier consumes the
  variants through ONE exact per-slot mapping:
  | Upstream surface (@ `12965da2` — round-137 #5: REAL enums,
    never message inspection) | FanoutError |
  |---|---|
  | `BadGateway { reason: BadGatewayReason::Transport }` | `Transport(TransportError { kind: Initial })` — the send failed before/without a response (round-147 #6: exact variant; no message parsing) |
  | `BadGateway { reason: BadGatewayReason::Protocol }` | `BadServerResponse` — the upstream answered but its framing/headers were malformed; same wire class (code 4, seatnonbid 100) as a 3xx or unparsable body |
  | `BadGateway { reason: BadGatewayReason::Unspecified }` | `Transport(TransportError { kind: Initial })` — no finer discriminant is available; never `Other(..)` (which would leak an unbounded category) |
  | `BadGateway { reason: BadGatewayReason::Unreachable }` | `Transport(TransportError { kind: Unreachable })` — code 999 / seatnonbid 103 (the discriminant LANDED at `120dd2aa`, round-142) |
  | `BadGateway { reason: BadGatewayReason::Decode(Brotli) }` | `DecoderFailure { layer: ContentCoding::Brotli, .. }` |
  | `BadGateway { reason: BadGatewayReason::Decode(Gzip) }` | `DecoderFailure { layer: ContentCoding::Gzip, .. }` |
  | `BadGateway { reason: BadGatewayReason::Decode(Json) }` | `BadServerResponse` — round-147 #6: `ContentCoding` has NO Json variant (it enumerates content-codings, and JSON is not one); a JSON decode failure reported by the host is the same "unparsable body" class auctioneer assigns to its own parse failures |
  | any future `BadGatewayReason` / `BadGatewayDecodeReason` variant (both enums are `#[non_exhaustive]`) | `Transport(TransportError { kind: Initial })` — the mapper's wildcard arm; a NEW upstream variant is a pin-move review item, never a silent `Other` |
  | `ResponseLimitReason::BufferedBody` | `OversizeResponse { source: Transport }` — round-147 #6: compatibility-only (the buffered-facade cap); auctioneer's caps make it unreachable in practice but the arm is explicit |
  | `ResponseLimitReason::DecoderMemory` | `OversizeResponse { source: Decoded }` — round-154 #10 / round-155 #5 (this row was left stale when the decision was recorded elsewhere; it is now the decided mapping): the reason carries NO content-encoding field, so `UnsupportedContentEncoding { actual: <coding> }` had nothing to fill `actual` with. The host declined to allocate a decoder whose state exceeded `max_brotli_decoder_bytes` — a SIZE refusal on the decoded side. Wire pair is unchanged (code 4 / seatnonbid 100). `BrotliWindow` keeps `UnsupportedContentEncoding { actual: "br" }`, which IS brotli-specific by construction. |
  | any future `ResponseLimitReason` variant (`#[non_exhaustive]`), incl. `Unspecified` | `OversizeResponse { source: Transport }` |
  | `GatewayTimeout { cause }` | `Timeout(..)` via the BudgetSource provenance rule below — NEVER a direct code-12 |
  | `ResponseLimitReason::EncodedBody` | `OversizeResponse { source: Transport }` |
  | `ResponseLimitReason::DecodedBody` | `OversizeResponse { source: Decoded }` |
  | `ResponseLimitReason::HeaderBytes` / `HeaderCount` | `OversizeResponse { source: Headers }` |
  | `ResponseLimitReason::BrotliWindow` | the Prereq-8 window refusal — `UnsupportedContentEncoding { actual: "br" }`, bidder code 4 + seatnonbid 100 |
  | `StoreExtraction`, `BadRequest`, `Internal`, `ServiceUnavailable`, any other `EdgeError` variant reaching a slot | `Transport(TransportError { kind: Initial })` — round-147 #6: the table is TOTAL; `Other(..)` is never produced by the slot mapper (it survives only for `FanoutInternalError`-adjacent internal-bug channels such as "finalize reserve overflow") |
  Pinned by `slot_error_mapping_is_total_and_typed()`: a `match` over
  every `BadGatewayReason`, `BadGatewayDecodeReason` and
  `ResponseLimitReason` variant (with the `#[non_exhaustive]`
  wildcard arms) compiles, and each named row above yields exactly
  the listed `FanoutError` (`ContentCoding::Deflate` /
  `Identity` never come from this mapper — they are auctioneer's own
  passthrough-decode layers).
  The plan's `TypedProxyError`/`TypedBodyError` snippets remain as the
  REQUIREMENTS LIST those variants must cover, not as upstream API.
  ROUND-121 #6 (LANDED @120dd2aa — round-148 #5): the widened G10
  discriminator ask is satisfied by `BadGatewayReason::{Unreachable,
  Protocol, Transport, Unspecified, Decode(BadGatewayDecodeReason::
  {Brotli, Gzip, Json})}` (matrix row "Unreachable / decode-corruption
  discriminants"); the mapping table above consumes it. The original
  ask text is in the historical appendix §2.
- Per-call deadline/timeout plumbing → upstream `Deadline` (equality
  expires — `deadline <= now` ⇒ expired, phase1a Task 2 — matching this
  plan's round-98 M9 / GCL tie rule) + `dispatch_budget` with
  `BudgetSource` attribution. **THE REAL TIE RULE (round-176 #2 —
  this paragraph said "equality attributes to `PerCallTimeout`",
  which is not what upstream does):** `dispatch_budget` builds the
  candidate list in the order `BatchCutoff`, `PerCallTimeout`,
  `RequestDeadline`, `Default` and selects with `min_by_key`, which
  is STABLE — so on an exact tie the FIRST candidate wins and the
  reported cause is **`BatchCutoff`**, as upstream tests explicitly.
  A three-way tie therefore reports `BatchCutoff`, never
  `PerCallTimeout`. (auctioneer's OWN composition rule is unaffected and
  still stands: `min(bidder_at, auction_at)` with equality ⇒
  `Bidder`, round-98 M9 — that governs which instant auctioneer SENDS,
  while this rule governs which cause EdgeZero REPORTS back.)
  ROUND-121 #2: `BudgetSource` is PROVENANCE (which budget was
  selected), not PROOF that the deadline elapsed — a BestEffort host
  can report `BatchCutoff` for an earlier platform-phase timeout.
  Translating it directly to auction-global code 12 would falsely
  expire live auctions. auctioneer keeps its OWN absolute deadlines
  authoritative: code 12 is asserted only when auctioneer's clock shows
  the auction deadline actually expired; otherwise a
  `BatchCutoff`-attributed failure is a per-bidder timeout. Submission-time deadlines are
  carried through later dispatch waves — never re-anchored per
  upstream call. Regression test (round-123 #15):
  `batch_deadline_report_with_live_auction_is_bidder_timeout_not_code_12()`
  — a fanout result carrying `BudgetSource::BatchCutoff` (round-159
  #5 renamed it from the retired name) while
  auctioneer's auction clock shows time remaining must produce the
  per-bidder timeout entry and MUST NOT emit the auction-global
  code 12.
- Per-slot elapsed time → LANDED @120dd2aa (round-148 #5):
  `OutboundSlotResult { elapsed, outcome }` carries per-slot elapsed on
  BOTH outcomes (matrix row "Per-slot elapsed (G15)"); the facade's
  `BoundedSlotResult.elapsed` consumes it and the round-134 #3
  no-degraded-fallback rule is scoped by target in the ELAPSED
  SEMANTICS banner (Fastly reports observation time). The original
  G15 ask is in the historical appendix §3.
- `supported_body_sources()` capability negotiation → upstream's manifest
  capability system (`Capability` / `CapabilitySupport`,
  `Adapter::capability()`, required-vs-optional enforcement ladder —
  §3.5.1–§3.5.3). Note the granularity differs: upstream capabilities are
  static adapter facts, and there is NO per-request body-source
  negotiation (see the Prereq 2 round-120 block for the resulting gaps).
Where a round-120 tag below marks a GAP, do not weaken or silently rewrite
the auctioneer requirement — the feedback file is the channel to upstream.

- [x] **Prereq 1 — typed, categorized store-config errors — LANDED
  @120dd2aa and CONSUMED (round-148 #5).** `EdgeError::StoreExtraction
  { reason: StoreExtractionReason (16 variants, non_exhaustive),
  message, field_path }` plus `ConfigStore::get_bounded` are the
  upstream surface; auctioneer's mapping is CODE, not a table in this
  plan: `classify_store_extraction_with` / `classify_load` in
  `crates/auctioneer-core/src/config/runtime.rs` (M3b-1a, commit
  f461f8c; runtime bootstrap `allow_missing_config` governs the
  MissingBlob and absent-binding rows), pinned by the
  `runtime_config_failclosed.rs` / `resolve_runtime_config.rs`
  suites and `config_error_logs_structured_allowlist_no_raw_text()`.
  The pre-landing requirement text (the `StoreAppConfigError`
  mapping table, the round-27/28 audits, the G4 gap note) is in the
  historical appendix §4. *Blocks:* nothing.

- [x] **Prereq 2 — Bounded outbound COLLECTION through every
  adapter — LANDED @120dd2aa (round-151 #7; Cargo.lock pins it):
  buffered `send`/`send_all` with `ResponseMode::Buffered`, the
  dual encoded/decoded caps (G5), header byte/count caps, brotli
  window + decoder-memory limits, per-slot elapsed (G15), typed
  `BadGatewayReason` / `ResponseLimitReason` discriminants (G10),
  and Fastly's dispatch-before-harvest `send_all` (the former
  Prereq 4). The ONE open residue is **G6** — no bounded
  source-read allocation (`max_chunk_bytes` rechunks OUTPUT only) —
  which does NOT block any task: it bounds the certificate SCOPE to
  `managed` (below). Everything historical about how this
  prerequisite was derived (the 9e661ae inventory, `ProxyResponse`
  accessors, the round-34 error mapping, the streaming asks, source
  negotiation, header normalisation on decoded sources, per-adapter
  chunk reads) is in the historical appendix §10.**

  THE NORMATIVE ASK, complete: upstream `OutboundHttpClient::send`
  / `send_all` with `ResponseMode::Buffered` enforced DURING each
  adapter's internal collect loop (never collect-then-check),
  carrying BOTH caps (G5 LANDED @120dd2aa: `max_encoded_response_bytes`
  + `max_decoded_response_bytes` on the builder, enforced
  during the adapter's read and selected after response headers;
  the pre-G5 single-slot interim is RETIRED — round-149 #6,
  historical appendix §5);
  per-slot metadata (round-140 #3 — the correctness needs are
  covered by REAL surfaces): elapsed on both outcomes (G15, code
  at the candidate head), disposition via retained
  `Content-Encoding`, overflow origin via `ResponseLimitReason`
  (the G14 metadata ask stays analytics-only); typed `EdgeError`
  variants (the G10 codec-discrimination ask is LANDED —
  `BadGatewayReason::Decode(BadGatewayDecodeReason)`, round-149 #6);
  drop-cancellation of the pending future (Prereq 3 wiring); the
  static capability description is a AUCTIONEER-SIDE per-adapter
  constant table — no `supported_body_sources()` method exists
  upstream (the name survives only in requirements records; no
  negotiation, round-133 #6). ITS PATH INTO THE FACADE (round-143
  #6 — generic auctioneer-core builds the fanout from
  `ctx.http_client()` alone today, so adapter facts had no
  carrier). Superseded 2026-10-08: no registry is needed because these
  facts describe platform support rather than choosing runtime behavior.
  The adapter-owned static declaration is:
  ```rust
  // auctioneer-core::outbound::TargetOutboundProfile:
  // Every NUMBER lives in the validated envelope
  // (`ResolvedIsolateLimits.guest_decoder_bound_bytes`, derived from
  // `max_brotli_wbits` as brotli_state_bytes(wbits); `.max_active_
  // body_streams`, the certified per-target default) and is read from
  // there; the profile carries ONLY adapter-owned timing/support facts.
  // It has no
  // numeric field. It is documentation checked against SDK expectations,
  // not installed runtime state or a promise stronger than the SDK's.
  pub struct TargetOutboundProfile {
      pub elapsed_semantics: ElapsedSemantics, // CompletionTime | ObservationTime
      pub deadline_support: SupportLevel,      // Native | BestEffort
      pub cancellation_support: SupportLevel,  // Native | BestEffort
  }
  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub enum ElapsedSemantics { CompletionTime, ObservationTime }
  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub enum SupportLevel { Native, BestEffort }
  ```
  THE FOUR ROWS (authoritative — each adapter crate's
  `limits.rs::OUTBOUND_PROFILE` is this literal, byte for byte;
  decoding is selected separately from SDK response metadata, not
  from this declaration):

  | target | elapsed_semantics | deadline_support | cancellation_support |
  |---|---|---|---|
  | `axum` | `CompletionTime` | `Native` | `Native` |
  | `cloudflare` | `CompletionTime` | `BestEffort` | `BestEffort` |
  | `fastly` | `ObservationTime` | `BestEffort` | `BestEffort` |
  | `spin` | `CompletionTime` | `BestEffort` | `BestEffort` |

  MISMATCH TESTS (one per target, roster members):
  `outbound_profile_matches_capability_table_<target>()` —
  `deadline_support` equals the expected-support table's
  `outbound-deadlines` cell and `elapsed_semantics ==
  ObservationTime` iff that table's `send-all-slot-isolation` cell
  is `BestEffort`, so the profile can never disagree with the
  capability table; and `bounded_fetch_reads_limits_not_profile()`
  (auctioneer-core) — `BoundedFetch` charges `guest_decoder_bound_bytes`
  and sizes waves from `ResolvedIsolateLimits` only, pinned by a
  compile-time check of the numeric limit source. Each adapter owns
  its constant (no target cfg-guards in auctioneer-core):
  `auctioneer-adapter-axum/src/limits.rs::OUTBOUND_PROFILE`, and the
  same file in the cloudflare/fastly/spin crates. There is no profile
  installer, handle, test extension, or missing-profile panic.
  `BoundedFetch` selects decoding from response metadata and uses
  resolved envelope limits. The concrete SDK client supplies elapsed
  timing; adapter-owned timers/cooperators drive scheduling. Capability
  declarations cannot upgrade those guarantees.
  Everything below records how these requirements were derived and
  maps them onto upstream vocabulary; where a passage below still
  speaks in auctioneer-visible streams, it is HISTORY, not contract.

  Round-118 F11 (binding on the CF prerequisite): whatever fix
  upstream prefers, it may NOT shrink the codec contract — deflate and
  stacked encodings stay REQUIRED on Cloudflare — and at `120dd2aa`
  they ARE served, as raw passthrough that auctioneer's own chain
  decodes (round-144 #3: CF's profile is `&[RawCompressed,
  GuestDecoded]`, not GuestDecoded-only; this paragraph's "raw
  fallback CF does not offer" premise is historical) (so
  "preferred" cannot mean "reject deflate"). Positive deployed
  fixtures: CF deflate and a stacked pair decode END-TO-END; a
  negative compressed-never-reaches-JSON test does not establish
  support. Round-38 B5: the round-33 B2 draft required RAW COMPRESSED
  streams from EVERY adapter. That contradicted Cloudflare's
  Workers Fetch API (raw passthrough only survives when the
  Worker doesn't consume the body — auctioneer necessarily does)
  and forced identity-only rejection to sit alongside the
  RawCompressed contract. Round-39 B8 introduced the
  three-way `BodySource { RawCompressed | GuestDecoded |
  HostDecoded }` + `supported_body_sources()` as the
  canonical shape (round-37 B7's two-way retired by
  round-39 B8; round-40 B1 deleted the leftover two-way
  duplicate). Round-38 B5 deletes the superseded "raw for
  all" + "identity-mode reject" instructions from
  Prereq 2 and points to the round-39 B8 shape as the
  single contract.

  **Contract (round-38 B5, REWRITTEN round-135 #1 in the adopted
  buffered vocabulary):** every adapter ships the upstream
  BUFFERED `OutboundHttpClient::send` / `send_all` with
  `ResponseMode::Buffered { max_bytes }` enforced DURING its
  collect loop, carrying per-slot elapsed (G15 — code at the
  candidate head); the disposition is read from RETAINED
  `Content-Encoding` and the overflow origin from
  `ResponseLimitReason` (round-140 #3 — the G14 metadata ask is
  analytics-only, and no metadata channel is required for
  correctness). There is NO per-request source
  negotiation (round-133 #6) and NO stream crosses the fan-out
  boundary — `supported_body_sources()` is a static capability
  description budgeting reads. The historic round-33 B2 "raw
  compressed from every adapter" text and the round-38
  negotiate-then-stream contract are both DELETED.

  What each adapter ships — and DOES ship at `120dd2aa` (round-135
  #1 — the BUFFERED requirements; round-151 #7: verified landed,
  matrix rows 108-113):
  - the upstream buffered `send`/`send_all` with
    `ResponseMode::Buffered { max_bytes }` enforced during the
    read (never collect-then-check), per the alignment
    cap-ownership rows;
  - the RETAINED `Content-Encoding` on passthrough responses (the
    ONLY disposition signal — round-152 #6: there is NO G14 metadata
    channel; the facade DERIVES `CapRow` from the header), the
    `ResponseLimitReason` on every size error, and the G15 per-slot
    elapsed on both outcomes;
  - drop-cancellation of the pending send future (Prereq 3's
    AbortController / WASI teardown wiring).
  **G6 residue and certificate scope (kept live — round-151 #3/#7;
  the surrounding streaming asks are in the historical appendix
  §10):**
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
    NAMING (round-139 #6): with this exclusion, what M3b-2
    certifies is the **MANAGED-MEMORY certificate** — the
    "whole-isolate certificate" label is RETIRED (it would
    overclaim); the equation section's historical "whole-isolate
    budget" title survives as a section name only, and egress
    completion (Prereq 9) remains a SEPARATE
    production-certification prerequisite it never subsumes.
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
  issuable on Axum. The machine-readable certificate (M3b-10
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
  convenience layer. M3b-2 lands the weaker
  guarantee; the Limited-wrapper alternative is a
  documented follow-on for deployers who need it.

  Under this contract, auctioneer's facade (`BoundedFetch`) wraps the
  upstream buffered client directly (round-135 #1; the earlier
  `send_one_bounded` auctioneer-owned helper stays removed). A
  returned slot carries the COMPLETE collected body plus its retained
  headers, from which the facade derives `CapRow` (round-152 #6 — no
  disposition metadata exists); auctioneer's passthrough decoder runs AFTER the slot
  completes, under the wave lease, on the collected bytes — no
  stream items, no item-error matching.
  Status and headers ride through the response accessors
  untouched. Round-32 M4: repeated `Content-Encoding`
  headers are preserved because `HeaderMap` supports
  `append` (see M1 below), so auctioneer can iterate them.
  - Decode-time failures inside auctioneer's pipeline →
    `FanoutError::DecoderFailure { layer, source }` per M4.

  The pinned upstream SHIPS the buffered dual-cap collection
  (round-151 #7): the four adapters' bounded collect loops with
  during-read enforcement and G15 elapsed are in the tree
  (round-136 #1: incremental reading is each adapter's internal
  loop, not a handed-to-auctioneer stream).

  **Round-28 H4 rewrite — TWO independent budgets, not a ratio.**
  Earlier draft compared transport bytes with a decoded cap and
  allowed compressed input up to 4× the decoded cap; that
  neither bounds transport memory nor bounds decoded output
  cleanly (a 10 000:1 gzip bomb whose compressed size is 4×
  decoded cap still explodes to 10 000× on decode). The correct
  contract is two orthogonal limits enforced at the same
  BOUNDED COLLECT LOOP (round-136 #1):
  1. **Per-adapter bounded collection.** EACH adapter wrapper
     (Axum, Cloudflare, Fastly, Spin) MUST enforce the caps
     INSIDE its own incremental read loop — no
     `response.bytes().await` or equivalent full-buffer
     materialisation followed by a size check. The incremental
     read is the adapter's implementation detail; auctioneer receives
     only the completed, within-cap body.
  2. **`max_transport_bytes` scoping** (round-37 B7 canonical
     — the round-28 H4 draft called this "compressed / on-wire
     bytes" unconditionally, which is unimplementable on
     Cloudflare where the runtime consumes the body before
     auctioneer sees a single wire byte):
     - `BodySource::RawCompressed` sources (Axum, Fastly,
       Spin under their `RawCompressed` capability): the
       reader tracks bytes-read-from-socket (wire bytes).
     - `BodySource::GuestDecoded` sources (Cloudflare
       always per round-40 B2; Fastly/Spin when deployer
       opts in): the reader tracks bytes-delivered-to-
       auctioneer, which equals decoded bytes because the
       adapter's IN-GUEST decoder already decompressed. On
       these sources `max_transport_bytes` IS the
       decoded-side ceiling for the underlying stream —
       auctioneer still enforces `max_decoded_bytes` on the
       post-pipeline output.
     - `BodySource::HostDecoded` sources (currently no
       adapter — reserved for future host-decoded
       platforms): identical scoping to GuestDecoded but
       the decoder memory is charged to the host runtime,
       not the guest. Admission `guest_decoder_bound_bytes`
       for this variant is 0.
     Either way, `FanoutError::OversizeResponse { source:
     OversizeSource::Transport }` fires as soon as the
     running total exceeds `max_transport_bytes`. Default
     value: same as `max_response_bytes` (there is no useful
     case where a legitimate compressed body is larger than
     the decoded body auctioneer is willing to accept). Operators
     can raise it via `[bidders.<name>.max_transport_bytes]`
     for endpoints known to serve poorly-compressible JSON.
  3. **`max_decoded_bytes` bounds decoded output.** After the
     decode step (gzip / brotli / deflate / identity — a
     no-op passthrough for `GuestDecoded` / `HostDecoded`
     sources whose upstream already ran the decoder), the
     reader tracks decoded-bytes-produced and errors with
     `FanoutError::OversizeResponse { source:
     OversizeSource::Decoded }` as soon as the running total
     exceeds `max_decoded_bytes`. Default value:
     `max_response_bytes` (the historic knob). Set to the same
     value the M2 post-buffer check used.

  4. **Bounded per-chunk reads at the adapter transport** —
     round-151 #3: the per-adapter streaming chunk-read
     instructions and the round-36 B4 canonical-shape pointer that
     stood here are in the historical appendix §10; the live
     residue is G6 (see "G6 residue and certificate scope" above):
     `max_chunk_bytes` bounds auctioneer's OWN pipeline chunking (the
     `MAX_CHUNK_SIZE` staging terms), not the provider's source-read
     allocation.
  5. **Content-Encoding handling** (round-28 H4 new — the
     earlier draft did not define this):
     - `Content-Encoding` values auctioneer MUST support: `gzip`,
       `br`, `deflate`, `identity`, and absent header (treated
       as `identity`).
     - A response bearing an UNKNOWN `Content-Encoding` value →
       `FanoutError::UnsupportedContentEncoding { actual: String
       }`, `seatnonbid.statuscode = 100`. Do NOT attempt to
       decode.
     - A response bearing MULTIPLE `Content-Encoding` values
       (e.g. `gzip, deflate`) → the reader applies decoders in
       **REVERSE** listed order — **RIGHT-to-LEFT** — because
       RFC 9110 §8.4 lists codings in the order the sender
       applied them, so the receiver must undo the last-applied
       coding first. **Concrete example:** the header
       `Content-Encoding: gzip, deflate` means the sender
       gzip-ped first, then deflate-ped; the wire body is
       `deflate(gzip(payload))`. The receiver decodes deflate
       FIRST (undo the last-applied coding), yielding
       `gzip(payload)`, then decodes gzip, yielding `payload`.
       Round-29 H2 correction — the round-28 draft said
       "LEFT-to-RIGHT" and would have failed the positive
       fixture. Each layer's output feeds the next layer's
       input, and each layer independently tracks
       `max_decoded_bytes` on its output. If any layer exceeds
       `max_decoded_bytes`, error immediately without draining
       the remaining layers.
     - **Cap on the number of encoding layers** — 3. Any
       response listing more than 3 comma-separated
       `Content-Encoding` values → `FanoutError::TooManyEncoding
       Layers { actual: usize }`. Prevents pathological
       nested-encoding chains.
     - **Round-31 M4 + round-32 M1 — complete HTTP header
       contract.**
       - **Repeated headers.** RFC 9110 §5.3 permits multiple
         `Content-Encoding` headers to be sent as separate
         field lines; they MUST be concatenated with `, `
         before parsing (the semantics are identical to a
         single comma-joined header). Total layer count across
         all field lines still capped at 3.
       - **ADAPTER HEADER PRESERVATION (round-32 M1
         precondition).** The pinned adapters currently copy
         response headers via `HeaderMap::insert`, which
         REPLACES prior entries with the same name; a response
         with two `Content-Encoding` field lines loses one
         before auctioneer sees it. See
         [`edgezero-adapter-axum/src/proxy.rs:66`](https://github.com/IABTechLab/edgezero/blob/9e661ae/crates/edgezero-adapter-axum/src/proxy.rs#L66)
         for the current shape. Prereq 2 REQUIRES each adapter
         to switch to `HeaderMap::append` (which preserves
         duplicates as multiple field lines) OR to
         `HeaderMap::extend(headers.iter())`. This is a
         concrete adapter-side edit, not a auctioneer-side
         workaround.
       - **Case + OWS normalization.** Coding tokens compare
         **case-insensitively** per RFC 9110; auctioneer
         lower-cases every token before matching. Optional
         whitespace (OWS) around tokens and commas is
         stripped. `Content-Encoding: GZIP` matches `gzip`;
         `Content-Encoding:  gzip ,  deflate` matches
         `gzip, deflate`.
       - **Malformed compressed data** — a decoder that
         reports "invalid compressed input" mid-stream →
         `FanoutError::DecoderFailure { layer:
         ContentCoding, source: String }`,
         `seatnonbid.statuscode = 100`. Do NOT retry on the
         same layer with a different codec.
       - **Deflate = zlib, NOT raw.** The
         `deflate` coding on the wire is `zlib` (RFC 1950
         framed). `flate2::read::ZlibDecoder`, not
         `flate2::read::DeflateDecoder`. Round-31 M4:
         explicit call-out — the two decoders wrap different
         framings and picking the wrong one silently produces
         garbage.
       - **Post-decode header rewriting.** Round-38 B6
         canonical: after auctioneer finishes emitting the
         fully-decoded body to the classifier, the response
         headers seen by downstream code have
         `Content-Encoding` REMOVED and `Content-Length`
         set to the observed decoded byte count. The
         `Content-Length` write happens POST-COLLECTION —
         auctioneer knows the length once the bounded body
         stream returns `None`. Between `send_bounded`
         return and body-stream exhaustion, downstream
         code MUST NOT assume `Content-Length` is
         populated (it will be absent — the adapter
         stripped it per B6 above, and auctioneer doesn't
         backfill until the body is fully drained).
         Round-32 M1's "always write the decoded length"
         still holds; the correction is on WHEN
         (post-collection, not per-stream).
     - Any adapter's incremental decoder MUST short-circuit
       decode as soon as decoded output would exceed
       `max_decoded_bytes + MAX_CHUNK_SIZE`.

  Task M3b-7's memory-bound regression test asserts BOTH bounds
  once the prereq lands. Additional required regression cases
  under Task M3b-7:
  - `gzip_bomb_10000_to_1_short_circuits_at_decoded_cap()` —
    compressed body whose expanded size would exceed
    `max_decoded_bytes` by 10 000×; assert
    `FanoutError::OversizeResponse { source: Decoded }` fires
    within `max_transport_bytes + MAX_CHUNK_SIZE` transport
    bytes read and within `max_decoded_bytes + MAX_CHUNK_SIZE`
    decoded bytes emitted; harness peak allocation stays under
    the canonical bound `per_stream_bytes(max_transport_bytes,
    max_decoded_bytes, max_brotli_wbits, max_brotli_layers, 0)`
    (round-135 #2 — the five-parameter max-rule form; guest bound 0:
    this fixture exercises auctioneer's own chain)
    (round-45 H3 — the earlier hand-written `LAYERS *
    DECODER_STATE_MAX` form used the deleted constant and the
    superseded per-layer-brotli model; the helper already
    accounts for `MAX_BROTLI_LAYERS = 1` brotli state plus
    flate2 for the remaining layers and the chunk staging).
  - `transport_bytes_bound_hit_before_decode_starts()` —
    round-136 #7 splits this by provability: the NAMED case sends
    an EARLY OVERSIZED Content-Length (larger than
    `max_transport_bytes`), so the overflow is decidable pre-body
    and "no decode step ran" is assertable; the chunked/no-length
    VARIANT (bomb-compressed payload with no length header)
    asserts the counters instead — transport reads stop within
    cap + one chunk and `decoded_bytes_emitted()` stays under the
    decoded cap (a decode step MAY have started; zero-decode is
    not provable there). Both yield
    `FanoutError::OversizeResponse { source: Transport }`.
  - `brotli_streaming_stops_at_cap()` — brotli-encoded response
    of decoded size `2 * max_decoded_bytes`; assert decode
    short-circuits.
  - `deflate_streaming_stops_at_cap()` — same for deflate.
  - `identity_still_bounded_by_transport_cap()` — sanity check
    the uncompressed path stays inside both bounds.
  - `unsupported_content_encoding_errors_before_decode()` —
    `Content-Encoding: zstd` → `UnsupportedContentEncoding`,
    zero decoder invocations.
  - `multiple_encodings_decoded_right_to_left()` — round-29 H2:
    response with `Content-Encoding: gzip, deflate` and a
    payload that the sender first gzip-ped THEN deflate-ped
    (wire body = `deflate(gzip(payload))`); the decoder must
    apply deflate first (rightmost header value), then gzip →
    yields the original payload. Swap gzip for something bogus
    on the sender side → decoder errors at the gzip layer;
    swap deflate → decoder errors at the deflate layer.
  - `too_many_encoding_layers_rejected()` — `Content-Encoding:
    gzip, br, gzip, br` → `TooManyEncodingLayers { actual: 4 }`.
  Until then, auctioneer cannot make a memory-bounded guarantee on
  the production fanout path.

  **Round-120 alignment against the upstream outbound design (per-point
  verdicts; citations are to `2026-05-21-outbound-http-design.md`):**
  - **ADOPT — the bounded transport vehicle.** `send_bounded` /
    `BoundedProxyResponse` become upstream's `OutboundHttpClient::send` /
    `send_all` + `ResponseMode::Buffered { max_bytes }` (§3.1.1, §3.1.3);
    the typed failures arrive as each slot's `Err(EdgeError)` —
    `BadGateway` / `GatewayTimeout{cause}` / `ResponseTooLarge`
    (§3.4.3; round-136 #1 — the `Body::Stream` chunk-error
    migration in §6 matters only for non-fanout raw-body paths). All outbound caps/counters are
    `u64` upstream (§3.1.3 cap note) — matches this plan's `u64` fields.
    Pre-append checked accounting and the incremental decoded-byte cap
    are mandated upstream (§3.4.1) — the round-28 H4 "two orthogonal
    limits at the same streaming reader" model matches on the DECODED
    side only (transport side: gap below).
  - **ADOPT — adapter header preservation.** Upstream mandates
    `HeaderMap::append`/`get_all` (never `insert`/`set`) on every adapter
    and names the two concrete CF fixes (§3.1.4 "Multi-value headers
    preserved", §4.2) — the round-32 M1 precondition is satisfied by
    design. Post-decode `Content-Encoding`+`Content-Length` stripping is
    also mandated upstream (§3.4.1) — matches round-38 B6.
  - **ADOPT — the round-43 H2 deflate hazard is closed by upstream's
    policy table, differently than either option (a) or (b).** §3.4.1
    decodes ONLY bare single `gzip`/`br`; deflate, stacked lists,
    parameterized values, and repeated field lines pass through
    UNTOUCHED with `content-encoding`/`content-length` PRESERVED. The
    failure mode round-43 H2 flagged (compressed bytes relabelled
    identity) cannot occur: bytes the adapter did not decode are never
    stripped of their labels. Consequence for this prerequisite: auctioneer
    RUNS ITS OWN decoder pipeline (deflate, stacked chains, case/OWS
    normalization per this task) on the preserved passthrough form — the
    round-118 F11 codec contract (deflate + stacked REQUIRED end-to-end)
    is met by auctioneer-side decode over upstream passthrough, and the
    positive deployed fixtures move to auctioneer's pipeline tests.
  **Round-120 gap assessment (G5 / G6 / G7 / G10 / G11 / body-source
  negotiation) — RESOLVED at `120dd2aa` except G6 (round-152 #4; the
  original assessments are in the historical appendix §11):** G5 →
  `max_encoded_response_bytes` + `max_decoded_response_bytes` (dual
  caps, matrix row "Encoding-sensitive dual caps"); G7 →
  `ResponseLimitReason::HeaderBytes` / `HeaderCount` (matrix row
  "Encoded-byte / header caps"); G10 → `BadGatewayReason::{Unreachable,
  Decode(..)}` (matrix row "Unreachable / decode-corruption
  discriminants"); G11 → multi-member gzip drained to native EOF
  (matrix row "Brotli window gate; multi-member gzip/EOF"); body-source
  negotiation → none exists and none is needed (the static profile
  above). G6 stays OPEN and bounds the certificate scope only.
  *Blocks:* nothing.

- [x] **Prereq 3 — Cancellable proxy futures on Cloudflare and
  Spin — LANDED @120dd2aa (round-152 #4): Cloudflare wires
  `web_sys::AbortController` with `controller.abort()` on expiry and
  an `abort_guard` on the body path (`edgezero-adapter-cloudflare/src/
  outbound.rs` L222/L240/L347/L421-539); Spin's owned exchange is
  drop-driven and BestEffort by design (`edgezero-adapter-spin/src/
  outbound.rs` L28/L131/L363), which M3b-4 documents and tests as
  such. The text below is the derivation record.** Historical:
  Cloudflare's `worker::Fetch::send()` returns a promise
  that auctioneer can only cancel via `AbortController` — the pinned
  EdgeZero wrapper does not wire that. Spin's async `send(...)`
  future may or may not be drop-cancellable in the current
  EdgeZero shim; verify.

  **Round-120 alignment:**
  - **ADOPT — Cloudflare.** The upstream design wires exactly this:
    `worker::AbortController` + `Fetch::send_with_signal(&signal)`, with
    `controller.abort()` on expiry (never a bare future-drop) and the
    controller moving INTO the streamed-body wrapper, aborting on
    deadline, mid-stream transport failure, AND consumer drop (outbound
    design §4.2; drop-cancellation table §3.3.4; §8 risk 3 marked
    RESOLVED with a Tier 3 origin-observed test). Round-118 F06's "the
    pinned Rust SDK's equivalent — Prereq 3 names the exact call" is
    answered for redirects and cancellation; the raw-encoding half is a
    gap (below).
  - **MATCH — Spin, verified with an honest answer.** Upstream replaces
    the SDK's detached uncancellable body pump with a hand-built
    `wasi:http` request whose pump/completion futures are owned by one
    raced exchange (§4.4); the timer selects a guest-visible attributed
    504, but Component Model teardown is cooperative, so
    `outbound-deadlines` stays `BestEffort` on Spin (§3.5.2 footnote 8,
    §8 risk 15). Auctioneer's "verify" resolves to: guest-side cancel yes,
    bounded host teardown no — plan cells relying on Spin host teardown
    inherit the BestEffort caveat.
  - **GAP vs upstream outbound spec (round-120) — feedback G9:** §4.2
    mandates `EncodeBody::Manual` only for the RESPONSE-OUT direction and
    keeps "the existing gzip/br decompression path" for the outbound
    fetch WITHOUT requiring manual response-body encoding on the
    constructed request — the round-118 F06 raw-delivery requirement
    (workerd defaults fetch response decoding to automatic) is
    unaddressed, and no deployed-Workers probe row covers it.
  - **GAP (round-120) — feedback G12:** the round-118 F12
    frozen-clock hazard (Date.now/performance.now frozen across CPU-only
    work on DEPLOYED Workers) is not acknowledged by upstream's
    cooperative `is_expired()` checks; the clock-refreshing yield this
    plan requires ("Prereq 3 names the exact yield") has no upstream
    counterpart yet.
  *Blocks:* Tasks M3b-3, M3b-4.

- [x] **Prereq 4 — Fastly nonblocking outbound primitive — SATISFIED
  @120dd2aa and RETIRED as a gate (round-142 / round-151 #7):
  `send_all` on Fastly dispatches every slot before the first
  harvest (`outbound.rs` L496, `send_async` L389/L456, `wait_pending`
  L392); mid-fanout cancellation stays BestEffort by design, which
  M3b-5 documents rather than waits on. The text below is the
  derivation record.** Historical: Fastly's `pending_request.wait()` at
  `edgezero-adapter-fastly/src/proxy.rs` is blocking. There is no
  way for auctioneer to achieve mid-fanout cancellation on Fastly
  today. Either EdgeZero exposes a native fanout primitive that
  carries per-call timeout + auction deadline + positional
  results + bounded response reads, OR EdgeZero adds an async
  Fastly proxy path.

  **Round-120 — ADOPT: upstream chose the first branch.** `send_all` on
  Fastly is dispatch-all-then-harvest (`send_async` every slot, then
  indexed `wait()`/`poll()` harvest — outbound design §4.3), carrying
  per-call timeout + shared absolute deadline via `dispatch_budget`
  (§3.3.2), positional index-aligned results (§3.1.1), and bounded
  buffered response reads per slot. This IS the "native fanout
  primitive" this prerequisite asked for. **MATCH on the residual
  limitation, honestly documented upstream:** mid-fanout cancellation
  remains impossible on Fastly (no async-cancellation primitive; a
  dispatched `PendingRequest` is always harvested — §3.3.4
  drop-cancellation table), harvest-order body drains can induce
  cross-slot 504s, and non-empty request uploads have no finite
  host-write bound — all owned by the `send-all-slot-isolation` /
  `outbound-deadlines` `BestEffort` cells (§3.5.2 footnotes 1/2/4, §8
  risks 7/8). Task M3b-5's Fastly cells inherit those documented
  bounds rather than assuming cancellation.
  *Blocks:* Task M3b-5.

- [ ] **Prereq 5 — Per-route ingress hook + normalized `FramingState`
  (round-70 H1 / round-74 H1).** EdgeZero must expose a per-adapter
  ingress hook that runs BEFORE the handler reads the request body, so
  auctioneer can enforce the route table (method/path → body cap) at
  ingress rather than relying on a handler-local constant. The pinned
  `9e661ae` adapter shims call the handler directly with no pre-body
  route gate. This is the §12-item-14 hook the spec's
  production-complete boundary (§14) requires. **Round-74 H1 — the hook
  MUST also receive a normalized `FramingState { Length(u64) | Chunked
  | None | Malformed }` from the adapter** (the adapter parses any
  `Content-Length` into `u64` — portable on 32-bit wasm — and surfaces a
  syntactically-tolerated-but-invalid length as `Malformed`).
  **Round-75 B2 — the `FramingState` ALONE is NOT sufficient for
  anti-smuggling, because the platform HTTP parser resolves rather than
  rejects the dangerous shapes.** Specifically, the pinned
  `hyper-1.10.1` accepts `Content-Length`+`Transfer-Encoding` as chunked
  and SUPPRESSES the `Content-Length` before building the `HeaderMap`
  (`proto/h1/role.rs`), so a `FramingState` derived from the parsed
  request reports `Chunked` and the CL+TE ambiguity is lost. Prereq 5
  therefore ALSO requires a **RAW-boundary framing guarantee**.
  **Round-77 B2/B3 — that guarantee is STRICT RAW-BOUNDARY REJECTION,
  the SINGLE required contract (the round-76 "framing provenance"
  alternative is REMOVED).** A small provenance struct handed to the
  hook could only carry a CL count + two presence bits; it could NOT
  classify repeated `Transfer-Encoding`, non-`chunked` TE, illegal
  chunked framing, or multiple comma-separated `Content-Length` values
  without auctioneer re-implementing the HTTP framing parser on top of an
  already-normalized `HeaderMap` — which it cannot. The raw parser is
  the only layer that sees all these shapes, and it lives in EdgeZero.
  So EdgeZero configures/replaces the raw HTTP parser so that CL+TE,
  conflicting-OR-duplicate `Content-Length` (even byte-identical, see H1
  policy below), non-`chunked`/repeated `Transfer-Encoding`, and illegal
  chunked framing are REJECTED with a fail-closed 400 BEFORE
  normalization **and before the ingress hook is invoked** (the default
  Hyper accept-as-chunked behaviour is unacceptable). The hook is never
  invoked for these, so it needs no extra provenance argument, and there
  is no question of who owns the rejection body — EdgeZero's parser does,
  end to end. The `FramingState { Length(u64) | Chunked | None |
  Malformed }` the hook DOES receive covers only the
  legitimately-parseable cases plus the adapter-detectable `Malformed`
  (a syntactically-tolerated-but-invalid single `Content-Length`), which
  auctioneer's `decide` maps to a 400 `FRAMING_ERROR_BODY`. See M3b-10 step
  1b, Layer A.
  **This prerequisite has TWO parts with DIFFERENT scope (round-78 H1):**
  1. **The hook API + normalized `FramingState` — GLOBAL.** A single
     EdgeZero deliverable, needed by every adapter; blocks all of M3b-10
     until it lands in the pinned SHA.
  2. **Strict raw-boundary rejection — PER-TARGET conformance, NOT one
     global parser.** A managed wasm runtime may not expose or permit
     replacing its raw HTTP parser, so a global "one raw-parser impl"
     requirement would make the wasm targets IMPOSSIBLE, not deferred.
     Scope it per target: **Axum** strict raw-boundary conformance is the
     M3b-10/Axum production gate; **Cloudflare/Fastly/Spin** strict
     framing conformance is each target's **M6** production gate
     (EdgeZero adapter conformance + deployed probes). Without the Axum
     part, M3b-10's Axum anti-smuggling cells cannot be met.
  **GAP vs upstream outbound spec (round-120) — feedback G2 (P0-class).**
  Neither the outbound design (out of scope by charter) nor the
  inbound-body design defines a pre-body ingress hook, a normalized
  framing state, or raw-boundary rejection of the smuggling shapes; the
  inbound design's adapters hand the platform body to `BodyCell` as-is
  and the first consumer is an extractor. Both parts of this
  prerequisite stand unchanged and OPEN.
  *Blocks:* Task M3b-10 production-completion — the hook API globally,
  and the Axum anti-smuggling cells on the Axum raw-boundary part; the
  wasm anti-smuggling cells are an M6 per-target gate. Behavior-complete
  (M3b-1..9) does NOT depend on any of it.

  *(Round-95 L4 — numbering note: **Prereqs 6 and 7 were RETIRED in
  earlier review rounds** (their asks were folded into the canonical
  `ResponseBodyPolicy`/`ProxyClient` blocks of Prereqs 2–3 and the
  Prereq 5 raw-boundary contract). The numbers are NOT reused, so
  issue-tracker and cross-plan references to "Prereq 8" stay stable —
  the jump from 5 to 8 is intentional, not an omission.)*

*(Round-124 #11 as corrected by round-125 #10: Task M3b-10 declares
Prereqs 5 AND 10 — now stated at the task itself. decide() adds the
431 header-cap branch (over `max_admitted_header_bytes/count` ⇒ 431,
exact body `too many header bytes\n`, via the static class, zero body
bytes read). PRECEDENCE (corrected round-128 #3): the PARSER-LEVEL caps fire
first PHYSICALLY — they live in the serve builder and reject before
any auctioneer callback can run, so nothing precedes them. WITHIN
decide() itself (whose 431 arm is defense-in-depth): aux-overload
first (round-117), then the 431 header-cap arm, then THE CANONICAL
DECISION TABLE IN ITS OWN ORDER —
the earlier version of this note reordered the table's
encoding-before-route rule, which was wrong; the table is canonical
and unmodified. Ingress-read-deadline expiry answers 408, exact body
`request timeout\n`. Appendix F gains both wire rows. All refusals
via static egress; each branch tested saturated and unsaturated.)*

- [ ] **Prereq 10 — the ingress seam (CONSOLIDATED contract,
  replacing the rounds 117/118/119/126 amendment layers; provenance:
  those rounds).** For routes declaring an ingress policy, EdgeZero
  performs, in order, BEFORE routing dispatch:
  1. **Parser-level header caps** — enforced where the raw parser is
     configurable: http/1 via `http1_max_headers` + `max_buf_size` —
     and the count knob MUST be configured (round-130 #6): hyper's
     default is 100, below the 128 logical entries auctioneer admits, so
     an unconfigured h1 parser rejects valid requests; set 128, with
     the exact accept/reject semantics pinned by the M3b-10 boundary
     conformance test rather than assumed.
     **h2 via `http2_max_header_list_size`** (a different knob — the
     http/1 buffer bound does NOT cover h2 header lists); both owned
     by the same adapter serve-builder as the
     TargetPlatformConstants row. Breach semantics are
     PROTOCOL-SPLIT (round-128 #3), and the interception seam is
     DEFINED: on http/1, hyper surfaces header overflow as a
     connection-level parse error BEFORE any request or HeaderMap
     exists — EdgeZero's connection driver intercepts that error
     class and writes the canned 431 wire bytes (status line +
     `text/plain; charset=utf-8` + `too many header bytes\n`,
     rendered from the static class) then closes the connection; on
     http/2 an oversized field section is rejected at the protocol
     layer per RFC 9113 §4.3 — a stream reset / GOAWAY, where NO
     application response is possible — so the documented h2 breach
     behavior IS the reset and the 431 body is http/1-only (the
     Appendix F row carries the annotation). On managed wasm
     runtimes the platform's documented header ceiling is the
     charged bound (cited per target); decide()'s own 431 arm is
     defense-in-depth only. Counting is the ENFORCING KNOB'S
     accounting, and the equation charges the same one: http/1
     counts Σ(name+value) bytes plus entry count, repeats included,
     caps inclusive; h2 uses RFC 9113 field-section size —
     Σ(name+value+32) per entry, request pseudoheaders included —
     exactly as `http2_max_header_list_size` measures it. The
     INBOUND ceiling has its OWN derivation
     (INBOUND_H2_MAX_FIELD_SECTION = max_admitted_header_bytes +
     (max_admitted_header_count + 4 pseudoheaders) x 32 +
     PSEUDOHEADER_ALLOWANCE = 21_632 at the axum defaults). The
     ENFORCING KNOB is EXCLUSIVE (round-115 B1 h2 semantics:
     rejection at headers_size >= max), so it is set ONE ABOVE the
     ceiling — 21_633 admits exactly through 21_632. h2 exposes NO
     header-COUNT knob, so the parser-time entry maximum is DERIVED:
     floor(21_632 / 32) = 676 tiny fields can materialize before the
     logical 128-entry check (decide() defense-in-depth) runs — the
     equation charges THAT derived count, exactly as the outbound
     round-112 B2 layer already documents for responses. The
     outbound section's transient model is the PATTERN only
     — its 8_192 + 65 x 32 + 64 numbers are outbound's, not this
     bound's. A cap stated in logical bytes but
     enforced in field-section bytes (or vice versa) fails
     validation.
  2. **`request_start` captured** — and
     exposed with the same precision as the grant (round-129 #5):
     `ctx.request_start() -> MonotonicInstant`, a Copy value set
     EXACTLY ONCE by this step before any body byte, present by
     construction on every policy route (no Option, no take-once —
     unlike the grant it is freely re-readable). ROUND-142 #3 — ONE
     APPLICATION CLOCK, EdgeZero-owned. This REPLACES the rounds
     130-136 `ClockBridge` / `TimeDomain` / `instant_of` /
     `duration_of` / `TimeProjectionError` design IN FULL: that
     design bridged a auctioneer Duration-epoch clock onto an EdgeZero
     instant domain, and PR 275 removed the need — the context
     exposes `RequestContext::monotonic_clock()` and
     `RequestContext::request_start()` directly. Auctioneer adopts that
     clock as its SOLE M3b time source:
     - `request_start` is `ctx.request_start()`, read as-is;
     - deadline composition happens in the app clock's own instant
       domain — `composed_at = min(fanout_entry_at + call.timeout,
       auction_deadline_at)`, every operand read from or derived
       from `ctx.monotonic_clock()` — and the upstream `Deadline`
       is built from `composed_at` with NO projection step;
     - `ProductionFanout` receives the app clock at construction
       (the adapter hands it the context's clock); the wave loop's
       timer sleeps `earliest.saturating_duration_since(clock.now_monotonic())`
       and the inclusive recheck reads the SAME clock;
     - the exchange's Duration-domain `request_start` seam
       (auction.rs) migrates to the app clock's instant type in
       M3b-2b.0 (round-148 #1 — the atomic pin move at 6a0ce36
       landed WITHOUT this migration; it is now its own executable
       unit below) — no parallel per-request bridge, no second
       capture, no conversion error to map to a 500.
     TEST SEAM: tests drive EdgeZero's application clock (its
     controllable test clock); auctioneer's `FakeClock` /
     `TestClockHandle` pairing is retired for M3b handlers — one
     clock, one seam — and the round-136 #6 tests re-target it (see
     the roster). CODE TODAY (round-143 #3, so no implementer
     mistakes the M3a shape for the target): auction.rs still
     captures auctioneer's Duration clock (`clock_handle(&ctx)` +
     `now_monotonic()`, with the `TestClockHandle` override) —
     M3b-2b migrates the handler to `ctx.request_start()` /
     `ctx.monotonic_clock()` and retires `TestClockHandle` for M3b
     handlers; until then the M3a capture is the shipped
     behavior, not a second design.
  3. **Admission callback** — the CONTRACT is EdgeZero-owned and
     exact (round-129 #5; previously only "app-supplied,
     route-classed" prose an implementer had to invent from):
     (ROUND-140 #6 — REWRITE REQUIRED BEFORE M3b-10 EXPANSION:
     EdgeZero's landed ingress surface at `120dd2aa` uses STRING
     route classes, route-table introspection + route resolution,
     an APP-OWNED monotonic clock, request start, and transferable
     grants — NOT the numeric `IngressClass(u16)` / `known_classes`
     / `default_class` / `ctx.time_bridge()` / separate-BoundedRead
     shapes below, which are auctioneer's design-era vocabulary. The
     INVARIANTS survive the rewrite unchanged — total
     classification with a required default, fail-closed unknown
     class, move-safe grant transfer, one paired time domain,
     parser-level caps — but the task body must be rewritten
     against the real APIs first; do not transcribe the literal
     shapes below.)
     ```rust
     // edgezero-core; names NO auctioneer type AND no auctioneer vocabulary
     // (round-130 #1 — the round-129 Auction/Aux/Fallback enum was
     // auctioneer's taxonomy leaking into EdgeZero):
     #[derive(Clone, Copy, PartialEq, Eq)]
     pub struct IngressClass(pub u16);   // OPAQUE app-defined token
     pub enum IngressAdmission {
         Admit { guard: IngressGrant },
         Refuse { status: u16, body: &'static [u8] }, // rendered via
                                                      // the static
                                                      // class
     }
     // ONE registration per App; default_class covers requests that
     // match NO route; known_classes is the callback's declared
     // domain (round-133 #2 — the startup validation promised in
     // round-132 now has its input):
     App::with_ingress_admission(
         default_class: IngressClass,
         known_classes: &'static [IngressClass],
         impl Fn(IngressClass) -> IngressAdmission + Send + Sync)
     ```
     The callback is INFALLIBLE — refusal is an ANSWER, not an error,
     so there is no error type. CLASS ATTACHMENT (round-131 #2 — the
     registration mechanism the round-130 text lacked): a class is
     attached PER ROUTE ENTRY at registration, and entries are
     METHOD-QUALIFIED —
     `App::route(method, template, handler).ingress_class(token)` —
     so multiple methods on one path are separate entries, each with
     its own class, and no ambiguity exists. CLASSIFICATION RULES
     are total, with exactly TWO cases
     (round-130 #1 retired the round-129 skip rule; round-131 #2
     retires round-130's "wrong method inherits the entry's class" —
     it contradicted both the F3 aux rule and the master spec's
     default-4-KiB-class rule for method mismatches):
     - `RouteResolution::Matched` (method AND path) → that
       entry's registered class;
     - `MethodNotAllowed` or `NotFound` (round-134 #1 vocabulary) →
       `default_class` (a REQUIRED
       registration argument, not an option).
     GET /openrtb2/auction therefore takes `default_class` → the aux
     gate, agreeing with F3 and the spec. Once
     `with_ingress_admission` is registered, NO request skips
     the callback. Auctioneer's mapping: the auction POST entry
     registers the auction token → `AdmissionGate`; aux entries and
     `default_class` map to the aux gate. UNKNOWN TOKENS FAIL, they
     are never mapped (round-132 #2 retires the round-131 aux
     fallback — a MISREGISTERED AUCTION route silently running under
     the cheaper aux class is exactly the envelope breach the seam
     exists to prevent): `with_ingress_admission` registration
     VALIDATES at STARTUP that every route-registered token and
     `default_class` are members of `known_classes` (the API's
     second argument) — any mismatch fails boot, fail-loud; and should an unknown
     token ever reach the callback at runtime (impossible after boot
     validation, by construction), the callback answers
     `Refuse { 500 }` via the static class — it never admits under
     ANY gate.
     ONE CLASSIFICATION SOURCE (round-131 #3 — the round-130 "same
     registered table" claim was aspiration, not mechanism: today
     there are two hand-maintained lists pinned only by
     route_table_parity.rs, and auctioneer's own template matcher in
     body_caps.rs — string parity does not prove identical
     normalization): today's `RouteTable` (body_caps.rs) carries
     ONLY method/template/cap, and handlers live only in
     edgezero.toml — neither can generate the other. ONE EXACT
     MECHANISM (round-135 #7, replacing the round-133 iterate-and-zip
     construction, which had no source for the macro's class
     metadata): THE MANIFEST IS THE SOURCE. Each edgezero.toml route
     entry gains a STRING `class` key — `class = "auction"` /
     `class = "aux"` — matching EdgeZero's landed string route
     classes (round-142 #4 retires the numeric `ingress_class =
     <u16>` shape; the string constants live in auctioneer-runtime). The
     `app!` macro (G16) reads the manifest and emits BOTH artifacts:
     (i) the handler registrations, each carrying its class token
     and receiving an issued `RouteId`, and (ii) the exposed
     completed registration table. Auctioneer's `RouteTable` is
     GENERATED from the same manifest at build time — a build-script
     consumer replaces the hand-maintained body_caps.rs list, with
     per-route caps resolved from the class token via auctioneer's own
     class → cap map. The boot step is then a pure 1:1 VALIDATION
     that the two generated artifacts agree (fail-loud) — a check,
     not a construction — and it REPLACES route_table_parity.rs.
     SEMANTIC CLASS ASSERTIONS (round-136 #4 — parity and token-set
     validation cannot catch a KNOWN-but-WRONG token: an auction
     route mislabeled AUX passes every structural check while
     auctions run under the cheap gate): auctioneer-runtime carries a
     hardcoded `EXPECTED_CLASSES: &[(Method, &str, &str)]` map — the
     build-script consumer asserts the generated table EQUALS it
     entry-for-entry, so a wrong class on ANY route fails the
     BUILD; `manifest_class_semantics_match_expected_map()` is the
     M3b-10 roster member pinning it.
     The zip's product is `RegisteredRoutes` — a AUCTIONEER-CORE-owned
     type (`auctioneer_core::ingress::RegisteredRoutes { by_id:
     HashMap<RouteId, RouteMatch>, table: RouteTable }`; round-134
     #1: `decide` accepts it, so auctioneer-core must own it — an
     adapter-defined type would reverse the dependency direction),
     CONSTRUCTED by the adapter's boot zip — the map EXISTS because
     the zip built it. UPSTREAM REGISTRATION SURFACE (round-134 #1 —
     "zip at boot" needs an API that does not exist at the pinned
     rev: `app!` generates handler registrations straight from
     edgezero.toml and `RouterBuilder::route` returns only the
     builder — no RouteId, no metadata, no table access; app.rs:140
     / router.rs:186). Prereq 5 therefore carries three CONCRETE
     upstream asks (feedback G16): (i) registration issues a stable
     `RouteId` per route (declaration order); (ii) routes accept
     attached metadata (`.ingress_class(u16)` on the builder, or a
     macro attribute); (iii) the COMPLETED registration table —
     `&[RegisteredRoute { id, method, template, class }]` — is
     exposed to the app after build, and THAT is what the adapter
     zips against auctioneer's RouteTable. EdgeZero hands
     the `RouteResolution` (Matched(id) / MethodNotAllowed
     { allowed } / NotFound — round-134 #1: a bare Option conflated
     404 with 405) through the bridge
     closure into `decide` (the CANONICAL five-parameter callback
     shape — round-132 #2 amended it), which resolves Matched via
     the registration-built `RouteId → RouteMatch` map — nothing
     re-matches. `body_caps::match_route` remains the M3a/dev-path
     matcher only; the dual lists and the parity test RETIRE in
     M3b-10 when generation lands. Admission class, body cap, and
     the final 404/405 decision then derive from one matched entry
     and cannot disagree.
     Auctioneer's callback: `Refuse` ⇒ answered via
     the static class, ZERO body bytes buffered; `Admit { guard }`
     where `guard` is EdgeZero's opaque
     `IngressGrant(Box<dyn Any + Send>)` newtype, stored in a
     DEDICATED `RequestContext` field, NOT in `http::Extensions`
     (whose Clone bound a boxed Any cannot meet). Ownership API is
     MOVE-SAFE and exact:
     `ctx.take_ingress_grant() -> Option<IngressGrant>` — takes the
     grant out exactly once (subsequent calls return None), so the
     handler can move the RAII permit into the response body without
     a Clone bound anywhere. Auctioneer boxes its RAII
     `AdmissionPermit`; the handler takes and downcasts
     to verify; dropping the context on ANY later rejection path
     drops the grant ⇒ the permit releases. Exactly-once across
     every terminal (complete, overflow, deadline, disconnect).
  4. **decide()** runs (the canonical Decision table, unchanged —
     `Decision` itself carries no grant; admission already happened).
  5. **Bounded body read** under the lease and the boot-owned
     absolute `max_ingress_read_ms`; expiry CANCELS the platform
     read and answers 408 (`request timeout\n`) via the static
     class. An under-cap trickle cannot hold a permit past the
     deadline. Config extraction afterwards runs under
     `max_config_fetch_ms` (M3b-1b) while the lease is held.
  Adapters predating this seam run handler-first admission in DEV
  builds only and cannot earn a memory certificate. (Header-cap
  enforcement layers, protocols, counting, and knob values are ALL
  defined in step 1 above — round-129 #1 removed the older duplicate
  counting paragraph here, which predated field-section accounting
  and contradicted it.) Gate-full ⇒ the
  static-class 503 with zero body bytes read. Without this hook the
  whole-isolate envelope cannot cover not-yet-admitted concurrent
  uploads, and slow-upload time escapes tmax; an adapter lacking it is
  certifiable for DEV only (§1 item 1). **Round-118 F04 — the lease is
  time-bounded, not just size-bounded:** the ingress read runs under a
  boot-owned ABSOLUTE deadline (`max_ingress_read_ms`, envelope const —
  the request's own tmax is unknowable until the JSON arrives, so it
  cannot govern this phase). Expiry CANCELS the underlying read (a
  notification cannot unblock a pending read), answers 408 through the
  static-egress class, and releases the lease EXACTLY ONCE across every
  terminal: complete, overflow, deadline, disconnect. Without this, an
  under-cap trickling upload holds Cloudflare's sole auction permit
  indefinitely. Conformance rows: fully stalled upload, continuous
  1 B/s trickle, disconnect mid-read, and capacity recovery after
  each.

  **GAP vs upstream — feedback G1, RESCOPED round-137 #6 (the
  round-120/121/123 description below it is historical).** At
  `12965da2` EdgeZero HAS: an all-route pre-body admission seam, a
  canonical pre-routing `request_start`, a raw framing policy, a
  read deadline, and a cancellation contract — the earlier
  "no policy point / no time bound / no start capture" asks are
  CLOSED. What remains OPEN — the G1/G16 residue this prerequisite
  now names exactly — is: (i) ROUTE-AWARE classification (the
  opaque `IngressClass` token + `RouteResolution` at the seam);
  (ii) GRANT TRANSFER (the move-safe `IngressGrant` container and
  `ctx.take_ingress_grant()`); (iii) — LANDED at `120dd2aa` as
  `RequestContext::monotonic_clock()` / `request_start()`
  (round-142 #3; no longer residue); and (iv) the PARSER header / request-target caps
  (`http2_max_header_list_size` companion knobs and the 414 seam).
  Prerequisite stands OPEN for those four; without them the adapter
  remains DEV-only certifiable (§1 item 1).

- [ ] **Prereq 9 — Egress-lifetime completion hook (round-99 B4).**
  The pinned adapters drain `Body::Stream` into a `Vec` before
  transmission, so nothing auctioneer owns survives until the client has
  the bytes. **Round-117 P1-6 — a notification alone cannot stop a
  stalled write, so this prerequisite additionally requires:** the
  adapter accepts an ABSOLUTE WRITE DEADLINE per response (the §5
  transmission leg's `deadline_at`), enforces it by ABORTING the
  connection when it expires mid-write, and exposes an idempotent
  abort handle with EXACTLY-ONCE completion reporting — one terminal
  reason per response from {completed, client_disconnected,
  deadline_aborted, transport_error}, delivered after which the
  permit release is safe. Retained-ownership-without-abort does NOT
  satisfy the prerequisite. EdgeZero must expose a per-response
  egress signal — a
  completion/drop callback (or a `Body` variant whose ownership the
  adapter holds until the final write / client disconnect) — so
  auctioneer's `PermitBody` can hold admission — and the §5 EGRESS GATE
  can hold its egress permits — through REAL transmission, with a
  response-write deadline. **Round-101 B6 — ONE contract (this
  supersedes the earlier "reservation is sufficient" phrasing): the
  egress gate (`egress_permits x final_response_bytes`, permits
  released ONLY on this hook's write-complete/disconnect signal) is
  REQUIRED, and Prereq 9 BLOCKS the whole-isolate memory
  certification** — M3b-2 implements the gate against this interface,
  and M3b-8 records the certification as an open item until the hook
  lands. *Blocks:* the M3b-2 whole-isolate memory certification (NOT
  behavior-complete); also enables the EOF/disconnect/slow-client
  adapter tests.

  **GAP vs upstream outbound spec (round-120) — feedback G3.** The
  branch covers outbound requests and inbound request bodies; the
  response-OUT direction appears only inside adapter response
  converters, which on Axum/Fastly/Spin BUFFER streamed bodies into a
  16 MiB adapter constant and hand off (outbound design §3.5.2
  footnotes 3/6/7) — no completion/disconnect signal, no per-response
  write deadline, no abort handle, and no exactly-once terminal reason
  reaches the app on any adapter. The §3.1.1 "abort the downstream
  body" rule is adapter-internal error handling, not the hook this
  prerequisite requires. Prerequisite stands unchanged and OPEN.

- [x] **Prereq 8 — EdgeZero must ENFORCE the Brotli window
  ceiling before decoder allocation, and report the bound it
  enforces (round-44 B3) — LANDED @120dd2aa (round-152 #4):
  `window_bits > max_window_bits` ⇒ `ResponseLimitReason::BrotliWindow`
  BEFORE the decoder is built (`edgezero-core/src/compression.rs`
  L181-190), the decoder charge is `brotli_decoder_memory_charge
  (window_bits)` (L79-92) against the builder's
  `max_brotli_decoder_bytes` (`ResponseLimitReason::DecoderMemory`),
  and auctioneer SETS that limit to its own `brotli_state_bytes(wbits)`
  charge so policy == charge by construction (round-140 #1). The
  text below is the derivation record.**

  auctioneer's admission gate charges every `GuestDecoded` stream
  a decoder-state weight derived from `max_brotli_wbits`
  (round-44 B2/B4). On the `RawCompressed` path that number is
  self-enforcing: auctioneer owns the decoder and its
  `parse_brotli_wbits` pre-check rejects oversized windows
  before allocating. On the `GuestDecoded` path it is
  **currently an assumption, not a bound** — the pinned CF
  adapter allocates its decoder inside EdgeZero
  ([`transform_stream`, proxy.rs:137](https://github.com/IABTechLab/edgezero/blob/9e661ae/crates/edgezero-adapter-cloudflare/src/proxy.rs#L137))
  before auctioneer sees a single byte, and nothing auctioneer passes
  to `install_admission` constrains that allocation. A bidder
  sending `br` with WBITS 24 would make EdgeZero allocate
  16.5 MiB against an isolate auctioneer budgeted at 4.5 MiB per
  layer.

  What EdgeZero must ship (round-45 B6 landed the auctioneer-side
  API into the canonical `ResponseBodyPolicy` + `ProxyClient`
  + `TypedBodyError` blocks above; these three items are the
  matching EdgeZero-side work):
  1. **Accept the ceiling.** `ResponseBodyPolicy` now carries
     `max_brotli_wbits: u8` + `max_brotli_layers: u8`
     (round-45 B6). auctioneer passes the same values it used for
     admission arithmetic, so the enforced window and the
     charged weight agree by construction.
  2. **Enforce before allocation, per layer.** The adapter
     parses each Brotli layer's header (RFC 7932 §9.1) and,
     if the declared WBITS exceeds `policy.max_brotli_wbits`,
     ends the slot with
     `ResponseLimitReason::BrotliWindow` BEFORE constructing the
     decompressor (LANDED @120dd2aa: `compression.rs` L181-190 —
     round-151 #3 replaces the retired `TypedBodyError::WbitsTooLarge`
     name). A chain with more than `policy.max_brotli_layers` brotli
     layers is auctioneer's own passthrough-decode refusal
     (`TooManyEncodingLayers`). Enforcement applies to EVERY brotli
     layer, not just the first.
  3. **Report the bound.**
     `ProxyClient::guest_decoder_memory_bound(&self, policy:
     &ResponseBodyPolicy) -> u64` (round-45 B6: now
     policy-aware) returns the state the adapter enforces for
     that policy, so `install_admission`'s round-43 B2 floor
     check compares like with like. An adapter that cannot
     enforce (1)+(2) returns `u64::MAX`, failing the boot
     check loudly rather than under-charging.

  Until this lands, the `GuestDecoded` admission weight is
  unsound and the 6-stream proof holds only for adapters on
  the `RawCompressed` path. This is the SECOND reason
  (alongside round-43 H2's `deflate`-passthrough bug) that
  Cloudflare's `GuestDecoded` claim is not yet a contract
  auctioneer can rely on.

  **Round-120 alignment:**
  - **MATCH — layers cap holds by construction.** Upstream's §3.4.1
    policy decodes at most ONE bare `br` layer per response (stacked or
    repeated encodings pass through untouched), so
    `max_brotli_layers = 1` is enforced by the policy table itself; no
    `BrotliLayersExceeded` mechanism is needed upstream. (The round-43
    H2 deflate half is also closed there — see Prereq 2's round-120
    block.)
  - **GAP vs upstream outbound spec (round-120) — feedback G8.** The
    WINDOW half stands: upstream's shared streaming decoder allocates
    the brotli decompressor with NO RFC 7932 window-header inspection,
    no `max_brotli_wbits`-style policy field on `OutboundRequest`, no
    typed pre-allocation rejection, and no
    `guest_decoder_memory_bound`-equivalent reporting of the state an
    adapter enforces (§3.4.1, §7 `src/compression.rs`; the §3.4.4
    memory model explicitly EXCLUDES "gzip/brotli decoder working
    state" from its bound). **⚠ ROUND-164 #5 — THE PARAGRAPH ABOVE
    AND THE *Blocks:* LINE BELOW ARE SUPERSEDED HISTORICAL
    ANALYSIS.** They state items 1–3 as OPEN and this prerequisite as
    blocking M3b-2..M3b-5, which contradicts this block's OWN heading
    (`[x] LANDED @120dd2aa`) and the executable checklist, where
    Prereq 8 is landed with file:line evidence: the window gate runs
    BEFORE decoder allocation (`compression.rs` L181-190) and
    `max_brotli_decoder_bytes` is the reported bound auctioneer sets to
    its own charge. The G8 feedback entry is closed by that landing.
    Retained for the derivation of the original ask only.
  *Blocks (VOID — round-164 #5):* ~~Tasks M3b-2, M3b-3, M3b-4,
  M3b-5 (round-133 #8)~~ — nothing. Prereq 8 is LANDED.

## Non-goals for M3b

- Anything already in M3a — do not re-land it here.
- GPP-derived GDPR consent (D.1) — that's M4.
- (No exclusion — round-27 H5 pulled response-decompression
  streaming + compression-bomb regressions into Prereq 2 /
  Task M3b-7 because the cap must be defined on decoded bytes.)

## Next steps

Round-112 H3: the expansion protocol is defined at the top of the
Tasks section — per-task, gated on each task's own prerequisites
landing upstream, expanding the task body into M3a-style TDD steps
against the REAL pinned APIs, re-reviewed before implementation.
The design layer (deliverable lists, contracts, envelope, roster)
is already normative and is not reopened by expansion. Until the
first prerequisite ships, the M3a plan carries M3 forward on its
own.

---

## Carried from the M3a final whole-branch review (2026-09-04, branch `m3-gdpr` @ 43477f2)

The M3a final review returned no blocking findings. Five advisory items
were adjudicated record-don't-fix at the final gate and are carried here
so they survive the (git-ignored) execution ledger. A1 and A2 are the
mechanical content of the `DevComplete → BehaviorComplete` promotion.

1. **A1 — the load-bearing caveat: schema-invalid config degrades to
   GDPR-off.** `config/runtime.rs` folds EdgeZero's `ConfigOutOfDate`
   ("no blob yet" and "blob fails deployed schema" are
   indistinguishable in the pinned extractor) into
   `AuctioneerAppConfig::default()`, whose baked default has
   `[gdpr.tcf2] enabled = false`. One mistyped value in a pushed blob
   → every auction forwards IDs/geo/IP to every bidder, WARN-only.
   Corrupt-blob and store-outage classes already fail closed; only
   OutOfDate degrades. This is the stated reason
   `MILESTONE = DevComplete` and the promotion barrier refuses such
   builds for production. Closing it (typed store error upstream, then
   route schema-invalid to the 500 arm) is a hard prerequisite for
   flipping the milestone constant.
2. **A2 — the promotion barrier's mechanical half ends at
   `promote-production.sh`. ⚠ ROUND-179 #8 — NEITHER HALF OF THIS
   ITEM HAS AN EXECUTABLE OWNER.** The central response finalizer and
   the `--expect-milestone` flag are both stated as hard promotion
   prerequisites below, and no task in this plan creates either: they
   are not in M3b-8 (which flips the constant but does not stamp
   responses), not in any 2a/2b unit, and not in M3b-10. They belong
   to the PRODUCTION gate rather than behavior-complete, so they do
   not block M3b-8 — but promotion cannot happen without them, and an
   unowned prerequisite is how a barrier silently fails to exist.
   They need a unit, provisionally **M3b-13 (promotion barrier)**
   (round-184 #1 renumbers it from M3b-12, which was a NAME COLLISION
   with the retired second-pin task — two different units carried
   the same label),
   expanded before any production claim. Filed here beside the
   requirement rather than in a separate note so the two cannot
   drift.** `auctioneer-cli deploy` and the raw adapter
   commands in `edgezero.toml` are not themselves milestone-gated; the
   barrier binds under the documented invariant that production
   credentials live only in the CI job running the script. Close the
   loop mechanically: an `--expect-milestone` flag on `healthcheck`
   that asserts the `x-auctioneer-milestone` response header — and
   round-118 F17: stamping becomes a CENTRAL RESPONSE FINALIZER, not a
   status-handler courtesy. Task M3b-1's early error paths currently
   bypass the only stamping site, so exactly the config outage a probe
   investigates would remove the stamp; and once MILESTONE is the
   cross-target FLOOR (round-117), an uncertified-target floor would
   understate a certified Axum deploy. The finalizer stamps EVERY
   response — success, degrade, 500/503, overload — with the RUNNING
   adapter's target_certification(), and tests cover config-error,
   overload, and mixed-certification cases. It is the same finalizer
   post-deploy probe fails a wrongly-promoted build instead of merely
   printing a suggestion.
3. **A3 — host-alias-typo params passthrough.** A host alias whose base
   is unknown is dropped WITHOUT joining `dropped_alias_names`, so
   `is_known_bidder` does not recognize it and the competitor strip
   retains `imp.ext.<name>` in EVERY bidder's outbound body (bidder
   params/FPD, not user IDs — not a privacy breach). NOTE the tempting
   one-liner (add the name to `dropped_alias_names`) CHANGES a
   documented contract: the aliasgvlids guard skips dropped names, so
   a genuinely-unknown alias named in aliasgvlids would stop 400ing.
   Decide the pair together, or document the passthrough edge.
4. **A4 — suppression census at end of M3a:** every suppression in the
   tree is a narrowly-scoped, reasoned `#[expect]` (zero `#[allow]`);
   the final review judged all defensible and none touch a correctness
   lint on the request path. Census list lives in the review record.
5. **A5 — `Step 0..Step 11` comments in
   `crates/auctioneer-refresh-gvl-archive/src/main.rs`** document the transaction
   algorithm's fixed order, not review lifecycle; twice ruled
   legitimate, but renaming to descriptive phase names would satisfy
   the letter of the no-markers rule at zero cost. Owner's call.

Operational notes worth keeping: the GCL freshness gate trips WEEKLY
(next 2026-09-10T16:01Z); it cannot be pre-empted, only healed
post-expiry via `cargo run -p auctioneer-refresh-gvl-archive` + commit (~2 min).
The snapshot fixtures pin GVL list 174, aging out of retention ~2027-02;
the affected row fails with a self-explaining re-encode message.

- **Carried from post-review round 3 (concern raised in execution):** a
  rejected config-store key (`BadRequest`) now fails the auction and
  `/info` closed, but `/status` keeps its permissive liveness contract —
  and unlike an outage this condition never self-heals, so a probe
  watching only `/status` sees a healthy instance whose config can never
  load. M3b should decide the deep-health surface for
  config-load-permanently-failing (a field on `/internal/health/gvl`, or
  a dedicated readiness route) rather than overloading the liveness
  probe.

# EdgeZero Outbound Adoption Plan

Current integration: [the subsequent PR #275 update](../../testing/evidence/edgezero-pr275-update-20261030.md)
advances the workspace to `9fe6c56c`. The completed adoption and evidence below
remain the historical `0a04ddb0` migration, not a recertification of that evidence.

> **For agentic workers:** Use superpowers:subagent-driven-development or
> superpowers:executing-plans. Preserve the existing Auctioneer outbound requirements;
> this is an SDK migration, not permission to weaken them.

**Goal:** Adopt IABTechLab/edgezero PR #275 at
`0a04ddb083147a1161eb46610bdd9644b3c92cbe`, verifying the retained outbound
contract and identifying any capability or accounting gap explicitly.

**Architecture:** Keep the bounded Auctioneer facade and wave scheduler. Use the
SDK's prepared dispatch, immutable batch receipts, paired time and scope-owned
operations directly. Remove retired API uses rather than adding compatibility
constructors or bypassing preparation in test clients.

**Tech Stack:** Rust 1.98.1, existing Cargo workspace, all four EdgeZero adapters.
The SDK's tool-version declaration currently matches Auctioneer's.

## Release Source Availability

The immutable `0a04ddb0` integration pin is not on EdgeZero `main`. A
[GitHub comparison](https://github.com/IABTechLab/edgezero/compare/87456c708ffe9edf86b3a0f8b06dfd15d60b07b5...0a04ddb083147a1161eb46610bdd9644b3c92cbe)
on 2026-10-30 finds it 485 commits ahead of `main` and still in the
open draft PR #275's ancestry. This continues the prior PR-branch integration
practice; it is not a new outbound-contract regression. Keep the exact reviewed
revision rather than silently switching to `main` or a moving branch.

A rebase does not itself change the pinned bytes or automatically make them
unavailable. Fresh builds do depend on upstream continuing to serve the commit.
Before release, resolve that availability dependency through a merged/audited
revision or a maintained immutable release reference. The same source review
must retain the required Spin SDK fork at `6c59ed5b`: equal package versions do
not make registry and Git SDK sources interchangeable. The fork change is
already recorded in section 1 and the local verification record below.
No upstream merge, durable release reference or SDK upgrade is claimed here.

## Source Audit Checkpoint (2026-10-30)

Local-file comparison of `bc300ace` and the exact PR head establishes the
following source-level facts, not a verified Auctioneer upgrade. SDK paths below
refer to `0a04ddb083147a1161eb46610bdd9644b3c92cbe`. Builds, runtime acceptance,
allocation correspondence and hosted qualification remain separate evidence;
this checkpoint checks no implementation or verification box below.

| Requirement | Source-audited disposition |
| --- | --- |
| Cache bypass and no implicit redirects | Retain explicit `OutboundCachePolicy::Bypass`. Plain sends/batches do not opt into the new explicit redirect-following API. All four SDK capability declarations retain Native cache bypass. Auctioneer's required manifest rows remain `ingress-admission` and `outbound-cache-bypass`. |
| Absolute deadlines and timing | Prepared dispatch captures the original absolute request deadline and paired application clock/timer. Earlier-own-deadline timeouts now use logical expiry, not delayed observation, for their elapsed value; queued receipts sort by that terminal time and input index. Successes and earlier typed failures retain their observed terminal sample. This is a semantic delta, not an accessor rename. |
| Positional receipts, EOF and partial results | Consume immutable receipts without reconstructing them. Completion still requires all slots and clean driver EOF. Mixed observation/collection distinguishes `Resolved`, `AlreadyDelivered` and `Unresolved`; cutoff holes must not be mistaken for delivered slots. Preserve Auctioneer's fatal-driver rollback rather than publishing partial success after a driver invariant failure. |
| Encoded, decoded and final-buffer caps | Independent controls remain. The decoded counter covers identity and SDK-decoded output, not raw encoded passthrough. The SDK now leaves recognized codings encoded on all `206` responses, even with default Decode policy. Final buffered collection remains independent and includes raw passthrough. |
| Header, chunk, decoder and window limits | Header caps charge adapter-visible fields before normalization, plus the synthetic marker when enabled. `max_chunk_bytes` bounds emitted items after decoding/passthrough, not native receive allocations or decoder-input allocation. Decoder/window checks retain their pinned policy charges, not a complete allocator guarantee. |
| Fidelity and cancellation | Spin's advertised header fidelity changes from Native to BestEffort; this is not a blanket loss of adapter-visible Content-Type provenance. Cloudflare remains Unknown for original Content-Type evidence. Axum alone advertises Native outbound deadlines/cancellation; the other three remain BestEffort. |
| Ownership and resource accounting | Prepared contexts, scope roots, progress/task ownership, batch ledgers, queues and boxed receipts change SDK bookkeeping. Local exactly-once cleanup is not proof that opaque host work stopped or that the new bookkeeping fits existing charges. Complete outbound resource accounting remains Unsupported on all four adapters. |

Source anchors: SDK `crates/edgezero-core/src/outbound/dispatch.rs:81`,
`outbound/batch.rs:608` and `:738`, `outbound/head.rs:145` and `:254`,
`outbound.rs:1515`, `outbound/scope.rs:540`, and the four adapter `src/cli.rs`
capability declarations. The exported limit setters and
`docs/guide/capabilities.md:533` agree with the payload-policy implementation's
independent cap scopes and raw-`206` exception. Bodyless suppression in the head
finalizer precedes payload shaping; an encoded-counter claim must not be read
as a bound on bytes already read or allocated by the provider. Header parsing,
hidden field sections/trailers, native chunks, spare capacity and runtime
copies remain outside the complete guarantee. The guide's approximate buffered
payload formula is not a bound for the new scope/task bookkeeping.

The selected minimal Auctioneer adaptation changes only post-SDK encoding
classification: `Identity` is `ReadyToParse`; surviving `Brotli`, `Deflate` or
`Gzip` is `RawPassthrough(Unclassified)`. The SDK strips those recognized
headers only when it actually decoded the body. Refuse surviving encoded bytes
before bidder parsing/debug projection; do not label a recognized codec
Unsupported, introduce another enum, decode locally, or add a global status
rule. This protects raw `206` coding without changing existing identity/2xx
status handling. Existing malformed/stacked/unsupported coding classifications
remain distinct.

Forward the new SDK's authoritative elapsed values without sampling a consumer
clock to invent missing observation timestamps. For example, an earlier own
deadline at 10 ms observed at 30 ms before a 100-ms batch cutoff now reports
10 ms. Accept and document that new meaning and logical-expiry ordering;
preserve Auctioneer's originally selected timeout cause and absolute deadlines.

### Integration Changes

- SDK batch observation drains and orders already-ready provider events before
  returning a receipt. Auctioneer still prioritizes its ready settlement work when
  polling the consumer boundary, but cannot promise interleaving inside the
  SDK's own drain. Separate ready-burst and staged-source controls distinguish
  these contracts; a staged fixture is not evidence of provider preemption.
- Standalone Axum outbound calls now require an active Tokio `LocalSet` or
  `LocalRuntime`. The production SDK entrypoint supplies this. Direct socket
  tests must use the same context, not replace the SDK progress driver.
- SDK body-cap failures now carry typed `PayloadTooLarge` with an overflow
  origin. Auction and cookie-sync handlers retain their exact endpoint-specific
  400 size responses for inbound overflow. Generic auxiliary drains forward
  the SDK's 413 and fixed `payload_too_large` category. Outbound-upload and
  unrelated source failures must not be mistaken for inbound size errors.
- Response-egress fixtures select `ExplicitBufferedLength` when beginning an
  explicit writer attempt. Fastly uses the SDK-owned application lifecycle;
  Spin returns the SDK's transmission-owned response wrapper. Assembly errors,
  response-completion ownership and bounded liveness are not bypassed.
- Buffered fixture payloads move into Send/Sync script recipes before SDK
  preparation, then into the prepared provider's response. Tests do not forge
  immutable receipts, construct hidden contexts, use unsafe thread markers, or
  shuttle contexts through a global/thread-local bridge.

The allocation audit found no established numerical underbound, but also no
correspondence proof assigning the SDK's new budgets, ledgers, queues, boxed
receipts, scope registrations and cleanup scratch to a funded Auctioneer owner.
Auctioneer's request-derived response-bookkeeping term is not such a proof merely
because it is large. This remains an explicit resource-qualification gap; no
workload limit, ceiling, budget coefficient or historical artifact is changed.

## 1. Pin And Audit

- [x] Compare the old SDK `bc300ace` with the exact PR head. Review request/body,
  dispatch/batch, time/scope, error, assembly and provider changes.
- [x] Update six workspace dependencies and regenerate Cargo.lock. Preserve
  audited codec pins and all unrelated dependency versions unless required by
  the new SDK. Do not edit historical evidence, frozen profiles or baseline
  records to disguise incompatible inputs.
- [x] Align Auctioneer's direct `spin-sdk` and EdgeZero's transitive dependency to
  the same source: version `=7.0.0`, Git URL
  `https://github.com/stackpop/spin-rust-sdk.git`, revision
  `6c59ed5b4fcb377072763aee0b27a3abdd8edb72`, with default features disabled
  and `http`, `key-value`, `variables` as declared. Check the resolved lockfile
  source and selected WASI/error-type correspondence, not just the equal
  `7.0.0` version string. This is a required source change from the old
  registry dependency, not unrelated dependency churn.
- [x] Update active CI source selection, current pin guards and SDK-root lookup.
  Keep historical pin expectations historical. Run codec/workflow guards.

## 2. Preserve The Production Outbound Contract

- [x] Compile core and the four applications against the new SDK, capturing the
  actual failures before adapting calls.
- [x] Consume indexed receipts through accessors/consuming decomposition. Retain
  the SDK's authoritative terminal ordering, including logical-expiry ordering,
  original positional ownership, clean EOF validation, Completed/Cutoff
  distinction, partial results and exactly-once release.
- [x] Preserve the original per-slot absolute deadlines and timeout cause;
  forward the SDK's elapsed values with the changed logical-expiry meaning
  recorded above. Use the SDK's paired clock/timer and scope contract; do not
  restart budgets after preparation or wave contention, or invent unavailable
  observation timestamps.
- [x] Retain cache bypass, no implicit redirect following, independent transport,
  decoded and final-buffer caps, header count/byte caps, decoder/window limits,
  source diagnostics, content-type provenance and failure classification.
- [x] Apply the selected post-SDK encoding classification above so surviving
  recognized coding, including raw `206` content, fails closed before parsing.
  Keep independent transport/final caps, identity decoded caps and the existing
  unsupported/stacked/malformed distinctions; no new global status predicate.
- [x] Record Spin header fidelity as BestEffort and retain evidence-based
  Content-Type/debug eligibility. Do not upgrade Unknown provenance or infer
  raw provider field fidelity from a normalized singleton header.
- [x] Replace private Body variant matching with the supported Body/BodyPayload
  API without copying or streaming buffered payloads unnecessarily.
- [x] Adapt fallible application assembly/provider entrypoints without removing
  bounded health/liveness, admission or response-completion ownership.

## 3. Test Clients And Regression Evidence

- [x] Migrate supplied clients to prepared requests/batches and immutable timing
  contexts. Use shared test-support APIs, no retired receipt constructors,
  handwritten source imports or fake default timer/progress implementations.
- [x] Run core outbound/wave/classification and deadline/cancellation tests;
  preserve existing populations and unaffected fixed expectations. Change only
  expectations justified by the source-audited semantic deltas above, with
  explicit regression controls rather than accepting false greens.
- [x] Exercise raw `206` Brotli/deflate/gzip as Unclassified refusals, ordinary
  decoded responses as parse-ready, and unchanged identity/2xx and existing
  passthrough behavior. Require an actual failing old-classification control.
- [x] Exercise earlier-own-deadline expiry observed late, logical-expiry
  ordering against a successful sibling, and original-index publication.
  Assert authoritative SDK elapsed values without reconstructing timestamps;
  retain own-deadline versus observation-cutoff and trailing-EOF fault controls.
- [x] Run adapter fanout/conformance and Axum socket tests, plus native and three
  guest builds. Local socket permissions may be needed; no hosted deployment or
  credential/config mutation is authorized by this plan.
- [x] Inspect changed SDK allocations/layouts against existing funded bounds.
  A passing compile is not a resource proof. Do not raise limits, lower demand
  or promote unknown host guarantees to make the update pass.
  Include scope registration/revocation and cleanup scratch, task/channel
  ownership, prepared contexts, per-batch ledgers/queues and receipt boxing.
  Record unresolved correspondence explicitly; do not treat unchanged payload
  caps, `size_of` checks or finite allocation observations as complete bounds.

## 4. Publish The Result

- [x] Run formatting, strict affected-package Clippy, pin/workflow/source-import
  guards and affected regression/integration suites.
- [x] Record exact verified commands and any remaining SDK/provider limitations
  in ISSUES.md and the SDK update record. State whether each outbound requirement
  is retained, changed or unverified. Do not recertify historical observations.
- [x] Commit and push the verified repair round. Keep M7's schema qualification,
  priority activation and unrelated bidder-layout work separate and explicit.

## Local Verification Record

Verified on October 30 against PR #275 head
`0a04ddb083147a1161eb46610bdd9644b3c92cbe`; a final GitHub API read still
reported that head. All six `.tool-versions` entries match the SDK byte for
byte, including Rust 1.98.1. The exact public Spin fork is aligned at
`6c59ed5b4fcb377072763aee0b27a3abdd8edb72`. WASIp3 was already used before
this update; the fork changes its bindings from WASI 0.3.0 to 0.3.2. The SDK's
exact-fork audit reports Spin 4.1.0 wire checks, not independent Auctioneer hosted
qualification. No extra Spin runtime flag or tool-version change was justified
by the inspected source.

Commands used `CARGO_INCREMENTAL=0`, `--locked --offline -j 2`. Source-dependent
tests supplied `AUCTIONEER__BIDDER__SOURCE_DIR`,
`AUCTIONEER__BIDDER__SUPPLEMENTAL_SOURCE_DIR`, and the exact-head `EDGEZERO_SDK_ROOT`.
Local socket tests ran with loopback access. These are local test permissions,
not hosted deployment or credential/config mutation authorization.

| Check | Observed result |
| --- | --- |
| `auctioneer-core --features conformance-suite --all-targets` | 100 binaries, 1,263 passed, 0 failed, 1 existing ignored test; includes 570 core unit tests and 123 workflow guards. |
| All four application `--all-targets` suites | 34 binaries, 204 passed, 0 failed, 6 existing ignored vendor-loader/guest-only tests. Includes 14 real Axum transport rows and the real-socket privacy matrix. |
| Three explicit `--features conformance-suite --test fanout_conformance` targets | 14 passed on each adapter, 42 executions total. Feature-gated targets were selected explicitly, not assumed covered by `--all-targets`. |
| `auctioneer-cli --bins --lib` | 710 binary tests and 36 library tests passed; 0 failed, 2 existing ignored tests. |
| Final-source CLI `bidders::resources` filter | 146 passed, 0 failed; rechecks resource/source identity after compile-only lint fixes. |
| `auctioneer-bidder --lib` | 1,133 passed, 0 failed, 9 existing ignored compiler/source probes. |
| `auctioneer-bidder --test test_isolation` | 16 passed, 0 failed. |
| Native strict Clippy, core/CLI/all four applications, `--all-targets --features conformance-suite -- -D warnings` | Passed. |
| Production guest Clippy, each application `--lib --bins --target TARGET -- -D warnings` | Passed for wasm32-unknown-unknown, wasm32-wasip1 and wasm32-wasip2. |
| Formatting and whitespace checks | `cargo fmt --all -- --check` and `git diff --check` passed. |

The compressed-partial regression was observed failing under the old
classification (`ReadyToParse` versus `RawPassthrough(Unclassified)`), then
passing after the fix. Actual SDK body-policy processing preserves the
partial bytes and coding in that test. Socket rows separately prove ordinary
gzip decoding, distinct encoded/decoded caps, EOF refusal, deadline propagation,
cancellation and original header evidence. Ready-burst controls pin SDK
draining, immutable indices and logical-expiry ordering; staged-source controls
test Auctioneer consumer fairness without claiming SDK preemption. Pre-expiry
misleading-cause rows independently retain timeout authorship/discrimination.

Initial migration runs caught and repaired old receipt constructors, lack of
Tokio local-task context, assumptions that finalization never yields, changed
inbound overflow taxonomy, stale Fastly lifecycle source guards and one active
CLI fixture carrying the previous SDK pin. A CLI run without reference-path
variables/loopback access was not counted as acceptance. Existing frozen
resource profiles, captured evidence, codec versions, vendor patches and all
workload/accounting coefficients remain unchanged. The two existing Serde
callback lint expectations now apply only on 64-bit targets, where the lint
actually triggers; no new broad lint allowance was added.

### Remaining Limits

The broader guest CI command selecting exchange/privacy `--all-targets` did
not pass. Cloudflare encountered the unchanged
`auctioneer-exchange/tests/run_auction/main.rs` import of native-only
`test_support::serving_catalog`; Fastly and Spin encountered the unchanged
`construction_bound.rs:1015` Option-style call on `JsonField`. Those M7 test
portability/setup blockers are recorded in ISSUES.md. They are not a successful
guest test build, a new SDK API failure, or permission to skip CI coverage.
Only production guest checks are green. Hosted runtime acceptance was not run.

The existing proc-macro-error2 future-incompatibility warning remains tracked;
no dependency patch or suppression was added. Complete outbound allocator
qualification, the new SDK-owner correspondence proof, absolute WASM fit,
selected-validator admission and full M7 completion remain open. Local passing
tests do not turn Unsupported/BestEffort facts into Native guarantees or
recertify historical qualification artifacts.

### Publication

Migration commit `d7a5e85928d960cde2b12e56966fb7c83d196004` was pushed to
`origin/m7-bidder-catalog`. After the commit, the exact
`report_uses_normalized_baked_inputs_and_all_runtime_owned_terms` CLI test
rebuilt and passed (1 passed, 0 failed), confirming that the reporter binds
the published source state rather than relying on the pre-commit build.

### Guest Test Repair Follow-Up

The [bidder maintainability repair](2026-10-30-bidder-maintainability.md)
addresses the failed broader guest checks recorded above. Filesystem-backed
provider conformance in `run_auction`, `sparse_request_copies`, `auction_debug`
and `targeting_prefix` is now explicitly native-only; the existing portable
fake-based controls remain in each guest test target. The native full exchange
run still executes those provider modules (641 passed across 36 targets, with
one existing ignored test). `construction_bound` now constructs the existing
presence-aware `JsonField` explicitly instead of calling an Option method.

The unchanged CI strict-Clippy command selecting the adapter, exchange and
privacy with `--all-targets --features conformance-suite` now passes for all
three actual deployment targets. No feature was removed, no test was newly
ignored and no lint exemption or CI narrowing was added. This supersedes only
the local test-portability blockers; the historical failed runs above remain
failed. The SDK pin, outbound semantics and open resource correspondence and
hosted-acceptance limits are unchanged.

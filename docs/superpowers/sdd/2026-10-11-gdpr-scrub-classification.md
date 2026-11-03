# Classified Disclosure Verification

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

Date: 2026-10-11
Branch: `privacy-enforcement`, worktree `.claude/worktrees/privacy-enforcement`
Implementation base: `3f0f7f5aeb01d96742e0dd2012f179887ee474bc`
Implementation commit: `c2b00a2a86a564301bd51ac9582055542b08010e`, pushed to
`origin/privacy-enforcement` and verified against the remote head.
Status: local implementation qualification passed and published; verified
source integrations published as user sync `9ff017f` and diagnostic qualification `6177742`.
Ready for code review, not an invented human merge approval.

This record covers the classified-scrub cutover and construction funding
amendments committed above, not the implementation base alone.
The [design](../specs/2026-10-09-gdpr-scrub-classification-design.md) and
[plan](../plans/2026-10-09-gdpr-scrub-classification.md) govern the scope.
The older [privacy enforcement verification](../plans/2026-09-28-privacy-enforcement-verification.md) is
chronological baseline evidence, not proof of this cutover.

## Changes

- One live per-object mutator uses compiled disclosure rules for typed fields
  and flattened maps. The old implementation is a frozen test comparator only.
- Reviewed Geo context and `kwarray` survive permitted disclosure; unknowns,
  malformed protected shapes, accuracy and invalid coordinates remain closed.
  ID/geo/FPD/TID, activity, CCPA/LMT, COPPA and shadows compose in one path.
- Borrowed source and supplemental depth/node/byte checks precede cloning.
  Survivor validation keeps code-15 precedence over an expired deadline.
- The neutral construction descriptor funds actual audit ownership, reusable
  scratch and retained IPv6 growth. Core observes plain aggregate counts once,
  before releasing lease/reservations, without payloads or bidder identities.

## Funding Amendments

Standard IPv6 rendering can grow a tight destination. Preserve its canonical
output with `try_reserve_exact` before mutation; reserve failure aborts with a
static construction error. Two 39-byte address buffers establish the independent
retained allowance `G = 78` bytes per survivor. Admission checks
`P + K * max(installed_G, engine_G) + max(installed_S, engine_S)` before
materialization. Growth and scratch are not conflated, including at zero survivors.

Compact numeric tokens can expand during serialization: `1e15` occupies four
source bytes and seventeen normalized bytes. The source allowance is now
`3B + 8H + 189 + 23N`, using admitted input-node cap `N`. The bound follows
`serde_json 1.0.150`'s finite-float formatter, `zmij 1.0.21`, whose buffer is
24 bytes; signed integer widths are smaller. Independent finite-exponent,
mantissa and integer-boundary tests supplement this source bound, not an
exhaustive enumeration of all floats. Extra implicit nodes are funded separately.
All four installed proof rows retain their own old baselines plus explicit deltas.
No generic factor, node cap, target ceiling or artifact budget was increased.

## Executed Checks

Commands use Rust `1.95.0` (`59807616e`, LLVM `22.1.2`), locked dependencies,
offline Cargo and two build jobs. Native workspace execution uses Linux Docker,
four test threads and a 6 GiB container limit. Source mounts are read-only but
not snapshots: final test-only style repairs require subsequent fresh tests.

| Check                                                                                              | Result                                                                                                             |
| -------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| `cargo test --locked --workspace --all-targets --features conformance-suite`                       | 2,420 passed, 0 failed, 7 explicitly ignored, 177 target summaries; includes all 61 isolated config-build cases    |
| Workspace all-target strict Clippy, `conformance-suite`                                            | Passed after test-only lint repairs                                                                                |
| Fresh post-repair workspace execution, `conformance-suite`, excluding config                       | 2,155 passed, 0 failed, 7 ignored, 167 targets; config's complete 61-case harness passed in the preceding full run |
| Final workspace execution, `insecure-bidders conformance-suite`, excluding config                  | 2,164 passed, 0 failed, 7 ignored, 167 targets                                                                     |
| Workspace all-target strict Clippy, `insecure-bidders conformance-suite`                           | Passed after the final charged-failure test                                                                        |
| Exchange all targets, `insecure-bidders`                                                           | 477 passed, 0 failed, 1 ignored compiler mutation separately executed                                              |
| Real charged HTTP lifecycle, including later-scope failure                                         | 16 passed on Linux and then natively with four test threads after allocator-style repairs                          |
| Cloudflare/Fastly/Spin all-target strict Clippy, `conformance-suite`                               | Passed on `wasm32-unknown-unknown` / `wasm32-wasip1` / `wasm32-wasip2`                                             |
| Required ignored model-field compiler-mutation gate                                                | 1 passed, 6 filtered; actual disposable-workspace mutations executed                                               |
| Fresh WASI borrowed JSON preflight                                                                 | 4 passed with Wasmtime's maximum WASM stack set to 65,536 bytes                                                    |
| Fresh WASI typed decode, materialization, removal, cancellation and caller-owned rejection/cleanup | 6 passed with the same 65,536-byte limit                                                                           |
| Raw JSON fixture gate and its rejection fixtures                                                   | Passed: 92 fixtures in 36 files; compact numeric source has an exact-text exemption                                |
| Roster and its rejection fixtures; batch-API documentation gate                                    | Passed: 139 members; all negative cases reject as intended                                                         |
| Spin vendor `spin build`                                                                           | Passed; component 25,540,986 bytes against unchanged 50,000,000-byte ceiling                                       |
| Fastly vendor `compute build --package-name auctioneer`                                                | Passed; package 8,212,990 bytes against unchanged 100,000,000-byte ceiling                                         |
| Cloudflare vendor Wrangler dry-run                                                                 | Passed; reported 24,463.60 KiB raw / 3,829.31 KiB gzip against unchanged 60,000,000 / 9,000,000-byte budgets       |
| Local Fastly ignored logging/health-store drivers                                                  | 3 passed; fresh Viceroy guests execute the logging/filter and health store/flag cases                              |
| Catalogue importer check; formatting and whitespace checks                                         | Passed                                                                                                             |

Spin is `4.1.0`; Wrangler is `4.142.0`; Viceroy's host driver is `0.21.0`.
Fastly CLI is `16.0.0` rather than CI's `16.1.0`.
Fastly's optional binary-annotation tool could not change permissions in its
external cache; the vendor build nevertheless exited zero and produced the
measured archive. Wasmtime is `45.0.0`. Executed pure WASI engine tests do not
establish JavaScript-host or provider runtime behavior.

## Regression Evidence

- The old IPv6 zero-allocation assertion failed on a real canonical growth
  destination. New typed/shadow tests require bounded growth, repeated-call
  idempotence, no allocation with spare capacity and static reservation failure.
- The exact/below projection regression first rejected the implementation's
  missing `G` charge. Two earlier fixture trials were invalid witnesses because
  normalization removed their private IP or observation inspected encoded bytes;
  public-address and adapter-input presence controls replaced them.
- Numeric funding first failed the independent projection golden. The compact
  50,000-token HTTP request then returned 500 before the allowance amendment;
  its corrected real-handler test returns 200 and dispatches a scrubbed body.
- Fresh construction admission (17), retained projection (7), retained IPv6
  (2), disclosure differential (16) and privacy wire (12) tests pass. Retained
  IPv6 scopes sweep 1/2/50, assert positive exact growth, full drop repayment,
  and an independent whole-clone control exceeding the bound.
- Five deadline tests include original-deadline sibling cutoff and a separate
  allocation-triggered expiry during gzip finalization, with a live control.
- The charged HTTP failure control proves exactly two 20-to-21-byte IPv6 resizes
  and two canonical dispatched bodies. Failing the second resize after the
  intervening clock read yields static HTTP 500, zero fanout, one error summary
  with two attempted scopes, one failed scope and the earlier coarsen count.
  The observer sees the full ledger charge and admission permit; both then refund.
- A Fastly guest-driver setup trial used an incorrect artifact path and failed
  before executing the guest. Rerunning with Cargo's structured artifact output
  passed all three explicit ignored host drivers; the setup failure is not a
  behavioral red test.
- The first broad run passed all config child builds but failed a stale
  country-only Geo expectation. Updating it to the reviewed context set kept
  denial assertions; the complete amended-cost workspace run above then passed.
- Strict lint exposed test-only duplicate imports, shadowing, numeric suffixes
  and fixture complexity. Repairs retain assertions and do not weaken lint policy.
- Raw-fixture checks first rejected the numeric source fixture's missing
  exemption; documenting its necessary compact lexical spelling made both gates
  pass without converting away the regression's input shape.

## Downstream Integration

user sync `9ff017f` merges the implementation into `aa10b26`; diagnostic qualification `6177742` carries
that verified source into `b5b1ad3`. Both source-integration commits were pushed
and compared against remote branch heads. Subsequent publication metadata is
documentation-only. Main and its unrelated `.playwright-mcp/` remain untouched.

Conflict resolution preserves user-sync routes, auxiliary work and distinct GVL
populations, together with privacy enforcement's normalized wire/nodes, actual construction
descriptor, retained address destinations and audit lifetime. All four installed
envelope proofs use independent user sync baselines plus explicit privacy enforcement deltas. The synthetic
projection fixture additionally funds user sync's auxiliary peak; no production cap,
resource factor, archive or SDK pin was raised or changed.

| Executed Combined-State Check                                                                        | Result                                                                                    |
| ---------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| user sync locked workspace/all-targets, conformance feature, excluding config harness crate                 | 2,466 passed, 0 failed, 7 ignored; 186 target summaries                                   |
| user sync same sweep with insecure-bidders + conformance                                                    | 2,475 passed, 0 failed, 7 ignored; 186 target summaries                                   |
| user sync config library/codegen/disclosure/usersync/archive/safety targets, without isolated build harness | 232 default and 233 insecure-bidders passed                                               |
| user sync strict workspace/all-target lint, both feature selections                                         | Passed                                                                                    |
| user sync strict all-target conformance lint for CF unknown-unknown, Fastly WASIp1 and Spin WASIp2          | Passed with the installed native-host toolchain                                           |
| user sync exhaustive model compiler mutation, explicitly ignored harness                                    | 1 passed; 6 ordinary rows filtered; actual disposable source mutations                    |
| user sync catalogue import, raw fixtures, roster and batch gates                                            | Passed; 99 raw fixtures/40 files, 139 roster members; raw/roster rejection cases executed |
| diagnostic qualification core/exchange/runtime library baseline                                                            | 705 passed                                                                                |
| diagnostic qualification admission, audit, config degradation, probe and usersync integration baseline                     | 54 passed                                                                                 |
| diagnostic qualification response seal compiler probes and real Axum sockets                                               | 2 + 16 passed                                                                             |
| diagnostic qualification source/build/workflow/script comparison against user sync `9ff017f`                                      | Empty; diagnostic qualification-specific changes remain documentation-only                                      |
| diagnostic qualification format, raw-fixture, roster and batch checks                                                      | Passed                                                                                    |

The fresh user sync passes do not re-execute the expensive isolated config-build
harness or vendor guest drivers; those privacy enforcement executions remain separately recorded
above, not new user sync/diagnostic qualification claims. user sync/diagnostic qualification vendor packages, hosted handlers, remote CI,
physical-isolate measurements and new diagnostic qualification behavior are not qualified by propagation.
The diagnostic qualification plan records its verified foundation separately from unchecked debug/Q2/
hosted tasks. Automatic bidder registration remains an approved, unimplemented
design, not a consequence of this cutover.

Initial downstream attempts hit a missing cached native dependency artifact,
a verification container without the WASM target, and sandbox-denied loopback
binds. Targeted cache cleanup, the installed WASM toolchain and authorized socket
execution resolved those setup failures; they are not test passes. The projection
fixture then meaningfully failed 6 rows with `AuxRoutePeakBelowDerived`
(6,415,360 configured versus 28,302,336 required); funding its independent
auxiliary phase made all 7 rows pass. Strict lint exposed a nested checked-arithmetic
test helper; rewriting it with `?` preserved calculations and assertions.
Final verification containers exited 0 without OOM. Whitespace checks exclude
the hash-pinned, byte-preserved upstream Markdown, whose original whitespace is
intentional; catalogue/digest validation still checks that exact source.

## Review Boundary

Independent production and combined-state reviews found no confirmed P1/P2
in the final scrub, amended funding, allocation-failure fixture, usersync
integration or config generation. The seven ignored native rows comprise
two CLI probes driven by their parents, one baked/runtime parity probe driven by
the config harness, the separately executed compiler mutation, and the three
explicit Fastly guest drivers. No ignored row is represented as executed merely
because its binary compiled.

AG supplied the inventory reviewer name. The recorded review date and metadata
are provenance, not an invented final human code approval. No provider physical
isolate qualification, hosted CI success, legal certification or registry-discovery
implementation is claimed. Automatic bidder registration remains separate work.

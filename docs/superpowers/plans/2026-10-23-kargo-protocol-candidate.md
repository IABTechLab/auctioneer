# Kargo Protocol Candidate Implementation Plan

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

> **For agentic workers:** Use superpowers:subagent-driven-development and test-driven-development. Keep protocol implementation separate from activation and migration evidence.

**Goal:** Port the Kargo protocol named by the existing priority-wave contract,
with source-bound conformance, without promoting a public catalog definition.

**Architecture:** One stateless factory in the existing bidder crate, discovered
from adjacent `bidder.toml`. Borrow the scoped request into the capped JSON
writer; decode through the sealed parser and shared candidate-local media
classifier. No new endpoint client, crate, schema engine or resource limit.

**Tech Stack:** Existing Rust/Serde, bounded sink/parser, shared test harness,
adjacent TOML provenance and verbatim source JSON captures.

## Contract And Limits

This executes only the protocol-candidate slice of the
[priority-wave plan](2026-10-22-priority-display-bidders.md). The source is
immutable revision `d7f8515b86258688304b0d9b6668c6a0e258bc9e`, implementation
blob `c75b681280f168e53e46b9443c2893e4086d3a81`. Kargo's already recorded
`passthrough`, `media_status` and `status_envelope` requirements are unchanged.
No pending human review, Task 18A/18B acceptance or full migration is invented.

- Send one POST of the unchanged scoped typed request, covering every supplied
  impression. Use the common endpoint resolver and sink, including its shared
  JSON headers; do not add parameter rewrites or bidder-owned headers.
- HTTP 204 is no-bid without parsing its body. HTTP 200 must parse, including
  empty/wrong-shaped bodies failing. Every other status, including 400, is
  code-4 `BadServerResponse`, not another bidder's code-2 rule.
- A string `bid.ext.mediaType` equal to `video` or `native` supplies that
  adapter hint; missing, null, unknown and non-string values fall back to
  banner. Preserve the source's single-key case-insensitive lookup, including
  its Unicode simple-lowercase dotted-capital-I spelling, without allocation.
  Multiple matching spellings fail only that bid rather than guessing the
  source's lost wire-order precedence. AG approved this single-key/ambiguity
  boundary on 2026-10-23. That decision is not approval of the pending full
  wave contract, activation or complete parity.
  The existing shared classifier still rejects contradictory standard
  hints, unknown impression IDs and unoffered formats, preserving valid
  siblings and reporting every candidate-local error.
- Preserve response currency, with the existing shared USD default for absent
  or empty currency. Preserve the remaining bid fields. Diagnostics do not echo
  bodies. The shared parser's strict object-valued extensions and node cap
  remain unchanged, rather than reproducing permissive foreign decoding.
- AG approved reuse of the existing presence-aware `JsonField<T>` on
  2026-10-23. Typed `video.mimes` now preserves missing, null and value.
  `captured_video` and `captured_no_content` compare their unmodified expected
  requests, including explicit null, with before-state presence assertions.
  Do not remove arbitrary nulls or claim complete wire parity from these tests.
- The five verbatim fixtures retain source HTTP test URLs. Those are test
  inputs through the harness, not endpoint approval. Actual production routing,
  gzip, site-only eligibility, sync, parameter/schema/resource qualification,
  original-deadline precharging and publisher acceptance remain separate gates.

## 1. Source Contract And Captures

**Files:** `src/bidders/kargo/bidder.toml`, adjacent
`fixtures/protocol/{exemplary,supplemental}/*.json`, existing priority plan.

- [x] Read the pinned source and wave operations; distinguish the 400 rule from
      source families that treat it as bad input. No foreign source program runs.
- [x] Capture all three media and two status fixtures byte-identically, with
      source blobs, byte counts and SHA-256 values in adjacent TOML. Retain the
      source implementation tree and shared license; no second provenance manifest.
- [x] Set real factory presence without adding an effective public definition,
      endpoint approval or a reviewed/migrated status.

## 2. Protocol And Inline Units

**Files:** `src/bidders/kargo/mod.rs`; existing shared harness/discovery tests.

- [x] Write inline units against a refusing candidate and observe intended
      failures, not a zero-test filter or unrelated compile error.
- [x] Implement only the scoped-request passthrough and status/media/currency
      rules above, reusing shared helpers. No request clone or uncapped wire body.
- [x] Exercise independently varied raw-body, call and impression limits plus
      endpoint denial/refunds, all offered media/hint fallbacks, non-200 status
      classes, malformed/over-node bodies, currency and mixed valid/invalid seats.
- [x] Reconcile discovered factory sets and canonical schema witnesses, while
      proving the active public metadata set remains unchanged.

## 3. Source Conformance And Integrity

**Files:** adjacent `conformance.rs`, shared test-only `bidders::harness`.

- [x] Drive all five exact captures through real sink and sealed response
      decoding. Compare request body/coverage, response fields, media and statuses.
      Only floating-point price representation is normalized explicitly; both
      explicit-null MIME cases compare without removing the field.
- [x] Verify the complete adjacent TOML witness before even a single filtered
      conformance dispatch, then verify supplied fixture bytes against it. Reject
      missing/unknown captures, same-length edits and manifest/source drift.
- [x] Audit original-source factory trees, artifact identities and retained
      bytes in existing `auctioneer-cli/tests/bidders_source.rs`. Its required checkout
      and exact CI command already exist; add no ignored test, target or workflow.
- [x] Keep common capture-verification machinery in the shared harness rather
      than copying a private verifier or fixture macro into this bidder.

## 4. Review, Verify And Publish

- [x] Independent source/spec and quality review; repair and recheck findings.
- [x] Run secure and feature bidder libraries/conformance with explicit source
      verification, discovery/join/import/config controls, target compilation,
      strict lint, formatting and fixture gates. Report exact scopes and limits.
- [x] Update `ISSUES.md` and priority-plan progress, commit and push the verified
      candidate to the existing bidder catalog PR. Do not claim new serving activation, complete
      parameter validation, full migration, live-provider evidence or resource qualification readiness.

## Verification Record: 2026-10-23

Published implementation: `8b8e83246b9b8d463225e5435f91b105f3a36728` on the
existing `bidder-catalog` branch and PR #21. This completes this candidate
increment, not either wave-acceptance checkpoint or the overall capability.

All runs used Rust 1.98.1, the unchanged lockfile and SDK pin, offline locked
Cargo with two build jobs and incremental compilation disabled. Secure and
feature package runs were separate, not workspace-unified test claims.

| Scope                                                         | Secure Defaults | `insecure-bidders` |
| ------------------------------------------------------------- | --------------- | ------------------ |
| Bidder library, including source conformance                  | 305 passed      | 316 passed         |
| `registry_join`                                               | 10 passed       | 10 passed          |
| `bidder_request_allocation`                                   | 7 passed        | 7 passed           |
| `catalog_alloc`                                               | 5 passed        | 5 passed           |
| Config lib, codegen, bindings, storage, smoke, baked defaults | 244 passed      | 245 passed         |

Both library runs used `--include-ignored` with `AUCTIONEER_APS_SOURCE` supplied;
none were ignored. The existing isolated `factory_binding_build` and
`registry_build` targets each passed 10 tests, including their real secure/
feature child builds and intended incremental failures. Existing CLI
`bidders_import`, `bidders_source` and `bidders_wave` passed 6, 13 and 9 tests
respectively with the pinned source checkout; the two new original-factory
source controls passed again after checked-counter cleanup.

The 12 initial protocol units failed against a refusing stub; the three shared
capture-integrity controls failed before implementation. The added single-key
and ambiguity controls then failed on the old exact-key lookup. All 22 Kargo
units/conformance cases pass in the final library runs. The existing real-Git
checkout control now checks 13 inputs, adding Kargo's full manifest and a
protocol capture. It failed on manifest newline conversion before the scoped
`-text` rule and passed afterward, retaining its unprotected conversion control.

The bidder library passed `cargo check --lib` on `wasm32-unknown-unknown`,
`wasm32-wasip1` and `wasm32-wasip2`. Workspace/all-target Clippy passed with
`-D warnings`, separately with defaults and with
`--features insecure-bidders,conformance-suite`. Workspace formatting and direct
formatting of discovery-owned bidder sources passed. The raw-fixture gate and
its failure controls passed: 172 fixtures across 52 files. Existing deleted-GVL
and `proc-macro-error2` future-incompatibility notices remain unsuppressed.

Two independent read-only source/spec and quality reviews found no actionable
defects; final test-only lint cleanup was rerun. These are candidate-local and
native test/compilation results, not full-workspace test execution, application
packages, physical-memory qualification, provider acceptance or wave delivery.

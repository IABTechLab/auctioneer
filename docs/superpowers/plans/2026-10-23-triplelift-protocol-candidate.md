# Triplelift Protocol Candidate Implementation Plan

> **For agentic workers:** Use superpowers:subagent-driven-development and test-driven-development. Keep candidate code, source differences and serving acceptance distinct.

**Goal:** Add the real source-backed Triplelift request/response protocol to the
existing bidder crate without promoting its public metadata or claiming full
wave acceptance.

**Architecture:** An automatically discovered stateless factory and inline
borrowing serializers stream a filtered, transformed request through the
existing capped sink. Shared fixed construction diagnostics retain invalid
impression failures independently of accepted siblings. Responses use the
sealed decoder and shared candidate-local classification.

**Tech Stack:** Existing Rust/Serde, capped metadata/body writers, sealed response
API, adjacent TOML provenance and immutable external captures. No new crate,
dependency, provider client, feature, workload limit or accounting coefficient.

## Contract And Boundaries

This implements a non-serving protocol-candidate slice of the
[priority plan](2026-10-22-priority-display-bidders.md), not its pending review
or either Task 18 acceptance checkpoint. The source revision remains
`d7f8515b86258688304b0d9b6668c6a0e258bc9e`, implementation blob
`736773075eb175da44bb79dc74715a17f1a356de`. Do not run foreign source programs.

- Read only the already scoped `imp.ext.bidder` parameters. Banner or video
  must be present. Invalid parameters or unsupported media remove that
  impression, retain a fixed shared diagnostic, and preserve valid siblings.
  No valid impressions means no call plus a fixed bidder error, with no body
  allocation. Its fixed `No valid impressions for triplelift` error retains
  source code 999. Parameter-decode failures map source code 14 to shared
  construction code 2; unsupported media maps source 999 to shared code 2.
  Shared bounded suppression is deliberate and never a resource refusal.
- Borrow `inventoryCode` into `tagid`; absent/null string values use the source
  empty default. A finite optional numeric `floor` overrides `bidfloor`;
  absent/null preserves it. Preserve all unrelated request/impression fields
  and extensions; never clone caller trees or materialize a JSON body first.
  Exhaustive typed destructuring must make new model fields a review point.
  Zero (including negative zero) clears any old floor and omits `bidfloor`,
  matching source `omitempty`. Empty/default inventory clears any old tag and
  omits `tagid`; the original parameter extension stays unchanged. Missing or
  null root extension, or missing bidder container, is invalid; explicit
  `bidder: null` and `{}` allow zero defaults before supported-media checking.
  Decode parameters before testing media so one bad impression reports once.
  AG approved the existing presence-aware `JsonField<Value>` on 2026-10-23.
  Typed ingress now preserves missing, null and value: missing refuses, explicit
  null uses source defaults and remains null when serialized. Envelope-level
  alternate `bidder` spellings are not established by scoped parameter parsing.
  Never recover unscoped values.
- Send one POST of surviving impressions with exactly their coverage IDs.
  Use the common endpoint expansion/validation, the source JSON charset header
  and Accept header. Preserve configured query parameters; do not hardcode an
  endpoint, repair a scheme, perform compression in the bidder or activate
  unreviewed metadata. Exchange gzip/delivery acceptance remains separate.
- HTTP 204 ignores its body. HTTP 400 is code 2; other non-200 statuses follow
  the source's generic code 999, not another family's code 4. Numeric-only
  status messages are safe; malformed JSON uses shared fixed diagnostics.
- Nested `bid.ext.triplelift_pb.format` values 11, 12 and 17 mean video;
  other valid integers, absent or null values mean banner. A flat `format`
  cannot select video. Invalid nested shapes/types are bid-local errors;
  valid siblings survive. The shared classifier also rejects contradictory
  standard hints, unknown impression IDs and unoffered media. Require actual
  sent-call ID membership too: a filtered-out impression still in the scoped
  request must not regain a bid. Native belongs
  to a different implementation and is not implied here.
- Source response currency is always USD, even if the wire declares another
  value. Pin that independently of the other bidders' currency behavior.
- Existing typed ingress and sealed whole-response decoding are not replaced
  by this candidate. Retain malformed external captures, identify cases those
  boundaries reject before bidder dispatch, and record exact differences.
  Such controls are not passing source-protocol parity or decoder completion.
  Source distinguishes absent `bid.ext` (error) from explicit null (banner);
  the current shared model merges them. Keep the shared banner fallback for
  that merged representation explicitly unqualified, not claimed equivalent.
  Likewise, source parsing of unrelated recognized `imp.ext.ae`/`prebid` is not
  recreated by reading scoped parameters. This candidate preserves those
  fields instead of silently claiming their foreign envelope validation.
  Source per-bid extension decode failures are code 14; shared local media
  failures are code 4. All-invalid local results use Auctioneer's `Err` channel
  instead of a source empty response accompanied by errors.
- AG separately approved the single case-insensitive spelling/ambiguity rule
  on 2026-10-23. Accept a single spelling of `inventoryCode`, `floor`,
  `triplelift_pb` and `format`; reject competing spellings only for the
  affected impression or bid, including equal or null competing values.
  Preserve valid siblings and never choose precedence from sorted map order.
  Parameter decoding follows source's general lowercase lookup, including
  dotted capital I for `inventoryCode`; optimized media decoding matches
  ASCII only. Pin the shared JSON helper and its dependency manifest; test
  dotted-I extraction, malformed-value refusal and ambiguity independently.
  Exact duplicate-key ordering
  already lost in shared maps remains an inherited limitation, distinct from
  rejecting simultaneously retained case-variant names.
- Keep `[definitions]` empty. Site/app eligibility, sync, schema/resource
  qualification, shared response-decoding changes, original-deadline admission
  and composed publisher acceptance remain explicit open gates.

## 1. Audit And Retain Source Evidence

**Files:** `crates/auctioneer-bidder/src/bidders/triplelift/bidder.toml`, adjacent
`fixtures/protocol/{exemplary,supplemental}/`, existing CLI source checker.

- [x] Independently audit implementation, all 12 captures, typed parameter
      contract, tests and relevant shared helpers; resolve actual source behavior
      before writing production code. Record remaining case/null/type differences.
- [x] Import all capture bytes unchanged, with adjacent source tree/blob IDs,
      lengths and SHA-256 values. Record implementation/parameter/helper provenance
      separately from license, metadata and canonical schema. No duplicate license.

## 2. Implement Through Red/Green Tests

**Files:** adjacent `mod.rs`; existing shared harness and process allocation
target only where genuinely cross-cutting.

- [x] Add a refusing factory stub and inline tests. Observe intended failures
      before implementing transformations, filtering, status/currency and media.
- [x] Implement borrowed root/impression/sequence views without retaining a
      survivor list. Validate before push and repeat only allocation-free parameter
      inspection while streaming; use exact surviving IDs in metadata.
- [x] Independently vary raw bytes, call slots and impression-ID caps. Assert
      invalid siblings, all-invalid input, immutability, exact typed failures and
      reservation refunds. Exercise zero, negative, absent and overridden floors.
- [x] Exercise 200/204/400/other status, malformed/over-node response, nested vs
      flat format, each video code, malformed namespace, conflicts, unknown IDs,
      unoffered media, USD override and multi-seat valid/bad siblings.
- [x] Observe single case-variant and independent parameter/namespace/format
      ambiguity tests fail before implementing the approved policy. Require empty
      sent coverage to reject an otherwise valid bid, and a non-USD response with
      an accepted bid plus an invalid sibling to retain USD and exact partition.
- [x] Add process-level allocation controls to the existing common target:
      independently grow root, surviving-impression and rejected-impression trees
      at fixed body caps, with positive whole-request and `Imp::clone` controls
      plus cross-size differentials, rather than a bidder-specific executable.

## 3. Source Conformance And Discovery

**Files:** adjacent `conformance.rs`; shared `bidders::harness`, existing
`registry_join`, registry build harness and CLI source/import tests.

- [x] Verify supplied capture and whole adjacent TOML before any filtered
      dispatch using the shared verifier and fixture macro. Freeze exact capture
      membership and meaningful dispatch counts; distinguish typed-boundary controls.
      Freeze 10 end-to-end candidate cases plus two boundary controls: `badext`
      fails typed ingress before dispatch; `badresponseext` fails sealed response
      decoding before bid-local classification. Neither proves source-local errors.
      Nine cases construct calls; only five return accepted bids. No captured
      request needs a null-field exception. Only `optional-params`' integral f64
      floor representation is normalized explicitly, with before-state assertions.
- [x] Compare independent captured request/response expectations and exact
      survivor coverage. Name only independently demonstrated inherited model
      differences; never regenerate expected objects with Auctioneer's types.
- [x] Reconcile actual discovered factory/declaration expectations and canonical
      schema witnesses while keeping the public metadata set unchanged. Verify
      real source objects and retained assets through the existing source target.

## 4. Review, Verify And Publish

- [x] Independent source/spec review, then separate engineering-quality review;
      fix findings and rerun owning controls without lint suppressions.
- [x] Run secure/default and feature bidder/config tests separately, registry
      build controls, allocation and CLI source/import/wave checks, all three guest
      library checks, strict workspace lint, formatting and raw-fixture gates.
- [x] Update `ISSUES.md` and priority/master-plan progress with exact evidence
      and remaining gaps.
- [x] Commit and push the verified increment to the existing
      M7 branch/PR; do not relabel readiness, pending approval or migration.

## Verified Implementation Scope

The adjacent package retains three source trees, 22 source artifacts and
14 verbatim assets. The 12 protocol captures total 29,226 bytes. The two added
helper witnesses bind JSON-decoder selection and the source dependency version;
they add no foreign executable or copied helper program to the repository.

The initial refusing implementation produced 14 intended unit failures and
an allocation-control failure. The shared sent-ID fixture helper also failed
its ordered-coverage assertion before implementation. Four approved case-policy
controls then failed against canonical-only lookup. Three dotted-I parameter
controls failed against ASCII-only lookup. Weakening sent-ID validation to
bypass an empty coverage list made the named valid-bid control fail; the
original guard was restored before final tests. None of these failures were
compiler errors or ignored tests.

Independent source/spec and quality reviews found test gaps in empty coverage,
accepted-bid currency, impression-subtree allocations, noncanonical-only
ambiguity pairs and dotted-I parameter matching. Their repairs were re-reviewed
without further actionable findings. Static review does not replace the test
and build results, or certify the still-open acceptance gates above.

### Final Local Verification

Executed on macOS with Rust 1.98.1, offline locked Cargo and two build jobs.
The source-backed controls read immutable Git objects from the separately
supplied reference checkout; no foreign source program was executed.

| Owning target                  | Secure defaults | `insecure-bidders` |
| ------------------------------ | --------------- | ------------------ |
| `auctioneer-bidder --lib`          | 342 passed      | 353 passed         |
| `bidder_request_allocation`    | 8 passed        | 8 passed           |
| `catalog_alloc`                | 5 passed        | 5 passed           |
| `registry_join`                | 10 passed       | 10 passed          |
| Selected config targets, below | 244 passed      | 245 passed         |

All groups above had zero failures and zero ignored tests. The bidder library
run used `--include-ignored` with `AUCTIONEER_APS_SOURCE` supplied, executing the
existing source-object control rather than silently skipping it. The config
group comprises `--lib`, `baked_defaults_snapshot`, `bidder_bindings`,
`bidder_storage`, `codegen` and `smoke`; its existing socket controls ran with
local socket permission.

- The initial CLI source/import/wave group passed 28 tests; isolated factory
  binding and incremental registry discovery passed 20. After adding the JSON
  helper/dependency witnesses, the source-object/retained-asset CLI control
  passed again against the current three-tree/22-artifact/14-asset manifest.
- `cargo check -p auctioneer-bidder --lib` passed separately for
  `wasm32-unknown-unknown`, `wasm32-wasip1` and `wasm32-wasip2`. These are guest
  library compile checks, not four-application execution or memory evidence.
- Workspace/all-target Clippy passed under `-D warnings`, separately with
  secure defaults and `insecure-bidders,conformance-suite`.
- Workspace formatting and explicit discovered-bidder formatting passed.
  The raw-fixture gate passed at 172 fixtures across 52 files; all 12 gate
  self-test scenarios passed. No new raw Rust JSON fixture was added.
- Existing GVL deletion notices and the `proc-macro-error2` future-compatibility
  warning remain unsuppressed. No dependency, SDK pin, feature, production
  workload limit or funding coefficient changed.

The 36 Triplelift-owned tests are part of the library totals, not an additional
suite to sum into them. These selected tests and workspace lint do not claim
that the complete workspace test suite or hosted qualification was executed.

### Publication

Implementation commit `268d5e99c85d4247143eb8a89c9fc7196aee8ed1` was pushed
to `origin/m7-bidder-catalog` for [PR #21](https://github.com/IABTechLab/auctioneer/pull/21).
This record follows the successful push; it is not additional test evidence.
The public Triplelift definition remains unpromoted and M7 remains incomplete.

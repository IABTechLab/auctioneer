# Bidder Test Isolation Implementation Plan

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans. Check off completed steps only after verification.

**Goal:** Shared bidder tests use a capable synthetic bidder, not production bidder names, protocols, schemas or metadata.

**Architecture:** One test-only fake implements the real `Bidder` contract and uses the bounded sink, endpoint builder and sealed response parser. Synthetic declarations and schemas cover the supported infrastructure. Production-specific protocol and allocation evidence remains bidder-owned, with automatic discovery into a shared isolated conformance runner. Source captures and qualification claims must not be weakened or relabeled as synthetic evidence.

**Tech Stack:** Existing Rust workspace, Cargo integration targets, existing discovery, serde, syn and the existing test allocator. No additional workspace crate, production fake factory, feature flag, dependency or budget change.

## Acceptance

- Shared Rust tests contain no production bidder name checks, constructor lists or protocol-specific expected output.
- The fake supports aggregate and per-impression construction, endpoint bindings, headers, exact sent coverage, bounded compression, all media types, partial errors, currencies and browser-auction configurations through the actual production boundaries.
- Shared metadata/schema tests own synthetic inputs. Actual catalog provenance and resource evidence retains its separate production-data scope and original assertions.
- Ordinary bidder additions change only their own directory; discovery supplies local conformance without a central roster.
- Unit tests remain inline. Separate files are genuine compiler, allocation, source or protocol conformance.
- No proof is replaced by a test that merely exercises the fake. Assertions about production-specific behavior still drive that production factory beside its implementation.

## Tasks

### 1. Synthetic Shared Contract

**Files:** `crates/auctioneer-bidder/tests/support/fake_bidder/`, shared boundary integration tests.

- [x] Record the existing target/test inventory and production references before editing.
- [x] Add failing behavioral controls for the fake's supported construction and response paths, with independent literal expectations.
- [x] Implement the fake using the actual bounded interfaces; prove refusal, valid siblings and no input mutation.
- [x] Replace production constructors and name lists in ordinary shared boundary tests.

### 2. Bidder-Owned Allocation Evidence

**Files:** `crates/auctioneer-bidder/tests/bidder_request_allocation.rs`, shared allocation support, `src/bidders/*/` conformance, `build_discovery.rs`, `build.rs`.

- [x] Split existing production-specific allocation cases by owner without dropping assertions or changing charge coefficients.
- [x] Automatically include adjacent local allocation conformance in a dedicated native counting-allocator runner; preserve feature ownership and nonzero coverage checks.
- [x] Shared allocation tests use the fake's supported transforms and vary caps, payloads and call multiplicity independently.
- [x] Prove discovery observes an added, edited and removed synthetic conformance file without central registration.

### 3. Registry And Configuration Tests

**Files:** registry integration/build tests and `tests/support/registry_*`.

- [x] Use fake declarations, metadata, aliases, schemas and factories for registry semantics and scratch builds.
- [x] Remove production-specific request/header/response branches and migrate their evidence to their existing local factory/conformance fixtures.
- [x] Preserve actual generated-catalog integrity coverage separately from synthetic registry behavior; do not substitute expected output derived from generated output.

### 4. Compiler And Schema Tests

**Files:** schema integration tests, shared schema support, bidder-owned schema captures, catalog reference data and its readers.

- [x] Replace compiler unit/control examples with explicitly synthetic schemas covering numeric types, required fields, branches, patterns, formats and explicit caches.
- [x] Keep original parameter captures byte-identical with their provenance; locate production examples beside their owner and discover them rather than maintaining name tables.
- [x] Separate synthetic importer/qualification negative controls from complete production-catalog resource qualification. Preserve the latter's real input closure, four-target linkage and resource refusal gates.
- [x] Move any CLI production data out of the integration-test fixture directory and update all consumers atomically.

### 5. Audit And Publish

**Files:** bidder README, `ISSUES.md`, affected CI gates/fixture budgets and this plan.

- [x] Add a regression guard against production names/constructors in shared test logic, derived from declarations rather than another hand-maintained roster; synthetic path/control metadata must not be exempted to hide a dependency.
- [x] Reconcile tests/assertions across moves and check all newly discovered conformance actually runs.
- [x] Run affected targets, secure-default and feature bidder suites, strict lint, formatting, source controls, raw fixture gate and required guest checks against the frozen tree.
- [x] Independently review for fake-only false confidence, missing production evidence, shared-state races and unnecessary abstractions.
- [x] Commit and push the verified cleanup, reporting any remaining readiness blockers honestly.

## Verification In Progress

- Fake construction/response controls failed semantically against stubs before implementation; correlation and the observed identity envelope each had their own failing control.
- Shared boundary lane: 55 passed across seven targets; later identity-envelope lane: 11 passed.
- Native production allocation runner: 29 passed (28 local cases plus the generated nonempty guard). Original 26 tests retain their assertions; splitting the three-owner aggregate loop yields two additional test entries.
- Discovery: 78 passed, including actual rustc execution after synthetic conformance additions, edits, feature transitions and removal. Unix-socket controls require the existing unsandboxed native lane; their sandbox permission failure is not a code failure.
- Synthetic compiler controls: 163 passed including support tests. Full original production-source qualification remains a separate required validation, not replaced by this result.
- Logs are separate files under `/private/tmp/bidder-catalog-fake-*` and `/private/tmp/bidder-catalog-test-ownership-*`; these are interim results, not completion evidence for later edits.

## Final Review Reconciliation

- The fake's 16 contract cases cover the current public framework capabilities,
  including host selection, actual debug projection and exact reserved-header
  refusal with an independent call budget. Fake success is not provider proof.
- Production sealed-response controls remain owner-local: six cases per decoder,
  discovered under the owning factory's feature guard. Default discovery runs
  19 cases including its coverage guard; the insecure feature adds six.
- The real schema matrix retains all original compiler cases as 63 owner-local
  literals, including native format-disable and draft contrasts. Original schemas
  and the aggregate capture are byte-identical. Supplemental exclusions cannot
  authorize their own provenance; an independently checksum-pinned reference
  supplies their expected source and retained-artifact witnesses.
- Three generic `*_cases.rs` files retain 37 case tests and all 388 assertions.
  The real source target now declares its support tree once, not three times:
  `393 - 124` duplicate support instances `+ 1` population control = 270 tests.
  No unique support test was removed. Ordinary synthetic targets retain their
  separate fixture scope; one passing fake scope cannot satisfy the source gate.
- A temporary source-mode mutation failed the independent census with seven
  synthetic schemas instead of 261. Restoring real-source mode passed; the
  committed source mode remains true. Host/header controls also failed
  semantically before their repair.
- Independent source-preservation and architecture reviews report no remaining
  findings. Full publication checks below remain required; these reviews alone
  do not establish target readiness or complete bidder catalog.

## Final Verification

Executed offline with the locked dependencies and Rust 1.98.1. Native compiler,
process and socket controls use the existing unsandboxed verification lane.
`AUCTIONEER__BIDDER__SOURCE_DIR` and `AUCTIONEER_APS_SOURCE` point to the separately pinned
local Git checkout; no foreign source program was executed.

| Check                                                                 | Result                                                                                                             |
| --------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| `cargo test -p auctioneer-bidder --all-targets`                           | 1,902 passed, zero failed; three external-source tests separately driven below                                     |
| Same target set with `--features insecure-bidders`                    | 1,919 passed, zero failed; same three separately driven tests                                                      |
| Complete real `schema_source_conformance`                             | 270 passed in each feature state; 14 source blobs and all 300 captured cases checked in default and explicit modes |
| `cargo test -p auctioneer-cli --bin auctioneer-cli bidders::`                 | 251 passed, zero failed                                                                                            |
| CLI `bidders_source`, `bidders_import`, `bidders_wave`                | 23 + 7 + 9 passed, zero failed                                                                                     |
| `cargo test -p auctioneer-core --test ci_workflows`                       | 104 passed, zero failed                                                                                            |
| `cargo test -p auctioneer-bidder --lib -- --ignored` with source bindings | All three passed; no new ignored test                                                                              |
| All-target Clippy for bidder and CLI, defaults and insecure feature   | Passed with `-D warnings`, no new waiver                                                                           |
| Cloudflare / Fastly / Spin guest library checks                       | Passed for `wasm32-unknown-unknown`, `wasm32-wasip1`, `wasm32-wasip2` respectively                                 |
| Workspace and discovered bidder formatting; staged diff whitespace    | Passed                                                                                                             |
| Raw JSON fixture gate and its negative controls                       | Passed: 172 fixtures across 52 files; all rejection controls behaved                                               |

The frozen suite logs are `/private/tmp/bidder-catalog-isolation-*-complete.log`; guest and
formatting logs use the same prefix. Earlier failed/intermediate logs are not
the final acceptance record. An earlier CLI resource-identity run detected
concurrent source edits and correctly refused; the frozen 251-test run passes
without weakening that check. Original GVL deletion notices and the existing
`proc-macro-error2` future-incompatibility notice remain unchanged.

These checks validate this isolation change, not hosted bidder delivery,
complete schema-engine qualification, absolute guest memory fit or bidder catalog closure.

Published code commit `e826e10d` to `origin/bidder-catalog`. A fresh CLI run
against that committed tree also passed all 251 selected bidder tests, including
the source-identity assertions. This follow-up records the completed publication.

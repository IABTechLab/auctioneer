# Bidder Conversion Test Pipeline Implementation Plan

> **For agentic workers:** Use subagent-driven-development and test-driven-development. Keep writes disjoint and serialize Cargo in the existing target directory.

**Goal:** Adding a normal bidder changes only its adjacent folder, without losing
independent protocol, discovery, schema or provenance checks.

**Architecture:** Keep production registration and protocols unchanged. Shared
test drivers check common contracts; adjacent factory fixtures supply independent
requests and expectations, with automatically generated native conformance tests.
Protocol unit tests remain inline. Test inventories read manifests independently
of generated registry output. Immutable source inventory and local frozen metadata
remain the evidence owners, not duplicated central bidder/count/schema tables.

**Tech Stack:** Existing Rust, Cargo, Serde, TOML, SHA-256 and test harness. No new
crate, dependency, feature, runtime reader or test framework.

## Approved Scope

AG approved the test and batch-first conversion recommendations on 2026-10-24.
This changes test ownership and iteration scheduling, not protocol behavior,
privacy enforcement, resource limits, CI acceptance or migration readiness.
Preserve original captures and the distinction between candidates and promotion.

## Task 1: Freeze The Existing Assertions

- [x] Record the current factory smoke inputs/expected transformations, header,
      response sibling, media, 204, feature and schema/provenance guards.
- [x] Preserve fixed historical default-catalog snapshots as explicit snapshots,
      not growing lists of every newly implemented bidder.
- [x] Keep semantic expected results independent of production bidder output.

## Task 2: Bidder-Owned Factory Cases

**Files:** `crates/auctioneer-bidder/src/bidders/mod.rs`, native-only
`factory_contract.rs`, `build_discovery.rs` and adjacent factory fixtures.

- [x] Add a reusable real-factory contract assertion to the existing unit harness.
- [x] Move each protocol smoke input/expectation into its own `fixtures/factory.json`.
      Generate native-only conformance tests from existing discovery, not a second
      registration list. Freeze the complete manifest digest in each local fixture.
      Preserve request bodies, URL/header assertions, real sent coverage, sibling
      failures, source-specific status and media controls; no name-based shared driver.
- [x] Replace central factory/declaration lists with independent manifest scans.
- [x] Exercise dropped/extra/incorrect feature membership and a wrong wire
      expectation; empty selections must fail rather than silently pass.

## Task 3: Generic Registry And Schema Expectations

**Files:** `crates/auctioneer-bidder/tests/registry_join.rs` and its existing support
directory where a focused helper is warranted.

- [x] Derive expected declarations, factories, public names and implementation
      bindings independently from adjacent manifests. Feature ownership must still
      distinguish inactive declarations from compiled factories.
- [x] Replace the central canonical-schema pin list with bidder-owned witnesses
      checked against the independently pinned reference inventory where applicable.
- [x] Preserve exact historical defaults and deliberate cross-factory/alias
      regression fixtures without making them grow with every bidder.
- [x] Add negative controls for missing/wrong schema ownership and incomplete
      discovery, not just equality between two generated registry views.

## Task 4: Generic Immutable Source Verification

**Files:** `crates/auctioneer-cli/tests/bidders_source.rs`; bidder-local inline
metadata-integrity guards where their existing coverage needs preserving.

- [x] Remove the central factory-to-witness-count table. Discover source-backed
      factories from the independent filesystem input, then verify every declared
      immutable object and retained artifact.
- [x] Preserve the fixed main-source revision gate, retained capture integrity,
      canonical schema ownership and locally frozen manifest coverage. Do not make
      source completeness a claim based only on a self-derived count.
- [x] Keep the existing source-specific parameter/category extraction controls.
- [x] Add negative tests for omissions and corrupt witnesses without rerunning
      complete catalog import for each small unit edit.
- [x] Keep importer comparison aware of the exact locally authored
      `fixtures/factory.json` path for implemented factories only. Do not classify it
      as an external capture or permit arbitrary JSON in reference-only packages.

## Task 5: Verification And Publication

- [x] Document a fast development lane: affected bidder units/captures, relevant
      shared regression targets, formatting and focused lint. Shared code changes
      expand the affected set immediately.
- [x] Document the frozen publication lane: secure/default and feature suites,
      source/catalog checks, required allocation controls, strict lint and all three
      guest builds once per coherent batch. CI gates are not reduced.
- [x] Establish subsequent port-batch scheduling: one shared-capability owner
      and two disjoint bidder owners where dependencies permit; do not fork Cargo
      target directories. Reuse completed source audits and stay OpenRTB-first.
- [x] Independently review the refactor, verify the exact publication tree,
      and update `ISSUES.md` and bidder rules. Publish this verified round on the
      existing M7 branch/PR; do not start another port inside this frozen batch.

## Completion Boundary

This work is complete when an ordinary new bidder needs no shared name list,
schema-pin table, provenance-count row or protocol branch, and the existing
assertions still run in their new owners. It does not implement the five
remaining priority protocols, qualify schemas/resources, promote definitions,
close R0 or execute the deferred hosted pilot.

## Verification Record

The migration first passed the old eight shared factory tests. A new automatic
registration control failed against the old renderer, then twelve active
factory contracts failed against a compiling refusing driver. After moving the
literal expectations, all twelve passed before removing the old protocol switch.
The driver also rejects empty cases, wrong owners/versions, changed manifest
witnesses and a wrong independent body. Explicit package-version pointers have
separate positive and missing-field controls.

The importer exception's five-row control failed on the exact valid local path
before the repair. It now permits only an implemented owner's exact local
contract path; arbitrary names, nested paths, missing owners and reference-only
packages remain rejected. Main-checkout repository/revision substitutions also
failed their new negative control before the repository binding repair.

Completed checks on the refactor:

- Secure-default library: 727 passed, including the existing source controls;
  construction allocation: 26 passed; registry conformance: 24 passed.
- Insecure-bidder library: 738 passed, including source controls; registry
  conformance: 24 passed. No ignored test was added.
- Maintenance CLI: 156 passed; full immutable source suite: 23 passed; complete
  import suite: 7 passed; wave-manifest suite: 9 passed. Source objects were read
  from the separately pinned checkout, never executed.
- Discovery/codegen: 73 passed; factory-binding build controls: 10 passed;
  incremental discovery/build controls: 10 passed; final registry conformance:
  24 passed. Rebuilt probes exercised secure/feature/off transitions, source and
  nested-helper edits, add/rename/removal, metadata-only changes and missing-file
  refusal. Pristine filesystem baselines remained independent of probe output.
- Raw-JSON gate: 172 fixtures across 52 files; all gate rejection controls pass.
- Final response-allocation controls: 24 passed. Strict native Clippy passes for
  bidder and maintenance CLI all-targets in secure and insecure-bidder lanes.
- Guest library checks pass for Cloudflare (`wasm32-unknown-unknown`), Fastly
  (`wasm32-wasip1`) and Spin (`wasm32-wasip2`). Workspace formatting, direct
  discovered-source formatting and `git diff --check` pass.
- The documented exact MSFT factory command runs one test, not an empty filter;
  a second unchanged-tree invocation reused the build. This verifies the fast
  lane's selection, not a conversion-throughput benchmark.

The final encapsulated incremental probe was rerun: all ten tests passed,
including sixteen bounded child commands. Builds retain the existing GVL
deletion notices and tracked `proc-macro-error2` future-incompatibility warning;
no suppression was added.

Original external captures, bidder manifests, dependency/feature declarations
and CI workflows remain unchanged. New local factory cases are not external
capture provenance. These are scoped refactor checks, not schema/resource,
R0, protocol promotion or hosted-delivery qualification.

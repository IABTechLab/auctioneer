# Library-Owned Regex Layout API Implementation Plan

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans. Use TDD and separate specification/quality review before committing the verified checkpoint.

**Goal:** Implement the two approved shallow layout queries as one replayable
dependency patch, with verified consumers and no serving or accounting cutover.

**Architecture:** The locked library computes its own payload layouts using
`core::alloc::Layout`. Existing native schema conformance consumes those facts;
archive/patch/tree, manifest/lock/metadata and resolved-feature controls bind the
actual patched engine. Hidden child ownership and complete qualification remain
separate. No new workspace crate or feature is introduced.

**Tech Stack:** Rust 1.98.1, regex-automata 0.4.18, existing bounded Rust fixture
and process helpers, Serde, SHA-256, quote/syn for any generated probe source.

**Authority:** AG approved the [written design](../specs/2026-10-28-regex-resource-layout-api-design.md)
on 2026-10-28. Source baseline is the locked package, not upstream master.

## Guardrails

- Keep engine features, versions, matching, all schemas, builder/cache limits,
  workload, resource ceilings and the shared additive equation unchanged.
- Preserve all existing compiler patch files/pins and historical captures.
- No new matching-trait methods, representation changes, unsafe code, guessed
  private mirror structs or public heap traversal.
- Layout reports do not include Arc headers, child buffers or complete heap.
  Do not double-charge embedded fields or infer sharing from equal reports.
- Serialize Cargo with `CARGO_INCREMENTAL=0`, `--offline`, `--locked` where the
  approved source transition permits, and `-j2`. Reuse existing target caches.
  Initial free disk is about 21 GiB; do not start duplicate unconstrained builds.

## Task 1: Missing-API Red And Immutable Baseline

**Files:** `crates/auctioneer-bidder/tests/schema_source_conformance.rs`, new
`tests/schema_resource_layout/mod.rs` as needed for owning report controls.

- [x] Capture the original metadata/feature closure before path-patching. The
  local baseline is `/tmp/auctioneer-regex-layout-before-metadata.json`; structurally
  select the unique package by name/version and join its exact resolve-node ID.
- [x] Write an external API control using `Regex::resource_layout()` and
  `Prefilter::resource_layout()`, their typed `Layout` getters, and public auto
  trait requirements. Observe the missing-method red against the unpatched
  package before implementation. Do not mistake another build failure for it.
- [x] Verify/copy the exact cached `.crate` to
  `vendor/regex-automata-0.4.18.crate`, checking the approved SHA-256 and VCS
  identity; mechanically expand all original members unchanged. Exclude
  `vendor/regex-automata` from workspace membership. Original licenses stay.

## Task 2: Owner-Local Library API

**Files:** vendored `src/meta/{mod.rs,regex.rs}` and
`src/util/prefilter/mod.rs`; `vendor/patches/regex-automata-resource-layout.patch`.

- [x] Add `RegexResourceLayout` with private `Layout` fields and `value`,
  `regex_payload`, `metadata_payload`, `strategy_payload` getters. Derive
  Copy/Clone/Debug/Eq/PartialEq and export through the existing meta module.
- [x] Implement the immutable query with `Layout::new::<Regex>()`, actual
  `RegexI`/`RegexInfoI` types and `Layout::for_value(self.imp.strat.as_ref())`.
  Return the pointee layout, never the fat Arc handle's layout.
- [x] Add the analogous `PrefilterResourceLayout` and query under the existing
  `alloc` boundary, using `Layout::for_value(self.pre.as_ref())`. No-alloc
  builds omit the query rather than manufacturing an empty matcher.
- [x] Colocate private-owner unit controls. Compare against the actual private
  types and dynamic pointees, exercise unequal payload sizes/alignments, and
  make incorrect getter/handle substitution fail. Do not assert one ABI as a
  portable contract or change matcher types to manufacture test layouts.
- [x] Generate the portable diff mechanically against the verified original
  members; preserve unrelated bytes. Add the root path patch and allow only the
  engine source/checksum transition in `Cargo.lock`. No cargo-wide update.
- [x] Run the public missing-API control green and the new upstream unit
  controls in a separate target directory, preserving the upstream lockfile.
  If original dev dependencies are unavailable offline, report/fetch those
  locked test prerequisites without changing the serving feature closure.
- [x] Compile the existing no-alloc, alloc-only and minimal-meta feature
  boundaries in isolated probe manifests. No-alloc must reject naming the
  prefilter query/report; alloc-only and minimal-meta must expose the applicable
  APIs. Guest probes must actually reference both queries. These test modes
  do not change the production closure captured before the patch.

## Task 3: Provenance And Standalone Source Wiring

**Files:** new `tests/schema_build/schema_regex_vendor/mod.rs`; existing
`schema_source_conformance.rs`, `schema_build/{mod.rs,schema_artifact.rs}` only
where required, `schema_build_cases/mod.rs`, `schema_cache_controls.rs`,
`schema_matchers/mod.rs`, `vendor/README.md`.

- [x] Add bounded engine archive/patch/tree verification with exact member
  coverage. Replaying the SHA-pinned patch over the SHA-pinned original must
  recreate the complete tree. Refuse edits, omissions, extra paths, symlinks,
  wrong archive/patch, missing hunks and partial replay. Reuse existing safe
  helpers where their contracts fit; do not relax compiler pins/280-member
  membership or rebuild the existing compiler verifier unnecessarily.
- [x] Change `schema_build::manifest` from exactly one patch to exactly the two
  reviewed names/paths, with no extra fields or third patch. Verify both vendor
  trees and emit canonical absolute overrides in standalone manifests.
- [x] Extend `verify_lock` to require both exact local packages once:
  compiler 0.58.6 and engine 0.4.18 with absent source/checksum. All other
  packages must retain their existing reviewed registry identities. Missing,
  duplicate, registry-substituted, wrong-version and checksum-bearing local
  engine controls must refuse, independently of compiler refusal.
- [x] Extend standalone metadata checks: exactly one engine package, version
  0.4.18, explicit null source and exact canonical vendor manifest. Check the
  actual resolved feature set against the existing literal closure. Retain
  all compiler metadata controls and test each package independently.
- [x] Replace `ends_with("#regex-automata@0.4.18")` in the feature control with
  structural package-ID resolution. Keep its entire feature-set literal, and
  add missing/duplicate/wrong-package/refusal controls rather than accepting any
  path dependency with a convenient suffix.
- [x] Repair optional matcher metadata: the path package has no registry
  checksum. Record that absence plus separate original archive, patch/tree and
  resolved-feature identities. Exercise the metadata path without rerunning or
  rewriting historical benchmark captures.
- [x] Run the provenance, standalone manifest/lock/metadata and feature controls
  before the expensive all-schema builds. Wrong-source green builds must be
  impossible; a compiler-only witness is not engine provenance.

## Task 4: Native Candidate Consumer And Nonallocation Proof

**Files:** `tests/schema_resource_layout/mod.rs`,
`schema_source_conformance.rs`, existing meter/profile helpers and the bidder
build script's data-only compiler/target record; no serving code.

- [x] Consume actual `Layout` records in a strictly typed candidate report,
  bound to package/patch/tree, compiler/target/features and helper/constructor
  identity. Label reconstructed candidates as such, not observed generated
  singletons. Derive the full helper census and separate hostname owner from
  existing source controls; do not learn the expected row set from the report.
- [x] Bind compiler/target to the test binary's build, not whichever compiler
  happens to be available when the test executes. Emit a data-only TOML record
  from Cargo's `RUSTC`, `TARGET` and target ABI variables in the existing bidder
  build script; read it through `include_str!(OUT_DIR/...)`, validate captured
  ABI against binary `cfg` values, and test mismatches independently. Compiler
  host and measured target are distinct; do not relabel a translated binary.
- [x] Reject missing, duplicate or unknown roles/owners, stale identities,
  invalid layout/target shapes and arithmetic overflow. Vary size, alignment,
  owner multiplicity and sharing independently with literal controls. A valid
  zero-sized layout is not an unavailable/unproved owner.
- [x] Keep the old metadata bound/history intact; compare exact root/metadata
  payloads with that bound and account the selected strategy Arc separately
  using the existing checked native Arc layout helper. Do not add the Regex
  value twice to the lazy-cell charge or infer hidden child absence.
- [x] Preserve `NativeField::validate` and its existing rejection of missing
  zero-sized private fields. Add a narrow `Layout`-accepting Arc calculation
  for actual compiler-provided payloads, sharing the existing header/checked
  alignment derivation. A valid ZST still has a nonzero Arc header charge;
  test it independently from an absent owner, plus high alignment and overflow.
- [x] Query every discovered candidate plus an actual accessible prefilter in
  the existing allocation meter, with construction outside and repeated queries
  inside the window. Serialization/assertion formatting stays outside. Require
  zero allocation/deallocation and a genuinely allocating positive control.
- [x] Preserve unresolved hidden prefilters, literal children, capacities,
  initialization/work, target/physical and complete-serving gates explicitly.

## Task 5: Regression, Review And Checkpoint

**Files:** this plan, the approved spec status, qualification notes, master bidder catalog
Task 3 status and `ISSUES.md`.

- [x] Run focused new controls, compiler patch replay, current feature closure,
  public cache APIs, and complete `schema_source_conformance` with source inputs
  in secure defaults. Its actual all-schema native/three-guest builds must use
  both source patches. Report exact counts and compile versus execution scope.
- [x] Run applicable new/adjacent controls under `insecure-bidders` separately.
  Preserve all 300 captured parameter outcomes and default/explicit paths; do
  not recapture expectations, shrink workload or increase thresholds.
- [x] Run strict workspace Clippy (and targeted feature lane where applicable),
  formatting, source-import isolation and both raw-fixture gates. Check that
  original compiler patches, historical captures, root dependency versions and
  resolved engine features did not drift.
- [x] Obtain source/spec review, then independent code-quality review; fix
  findings and rerun affected controls. Record actual native layout observations
  as observations, not complete heap or guest measurements.
- [x] Update documentation/issue status with precise implemented and remaining
  scope. No new branch, crate, live test or deployment readiness claim.

Publish the verified patch and consumers together on the existing bidder catalog branch;
Git records the publishing commit and remote receipt.

## Verification Record

Implemented and verified on 2026-10-28:

- Initial public API red: four missing-method E0599 diagnostics. Owner-local
  library controls then passed 5/5 in default mode and 5/5 with the selected
  feature flags, using a disposable copy and the original upstream lockfile.
- Native Arc extension red: two Shape failures with the unimplemented extension;
  the complete helper group passed 11/11 after implementation.
- Candidate consumer red: 3/23 structural-only tests passed and 20 failed at the
  unimplemented operations. Implemented consumer controls passed 23/23.
- Isolated feature-boundary control passed: three positive builds (no features,
  alloc-only, minimal-meta), plus report-absence and method-absence failures
  without alloc. No new engine features were enabled in the production closure.
- Actual native census/nonallocation control passed: 24 independently rebuilt
  regex candidates and one accessible public prefilter; 100 query repetitions
  allocate nothing. The 257-byte allocating positive control and release pass.
- Both raw-fixture gates passed, including their negative cases: 188 admitted
  raw fixtures across 54 files, with no changed allowances.
- Source/spec review found the runtime-compiler/host binding gap. The build-time
  metadata repair produced the expected two missing-file compilation errors
  before implementation, then passed both native identity/census tests. A
  separate source review approved that correction. The first complete source
  run passed 403 checks and failed only the two local socket binds denied by
  the sandbox. The final full rerun with loopback access passed 406/406, with
  no failures or ignored tests. No test was skipped or weakened to bypass the
  socket checks. Default/explicit native and three-guest builds preserve all
  300 captured outcomes; guest checks prove compilation only.
- Bidder library/cache/isolation regression: 1,100 library tests passed (nine
  existing ignored), 238 cache-bound tests, 14 cache controls and 16 workspace
  isolation checks passed. No new ignored test was added. Formatting passes.
- All nine source-dependent library checks were then explicitly executed with
  the pinned primary/supplemental source directories: 9/9 passed, none ignored.
- Separate `insecure-bidders` invocations passed 24 report/public-API controls,
  two native identity/census controls and all 14 adjacent cache controls.
- Strict `cargo clippy --workspace --all-targets -- -D warnings` passed in
  default and `insecure-bidders` modes, using locked/offline resolution and two
  jobs. Existing GVL notices and the `proc-macro-error2` future-incompatibility
  notice remain. Workspace lint settings are unchanged; one test-helper-local
  `little_endian_bytes` expectation documents the fixed, host-independent
  length framing in the tree-identity digest. No production allowance was added.
- Separate API, source/spec, consumer and final independent quality reviews
  approve this bounded increment. Review approval does not claim whole-engine
  or deployment qualification.

These observations do not establish complete heap/work, selected guest layouts,
physical fit, serving admission or completed bidder catalog qualification.

### Reproduction

Use Rust 1.98.1, `CARGO_INCREMENTAL=0`, and the existing
`AUCTIONEER__BIDDER__SOURCE_DIR` / `AUCTIONEER__BIDDER__SUPPLEMENTAL_SOURCE_DIR` variables
pointing to reference checkouts containing the pinned Git objects. The full
source target needs localhost binding for its no-network-retrieval controls;
a sandbox bind refusal is a failure, not permission to omit those controls.

```sh
cargo test -p auctioneer-bidder --test schema_source_conformance --offline --locked -j2 -- --test-threads=1
cargo test -p auctioneer-bidder --lib --test schema_cache_bounds --test schema_cache_controls --test test_isolation --offline --locked -j2
cargo test -p auctioneer-bidder --lib --offline --locked -j2 -- --ignored
cargo test -p auctioneer-bidder --features insecure-bidders --test schema_source_conformance --offline --locked -j2 resource_layout
cargo test -p auctioneer-bidder --features insecure-bidders --test schema_source_conformance --offline --locked -j2 native_layout_
cargo test -p auctioneer-bidder --features insecure-bidders --test schema_cache_controls --offline --locked -j2
cargo clippy --workspace --all-targets --offline --locked -j2 -- -D warnings
cargo clippy --workspace --all-targets --features insecure-bidders --offline --locked -j2 -- -D warnings
cargo fmt --all -- --check
bash scripts/check-raw-json-fixtures.sh
bash scripts/check-raw-json-fixtures-fixtures.sh
```

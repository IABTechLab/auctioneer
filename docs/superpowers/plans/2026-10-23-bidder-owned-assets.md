# Bidder-Owned Assets Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans. Keep each verified cutover coherent; do not leave two production sources of truth.

**Goal:** Put each implementation's code, reviewed metadata, schemas, protocol/parameter captures and TOML provenance in one bidder-owned folder.

**Architecture:** `auctioneer-bidder/src/bidders/<key>/bidder.toml` owns registration, public definitions, aliases and provenance. Config generation consumes its data without importing bidder code. Imported-only packages remain non-executable and unpromoted; JSON schemas and protocol/parameter captures remain local, verbatim artifacts. Metadata YAML is read from the pinned external Git checkout, not retained locally.

**Tech Stack:** Existing Rust workspace, Serde/TOML/JSON, existing validators and offline source-object reader. No new crate, dependency, runtime scan, original catalog source pin, workload limit or budget equation. Supplemental bidder sources remain separately pinned.

## Contract And Status

Approved layout: [M7 sections 5.1 and 10](../specs/2026-10-16-m7-bidder-catalog-design.md#catalog-storage) and the [bidder rules](../../../crates/auctioneer-bidder/src/bidders/README.md).
This plan is part of M7, not a new milestone. Task 1's structural reader and
Task 2's production config/discovery/receipt cutover are implemented and tested.
The four reference-derived factories own verbatim canonical `parameters.json`
files. Generic remains explicitly local, without an invented parameter schema.
Tasks 3-4 relocated all original catalog assets and APS provenance, with
scoped cutover verification complete. The old central snapshot and APS JSON
manifest are removed; their readers use bidder-owned TOML and retained captures.
This is not completed catalog delivery, live registry promotion or qualification.

**Approved metadata boundary (2026-10-24):** Remove the 358 main-reference
metadata YAML captures and one supplemental APS metadata YAML capture from
bidder folders. Keep repository, immutable revision, source path, blob ID,
byte length, SHA-256 and reviewed differences in `bidder.toml`. Metadata has
source-only witnesses, with no local YAML or retained-path/digest witness.
The importer and source check continue reading immutable objects from the
pinned external Git checkout; they do not publish metadata YAML. Preserve
source interpretations, including relevant missing/null distinctions and
ignored spellings, in TOML notes rather than a duplicate YAML document.

Fresh main-reference staging must contain 521 files instead of 879: 259
manifests, 261 JSON schemas and one shared license. APS stays supplemental and
does not enter that staging count or the original 358-identity inventory.
Schemas and protocol/parameter captures remain byte-identical; source pins,
attribution, promotion and qualification requirements do not change. This
approved boundary supersedes earlier metadata-retention wording in this plan
and the linked design. The 2026-10-23 steps and evidence below remain historical;
they do not verify metadata removal. This amendment defines the approved
contract; the current implementation must pass the source-backed import and
provenance controls without relying on historical cutover results.

Use one folder per audited implementation key, not per public-name spelling.
Keep aliases and distinct bases sharing code separately attributed. The original
358 identities/261 schemas and APS's supplemental pin remain independent.

## Task 1: Freeze And Exercise The Structural Contract

**Files:** create `crates/auctioneer-config/build_catalog.rs`; modify config
`build.rs`, bidder `build_discovery.rs` and CLI `src/bidders/import.rs` as
consumers. Extend `crates/auctioneer-bidder/tests/discovery_codegen.rs` and config
build conformance at their actual owners. Unit tests stay inline.

- [x] Record the baseline commands/counts before changing readers. Inspect the
      existing config metadata validators, bidder feature reconciliation, import
      atomicity, isolated build copy closures and schema inventory controls.
- [x] Define one strict, build-only structural reader using existing dependencies.
      Root fields are the directory-matching `key`, required boolean `factory`,
      optional `required_feature`, effective `definitions` keyed by exact public
      identity, and `provenance`. Reference associations point to local original
      artifacts; no arbitrary constructor/module expressions are accepted. Freeze
      the concrete TOML shape and typed errors with positive and negative controls
      before switching production readers. Typed metadata stays with config's
      renderer; do not copy its validation algorithm into discovery or the CLI.
- [x] Write failing controls for executable and reference-only packages:
      `factory=true` requires `mod.rs`; `factory=false` forbids executable source and
      effective definitions. A compiled candidate without definitions is valid.
      No default factory value or inferred runtime promotion is permitted.
- [x] Reject unknown/duplicate manifest fields, duplicate public identities,
      invalid keys, feature mismatches, escaped/absolute paths, symlinks and
      nonregular inputs. Validate inactive definitions and references before feature
      filtering. Derive insecure URL relaxation only from `insecure-bidders`.
- [x] Provenance supports explicit source identities and per-artifact references
      so APS can use its supplemental pin. Require repository/revision, source
      path/blob/length/digest and retained-path/digest for retained artifacts only;
      metadata keeps source-only witnesses under the 2026-10-24 boundary.
      Artifact and nested retained records consistently use `source_path`,
      `source_bytes` and `source_sha256`; the retained path remains bidder-relative.
      Reject the former unprefixed retained field names rather than adding aliases.
      Local-origin definitions must not invent them. Do not include manifest
      self-hashes.
- [ ] Exercise provenance for implementation source trees/files and shared
      helpers, parameter schemas, protocol/parameter captures, metadata/alias
      mappings and attribution, not just JSON capture files. Keep Rust-port coverage
      associations/intentional differences separate from verbatim artifacts.
      Generated Task 16 receipts, not a guessed commit written into TOML, bind the
      actual Rust/generated/dependency closure. Independently mutate each category
      and require its affected evidence to become stale; checksum updates grant
      no renewed review or migration credit.
- [x] Observe each new control fail against the old reader before implementing
      it. Run `cargo test --offline --locked -p auctioneer-bidder --test discovery_codegen
-j2` and the relevant config inline/conformance tests. Commit verified work.
      This first commit adds the reader and its controls without switching production
      input ownership; Task 2 performs that atomic cutover.

Structural-reader evidence: before edits, `discovery_codegen` passed 24 tests,
`registry_join` passed 6, and config `codegen` passed 123. The initial 30 reader
controls failed against unimplemented behavior before the implementation; five
additional diagnostic/schema/helper controls failed before their fixes. The
resulting config `codegen` target passes 158 tests. Its Clippy target passes with
`-D warnings`. All 35 new unit tests live in `build_catalog.rs`; it is included
by the existing `codegen` integration binary. The first commit did not switch
production inputs; Task 2 now consumes that same reader. Full receipt-category
staleness controls belong to Tasks 3-4 and M7's Task 16 and are not discharged by
this structural preparation or the effective-metadata relocation.

## Task 2: Cut Over Effective Config And Factory Discovery

**Files:** config `build.rs`, `build_catalog.rs`, `build_bindings.rs`,
`build_storage.rs`; bidder `build.rs`, `build_discovery.rs`, `build_reconcile.rs`,
`src/bidders/*/bidder.toml`; core `src/bidder_registry.rs`; existing config build
tests, bidder `tests/{registry_join,registry_build,factory_binding_build}.rs` and
`tests/support/{build_workspace,registry_workspace}.rs`; CLI
`src/qualification/harness/providers/preparation.rs` and its conformance tests.

Preserve the current effective runtime catalog during this relocation. The
M7 plan's funded live-registry cutover and schema qualification remain separate
prerequisites before promoting new public definitions; these layout tests do
not discharge them. Exercise unknown-key execution through the existing
validated registry-plan harness without claiming that plan is installed.

- [x] Add real unknown-key/shared-implementation/alias controls before migrating
      production data. Show exact constructor execution and public attribution, not
      just generated text. Unknown bindings must refuse, never route to Generic.
- [x] Rewrite `factory_binding_build`'s actual child-build negative controls for
      unified ownership. Removing a whole bidder folder now removes metadata too;
      that is not a missing-factory test. Keep its effective definition and explicit
      `factory=true`, remove only `mod.rs`, and require the intended build refusal.
      Preserve separate inactive-feature, binding and forwarding mismatch controls.
- [x] Merge the current three secure definitions and feature-gated Mocktioneer
      into their owning `bidder.toml` documents. Generic is local origin;
      Mocktioneer derives from the `stackpop/prebid-server` fork's
      `feature/mocktioneer` branch, pinned to
      `c9ae35110d45f4dca35efc064d97f955a48445bf` (the verified `master` branch does
      not contain that adapter). Keep this supplemental source independent of the
      original catalog and APS pins. Record its implementation, metadata and schema
      witnesses and the existing local protocol/configuration differences without
      claiming parity or activating schema validation.
      Preserve every effective field and feature predicate; AppNexus's
      existing HTTPS rewrite remains a routing-review debt, not new evidence.
      APS keeps its real factory and no effective public definition.
- [x] Config reads the fixed sibling source root from `CARGO_MANIFEST_DIR`, not
      cwd. It emits data-only declarations and metadata; bidder build joins those
      declarations to its validated source tree. No config dependency on bidder code,
      constructor execution or another crate's generated output is introduced.
- [x] Remove the old production metadata readers and files in the same cutover.
      Keep arbitrary input overrides only behind `test-harness-override`, using the
      new manifest shape. Missing sibling data is fatal; no legacy fallback remains.
- [x] Before that removal, update provider preparation's old `generic.toml`
      receipt reader, explicit metadata overrides and build-input classification.
      Include the sibling data closure and shared reader. These consumers must ship
      in this Task 2 commit, not wait until Task 4; run their actual tests before
      committing. No path rewrite may promote a reference-only definition.
- [x] Register root, member, referenced-data and feature inputs for incremental
      rebuilds. Include the sibling data and shared reader in isolated config builds.
      Preserve secure-default and feature validation independently.
- [x] Run `cargo test --offline --locked -p auctioneer-config --all-targets -j2` and
      bidder `discovery_codegen`, `registry_join`, `registry_build` separately in
      secure and `insecure-bidders` lanes, plus `factory_binding_build` and CLI
      provider preparation conformance. Use existing bounded source/build harness
      prerequisites; execute the add/edit/rename/remove and feature off/on/off cycle.
      Commit only with correct copy closures and no production dual readers.

Task 2 verification (2026-10-23), using Rust 1.98.1, `--offline --locked`,
`CARGO_INCREMENTAL=0`, `-j2`, and two test threads:

- Config `--all-targets --no-fail-fast` passes all 572 tests, including the
  93 child-build controls. The redaction control independently marks the
  loopback fixture as secure; an owning insecure feature correctly permits
  that endpoint and would not exercise the refusal being asserted.
- Bidder `--lib` passes 273 secure-default tests and 284 with
  `insecure-bidders`. In both lanes, `discovery_codegen` passes 60,
  `registry_join` passes 10 and `registry_build` passes 10. The latter executes
  addition, edit, rename, removal and feature off/on/off without a clean build.
- `factory_binding_build` passes 10 tests, including real child-build refusals
  that retain metadata when removing only `mod.rs` and independently check
  feature ownership and forwarding.
- The normally ignored APS source-object test is explicitly executed with
  `AUCTIONEER_APS_SOURCE` pointing to the supplied immutable reference checkout:
  one test passes. Relocated schema and remaining capture bytes match the
  original objects; only local descriptor paths and license retention change.
- CLI `qualification::harness::providers::` passes 75 tests. It preserves every
  bidder source/data input, rewrites only `definitions.generic.endpoint`,
  accepts source/capture files above the metadata-only 64-KiB ceiling, and
  refuses changed, deleted, added and untracked frozen inputs. Its temporary
  Git fixtures own their commits; they do not borrow the worktree's `HEAD`.
- All-target Clippy for config, bidder and CLI passes with `-D warnings` under
  the existing workspace lint policy. Fixture setup helpers propagate fixed
  errors instead of panicking inside `Result` functions. Formatting, diff
  whitespace, the 170-fixture/51-file raw-JSON gate and its refusal controls
  pass. The existing GVL deletion notices and dependency future-incompatibility
  warning remain visible; this round does not suppress them.
  After lint cleanup, the full 572-test config suite, both 60/10
  discovery/registry lanes and all nine provider-preparation tests pass again.
- Independent spec-compliance and code-quality reviews of Task 2 and the
  four canonical source schemas found no remaining scoped defects. These
  checks do not activate schema validation, install the funded live registry,
  promote APS or complete the reference/capture migration below.

The importer is never rerun destructively over hand-written packages. Tasks 3-4
switch the reference format and consumers together; fresh staging is still
non-overwriting, and check mode verifies the pinned source projection without
replacing reviewed local sections.

## Task 3: Import Reference Packages Without Promotion

**Files:** CLI `src/bidders/{import,inventory,check}.rs`,
`tests/{bidders_import,bidders_source}.rs`; original catalog reference files;
the per-implementation bidder folders and shared inventory witnesses.

Tasks 3 and 4 form one atomic reference/provenance cutover batch. Task 3 may
prepare the new tree, but must not delete or commit removal of the old reference
tree before all Task 4 readers/includes and their tests switch. Do not commit an
intermediate state with broken Cargo includes or reference consumers.

- [x] Derive package ownership from the complete audited source binding map,
      including bases sharing implementations and aliases. Reconcile all 358 exact
      public names, 261 original schemas and APS separately. Do not infer ownership
      from normalized public names or create one schema per implementation by loss.
- [x] At the 2026-10-23 cutover, import emitted reference-only `bidder.toml`
      documents and local original metadata/schema captures, with per-bidder
      witnesses in TOML rather than a second JSON provenance manifest. Verbatim
      metadata preserved null/missing values and ignored spelling; effective
      definitions stayed absent. The 2026-10-24 amendment supersedes metadata
      retention only; schemas stay verbatim and metadata source witnesses and
      relevant interpretations stay in TOML.
- [x] Keep `parameters.json` for a single canonical schema; use explicit local
      `schemas/<public_name>.schema.json` associations for independently sourced
      schemas sharing code. Preserve every raw schema byte and draft.
- [x] Test enabled HTTP `mediafuse`, disabled/endpoint-less bases, a shared-code
      base pair and aliases with independent vendors. All source-only packages remain
      non-callable with zero constructors/definitions despite source activation.
- [x] Preserve atomic no-replace fresh staging and no-write `--check`. Import
      must refuse an existing destination rather than overwrite code or reviewed
      definitions. Any promotion merge is a separately reviewed operation, not an
      import side effect. Check mode verifies importer-owned reference projections
      against pinned source objects; it must not equate the reviewable serving tree
      with a fresh reference-only staging tree or overwrite local reviewed sections.
- [x] Reconcile the moved reference tree against the old witness set before
      deleting it; retain shared cross-bidder inventory/legal owners only. Remove
      per-bidder duplicate sources only after all Task 4 consumers have switched.
- [x] Run CLI `bidders_import`/`bidders_source` with
      `AUCTIONEER__BIDDER__SOURCE_DIR` pointing to the supplied immutable-reference checkout.
      Require exact operation/member counts and drift/no-write/link controls. Continue
      through Task 4 before committing this reference cutover.

## Task 4: Move Qualification And APS Provenance Consumers

**Files:** bidder `tests/support/{schema_qualification,schema_build}.rs`,
`tests/{schema_source_inventory,schema_compiler_controls}.rs`, APS
`bidder.toml`, `conformance.rs`, `fixtures/`; `.gitattributes`; CLI
`src/qualification/harness/providers/preparation.rs` and its conformance tests.
Also migrate all four direct reference-tree path constructors in bidder
`tests/schema_cache_bounds.rs`; changing the shared inventory reader does not
change those allocation/descriptor audit inputs.

- [x] Make schema qualification resolve the complete declared schema set from
      bidder-owned files and TOML witnesses. Preserve independent source revision,
      blob, length and digest expectations, exact counts and all malformed-input
      controls. A manifest re-pin must not silently change the trusted revision.
- [x] At the 2026-10-23 cutover, APS's existing JSON provenance moved into its
      owning TOML, preserving the complete witness tree, source pins,
      parameter/protocol counts and independent comparisons. Original
      protocol/parameter/metadata/schema/license bytes stayed unchanged, and the
      redundant JSON manifest was deleted after all consumers switched. The
      2026-10-24 amendment removes only metadata YAML and its retained witness;
      its source witness stays in TOML and all other captured bytes stay unchanged.
- [x] Keep source attribution in the root README and shared license material
      with one cross-bidder owner, not in every bidder folder. Remove duplicate
      bidder-local license captures in the same cutover as their integrity readers;
      preserve source object IDs and independently pinned license bytes at the
      shared owner. Do not weaken path confinement to permit arbitrary `../` links.
- [x] Use `parameters.json` consistently for canonical schemas and
      `fixtures/parameters.json` for externally captured parameter test cases.
      They are different document shapes, not interchangeable inputs. Do not
      manufacture empty captures or an unreviewed schema solely for visual symmetry.
- [x] Keep capture verification before protocol dispatch, including a filtered
      single-case run. Re-run same-length corruption controls and source-object
      verification; compare original versus relocated artifact digests independently.
- [x] Update provider preparation's explicit metadata rewrites, build-input
      classification and receipts together. The runtime-metadata consumers were
      already switched in Task 2; recheck any additional reference/provenance paths
      affected by this batch. No generic path rewrite may convert a
      source-only record into an effective bidder or misclassify code as reference.
- [x] Extend checkout attributes to every relocated byte-pinned input. Execute
      actual LF/CRLF checkout filters with an unprotected positive control. Update
      no constants, schema engines, fixtures or workload bounds merely to pass.
- [x] Run schema inventory/build controls and `--test schema_cache_bounds`,
      APS protocol/source conformance,
      allocator and provider preparation tests. Artifact/qualification descriptors
      whose input identities changed remain stale until revalidated; relocation is
      not engine, resource or deployment qualification. Commit the completed
      Tasks 3-4 batch together only after old-path includes/readers are gone.

## Task 5: Verify The Closed Cutover

- [x] Re-read the M7 contract and scan every active reader, include path,
      isolated-workspace copier and documented command. Legacy per-bidder locations
      may appear in clearly historical evidence only, not as a fallback or a second
      editable catalog. Runtime input must remain compiled data, never a directory.
- [ ] Run secure-default package tests separately from feature-unified workspace
      tests, then workspace/all-target Clippy with `-D warnings`, formatting and
      raw-JSON fixture gates. Preserve conformance/ignored-test CI coverage.
- [x] Check the real Cloudflare/Fastly/Spin target builds using the pinned SDK
      and existing workload. Do not infer memory fit from compilation or lift R0.
- [x] Record commands, counts, failed controls and actual remaining gates;
      independently review spec compliance and implementation quality. Update this
      checklist incrementally, commit and push verified rounds to the existing M7 PR.

### Cutover Verification (2026-10-23)

Historical evidence for the original retained-metadata layout, not verification
of the approved 2026-10-24 metadata-removal boundary.

Commands use Rust 1.98.1, `CARGO_INCREMENTAL=0`, `--offline --locked`,
`-j2`, and two test threads. Source-dependent commands set
`AUCTIONEER__BIDDER__SOURCE_DIR` or `AUCTIONEER_APS_SOURCE` to the supplied immutable
reference checkout, as appropriate.

- The pre-cutover CLI baseline passed five import and 11 source tests. The new
  complete-owned-layout import control failed on the missing bidder manifest,
  and all 16 new owned-schema controls failed on the old layout before their
  readers changed. These are observed failures, not inferred coverage.
- Fresh staging then contained exactly 879 files: 259 manifests, 358 raw YAML
  metadata captures, 261 raw schemas and one shared license. A one-time Rust
  migration reconciled all 620 original source-path/blob/digest witnesses,
  checked retained bytes, preserved the current factories/definitions, then
  removed the obsolete central snapshot. No foreign-language tooling or second
  editable provenance manifest remained. The CLI source-projection check could
  revalidate that retained tree directly against the pinned Git objects.
- `cargo test -p auctioneer-cli --no-default-features --bin auctioneer-cli
--test bidders_import` passes 152 unit and six command tests; the separate
  `bidders_source` target passes 11. The six command tests include actual Git
  checkout filters with an unprotected conversion control. Check mode verifies
  current reviewed packages, not just fresh staging. Its initial integration
  failure came from the fixture supplying a forbidden `..` component; the
  fixture now supplies its canonical path, without weakening confinement.
- An inline tree-witness control catches changed, missing and duplicate
  implementation witnesses. Temporarily skipping the tree comparison made it
  fail by accepting a changed category; restoring the comparison passed. Source
  revision, repository, schema ownership, independent digests and supplemental
  exclusions retain separate negative controls.
- `cargo test -p auctioneer-bidder --lib --test schema_source_inventory
--test schema_compiler_controls --test schema_cache_bounds -- --include-ignored`
  passes 280, 40, 146 and 154 tests respectively after cleanup. APS's normally
  ignored source-object verification executes in that command, not merely
  compiles. At that cutover its TOML owned 29 blob witnesses, three source trees
  and 19 retained artifacts, including metadata YAML. Protocol dispatch verified
  those witnesses before execution. Metadata's source witness remains required
  under the 2026-10-24 amendment, but its retained witness does not.
- Config `--all-targets --no-fail-fast` passes all 572 secure-default tests,
  including 93 isolated child-build refusals and 158 codegen tests. Its runtime
  metadata probe still reports three active definitions, 1,895 heap bytes and
  23 allocations. Discovery passes all 60 tests outside the socket-restricting
  sandbox; the initial two socket setup failures are not suppressed or skipped.
- Registry build/join controls pass 10/10, including 16 real add/edit/rename/
  remove and feature off/on/off children. The optional-bidder lane separately
  passes 60 discovery, 10 join and 10 factory-binding tests; the last drives
  17 real children. The isolated copier now includes raw captures, the shared
  license and the data-only `bidder_manifest.rs`. Missing-copy controls failed
  before these fixes. No new workspace crate or dependency was added.
- All 78 provider verification tests pass. New native receipt controls first
  failed independently at the old 128-member and 64-KiB-listing bounds. Receipt
  collection now uses the existing native 10,000-input and 8-MiB-receipt bounds,
  preserving manifest/per-file caps; exact membership rejects truncated and
  duplicate receipts. This is native harness capacity, not a serving limit or
  isolate-budget change.
- After lint cleanup, all 12 provider-preparation tests pass again, including
  the extracted mutation checks. The final native CLI unit/import rerun passes
  152/6. Broader feature-unified workspace assertions remain a separate pending
  Task 5 acceptance run; workspace Clippy is not reported as that test run.
- Workspace `--all-targets` Clippy passes with `-D warnings`. Tests use ordinary
  assertions and fallible setup rather than new lint suppressions. Formatting,
  diff whitespace and the raw-JSON gate pass (170 fixtures in 51 files), along
  with the gate's refusal controls. The existing GVL deletion notices and
  dependency future-incompatibility warning remain visible.
- Deployment library checks pass for Cloudflare `wasm32-unknown-unknown`,
  Fastly `wasm32-wasip1` and hosted Spin `wasm32-wasip2`, with the unchanged
  EdgeZero pin. Compilation does not establish WASM memory fit, schema-engine
  qualification, provider readiness, live-registry installation or bidder
  protocol completion. Evidence whose source identity changed stays stale
  until its owning qualification is rerun. M7's remaining tasks and explicit
  R0 readiness blockers are unchanged.
- Separate spec-compliance and quality reviews found no scoped correctness
  defects. The quality review's missing tree-witness controls were added and
  verified rather than deferred.

### Full Linked-Consumer Follow-Up (2026-10-23)

The subsequent complete `schema_build_controls` run exposed one missed
reader: its explicit-cache membership check still used the deleted
`tests/fixtures/catalog/reference` directory. The run passed 160 tests and
failed that exact lookup; the earlier scoped checks did not establish the full
linked-consumer closure. It now reads `src/bidders` through the same strict
owned-inventory reader, with all 261-name, 300-case, persistent-cache and
compiler-refusal assertions retained. The corrected secure-default target
passes all 161 tests, including the new fallback/receiver controls. Its native
and three guest artifacts retain complete callback membership in both compiler
modes; this is not resource or deployment qualification.

The CI-specific discovered-source formatter also exposed an existing APS
formatting failure invisible to `cargo fmt --all`. Formatting that file and
correcting its shared-license comment makes the exact discovered-source check
pass without changing protocol behavior or captured bytes. Broader workspace
test execution in Task 5 remains distinct and pending.

# Configuration Model Ownership Implementation Plan

> **For agentic workers:** Use superpowers:subagent-driven-development or superpowers:executing-plans. Execute each checkpoint with spec and quality review.

**Goal:** One canonical shared configuration model crate, bidder-owned settings,
and cycle-free generation without increasing the workspace package count.

**Architecture:** Reuse today's schema library as `auctioneer-config` and
today's generated-data package as `auctioneer-config-data`. Move shared core
DTOs into the model library, parameterizing only bidder settings. Generate the
concrete settings aggregate inside the bidder crate; keep orchestration in core.

**Tech Stack:** Existing Rust workspace, Serde, validator, quote/syn and Cargo
metadata. No new external dependency, SDK configuration framework or resource
equation. Follow the
[reviewed ownership contract](../specs/2026-11-03-configuration-model-ownership-design.md).

Tracking: #124/#127, with product-default ownership under #126. This is an
ownership prerequisite inside PR #140, not another runtime-format or SDK project.

Status: package-role rename and implementation approved by the maintainer on
2026-11-03. Serving adoption and startup/resource acceptance remain separate.

## Boundaries

- This worktree is `implementation/operator-runtime-config` for PR #140.
  Existing uncommitted defaults/account/sync work is parent-owned; preserve it.
- Do not publish, merge, update dependency pins, refresh archives, change
  limits or waive operator-runtime startup/storage review in this task.
- Do not use handwritten include/path imports, parallel heavy Cargo jobs,
  real bidders as shared fakes, compatibility package aliases or opaque JSON
  in place of typed settings.

## Task 1: Freeze The Actual Migration Baseline

**Files:** Existing manifests, `config/mod.rs`, `policy.rs`, `override_value.rs`,
config-schema model modules, bidder settings/build helpers and test owners.

- [ ] Record Git status, model/data package roles, workspace package count,
  toolchain and locked SDK. Inventory model declarations separately from
  immutable compiled metadata and engine/SDK state.
- [ ] Complete the already-red response-cap slice before taking a green
  migration baseline; do not hide its seven new cases as migration failures.
- [ ] List and run affected library and root/override/policy cases, actual
  config codegen/default/callback targets and core store/refusal/funding tests.
  Record actual dispatched names/counts, not an assumed count from source.
- [ ] Read the existing compiler fixture's owned source closure and command
  owners. Preserve exact reconciliation and secure/default feature lanes.
- [ ] Record current settings metadata and target-layout/allocation controls.
  Keep immutable captures and historical evidence outside mechanical rewrites.

## Task 2: Place Concrete Settings With Bidder

**Files:** `auctioneer-bidder/{build.rs,src/lib.rs}`; new owner-local
`src/{bidder_settings,bidder_endpoint}.rs`; current config build/settings/endpoint
modules; native
settings generator, tests/generated_settings and copied child support.

- [ ] Add a failing owner-local compile/import control proving the actual
  generated settings type is available from bidder and the shared model can
  be instantiated with it without depending on implementations.
- [ ] Move settings/endpoint generation and the existing bounded helpers,
  preserving manifest discovery and updating generated module/helper paths.
- [ ] Retain or hand off discovery's complete unfiltered catalogue for both
  renderers; the existing reduced factory view loses reference-only manifests
  and full fields. Reuse the same scanned input. Independently preserve empty,
  reference-only, feature-disabled and target-ineligible type declarations
  without relaxing factory/publication eligibility.
- [ ] Keep config-data as a bidder build-dependency for metadata reconciliation,
  registry generation and schema staging. Update the explicit reconciliation
  edge to `auctioneer-config-data/<feature>`, including negative controls for
  missing forwarding; an import-only rename does not satisfy this step.
- [ ] Implement structural `Validate` through the existing limit function with
  fixed errors and no input params. Remove duplicate explicit structural scans
  in orchestration when nested validation takes over; retain owner-semantic
  callbacks and their exact one-call-per-identity controls.
- [ ] Update real bidder imports locally and shared harness imports through
  the bidder aggregate. No manual settings variant or real bidder fake enters
  model tests. Preserve every old settings boundary/credential control.
- [ ] Before child execution, split the mixed config build-failure ownership:
  move settings/endpoint compiler consumers to the existing bidder owner and
  keep data-build failures with config-data. Obtain generated files from bidder
  OUT_DIR; install one controlled bidder tree in the disposable source closure
  for both fixed-root discovery and data fixtures, without a production source
  override. Preserve all old empty/reference/inactive, namespace, insertion and
  credential controls and reconcile their actual new owners/case names.
- [ ] Execute the existing generated-settings owner once and require the
  enumerated complete child set. Retain missing/swapped callback and unavailable
  factory refusals; do not add another heavy compiler owner.

Task 2 and Task 3 form one compiling declaration cut: the old config model's
concrete settings dependency must not be left dangling between them.

## Task 3: Reuse The Two Existing Package Roles

**Files:** Root manifests/lockfile, current config/config-schema directories,
core shared DTO sources, affected consumer manifests/imports.

- [ ] Move the current generated-data directory to `crates/auctioneer-config-data`
  and current schema library to `crates/auctioneer-config`; update package names,
  workspace inherited dependencies and feature forwarding together. No new
  package or old-name alias remains.
- [ ] Keep config-data's build and runtime edges directed to the new config
  library. Keep model config free of build.rs and data/bidder/core/provider
  dependencies. Native build-support remains optional and non-serving.
- [ ] Move floor-currency, source/policy/override and reusable scalar DTO
  declarations and their pure validation to model config. Reuse effective
  model modules already present; do not copy their definitions again.
- [ ] Parameterize only settings-bearing roots/entries. Add red generic cases
  using a local non-Default synthetic settings type, then preserve omission,
  null, clear and JSON/TOML behavior without requiring Settings: Default.
- [ ] Add explicit Settings: Validate bounds/delegation rather than assuming
  validator_derive will infer them. Enable the existing validator dependency
  under config-model, not just build-support. Execute a model-only synthetic
  rejection/counter case with no build-support: supplied settings visited once,
  omitted settings unvisited, fixed nested codes and empty input params. Remove
  delegation temporarily and require failure. Keep existing derive-based scalar
  rules and independent programmatic request-binding/resource controls.
- [ ] Keep only type composition, semantic validation/resolution and lifecycle
  in core. Replace moved inherent methods with explicit orchestration functions;
  bridge SDK traits transparently where required without duplicate fields or
  guessed secret metadata. Update callers atomically.
- [ ] Update generation's destination paths and type imports. Build-only raw
  DTOs remain native helpers, not an alternate runtime policy model.
- [ ] Run Cargo metadata and checks for model/data/bidder/core/CLI. Inspect
  actual dependency kinds/features and compare package count with Task 1.

## Task 4: Preserve Tests, Fixtures And Feature Ownership

**Files:** Existing model/data library units and integration targets; bidder
compiler fixtures/build_workspace; .github workflows and colocated scripts;
raw JSON allowlists, development docs and active operator plan references.

- [ ] Move unit tests with their implementation and data integration tests
  with their owner. Preserve independent expected values, before-state guards,
  all existing case names/feature conditions and process/allocator isolation.
- [ ] Update copied compiler source closures to the actual package graph and
  generated settings owner. Do not reintroduce cross-crate source includes or
  copied private code that substitutes for a normal dependency.
  The settings/endpoint owner split and its closure are already required before
  Task 2 execution; this step covers the remaining shared source consumers.
- [ ] Update raw JSON allowlist paths without increasing counts. Run the
  fixture gate and existing source-import/ownership target.
- [ ] Update CI selectors and feature forwarding to retain every existing
  once-owned compiler, secure-default, CLI and adapter lane. Do not revive
  workflow-source snapshot tests or add redundant execution owners.
- [ ] Update current asset paths in CLI GVL maintenance/refresh, OpenRTB/catalogue
  maintenance and existing refresh/check scripts. Run the maintenance-only CLI
  lane without config-data's build dependency; move archives without changing
  their bytes or refreshing them incidentally.
- [ ] Compare recorded before/after dispatch sets, not just total green counts.
  Explicitly explain any genuine owner-only relocation; no skipped case counts
  as acceptance.

## Task 5: Verify And Resume Operator Cutover

**Files:** Evidence/checkpoint section in the existing operator plan and crate
descriptions/development documentation.

- [ ] Run model/data/bidder/core tests and strict lint, full generated-settings
  compiler owner, existing allocation/layout guards and live store/refusal
  integration owners. Serialize heavy builds and preserve caches.
- [ ] Check CLI default and maintenance-only dependencies; compile/lint the
  three actual guest target graphs as well as native. Distinguish compile,
  execution, observations and qualification in the evidence.
- [ ] Run formatting, whitespace, raw JSON and source-isolation controls.
  Record known external notices rather than suppressing them.
- [ ] Independently review specification compliance and code quality. Fix and
  rerun concrete regressions before calling the ownership move complete.
- [ ] Update current ownership documentation and continue the parent operator
  runtime plan at shared semantic context/host resolution. Its startup/input
  funding review, reader cutover and same-binary/two-config acceptance remain
  required; this move alone does not close #124 or PR #140.

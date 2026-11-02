# Catalog Review Follow-Ups Implementation Plan

> **For agentic workers:** use superpowers:executing-plans or
> superpowers:subagent-driven-development. Preserve independent oracles,
> original source captures and the approved controlled-native scope.

**Goal:** Close the post-merge review gaps without changing bidder publication,
workload limits, schema reservations or license/attribution PR #25.

**Architecture:** Execute the generated catalog and registry inside a portable
WASI integration target. Derive expected membership independently from source
manifests, and compare provenance with actual pinned Git objects rather than
only a local inventory. Retain both raw-capture refusal boundaries.

**Tech Stack:** Rust 1.98.1, existing Cargo workspace, TOML/Serde, pinned Git
objects, existing Wasmtime 48.0.2 and workflow-colocated Bash scripts.

## Scope

- The M7 historical-plan and APS no-sync-template clarifications are already
  merged. Do not re-open them or invent sync capability.
- SDK `9fe6c56c` stays pinned. Separate first online dependency acquisition from
  offline verification; preserve historical captures and their original pins.
- GitHub Actions billing is an external execution blocker, not permission to
  weaken jobs, ignore tests or claim hosted CI passed.

## Execution

### Guest Publication

Files: `crates/auctioneer-bidder/tests/catalog_publication.rs`,
`crates/auctioneer-bidder/Cargo.toml`, `.github/workflows/test.yml`.

- [x] Add a portable integration oracle that reads bidder manifests independently
  and checks generated metadata/bindings and actual serving-registry membership.
  Include nonempty portable/native-only and feature-selected control populations.
  Use no hardcoded bidder names and no generated expected inventory.
- [x] Execute the target natively and in WASI secure/development lanes. Prove
  removal of the native-only publication predicate fails the actual guest tests.
- [x] Execute both guest feature lanes in CI with the already pinned Wasmtime.
  Require nonempty passing summaries and propagate Cargo/pipeline failures.

### Workflow Script Ownership

Files: `.github/workflows/`, `.github/workflows/scripts/`,
`crates/auctioneer-core/Cargo.toml`, active development documentation.

- [x] Extract substantive multiline shell programs into workflow-colocated scripts;
  leave short commands inline. Preserve pins, working directories, conditions,
  outputs, exact dispatch counts and failure propagation.
- [x] At AG's explicit direction, remove the Rust workflow-configuration
  snapshot target and its unused YAML dependency. Keep actual executable CI
  checks; review configuration directly and validate scripts through execution.

### Source Provenance

File: `crates/auctioneer-cli/tests/bidders_source.rs`.

- [x] Verify applicable reference factories through their declared pinned Git
  objects, byte counts and digests, including retained artifacts and trees.
  Discover the scope rather than adding a list of six bidder names.
- [x] Add independent negative controls for changed source witnesses and ensure
  the driver cannot pass with an empty checked population. Reuse the existing
  mandatory source-test CI target.

### Raw Capture

File: `crates/auctioneer-runtime/src/bounded_json.rs`.

- [x] Verify the pinned Serde JSON token for borrowed and owned raw values,
  correct the inaccurate owned-marker claim and remove obsolete code if safe.
  Preserve map-key and newtype-struct refusals, including flatten routes.
- [x] Run focused raw-value and full runtime tests with a regression/mutation
  control for the actual capture path.

### Reproduction And Handoff

Files: `docs/testing/development.md`, `docs/testing/provider-handlers.md`,
`ISSUES.md` and a scoped verification
record under `docs/testing/evidence/`.

- [x] Clarify current online bootstrap/offline verification and historical SDK
  evidence; do not rewrite historical captures as current observations.
- [x] Run focused source, guest, runtime, script and strict lint checks.
  Record actual counts and exclusions. Update only genuinely resolved issues.
- [x] Self-review, commit/push verified repairs and open a separate PR to main.
  Report billing-blocked hosted CI separately from local verification.

Handoff: [PR #26](https://github.com/IABTechLab/auctioneer/pull/26), based on the
verified repair commit `d51e4799`. Historical captures and the SDK pin remain
unchanged. Rust workflow-configuration snapshot tests were removed at AG's
direction; the executable CI checks were retained.

## Subsequent Review

- [x] Execute the config-owned census on WASI in both feature lanes and prove
  it rejects bypassing its native-only constructor filter.
- [x] Exercise actual native/WASI baked-alias builds with synthetic owners;
  pin explicit targets, vary inherited defaults and prove the absent-base
  rejection detects removal of the build-script filter.
- [x] Retire stale forward-looking references and file ownership entries in
  the original catalog plan while retaining dated historical evidence.
- [x] Align existing checkout/cache/Rust setup action families to v7/v6/v2;
  validate YAML and the adopted warning mode without snapshot tests.

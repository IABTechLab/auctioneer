# Bidder Endpoint Bindings Implementation Plan

> **For agentic workers:** Use subagent-driven-development and test-driven-development. The coordinator owns repository integration, serialized Cargo checks and publication.

**Goal:** Supply typed, bidder-owned endpoint bindings without central protocol names.

**Architecture:** The existing manifest scanner validates an adjacent field-to-placeholder
mapping. Build-time Rust generation produces borrowed values and fixed stack pairs;
the existing capped endpoint builder consumes those pairs unchanged.

**Tech Stack:** Existing Rust, Serde/TOML, `quote`, `syn`, and endpoint/sink APIs.

Design: [bidder endpoint bindings](../specs/2026-10-24-bidder-endpoint-bindings-design.md).

## Tasks

### 1. Validate Adjacent Contracts

- [x] Add compiling red controls to `crates/auctioneer-config/build_catalog.rs`
      for arbitrary valid endpoint declarations and independent malformed, empty,
      duplicate, reserved and unknown-key cases. Observe semantic failures.
- [x] Add the strict optional declaration to `bidder_manifest.rs`. Validate
      lowercase Rust field names in the scanner; share placeholder validation with
      `build_support.rs` rather than duplicating the runtime URL safety policy.
- [x] Update the importer struct initializer with an absent declaration; do not
      rewrite reference data, schemas or captures. Run the owning config tests.

### 2. Generate Typed Borrowed Bindings

- [x] Add inline generator controls in `build_bidder_endpoint.rs`, using an
      arbitrary implementation and distinct field/placeholder names. Observe red
      against an empty-output generator before implementing generation.
- [x] Generate public `Values<'a>` fields and `as_pairs()` with `quote!`; expose
      it via `src/bidder_endpoint.rs` and the existing build coordinator. No new
      dependency or runtime storage owner is required.
- [x] Declare TTD's `supply_id = "SupplyId"` without enabling its factory or
      definitions. Verify a compiled consumer with pointer-identity/value controls.

### 3. Bind Template Validation To Its Owner

- [x] Extend codegen and URL controls with independently varied declared and
      referenced names. A declared arbitrary placeholder must succeed; an undeclared
      sibling and a reserved placeholder must refuse. Observe semantic red.
- [x] Remove the global AccountID/PublisherID-only gate. Carry the owning
      supported names as build-only context into each definition's renderer,
      including inactive definitions. Keep runtime output as extracted names only.
- [x] Declare common context placeholders only for implementations that already
      bind them. Update child-build fixtures to declare their actual contract; test
      cross-owner isolation and inactive-definition refusal through actual builds.

### 4. Verify And Publish

- [x] Compile and use generated TTD bindings through the shared builder, testing
      encoding/traversal and exact output boundaries. Reuse allocator controls;
      do not add a second builder or bidder-local encoder.
- [x] Run config library/codegen/URL controls, bidder libraries/registry/endpoint
      and allocation controls, importer controls, strict lint, format and raw-JSON
      gates. Serialize Cargo with offline/locked resolution and two jobs.
- [x] Obtain spec then quality review, fix concrete findings and rerun owners.
      Update bidder rules and the M7 integration plan, commit and push this slice.
- [x] Continue TTD's separately reviewed protocol integration with a refusing
      stub red and candidate green. Empty definitions and remaining delivery/schema
      requirements stay explicit; this shared slice does not satisfy them.

## Integration Evidence

The declaration controls first failed four cases against strict unknown-field
rejection. Owner-scoped rendering then failed the declared custom-placeholder
control against the old global allowlist; the added redirect-only reserved-name
control also failed before the shared vocabulary was completed. All seven
generator controls failed against a compiling empty-output renderer, then
passed with typed generation. These are real semantic reds, not missing APIs.

Config library/codegen/URL checks passed 60/185/108 tests. The initial sandboxed
run failed the pre-existing Unix-socket fixture with `EPERM`; the complete
owning command passed outside the sandbox. Two new child-build tests exercised
five independent owner/feature rows and definition self-grant rejection.

The bidder library passed 590 tests with one source-dependent control not
selected, plus 72 discovery and ten endpoint-builder controls. The allocator
fixture initially compared the source pool after `finish()` had intentionally
moved it into metadata; dropping the builder before pool checks corrected that
test ownership assumption. The allocator and registry reruns passed nine and
twelve tests. Typed binding construction/expansion measured zero peak heap
allocation at the exact output boundary, one byte over and oversized input.

Strict config/bidder all-target Clippy passed without new suppressions. Follow-up
lint repairs load the shared validator once per crate, emit an unsuffixed array
length with `proc_macro2::Literal`, use the existing harness's fallible helper
style and simplify AST test helpers. No runtime builder, funding coefficient,
workload limit, schema/capture bytes, public definition or factory changed.

Final verification completed on 2026-10-24:

- Secure-default config library plus seven explicitly selected integration
  targets: 432 passed. This includes code generation, URL policy, callbacks,
  activity, baked-default and fold controls; it is not the whole config suite.
- Secure-default bidder library/discovery/endpoint/allocation/registry command:
  590/72/10/9/12 passed, with one source-dependent check not selected there.
- Feature-enabled bidder library/allocation/registry: 601/9/12 passed. The
  source-dependent APS control then passed explicitly with `AUCTIONEER_APS_SOURCE`
  and `--ignored`; no ignored control is counted as an execution by default.
- Final child-build owner/isolation and self-grant tests: two passed across
  the five owner/feature rows and the independent definition-injection case.
  The existing PublisherID child-build positive control also passed independently.
- Maintenance-only CLI units/import/source/wave: 155/7/13/9 passed against
  immutable reference objects. The importer still emits no YAML or endpoint
  declarations for unreviewed reference packages.
  A final consumer assertion rejected an injected nonempty endpoint declaration
  in the real importer; after restoring the `None` initializer, all seven
  importer tests passed again. The mutation is not retained in the source tree.
- Strict all-target Clippy for config/bidder and both maintenance-only and
  application-enabled CLI passed without new suppressions. Application lint
  compiles the default config/deploy command surface; those commands were not
  executed against providers.
- Cloudflare `wasm32-unknown-unknown`, Fastly `wasm32-wasip1` and Spin
  `wasm32-wasip2` library checks passed. These are compilation checks, not
  hosted execution, deployment qualification or a resolution of R0 blockers.
- Formatting, whitespace and the unchanged 172-fixture/52-file raw-JSON gate
  passed. No Cargo dependency/lockfile, JSON schema/capture or workflow changed.

Spec review approved owner isolation and the borrowed API. Quality review
approved the corrected allocation measurement and independently varied bounds.
The prerequisite did not compile or activate the private TTD protocol draft.
Its subsequent coordinated integration, original captures and allocation controls
are recorded in the [TTD candidate execution record](2026-10-23-priority-bidder-parallel-execution.md#ttd-protocol-candidate-2026-10-24).
Public definitions remain empty and the full wave acceptance gates remain open.

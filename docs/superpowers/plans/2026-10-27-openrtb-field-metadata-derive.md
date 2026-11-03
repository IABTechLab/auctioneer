# OpenRTB Field Metadata Derive Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans. Follow the qualification gate before the scanner cutover; track steps incrementally.

**Goal:** Replace source-spelling storage classification with compiler-checked request-model metadata, without changing disclosure policy or request behavior.

**Architecture:** `auctioneer-macros` derives constants using an interface owned by `auctioneer-openrtb`. Build support walks the real `BidRequest` graph and emits both the unchanged disclosure tables and a target-side const agreement check. There is no handwritten type registry, runtime metadata walk, source-code inclusion or blanket unknown-type implementation.

**Tech Stack:** Workspace-pinned Rust 1.98.1; existing `syn`, `quote`, `proc-macro2`; workspace-managed native-only `trybuild` compiler conformance tests.

Approved design: [field-metadata derive](../specs/2026-10-27-openrtb-field-metadata-derive-design.md), approved by AG on 2026-10-27.
All edits stay in `.claude/worktrees/bidder-catalog`. Use serialized Cargo invocations, `CARGO_INCREMENTAL=0`, `--offline --locked -j2` after any authorized lockfile update. Do not change limits, resource coefficients, evidence bytes or SDK/toolchain pins.

## Interface And Files

- Create `auctioneer-openrtb/src/field_metadata.rs`: `Storage::{Typed, RawJson, JsonMap}`, `FieldMetadata`, constant `Model`/`Field` descriptors and structural signature comparison. `FieldMetadata` supplies `STORAGE` and `MODEL: Option<&'static Model>`; wrappers propagate raw storage and child links but reset direct-map capability. Explicit leaf implementations replace any permissive blanket.
- `Model` exposes wire name, diagnostic Rust source label and declaration-order fields, and retains compiler type identity for native duplicate/cycle checks. `Field` exposes name, flattened status, storage and child descriptor. `Field::of::<T>(name, flattened)` reads the actual type's constants. `Model::new::<T>` enforces names, duplicate named fields and at most one legal catch-all in const evaluation. Source labels are structural signature data, not globally unique type identities.
- `ExpectedModel`/`ExpectedField`/child identities are flat structural records for generated agreement checks. `graph_matches(root, expected)` is const, checks all fields and edges in both directions, and rejects missing/extra/duplicate records. It uses no custom hash or string-based type classification.
- Implement `OpenRtbFieldMetadata` in `auctioneer-macros/src/field_metadata.rs`, with inline unit tests; export through `src/lib.rs`. Default generated API path is `::auctioneer_openrtb::field_metadata`, overridable with `#[openrtb_metadata(crate_path = crate::field_metadata)]`. `name` overrides the wire-object name. Derived models are `Storage::Typed`; generated fields use `Field::of::<ActualType>`.
- Add compiler consumers under `auctioneer-macros/tests/`, using the real OpenRTB interface. No fixture may shadow the metadata interface, use handwritten `#[path]`/source `include!`, or select a private implementation path.
- Annotate the request graph in `auctioneer-openrtb/src/{types,ext}`; forward `BrandVersion` and mark `DebugSignal` explicitly beside their implementations. Do not annotate unrelated response models or change any field/Serde definition.
- Adapt `auctioneer-config-schema/src/build_disclosure.rs`; add OpenRTB only to optional `build-support`. Add the normal OpenRTB dependency needed by generated target assertions to `auctioneer-config/Cargo.toml`. Keep default/config-model dependency boundaries intact.

## 1. Baseline And Metadata Constants

- [x] Run current `auctioneer-config-schema --lib --features build-support` and relevant OpenRTB tests. Baseline: 230 schema tests and 96 OpenRTB units passed; the schema special-file cases require unsandboxed local socket setup.
- [x] Add failing inline tests for exact raw/map/wrapper storage, constructor refusals and structural agreement. Use fixed expected values, including asymmetric child identities and independent extra/missing records.
- [x] Observe the missing-API failure, then implement the small descriptor/trait vocabulary and const matcher. Cover `RawValue` behind `Box`, JSON values, direct maps, wrapped/non-flattened maps, raw-child versus typed-parent and all current scalar leaves.
- [x] Verify actual const use, not only runtime calls: correct signatures compile, altered raw flags/edges/names do not. Preserve the scanner and all existing policy tests at this stage.

Run: `cargo test -p auctioneer-openrtb --lib --offline --locked -j2`.

## 2. Derive And Compiler Cases

- [x] Write failing expansion tests before implementation. Cover explicit crate paths/names, raw identifiers, symmetric renames, defaults, omission conditions, flattened maps, missing/duplicate/unknown attributes and unsupported models/Serde forms.
- [x] Implement through `syn`/`quote`, with spanned errors. Generate only constant descriptors/trait implementations; no file access, discovery or policy inference. Prove compiler cfg handling without evaluating cfg expressions in the macro.
- [x] Add ordinary nonignored compiler pass/fail conformance tests with a standard runner. Run the real macro/interface dependency graph first; do not assume a dev-edge back to OpenRTB behaves like the production DAG. Pin new runner dependencies through root Cargo and lock changes without upgrading existing packages. Cases live under `auctioneer-macros/tests/conformance/{pass,fail}`; Cargo discovers `conformance/main.rs` and its ordinary `field_metadata` module.
- [x] Add `cargo test --locked -p auctioneer-macros --all-targets` as an explicit native step in `.github/workflows/test.yml`, with `timeout-minutes: 10`. Pin the exact command and non-disableable timeout in `auctioneer-core/tests/ci_workflows.rs`; missing/zero/raised timeouts, disabled steps and swallowed failures must be rejected. A timed-out compiler suite is an infrastructure failure, never a successful negative fixture.
- [x] Pass cases include cross-module/reexport/generic-default aliases, a shadowed `RawValue` name, renamed fields, active/inactive cfg fields and custom forwarding. Fail cases include unknown field types, invalid flatten owners/wrappers, duplicate metadata, invalid input forms, alias cycles and structural target-layout disagreement. Check intended diagnostics. Custom forwarding is checked against actual Serde in the owning OpenRTB unit tests.
- [x] Ensure missing/renamed fixture collections cannot yield a zero-case green run. Unit tests remain inline; compiler cases stay in this crate's `tests/`.

Run: `cargo test -p auctioneer-macros --all-targets --offline --locked -j2`.

## 3. Real Model Graph And Qualification Gate

- [x] Derive metadata for all named request models reachable from `BidRequest`. Set explicit `EID`, `UID` and `BrandVersion` wire identities. Use only the two reviewed custom adapters; do not generate a second Serde implementation. Compare their metadata with actual Serde output: empty/populated `BrandVersion` forwards wire fields without a `fields` key, while both `DebugSignal` variants serialize as scalars and expose no child object.
- [x] Collect the graph with duplicate/cycle refusal in build support. Preserve typed-parent/raw-child distinction and do not invent a `BrandVersion.fields` wire key. Add independent exact ten-raw-slot and missing-model tests.
- [x] Emit target agreement alongside the old scanner's policy output, before changing policy's storage source. Keep this as a const assertion over real OpenRTB metadata on every target, without a disable feature.
- [x] Prove native cfg-disagreement compiler controls and compile real positive agreement assertions for native and all three guest targets. Verify all named fields, flattened fields, child identities and raw/map flags bind independently.
- [x] Review dependency edges and generated artifacts: default schema stays serde-only, serving edges omit native build support, macros remain host tools, no request-side metadata processing is introduced. Do not claim a zero storage charge merely from constants.

Qualification commands:

```sh
cargo test -p auctioneer-openrtb --all-targets --offline --locked -j2
cargo test -p auctioneer-macros --all-targets --offline --locked -j2
cargo check -p auctioneer-config --target wasm32-unknown-unknown --offline --locked -j2
cargo check -p auctioneer-config --target wasm32-wasip1 --offline --locked -j2
cargo check -p auctioneer-config --target wasm32-wasip2 --offline --locked -j2
```

**Stop/go:** Do not start Task 4 unless this works on the pinned toolchain. If the constant graph/agreement cannot be implemented as approved, retain the live scanner and present a specific contract revision. Do not weaken the policy or target check to force a pass.

## 4. Scanner Cutover And Guard Migration

- [x] Make production policy storage consume only compiled metadata. Named raw fields map to the existing `RawValue` disposition; named non-raw fields remain typed; only a legal flattened JSON map supplies unnamed JSON storage.
- [x] Keep the old scanner and its original controls available only as a temporary test-side comparison during qualification; there is no production fallback. Do not delete them until all Task 5 acceptance checks pass. The final removal is followed by another affected-test/guest check pass.
- [x] Keep all membership, provenance, ownership, domain-closure and policy logic unchanged. Compare all generated disclosure descriptors with the existing hand-transcribed oracle, not output captured from the new derive.
- [x] Replace scanner-source unit fixtures with compiled models or real compiler controls using this migration map:

| Existing control family | Required successor |
| --- | --- |
| Expanded raw owners / opaque acceptance | Same policy matrix over compiled real metadata |
| Scalar-shaped raw slots | `context`, `id` and `control` still refuse for the raw-storage diagnostic despite exact wire membership |
| Missing storage / missing source root / empty tree | Missing root, model or field descriptors refuse policy admission |
| Root/nested relocation | Compiled model reachable across module/reexport moves; same raw policy refusal |
| Exact real raw slots | Same independent ten-owner `ext` set |
| JSON parents / typed children | JSON/map parents accept traversal with known child models and refuse when the child's model is missing |
| Catch-all owner/type/flatten alternatives | Real compiler and policy cases; direct map only, not wrapped or merely named |
| Model-driven raw-to-typed edits / renamed fields | Distinct compiled raw and typed model controls, correct renamed slot |
| Raw aliases and generic defaults | Compiler-resolved aliases remain raw; opaque accepts while traversal refuses |
| Alias cycles / malformed syntax | Diagnostic-specific compiler refusals, not a removed parser test |
| Projections and previously unsupported alias syntax | Valid aliases/projections are classified by their resolved type; invalid ones fail compilation |
| Duplicate wire slots | Specific derive/const-constructor refusal |

- [x] Record the actual old/new test mapping; preserve all test intent rather than deleting a disappearing helper's tests. Retain unrelated catalogue/policy tests and the original exact E0027 exhaustive-scrub mutation oracle.
- [x] After all Task 5 gates pass, remove the temporary scanner, source alias resolution and source-spelling map classification and their mapped old controls. Then repeat the affected suites and guest checks. No alternate permissive scanner or parser fallback remains.

## 5. Verification And Publication

- [x] Run macro/OpenRTB/schema/config suites, including the configuration child-build harness and explicit macro fixture execution.
- [x] Run secure and `insecure-bidders` Core privacy/response tests, exchange composition/differential/allocation suites and ignored exhaustive-model controls. Any dependency guard must not replace the intended scrub-adapter refusal.
- [x] Run all three adapter guest checks and relevant CLI resource/boot parity tests. Keep numerical limits and expected independent accounting equations unchanged.
- [x] Run strict workspace all-target Clippy, formatting, raw-JSON gates and all workspace isolation controls. Do not label an unrerun full workspace green.
- [x] Perform independent spec-compliance and code-quality review, fix findings, update crate docs/`ISSUES.md`/this checklist with actual evidence, then commit and push verified checkpoints. Do not change historical evidence or claim bidder catalog/resource qualification readiness.

## Progress

### Compiler Qualification Checkpoint (2026-10-27)

- Missing-API and expansion reds preceded implementation. Subsequent review
  controls reproduced the ancestor wire-name overwrite and source-label
  uniqueness refusal before their fixes. The temporary source parser also has
  a red/green slice/array recursion control; no files or owners were exempted.
- The native affected-package run passed 387 tests: schema 234 units + 4 root
  controls, macros 23 units + 3 integration tests, OpenRTB 109 units + 14
  integration tests. The compiler runner additionally checked all 3 pass and
  13 diagnostic-specific failure fixtures without `TRYBUILD=overwrite`.
- `cargo check -p auctioneer-config` passed natively and for
  `wasm32-unknown-unknown`, `wasm32-wasip1` and `wasm32-wasip2`, each with
  the real generated agreement assertion active. Strict all-target Clippy passed
  for macros/OpenRTB/schema with build support enabled. All 108 workflow guards
  passed, including the explicit locked macro command and ten-minute timeout.
- The native-only runner adds four previously absent lockfile packages
  (`trybuild`, `glob`, `target-tuple`, `termcolor`); existing package versions,
  SDK/toolchain pins, request/Serde definitions, policy data, budget constants
  and historical captures are unchanged. Default schema's normal dependency
  tree still contains only serde and its compiler tooling.
- Source labels are explicitly not type-identity proofs: native collection uses
  compiler `TypeId`, while target agreement compares labels and wire structure.
  Identical-label/identical-structure substitutions are not distinguished by
  the cross-target assertion. Independent raw-slot and privacy oracles remain
  required.
- A later review's lazy-constructor claim was not reproduced: after removing
  fixture-side `MODEL` references, duplicate fields, duplicate catch-alls and
  both invalid map flattenings still fail with their intended `E0080` diagnostics.
  The stronger unforced fixtures pass under the normal snapshot runner; no
  redundant generated assertion was added. All 16 workspace isolation guards
  and both raw-JSON gates pass (188 budgeted fixtures across 54 files).

At this checkpoint the scanner remained the production policy storage source;
the later cutover below supersedes that state. The checkpoint does not claim
complete bidder catalog delivery or deployment readiness.

### Compiled Policy Cutover (2026-10-27)

One compiled `REQUEST` graph now drives both policy admission and target
agreement. Source discovery, aliases and source-spelling classification compile
only under `cfg(test)` as temporary comparison controls; there is no production
scanner fallback. Native-only dev dependencies support real compiled fixtures.

The initial cutover passed all 238 schema units and all 324 config tests,
including the 104 bounded child-build cases. Secure and development-feature
privacy-outbound/response suites each passed 13 tests. The selected exchange
acceptance run passed 432 tests, with the exhaustive-field child test separately
scheduled rather than counted as passing while ignored. Its separate run then
passed the pristine/restored checks and all three exact `E0027` mutations. No policy/source
inventory data, expected descriptor oracle, workload limits, resource
coefficients or SDK/toolchain pins changed.

At this intermediate checkpoint, full mutation/resource/guest acceptance,
final guard mapping and scanner removal were still in progress. The final
execution record below supersedes that intermediate state.

### Guard Successors And Cutover Checks

The old source controls were retained through qualification, then removed.
Every retired test below has a compiled successor; these use real contracts,
not altered source strings or a copied metadata interface:

| Source control | Compiled successor |
| --- | --- |
| `model_tree_missing_storage_rejects_admitted_fields` | `compiled_missing_root_model_field_and_named_map_fail_closed` |
| `model_tree_discovers_root_and_recursively_nested_raw_slots` | `relocated_models.rs` with ordinary nested source modules |
| `model_tree_real_source_has_exact_committed_raw_ext_slots` | Independent ten-owner set in `compiled_request_graph_has_the_independent_raw_extension_inventory` |
| `model_tree_missing_source_root_and_empty_tree_fail_closed` | Missing-root/model controls in `compiled_missing_root_model_field_and_named_map_fail_closed` |
| `model_tree_json_members_accept_traversal_to_known_child_models` | `compiled_json_parent_requires_the_real_child_model`, with present-child controls |
| `only_a_serde_json_map_catch_all_proves_storage_for_unnamed_wire_members` | Named-map policy refusal plus unforced typed/raw/map/wrapper flatten refusals in `conformance/fail/` |
| `model_tree_json_parent_cannot_admit_a_missing_child_model` | Missing-child controls in `compiled_json_parent_requires_the_real_child_model` |
| `legacy_scanner_preserves_raw_storage_through_slices_and_arrays` | Six diagnostic-specific refusals in `unsupported_sequences.rs`; these forms are outside the first owned-field contract |
| `raw_slot_capability_follows_model_source_not_owner_or_field_lists` | Distinct raw/typed/renamed models in `compiled_raw_and_typed_models_keep_policy_dispositions_separate` |
| `synthetic_raw_owner_rejects_traversal_but_typed_child_accepts_it` | Raw/typed policy controls in `compiled_raw_and_typed_models_keep_policy_dispositions_separate` |
| `scalar_raw_slot_rejects_dispositions_even_with_exact_wire_inventory` | `compiled_raw_storage_refuses_scalar_dispositions_after_inventory_admission` |
| `raw_storage_aliases_reject_traversal_with_typed_and_opaque_controls` | Compiled raw/typed policy controls plus `raw_aliases_and_projection.rs` |
| `cyclic_local_storage_aliases_are_rejected` | Diagnostic-specific `alias_cycle.rs` compiler refusal |
| `unsupported_local_alias_syntax_fails_closed` | `undefined_type_macro.rs` refuses; valid generic-default aliases/projections compile in `raw_aliases_and_projection.rs` |
| `model_scanner_rejects_malformed_source_and_duplicate_wire_slots` | `malformed_source.rs` and unforced `duplicate_field.rs` constructor refusal; malformed expansion uses balanced source tokens without a source-import exemption |

The expanded normal compiler run passed all 5 positive and 21 negative fixtures.
Strict workspace all-target Clippy passed after the agreement renderer adopted
scoped imports instead of generated fully qualified expressions; no lint
exemption was added. All three real adapters passed their guest checks.

An optimized Cloudflare library built with the unchanged `wasm-release` profile.
The artifact contains the live auction/status route markers (positive control),
but no field-metadata labels, agreement-failure text or constructor diagnostics.
Its normal dependency graph selects schema `config-model`, not `build-support`,
includes the macro as a host procedural macro and excludes the native compiler
runner. This is artifact/dependency inspection, not a claim of measured zero
total memory or absolute platform fit. The 16 source-isolation controls and raw
JSON gate (188 fixtures, 54 files) pass with the expanded fixtures present.

CLI/boot parity and final scanner removal were pending at this checkpoint.

### Scanner Removal And Final Acceptance (2026-10-27)

The temporary scanner, source alias resolution, type-spelling classifier and
15 mapped parser tests are removed. Production and test policy admission now
use the compiled request graph only. The independent ten-owner raw-slot set,
86-field/11-schema policy oracle and exact `E0027` scrub-model mutation harness
remain unchanged. Separate spec/quality reviews of the cutover and removal
found no remaining issue; reviewers inspected source, while the controller
executed the commands below.

After removal, locked/offline verification on Rust 1.98.1 passed:

| Check | Result |
| --- | --- |
| Macro/OpenRTB/schema all-target suites with schema build support | 376 tests passed, including 5 positive and 21 diagnostic-specific negative compiler fixtures inside the conformance runner |
| Config all-target suite | 324 tests passed, including all 104 child-build harness cases |
| CLI resource commands and real boot parity | 16 + 7 tests passed; source remained frozen during execution |
| CI execution guards / workspace source isolation | 108 / 16 tests passed |
| Real Cloudflare/Fastly/Spin guest checks | Passed on their respective three guest triples, with generated const agreement active |
| Strict workspace all-target Clippy | Passed with `-D warnings` and no new lint allowance |
| Workspace formatting / diff whitespace / raw-JSON gates | Passed; 188 fixtures across 54 files and the gate's rejection controls |

The compiler fixture count is nested in the three integration-test results,
not 26 additional Cargo test functions. Before removal, the secure and
`insecure-bidders` Core privacy/response runs each passed 13 tests; selected
exchange composition/differential/allocation acceptance passed 432 tests and
the separately executed exhaustive-model child control passed all intended
mutations. CLI/boot parity and all three guest checks also passed before
removal. Removing test-only parser helpers does not change serving behavior.

Reproduce the post-removal commands with `CARGO_INCREMENTAL=0` and serialized
Cargo jobs; CLI tests use the existing `EDGEZERO_SDK_ROOT`,
`AUCTIONEER__BIDDER__SOURCE_DIR` and `AUCTIONEER_APS_SOURCE` bindings:

```sh
cargo test -p auctioneer-macros -p auctioneer-openrtb -p auctioneer-config-schema --all-targets --features auctioneer-config-schema/build-support --offline --locked -j2
cargo test -p auctioneer-config --all-targets --offline --locked -j2
cargo test -p auctioneer-cli --test bidders_resources --test resource_boot_parity --offline --locked -j2 -- --test-threads=2
cargo test -p auctioneer-core --test ci_workflows --offline --locked -j2
cargo test -p auctioneer-bidder --test test_isolation --offline --locked -j2
cargo check -p auctioneer-adapter-cloudflare --target wasm32-unknown-unknown --offline --locked -j2
cargo check -p auctioneer-adapter-fastly --target wasm32-wasip1 --offline --locked -j2
cargo check -p auctioneer-adapter-spin --target wasm32-wasip2 --offline --locked -j2
cargo clippy --workspace --all-targets --offline --locked -j2 -- -D warnings
cargo fmt --all -- --check
./scripts/check-raw-json-fixtures.sh
./scripts/check-raw-json-fixtures-fixtures.sh
git diff --check
```

Existing GVL-deletion notices and the `proc-macro-error2` dependency's
future-incompatibility notice remain visible. This is the completed metadata
refactor, not an unexecuted full-workspace test claim, full bidder-schema
qualification, bidder catalog completion or resource qualification/hosted readiness. No request/Serde field,
privacy policy, budget coefficient, workload limit, dependency pin or
historical capture changed during the cutover/removal. The next bidder catalog gates are
recorded in the [current execution priorities](2026-10-16-bidder-catalog.md#current-execution-priorities-2026-10-27).

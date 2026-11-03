# GDPR Scrub Classification Implementation Plan

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Preserve reviewed context while closing denied ID/geo channels and
retaining the full composed privacy boundary.
**Architecture:** Decode/validate scoped projections once, then mutate typed
slots and flattened maps through one exchange-owned per-object rule engine.
Derive domains from rules/children. Core owns plain audit counters and consumes
a bounded summary before releasing prepaid resources.
**Tech Stack:** Existing Rust/serde/TOML codegen, host-only Markdown import,
bounded JSON/resource utilities, colocated unit tests and existing native/WASM CI.
**Design:** [GDPR Scrub Classification Design](../specs/2026-10-09-gdpr-scrub-classification-design.md).
**Status (2026-10-11):** reviewed inventories, compiled rules, the production
per-object mutator, construction-cost plumbing, plain core audit consumer and
live `run_auction` integration are implemented in the privacy enforcement worktree. The temporary
prototype has been removed; generated rules are live. Targeted native regression
results are recorded below, separately from the chronological Tasks 1-3 baseline.
The user supplied AG as the human reviewer; inventory metadata records
2026-10-11. This provenance record is not final code sign-off. The earlier
independent spec/plan document reviews remain design evidence, not final code
sign-off. Fresh full-workspace native tests, both strict lint feature selections,
all three strict WASM checks, executed WASI depth/cleanup proofs, vendor packages,
compiler mutations and real charged-handler failure tests now pass. The final
alternate-feature workspace execution also passed. The atomic implementation
commit `c2b00a2` is pushed and remote-verified. Verified source integrations are
published as user sync `9ff017f` and diagnostic qualification `6177742`. Current evidence and limits are recorded in the
[execution ledger](../sdd/2026-10-11-gdpr-scrub-classification.md); earlier sections
retain chronological results rather than qualifying the amended costs.
**Execution base:** `.claude/worktrees/privacy-enforcement`, aligned with merged bounded fanout at
`5cefb05` (merge of `origin/main` at `1ff1907`). The initial document review used
`78f57b7`; it is not the execution head.
Recheck actual head/dirty state before starting. user sync/diagnostic qualification inherit verified changes;
bounded fanout receives documentation only, not a duplicate production implementation.

---

## Decisions And Execution Rules

This plan replaces the prior bounded fanout-first/full-request-Value design. Do not implement
borrowed validation plus a second consuming decode, a separate Value privacy
engine, an Arc/Mutex audit extension, or compatibility adapters with no production
caller. Existing privacy enforcement typed scrub code is the production boundary to replace.

Retain validation-first deadline precedence: expired allowed survivors still
validate FPD and can return code 15. Do not add a pre-materialization timeout
exit. Denied/sampled/capped candidates remain unmaterialized.

Intentional differences require reviewed goldens and updates to old assertions:
untyped Geo context and kwarray preservation, unknown removal under either domain,
accuracy omission, and invalid/non-finite/overflow coordinate removal. Successful
finite rounding arithmetic is unchanged. Device dnt/lmt remain control values.

Each task: add named failing tests, run them and verify nonzero execution with
the intended failure, implement the smallest change, rerun, review the diff, then
commit/push. Future module filters are not proof until their tests exist and
execute. Integration/compile-failure gates may not be silently ignored.
Keep committed execution ledgers under `docs/superpowers/sdd/`, alongside
the specs and plans, rather than in a root-level agent bookkeeping directory.

Use `serde_json::json!` for synthetic fixtures. Unit tests live with their code;
separate binaries cover real wire paths, allocation, workflow and cross-crate
behavior. Names describe behavior, not capabilities. No unrelated refactors,
raised resource factors/ceilings, archive refresh, or dependency pin changes.
If tooling/GVL freshness blocks a gate, record and stop that gate; do not bypass it.

The Task 8 live replacement is installed; the frozen old mutator remains only
as an offline test comparator. Local resource, target and review gates passed;
provider/deployment qualification remains a distinct requirement. A failed
feasibility proof requires a revised design; neither this plan nor old passing
tests authorize permissive fallback.

## File And Ownership Map

All implementation paths are relative to privacy enforcement. New files are marked `new`.

| Surface                     | Files                                                                                                                                                                                                                                                                           | Responsibility                                                                                                                   |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| Rule import                 | `crates/auctioneer-openrtb-catalogue/{Cargo.toml,src/lib.rs,src/main.rs}` (new); workspace/lockfile                                                                                                                                                                                 | Independent host-only deterministic membership generation and build library                                                      |
| Provenance/policy           | config `data/openrtb-2.6-source.{md,toml}`, `openrtb-2.6-catalogue.toml`, `supported-extension-catalogue.toml`, `request-disclosure-policy.toml`, `disclosure-sources/` (new)                                                                                                   | Explicit metadata, source artifacts, one rule source                                                                             |
| Compiler                    | config `build_disclosure.rs`, `src/disclosure.rs` (new), `build.rs`, manifests, `tests/disclosure_codegen.rs` (new), `tests/build_failure_harness.rs`                                                                                                                           | Membership/shape/child validation and derived masks                                                                              |
| Mutation                    | exchange `src/scrub/{mod.rs,classified.rs,classified_tests.rs,decision.rs,fields.rs,value.rs,audit.rs,limits.rs,boundary_proof.rs,composition_tests.rs}` (classified/action/adapter/audit/limits children new)                                                                  | Live typed/map mutator, shared actions, transforms, fixed counters and unit goldens                                              |
| Source/projection           | runtime `src/json_preflight.rs` (new), `src/json_size.rs`; exchange `src/context.rs`, `src/run/mod.rs`, `src/split/{mod.rs,resource.rs}`; openrtb `src/prepare/{mod.rs,implicit_fill.rs}`                                                                                       | Borrowed source admission, normalized/header allowance, single scoped validation, supplement bounds and required audit parameter |
| Cost ownership              | runtime `src/{budget.rs,auction_budget.rs,lib.rs}`; core `src/host_primitives/{bootstrap.rs,gates.rs}`, `src/handlers/probe_config.rs`; all four adapter boot/envelope callers and tests                                                                                        | Neutral cost descriptor, actual owning layouts, consistent prepaid derivation                                                    |
| Audit consumer              | core `src/handlers/{auction.rs,mod.rs,request_disclosure_audit.rs}` (audit module new)                                                                                                                                                                                          | Plain request scope, real bounded log observer, release ordering                                                                 |
| Semantic/deadline tests     | exchange `src/scrub/classified_tests.rs`, `tests/{privacy_policy_composition.rs,tmax.rs,run_auction.rs,expired_tmax_diagnostics.rs,outbound_construction.rs,construction_deadline.rs}` (deadline binary new), `tests/support/auction.rs`; core `tests/response_gdpr_applies.rs` | Independent 256 typed/shadow oracle, actual outbound bytes, original-deadline adapter counters and four resolved GDPR wire rows  |
| Construction/model proofs   | exchange `tests/{source_preflight.rs,disclosure_model_coverage.rs,disclosure_policy_diff.rs}` (new), `tests/{projection_scrub_bound.rs,construction_bound.rs,sparse_request_copies.rs}`, `tests/support/{disclosure_model_coverage.rs,frozen_scrub.rs}` (new)                   | Borrowed depth/resource bounds, actual model mutation compiler harness, frozen comparator and retained allocation guards         |
| Core audit/lifecycle proofs | core `src/handlers/request_disclosure_audit.rs` colocated tests; `tests/{admission_holds_through_auction.rs,bounded_path_in_the_real_handler.rs,boot_resource_charges.rs}`; all four adapters `src/envelope.rs` and `tests/boot_envelope.rs`                                    | Bounded production logger records, charged real-handler success/error/cancellation lifetimes, actual envelope cost descriptors   |
| Gates/docs                  | existing workflows, core `tests/ci_workflows.rs`, raw fixture scripts; master spec Appendix C.2                                                                                                                                                                                 | Active checks and reviewed live-contract cutover                                                                                 |

The map names `handlers/mod.rs`, not the nonexistent `handlers.rs`. Resolve
actual bootstrap/probe/adapter callers with `rg` before editing; do not transplant
bounded fanout's flatter module layout or sequential resource proof into privacy enforcement.

## Task 1: Prove The Smaller Production Boundary

**Files:** exchange `src/scrub/mod.rs`, a private test prototype, new
`tests/projection_scrub_bound.rs`, existing construction/sparse/node guards.

- [x] Add colocated tests around the actual privacy enforcement consume/decode/validate boundary
      followed by per-object in-place mutation. Cover permitted untyped city/metro,
      malformed context objects, supported interstitial descendants, sparse SUA/
      brands, raw extensions, nulls, numeric forms and typed/shadow conflicts.
- [x] Add a validation-precedence fixture: an expired allowed survivor with
      `{"banner":null}` FPD still returns code 15 and dispatches no calls. Also
      test denied malformed FPD never materializes. Use existing privacy enforcement harness helpers.
- [x] Run the newly added tests with `cargo test --locked -p auctioneer-exchange
--lib scrub` and the relevant `--test privacy_policy_composition`/`--test tmax` binaries.
      Confirm the new named tests execute. Expected red is missing prototype
      behavior/assertion, not an unavailable target or stale archive.
- [x] Implement only a test-local representative mutator to evaluate typed-slot
      access and shape-aware map retention. Do not add a production second path or
      overwrite old scrub behavior. Replace this prototype with the compiled-rule
      implementation in Task 4; its temporary rules are not a second policy source.
- [x] Measure source/grouped state, retained survivor typed vector, current
      projection/decode, in-place mutation, and capped adapter body in their actual
      lifetimes. Use 1/2/max survivors, at least three input sizes, independent
      byte/node caps and 64/512/513 sparse-brand boundaries. Include invalid-input
      errors and disjoint raw subtrees.
- [ ] Run `cargo test --locked -p auctioneer-exchange --test projection_scrub_bound
--test construction_bound --test sparse_request_copies --test node_charge`.
      Compare installed budget rows, not independent copied literals; test a
      cross-size delta that detects an extra whole-request clone.
- [ ] Record compile versus executed target evidence. Before Task 2, review
      single-decode semantics and funded retention. If they fail, revise the design,
      potentially toward sequential scoped construction, without changing privacy
      rules or raising caps. Do not resurrect the old double-decode design silently.
- [x] Commit/push `test(exchange): prove per-object disclosure mutation boundary`
      (`ff595d6`, verified remote).

### Native Feasibility Evidence (2026-10-11)

The six original boundary fixtures ran red with a no-op prototype: five failed
at the intended disclosure assertions and the unrestricted control passed.
The representative mutator then passed those six; independent review added a
seventh restricted null/empty-brand fixture and strengthened `get("h")`
presence assertions. The two expiry fixtures pin the real engine's existing
validation precedence, rather than pretending a test-only mutator implements it.

The allocation binary uses the public budget equation with synthetic rows and
actual baked configuration sizing. It measures retained scopes, consuming
projection/decode, in-place mutation and capped bodies. The in-place mutation's
peak growth is zero on these fixtures; a real extra request clone is held live
and rejected by the same cross-size predicate. This does not prove zero total
allocation activity, arbitrary input safety, finalization, or target execution.

Installed-row comparison, construction-cost descriptors and WASM qualification
remain pending in Tasks 5/10; no independent copied total is proof of bootstrap
equivalence. The initial native sweep covered 1/2/8 survivors; it was expanded
to the current 50-bidder envelope ceiling before committing. Byte and node caps
are independent, and sparse-brand populations cover 64/512/513 entries.

Retained Docker logs record the native red/green runs. Independent read-only
review accepted representative single-decode mutation and retained lifetimes;
it did not certify the unfinished complete policy or any provider target.
The reviewed native boundary/construction sweep passed 249 tests with zero
ignored tests. The subsequent 50-survivor allocation sweep passed all five
allocation tests; strict Clippy passed for the library and both edited test
binaries. Formatting and `git diff --check` passed. These checks do not claim
workspace-wide or hosted CI completion.

### Review Findings To Reproduce In Tasks 5/8

- Projection charging measures the normalized tree, including copied headers,
  while boot currently funds that receipt only from inbound body bytes. Reproduce
  small-body-cap plus large admitted UA at the real handler; fund normalized
  additions in their owning reservation with checked arithmetic, not new factors.
  Read-only reproduction analysis identified `boot_resource_charges.rs`'s valid
  1024-byte body cap, two funded bidders and 4096 copied-header bytes. A single
  survivor with 2048 ASCII UA bytes exhausts its 262144-byte projection receipt
  before JSON structure and parameter scratch. Reproduce header-free success and
  header-present refusal through the real ingress harness; this is not yet an
  executed handler result. Include domain enrichment and JSON escaping in the fix.
- The live engine checks expiry after scrubbing and does not recheck between
  adapter constructions or before dispatch. Preserve validation-first code 15
  while adding the specified post-validation/construction checkpoints.
- Borrowed grouping serializes before source depth admission. The new preflight
  must run even for unrestricted disclosure and count embedded raw JSON before
  recursive clone/serialization. Depth 128 remains a candidate, not a measured
  target guarantee.

## Task 2: Import Independent Membership With Explicit Review Metadata

**Files:** host-only importer, workspace/lockfile, config source manifest and
standard/extension inventories. No live scrub changes.

- [x] Add importer tests for scoped headings, inline formatting/links, GFM tables,
      multiline EID rows, duplicate/missing/empty sections and unsupported types.
      Select and lock an existing structured Markdown parser; no regex discovery.
- [x] Create the tool manifest and run `cargo test --locked -p
auctioneer-openrtb-catalogue --lib`; confirm extraction assertions fail before
      implementing extraction, then pass.
- [x] Vendor the exact IAB source from the design's pinned commit with attribution/
      license. Record URL/release/commit/digest and human reviewer/date in the explicit
      `openrtb-2.6-source.toml` manifest. No placeholders or clock-derived review.
- [x] Implement the design's `generate` and `check` CLI modes with `--manifest`,
      `--source` and explicit output/catalogue paths. Generate sorted membership/
      object/type/section rows only; do not copy privacy classes or duplicate review
      metadata into generated output.
- [x] Run:
  ```sh
  cargo run --locked -p auctioneer-openrtb-catalogue -- check --manifest crates/auctioneer-config/data/openrtb-2.6-source.toml --source crates/auctioneer-config/data/openrtb-2.6-source.md --catalogue crates/auctioneer-config/data/openrtb-2.6-catalogue.toml
  ```
  Generate twice to temporary files and compare. Vary the test clock/date and
  environment; output/check behavior remains deterministic and leaves review
  metadata unchanged.
- [x] Keep the importer independent of config/GVL; config may use it as build/dev
      dependency, never vice versa. Verify it has no deployed runtime dependency.
- [x] Independently source user-extension EIDs/data, device atts/interstitial and
      existing FPD/TID/control contracts. Store immutable digest-pinned artifacts
      under `data/disclosure-sources/`; local typed fields alone are not provenance.
      User extension EIDs are not standard User membership.
- [x] Review every inventory field, not just the illustrative table. Explicitly
      classify dnt/lmt controls and the ten previously unnamed Device fields. Keep
      deprecated standard hashes, customdata ID gating, kwarray FPD ownership,
      Geo context and sourced extensions distinct.
- [x] Write exactly one disposition per scoped member with shape, child/transform/
      delegate and rationale. Do not author domain masks. Unknown protected fields
      have schema-level ID+geo closure, not an enumerated universal denylist.
- [x] Commit/push `feat(config): source deterministic disclosure inventories`
      (`a1e3977`, verified remote).

### Inventory Evidence (2026-10-11)

The importer library ran red against intentional stubs (12 failures, five
passing validation controls); its CLI ran red with three failing behavior
assertions and three passing parser controls. The completed tool passes 33
tests: 18 library, 12 CLI-unit and three executable tests, with no ignored tests.
Strict Clippy passes on all tool targets. CLI check reproduces the committed
79-field standard catalogue from the vendored source; the source digest is
`1b730796e2776d576cb5749706f6638fd4aa35594c1182e7d0e6aedbce605728`.

Actual supplied dates exercise 179/180/181-day age boundaries and leave output
and review metadata unchanged. Process tests independently assert nonzero
refusals and no rewrite on source/catalogue corruption or output/input aliases.
Unix link protection and atomic replacement cleanup are tested; existing-output
replacement on other hosts is conservatively refused, not claimed qualified.

AG supplied the reviewer name on 2026-10-11; explicit metadata records that date.
Extension membership and policy are separate reviewed inputs, not generated
privacy classes or policy attributes on wire-model structs. Exact extension
artifacts, license, digests and an immutable Auctioneer ownership excerpt accompany
the seven extension rows. Compiler enforcement of these inputs remains Task 3;
no live transmission permission changes in this inventory round.

Independent read-only review verified all 86 dispositions, the 11 child schemas,
all recorded artifact digests and the immutable ownership excerpt's line range.
No missing/duplicate classifications, unresolved/cyclic children, shape mismatch
or opaque-context loophole was found. The review did not certify the unfinished
compiler, resource proofs or live enforcement.

The broader current privacy enforcement native model/privacy/exchange sweep passed 830 tests
across 41 suites, with zero ignored tests. This is not hosted CI, WASM execution,
full workspace qualification, or proof of the forthcoming live classified path.

## Task 3: Compile Rules And Derive Domains

**Files:** config compiler/descriptors/build entrypoint and mutation harness.

- [x] Add compiler tests for missing/extra/duplicate dispositions, invented
      standard ifv, wrong digest/release/commit, unknown TOML keys/enums, shape/
      transform mismatch, missing children/delegates, cycles and opaque context
      containers. Genuine untyped standard didsha1 passes.
- [x] Run `cargo test --locked -p auctioneer-config --test disclosure_codegen`;
      implement strict raw TOML types and validation. Missing classes fail builds;
      unknown request fields do not become malformed catalogue errors.
- [x] Emit SchemaId/FieldId, Rule/Shape/Transform/Delegate and sorted static tables
      through real `build.rs` into OUT_DIR. Share enum declarations with build
      support without a runtime importer dependency.
- [x] Derive direct domains from rule/transform/delegate and propagate child
      domains plus protected unknown ID+geo closure. Supplying a TOML `domains`
      field fails strict parsing rather than overriding generated behavior.
- [x] Emit context shapes even for rules with no direct domains. Every child
      schema retains protected unknown closure; do not prune context-only children.
      Task 4 owns execution of these shape checks and unknown-field removal.
- [x] Validate independent source reproduction, actual artifact digests, one rule
      per member, reproducible output and 180-day age warning. Age warns only.
- [x] Add the feature-gated disclosure-data directory override to existing isolated
      build-failure tests; production ignores it. Bound concurrent child builds,
      use isolated targets and clean every exit. Planted invalid builds must fail
      for policy validation, not unrelated dependency/GVL errors.
- [ ] Run `cargo test --locked -p auctioneer-config --test disclosure_codegen
--test build_failure_harness --test smoke`.
- [x] Commit/push compiled disclosure rules and requested host/document layout
      (`803f2f7`, verified remote).

### Compiler And Layout Evidence (2026-10-11)

The compiler emits 11 sorted schemas and 86 field descriptors from strict input
documents. Standard membership is reproduced from the independently pinned IAB
artifact; extension membership is bound to exact reviewed declarations, source
identities, artifact/license bytes and ownership excerpt ranges. Those pins are
membership/provenance checks, not a second privacy disposition table.

Six additional provenance regressions first failed because matching metadata
edits could relabel or rewrite extension evidence. All six pass with independent
binding checks. The final compiler integration binary passes 34 tests, including
deterministic output, strict malformed-input rejection and age-warning controls.
The four real child-build gates pass: reviewed data succeeds, missing disposition
and source drift fail for the intended reason, and production ignores the
fixture override. The remaining 57 build-harness cases have not run in this
round; the full harness checkbox above remains open.

The final config library/codegen/disclosure/smoke command passes 117 test
executions with no failures or ignored tests. This includes a shared vocabulary
unit test both directly and through the integration harness, not 117 distinct
new tests. Strict Clippy passes for config and both host packages on all targets.
The renamed catalogue and refresh packages pass 33 and 23 tests respectively;
shared config/GVL schema integration binaries pass four and 66 tests. The earlier
830-test native model/privacy/exchange sweep remains baseline evidence, not
qualification of an uninstalled mutator or any WASM/provider target.

Both host packages now live under `crates/` with `auctioneer-` names. Shared schema
and catalogue dependency paths are defined once in root workspace dependencies;
consumers inherit them. The binary-only refresh package is a workspace member,
not an unused dependency entry. Commands, source-relative paths and the refresh
workflow use the new names. No archive refresh or dependency pin update ran.
The committed SDD ledger moved to `docs/superpowers/sdd/`; root main and downstream
capability worktrees remain untouched. Independent read-only review verified the
provenance repair, but did not certify the pending live scrub or resource proofs.

## Task 4: Implement One Per-Object Mutator And Plain Audit

**Files:** exchange scrub children, parent module and tests. The entrypoint is
now live through Task 8. Checked implementation/test-authorship items below do
not substitute for pending full-suite execution or final qualification.

### Current Native Checkpoint (2026-10-11)

The parent reports the following targeted regression executions passed before
the retained IPv6/numeric charge amendment; these are chronological evidence,
not qualification of that amendment. The deadline row includes its later
five-test host execution.
Counts are suite executions, not newly added test counts. This documentation
update did not run Cargo and does not infer any other gate's result.

| Reported suite                       | Actual surface                                                         | Passed |
| ------------------------------------ | ---------------------------------------------------------------------- | ------ |
| Axum native regression               | `auctioneer-adapter-axum` adapter/envelope tests                           | 15     |
| Core charged lifecycle               | `auctioneer-core/tests/admission_holds_through_auction.rs`                 | 14     |
| Core GDPR wire                       | `auctioneer-core/tests/response_gdpr_applies.rs`                           | 7      |
| Exchange construction                | `auctioneer-exchange/tests/construction_bound.rs`                          | 11     |
| Exchange late deadline               | `auctioneer-exchange/tests/construction_deadline.rs`                       | 5      |
| Core audit observer                  | `auctioneer-core/src/handlers/request_disclosure_audit.rs` colocated tests | 17     |
| Offline disclosure differences       | `auctioneer-exchange/tests/disclosure_policy_diff.rs`                      | 16     |
| Borrowed source preflight            | `auctioneer-exchange/tests/source_preflight.rs`                            | 2      |
| Retained projection/scrub allocation | `auctioneer-exchange/tests/projection_scrub_bound.rs`                      | 7      |
| Sparse request copies                | `auctioneer-exchange/tests/sparse_request_copies.rs`                       | 8      |

At the previous checkpoint the explicitly ignored host-driven
`disclosure_model_coverage::tests::new_model_fields_require_exhaustive_scrub_adapter_coverage`
test actually executed and passed (1). That is compiler-harness execution, not
Clippy compilation or a default run that skipped the heavy test. It checks a
pristine scratch build and User/Device/BrandVersionFields mutations serially;
it is distinct from the config `build_failure_harness` baseline in Task 3.

Tasks 1-3 retain their original chronological evidence and checkbox state; their
then-pending statements are baseline snapshots, not today's live-path status.
The independent 256 composed typed/shadow wire oracle is in
`src/scrub/classified_tests.rs`; the 16-test offline comparator has separately
authored literals/goldens, not expectations derived from the new rule engine.
This targeted checkpoint did not establish broad qualification or publication.
The subsequent full native/strict-lint, target/package checks and published
atomic cutover are recorded in Task 10 and the execution ledger; user sync/diagnostic qualification combined
integration remains separate.

- [x] Add decision/action tables for all rules/shapes, four ID/geo combinations,
      RemoveAll, FPD-only, TID-only and unrestricted controls. Implement immutable
      Disclosure conversion without new signal/GVL/account lookups.
- [x] Add before-state presence assertions for every required standard/context/
      control field and each supported extension. Test both typed and flattened
      forms, EID/UID descendants, unknown object arrays, invalid shapes and all
      existing ID markers. Explicitly remove city/object and version/array-with-object
      under ID-only and geo-only denial, independently of direct rule domains.
      A context-only interstitial child must still visit unknown siblings.
- [x] Implement `fields.rs` typed-slot access and `value.rs` per-schema
      map mutation using the same compiled action function. There is no root-Value
      engine and no whole-request serde round-trip for privacy.
- [ ] Complete the fresh `cargo test --locked -p auctioneer-exchange --lib scrub`
      execution as part of full native qualification; do not infer it from authored
      unit tests or the targeted integration results above.
- [x] Exhaustively destructure User, Device, Geo, UserAgent, EID/UID and public
      backing/extension structs without `..`. Known slots use generated IDs;
      newly typed out-of-inventory fields take explicit unknown handling.
- [x] Keep Option/JsonField/Box sparse/null behavior and empty brands allocation-
      free. Remove unsupported RawValue extensions under restriction without
      disclosure parsing. No new raw-context exception.
- [x] Handle denied scalars/collections in every representation; retain privacy enforcement's
      Geo-only typed precedence, exact TID leaves and FPD-owned branches. TID-only
      never normalizes unrelated user/device or Source extension data.
- [x] Replace finite transform helper with optional-result semantics: unchanged
      successful half-up arithmetic, but remove NaN/infinity/intermediate overflow/
      invalid output in typed and flattened coordinates. Test negative/ties,
      f64::MAX and overflow boundaries. Remove zip/accuracy under coarsening;
      retain reviewed coarse context; COPPA empties existing Geo without fabrication.
- [x] Implement fixed enum-indexed audit counters and attempted/failed scope
      counts, saturating only diagnostics. Unknown subtree/EID removal counts once;
      absent, unchanged, repeated and already-empty COPPA changes count zero.
      Failures retain prior changes without per-bidder maps or copied identity.
      Eight change reasons include a distinct COPPA whole-Geo clear counter.
- [x] Add duplicate-aware serialized-byte tests, the independent 256 composed/
      shadow oracle, idempotence/monotonicity/union and original/sibling isolation.
      Property-test dependencies stay dev-only.
- [x] Add and explicitly execute hermetic exhaustive-adapter mutation coverage
      in `tests/disclosure_model_coverage.rs` using actual model source:
      baseline scratch cargo check passes, new User/Device/backing fields fail for
      missing adapter coverage. Never mutate live worktrees; bound/clean builds.
- [x] Replace prototype allocation mutation with the production classified
      engine and remove `tests/support/disclosure_prototype.rs`. The fresh projection
      (7), construction (11) and sparse-copy (8) suites passed.
- [x] Commit/push the typed/flattened classification in atomic `c2b00a2` (with
      the dependent funding, audit and live-boundary changes), after qualification
      and final independent code review.

## Task 5: Fund And Bound All Construction Lifetimes

**Files:** exchange limits/split/run, runtime budget/reservations/exports, core
bootstrap/gates/probe callers, all four adapters and allocator/envelope tests.

- [x] Add a runtime-neutral RequestConstructionCosts descriptor for fixed
      per-auction, retained per-scope growth and reusable scratch bytes. Exchange reports actual frames,
      transform buffers and audit layout; core composes guard/summary/format buffer
      overhead. Runtime accepts scalar costs, never depends on exchange/core.
- [x] Thread it through `validate_isolate_limits`, BootEnvelope, installation,
      stored derivation and diagnostic re-derivation. Find every caller with
      `rg -n 'validate_isolate_limits|derive_isolate_budget|install_boot_envelope|BootEnvelope' crates`.
      This includes `derive_isolate_budget_with_baked_config`, not just validation
      wrappers. Update Axum, Cloudflare, Fastly and Spin boot/envelope/proof
      consumers and all tests together.
- [x] Place fixed ownership and reusable scratch in disjoint prepaid phase
      charges; prove their sum equals per_auction_peak and they are multiplied by
      concurrency, not charged as isolate-once data or duplicated per survivor.
      Prepay before allocating audit/scratch. No independently pinned layout literal.
      Actual descriptor/equation assertions are authored for all four adapters;
      only the reported Axum/native results are recorded as executed here.
- [x] Finish the bounded IPv6 destination-growth and numeric reserialization
      repair gates. Preserve standard IP formatting; charge the owning 78-byte
      retained-per-scope allowance separately from reusable scratch. Derive numeric
      growth as 23 bytes per admitted input node from the independently tested
      24-byte pinned serializer bound. Verify exact/below projection admission,
      caller-descriptor floors, overflow refusal and the real HTTP compact-exponent
      request. Recheck all four proof rows and retained allocation bounds after
      these changes; earlier two-term results do not qualify the amended descriptor.
- [x] Add read-only borrowed source preflight before the engine's first clone/
      serialization. Bound all typed/flattened structure and embedded RawValue even
      for unrestricted decisions, using counting/streaming resource traversal with
      no temporary JSON tree and no over-depth descent. Pin/test RawValue protocol.
- [x] Define normalized allowance from input plus actual bounded implicit/header
      additions, and projection allowance from it plus scoped supplement growth.
      Extend aggregate projection_charge and supplement preflight with destination
      depth/work bounds before clone/merge. Count JSON keys and values; checked
      overflow refuses rather than grants capacity.
- [x] Document the programmatic ownership precondition for owned `prepare`:
      preflight caller-created inputs while still borrowed/owned by that caller.
      Do not claim engine preflight protects earlier owned normalization. Test
      borrowed engine/mutator rejection without recursive destruction by the callee;
      tests tear down over-deep caller data iteratively.
- [x] Add 127/128/129 source/mutator/projected nesting and deletion/cancellation
      fixtures. The fresh source-preflight binary passed 2 tests. The actual typed
      decoder fixture distinguishes depth-127 admission from 128/129 refusal;
      owned programmatic mutator/source depth admission is a separate boundary.
- [x] Execute borrowed depth, work, materialization, destruction and cancellation
      proofs on a wasm32 WASI guest with a 64 KiB stack, and compile/lint/package all
      three deployment targets. Native retained-lifetime measurements remain distinct
      from provider allocator or physical-isolate qualification. The latter remains
      downstream deployment work, not a claim of this shared-engine checkpoint.
      Frames are depth-sized iterators, not width-sized references; visits stay
      within total allowance, lookup stays bounded and no dynamic paths are stored.
- [x] Limits precede destructive mutations/recursive serialization. Static
      Internal scrub failures dispatch nothing; parser/supplement/code-15 errors
      retain their classes. Merge safe failure counts before returning.
- [x] Rerun native `projection_scrub_bound` (7), `construction_bound` (11) and
      `sparse_request_copies` (8) with the completed production engine and actual
      exchange construction descriptor. These synthetic/native allocation rows
      are not qualification of installed WASM/provider rows or finalization.
- [x] Finish runtime `--lib` budget/reservation and exchange `--lib scrub`
      qualification and all four installed-row comparisons.
      Measure retained vectors plus current projection/typed request/adapter and
      source/grouped state at 1/2/max survivors and independent sizes. Compare all
      four installed rows directly with proof rows.
- [x] Retention fits the measured native construction bounds and unchanged
      factors. The conditional fallback is not needed. If retention cannot fit on a
      subsequently qualified hosting runtime, revise toward one-survivor construction with the
      same validation-first/no-dispatch-on-failure contract and re-review ordering.
      No generic factor, target ceiling, node cap or artifact-budget increase.
- [x] Commit/push the dependent construction-cost repair in atomic `c2b00a2`.

## Task 6: Add The Concrete Core Audit Consumer

**Files:** core `src/handlers/{request_disclosure_audit.rs,mod.rs,auction.rs}`,
exchange audit exports, cost-composition and tests.

- [x] Implement AuctionDisclosureScope with plain counters and completion status.
      Its actual production observer, observe_request_disclosure, derives fixed
      totals and emits one bounded debug log record through the installed log sink.
      No Arc/Mutex, consumed-context handle or per-bidder retained state is added.
- [x] Fix a bounded summary/format buffer size and include actual layout in the
      core cost descriptor. Static labels, u64 totals and status only; no payload,
      dynamic key/path, bidder name, credentials or secret fragments.
- [x] Unit-test production formatting/observation through the existing logger
      capture helper: success/error/cancel statuses, exact aggregate counts,
      saturation, zero-change refusal, single observation and maximum-width numbers.
      A cfg(test)-only reader does not satisfy production consumption.
- [x] Scope is created after prepayment and before config/body awaits. Explicit
      finish consumes audit before drop(lease)/drop(reservations); Drop consumes
      an unfinished scope as cancelled. Arrange declaration/nesting for reverse
      drop order on every early return/cancel; observer never outlives its charge.
      Lease acquisition now precedes the audit scope and config resolution, while
      preserving its full lifetime and egress-resource borrow. Reverse drop order is
      audit, lease, reservations even when config resolution is suspended or fails.
      Explicitly finalize ordinary error returns as errors, not cancelled scopes.
- [x] Test normal and suspended-future cancellation with an event-recording
      reservation wrapper: observer event precedes reservation release. Test
      disabled debug logging too; scope still completes without retention or panic.
      Prepayment refusal creates no scope; post-prepayment refusals observe zero.
- [x] Thread mutable audit through `auction_inner` and the required `run_auction`
      argument; all production callers use the live plain aggregate. No old-signature
      production wrapper is retained.
- [x] Execute the fresh colocated production audit tests (17) and real-handler
      charged lifecycle integration `admission_holds_through_auction` (14). The latter
      observes live counters/resource ownership through config/body/fanout suspension,
      success, refusals and cancellation, not only a hand-built audit scope.
- [x] Finish native core budget/layout/allocation checks and both strict lint
      feature selections. The fresh charged HTTP lifecycle suite now has 16 tests,
      including a nonfailing two-scope growth control and a genuinely injected
      second-scope reservation failure. One error observation retains earlier counts
      while charges/admission remain held, then all ownership refunds.
- [x] Commit/push the bounded core audit consumer in atomic `c2b00a2` after
      final evidence.

## Task 7: Accept Offline Disclosure Differences Before Cutover

**Files:** new exchange `tests/disclosure_policy_diff.rs` with inline synthetic
goldens and committed OpenRTB corpus inputs; `tests/support/frozen_scrub.rs`;
existing wire tests and `src/scrub/classified_tests.rs`'s independent 256 oracle.

- [x] Freeze the reviewed old privacy enforcement mutator with explicit source revision/provenance,
      never a production toggle. Run old/new on the same validated scoped request,
      decisions and real adapter serialization. The comparator pins committed
      revision `3f0f7f5aeb01d96742e0dd2012f179887ee474bc`; the independent 256
      composed typed/shadow oracle is in the classified unit suite.
- [x] Include committed model/privacy corpus, every required context/control
      field, hostile descendants, raw/sparse forms, synthetic shadows and all
      four ID/geo combinations. Do not derive expected outputs from the new engine.
- [x] Supply independent goldens for retained city/metro/other Geo context and
      kwarray; removed zip/accuracy/unknowns; non-finite typed input and finite
      overflow behavior. Name old tests that will change at cutover, including
      country-only/shadow-only Geo expectations and non-finite pass-through.
- [x] Bound input/node/work/output reporting. Curated cases use fixed fixture/path
      labels; external-input reports are aggregate-only. No payload or arbitrary
      key names are printed on mismatch. Secret fragment tests exercise failure
      diagnostics as well as successful reports.
- [x] Run `cargo test --locked -p auctioneer-exchange --test disclosure_policy_diff`:
      the fresh targeted binary passed all 16 tests.
- [x] Complete current importer check and final review of every newly retained
      path and conservative removal;
      unexplained changes block final live-cutover acceptance.
- [x] Commit/push independently authored disclosure differences in `c2b00a2`.

## Task 8: Switch The Existing Live Boundary And Gate It

**Qualification prerequisite:** final acceptance still requires Tasks 1-7's
outstanding semantic/resource/target evidence and reviewed offline differences.
The live boundary is already replaced in the worktree; that implementation fact
does not imply all prerequisite gates or the final review have completed.
**Files:** privacy enforcement run/split/scrub/core callers, affected tests/master spec,
workflows, CI mutation tests and raw-fixture allowlist.

- [x] Add all four resolved GDPR gates to core `response_gdpr_applies`, including
      `(true,false)` and `(true,true)`: fresh core wire suite passed 7 tests. These
      wire rows resolve real core host/account config and capture serialized generic
      adapter bodies through `run_auction`, explicitly not through HTTP. Presence
      controls cover Device/DNT/interstitial/context, EID descendants and unknowns.
- [x] Retain/add alias and composed-restriction wire assertions in the existing
      privacy enforcement privacy harness, separately from real-handler lifecycle audit tests.
- [x] Complete the fresh full privacy enforcement privacy/native integration sweep; do not equate
      the core direct-engine wire matrix with HTTP-handler matrix coverage.
- [x] Change run_auction to require caller-owned mutable audit; update all real/
      test callers found with `rg -n -F 'run_auction(' crates`. Give the same plain
      aggregate to construction, retaining earlier counts on later failures,
      invalid endpoint and adapter zero-call paths.
- [x] Replace current apply_scrubs implementation/call boundary atomically with
      the compiled-rule per-object mutator after existing FPD decode/validation.
      Keep source/merged-scope preflight before clone; no second typed decode.
      Denied scoped user/device/FPD/TID values are not restored from the original
      request for adapter construction or response hooks. Original publisher/control
      metadata outside those protected boundaries remains intentionally available.
- [x] Preserve validation-first expiry and author regression fixtures for
      expired invalid survivor -> code 15;
      expired valid survivor -> existing timeout response/bookkeeping, zero dispatch;
      denied malformed survivor -> never materialized. Check original deadline
      after validation, after restricted construction, between survivors and before
      dispatch. Drop expired calls without granting fresh tmax.
- [x] Execute `construction_deadline` (3): deterministic clock advancement proves
      the first adapter spends the original host-capped budget, the next sibling's
      make never runs, accepted sink buffers drop without fanout, semantic-failure
      call slots refund, and prior errors/cap/normalization warnings survive. The
      one-millisecond-remaining control dispatches with the original deadline.
- [x] Add/execute a distinct expiry-during-transport-finalization regression;
      all 5 deadline tests passed in the host run. The new allocation-triggered
      gzip expiry row and independent live control exercise the last checkpoint;
      adapter-triggered expiry alone would not independently prove it.
      These direct-engine tests also do not claim handler funded-receipt repayment
      or refund of already-committed successful call-count slots.
- [x] Wire core audit scope through auction_inner/run_auction. Verify exact real
      observer records on success, error, zero calls, later-scope failure and
      suspended cancellation while reservations remain held.
- [x] Migrate old primitive/unit expectations to the live compiled mutator and
      retain useful baseline/regression fixtures; the frozen old path is test-only.
- [x] Finish final review of migrated expectations without deleting denial
      coverage, and reconcile the master spec's Appendix C.2 catch-all/
      Geo text and intentional-transform notes. Mark only evidenced tasks complete.
- [x] Finish exchange `--lib scrub` and `--test privacy_policy_composition --test source_preflight
--test tmax --test run_auction --test projection_scrub_bound
--test construction_bound --test construction_deadline`; core
      `--test response_gdpr_applies --test admission_holds_through_auction` plus
      audit/resource qualification. Targeted passed subsets are recorded above.
      No unit-name filter is applied to integration binaries.
- [x] Wire importer check, policy/build/exhaustive tests, wire/audit/diff and
      allocation guards into required push/PR jobs with locked dependencies.
      Reject skipped/filtered triggers, if:false, continue-on-error, missing flags/
      arguments and raised numeric budgets using structured YAML mutation tests.
      Actual names are `source_preflight`, `disclosure_model_coverage` (explicit
      `--ignored` host compiler test), `disclosure_policy_diff` and the core audit
      unit module; required workflow execution is still pending.
- [x] Run core `--test ci_workflows` and both raw-JSON fixture gate scripts.
      Preserve existing vendor packaging and size gates; host-only importer/codegen
      dependencies must not enter deployed artifacts.
- [x] Commit/push the atomic live cutover in `c2b00a2`; remote head verified.

## Task 9: Propagate Verified privacy enforcement To user sync And diagnostic qualification

- [x] Verify clean worktrees/current remote heads before merging; keep concurrent
      user changes. Merge privacy enforcement into user sync deliberately, retaining usersync/redirect,
      dynamic bidder and auction work. No wholesale conflict-side selection.
- [x] This mutator is auction-only: run user sync exchange/core/CLI and usersync suites,
      composed wire tests and retained-lifetime guards. It must not silently alter
      cookie-sync/redirect permission handling. Commit/push any adaptation.
- [x] Merge verified user sync into diagnostic qualification, retain roster/evidence/Q2 requirements and run
      affected suites/resource gates. Do not claim provider qualification from
      scrub tests. Commit/push adaptations and final docs.
- [x] Verify published source-integration heads and clean scoped source worktrees;
      publish final owned evidence metadata separately. bounded fanout may carry the
      revised spec/plan as follow-on documentation; no production backport or main
      changes are required by this plan.

## Task 10: Full Verification And Evidence Record

**Files:** spec/plan status and new verification document only after execution.
The parent owns the final spec reconciliation and new execution ledger. The
full native workspace and strict-lint gates now pass, alongside deployment-target
lint and packaging. Executed WASI engine proofs are distinct from hosted-provider
or physical-isolate qualification, which is not established here.
The earlier full workspace run passed all 61 isolated build-failure cases, then
failed an obsolete coarse-Geo expectation in `core/tests/privacy_outbound.rs`.
The expectation has been reconciled without removing denied-field assertions;
the correction is not a passing full-workspace result. A fresh amended-cost
run passed, followed by a fresh non-config workspace sweep and focused reruns
after test-only style repairs. The alternate-feature workspace sweep also passed.
The implementation is pushed as `c2b00a2`; combined-state user sync/diagnostic qualification integration is
verified and published as `9ff017f`/`6177742`. See the current execution ledger.

- [x] Run:
  ```sh
  cargo fmt --all -- --check
  cargo test --locked --workspace --all-targets --features conformance-suite
  cargo clippy --locked --workspace --all-targets --features conformance-suite -- -D warnings
  cargo test --locked -p auctioneer-exchange --features insecure-bidders --all-targets
  ```
  Explicitly run required ignored/host-driven suites as CI does; record actual
  assertion execution, not clippy compilation or zero-test filtering.
- [x] Run strict conformance-feature target lint for Cloudflare
      wasm32-unknown-unknown, Fastly wasm32-wasip1 and Spin wasm32-wasip2, plus actual
      Wrangler dry-run, Fastly Compute package and spin build with unchanged caps.
      Label compilation separately from executed resource/depth qualification.
- [x] Rerun source/projection/work/destruction/error/retained-lifetime guards in
      native CI profiles, importer checks, catalogue/structural/workflow mutations and
      offline goldens on privacy enforcement. Task 9 records the completed combined-state reruns.
- [x] Record Rust/serde/dependency pins, implementation base, commands, measured peaks/
      artifact sizes, accepted differences, failures and unavailable tools. Do not
      infer remote CI status or architecture qualification from old decoding probes.
- [x] Independent final code review found no confirmed P1/P2 in the live mutator,
      amended funding or new charged failure fixture. Deployment/propagation evidence
      remains separate; no human merge approval is inferred.
      Remove dead old production/prototype paths; keep only useful test comparator
      and regression fixtures. Verified implementation and initial evidence were
      committed/pushed together; subsequent publication metadata is documentation-only.

## Acceptance

These are local implementation acceptance gates, not hosted-provider,
physical-isolate, legal or human merge approval. Their executed scope and
downstream integration limits are recorded in the execution ledger.

- [x] One production per-object mutator after one existing decode/validation.
- [x] Independent sourced membership, deterministic explicit metadata and one
      complete rule source with derived masks and mandatory context-shape checks.
- [x] All context/control, unknown, shadow and 256-composition protections proven
      at actual outbound bytes; validation-first expired-FPD precedence holds.
- [x] Actual owning cost descriptor is prepaid/used by all four targets; native
      retained lifetimes and bounded work/cleanup pass unchanged-factor construction
      bounds. Provider physical-isolate qualification is not inferred from these rows.
- [x] Core really consumes aggregate audit on all charged exits before release,
      with bounded static diagnostics and no asynchronous/shared audit state.
- [x] Reviewed offline differences precede atomic cutover; native/WASM/package/
      gate evidence is recorded and propagated to user sync/diagnostic qualification.

**Handoff:** local privacy enforcement implementation/review gates and the execution ledger are
complete and the atomic implementation is pushed. Task 9's combined-state
user sync/diagnostic qualification integrations are verified and published. Code review readiness
does not assert provider qualification, remote CI success or human merge approval.

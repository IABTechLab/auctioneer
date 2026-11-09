# Shared Resource Ownership Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement the approved plan task-by-task. Steps use checkbox syntax. Merge the planning PR and obtain implementation authorization first.

**Goal:** Deliver #33's current proof index, #50's SDK fact correspondence and #48's bounded auxiliary response ownership without changing supported workload or claiming target fit.

**Architecture:** Reuse the runtime's single additive derivation and the existing admission/completion owners. Bound response construction and retained storage in their current core owners. Apply a smaller handoff charge only when complete route, lifetime, container and applicable host-copy proofs support it; otherwise retain the charge and report the unfulfilled acceptance explicitly.

**Tech Stack:** Existing Rust workspace, Serde, `CappedWriter`, SDK HTTP/body/completion APIs and existing allocation/process test owners. No new dependency, crate, SDK extension, configuration path or product limit.

**Specification:** [Shared resource ownership](../specs/2026-11-09-shared-resource-ownership-design.md). AG approved its written scope on 2026-11-09. This plan remains proposed for joint review; implementation and merge are not authorized.

---

## Baseline And Ownership

Start from Auctioneer `499f0e540e95e1c86b730c1030aabef28de01d0a`, SDK
`598f0a2a490eb83dd31c06c024a0cefa8e049628`. Recheck relevant source and issue
state after the planning PR merges; do not chase another SDK revision as an
implicit part of this work. Read `AGENTS.md`, `crates/AGENTS.md` and
[development/test ownership](../../testing/development.md).

All commands below are planned commands; their expected outcomes are not
executed evidence. Preserve feature selection, original captures, existing
test assertions, ignored-test owners, process isolation and source pins.
Run heavy Cargo commands sequentially through the cycle's ownership wrapper.
Keep incremental outputs; do not clean shared targets to prepare a run.
For every focused command, record its executable target and nonzero dispatch.
The CLI's `bidders` modules belong to `--bin auctioneer-cli`, not `--lib`;
a successful command that executes zero selected cases is not verification.
The platform module has nine existing tests at this baseline. Reconcile any
changed count to named additions/removals rather than accepting a zero match.

| Responsibility | Files To Create Or Modify | Existing Verification Owners |
| --- | --- | --- |
| Current proof/SDK index | Create `docs/testing/resource-proof-ownership.md`; add a current-index pointer in `docs/bidders/schema-qualification.md` and `docs/testing/resource-qualification.md` | Existing schema/source reports and resource capture; no new ledger framework |
| Sync final backing | `crates/auctioneer-core/src/handlers/sync_http.rs`, `src/serialize.rs` only for shared serializer factoring | Inline units; `tests/serialize_allocation_bound.rs`, `tests/usersync_handler_alloc.rs` |
| Header/cookie ownership | `crates/auctioneer-core/src/handlers/sync_http.rs`, `src/usersync/cookie.rs` where its formatter/conversion needs a bounded change | Inline units; existing sync allocation/HTTP/integrity targets |
| Completion lifetime | `crates/auctioneer-core/src/ingress.rs`, `src/host_primitives/ingress_lease.rs` only if a demonstrated gap requires a fix | `tests/egress_binding.rs`, `tests/aux_route_write_deadline.rs`, `tests/aux_routes_wrapped.rs`, `tests/probe_output_before_boot.rs` |
| Qualified accounting correction, conditional | `crates/auctioneer-runtime/src/{aux_work,budget}.rs`, core `src/host_primitives/{gates,resource_profile}.rs` and adapter-owned envelope/profile callers only as necessary | Inline runtime/gate units and current adapter envelope tests |
| Report/parity consumers, conditional | `crates/auctioneer-cli/src/bidders/resources/{mod,catalog_comparison,catalog_continuity,records/mod,records/capture}.rs` | Inline units; `tests/bidders_resources.rs`, `tests/resource_boot_parity/main.rs`; original pinned catalogue baselines |

Unqualified source paths in a row remain inside its named crate. Keep pure
unit tests inline and preserve the separate allocation/global-boot binaries.
Do not split unrelated large modules or add a test target for this package.

## Task 1: Freeze The Actual Closure And Completed Components

- [ ] Record a fresh clean baseline commit, SDK/lock/toolchain/features, actual
  test selections and any failure before editing production code. Use the
  current full acceptance matrix, not an old test-count assertion.
- [ ] Obtain executable definitions/aliases from build discovery and generated
  metadata/bindings. Join current schema selection/probe reports and format
  owners. Distinguish 17/18 native versus 3/4 guest serving bindings from the
  larger metadata/inspection populations at the baseline; derive membership
  from source rather than adding another name list.
- [ ] Create the proof index with separate C, I, V, config, sealed DTO, ordinary
  decoder-audit, auxiliary, SDK-bookkeeping, ingress, codec and host-artifact
  sections. For each row name status, source, units, acquire/release,
  simultaneous owners, evidence limits, issue, responsible role and next action.
- [ ] Credit existing component models, native schema reservations, owned
  receipts and #159's settings-first API. Assign unfinished discovery-derived
  schema selection and C/I/V certification to #46/#47, extraction proof to
  #147, DTO qualification to #150, audit to #148 and actual host handoff to
  Trusted Server #1510/#76. Do not reopen implemented APIs or add unshipped
  backends to the closure.
- [ ] Reconcile every index row to an actual report/source or explicit missing
  evidence. Keep historical checkpoint prose and artifacts unchanged except
  for a clearly separated current-index pointer. Commit the self-contained
  index checkpoint; it delivers no resource qualification or issue closure yet.

```sh
cargo test --locked -p auctioneer-bidder --test registry_join --test registry_source_conformance
cargo test --locked -p auctioneer-bidder --test schema_source_conformance schema_selection -- --test-threads=1
cargo test --locked -p auctioneer-bidder --test schema_source_conformance --features insecure-bidders schema_selection -- --test-threads=1
cargo run --locked -p auctioneer-cli -- bidders resources --check --all-adapters --capture-audit > "$RUNNER_TEMP/resource-audit-before.toml"
cargo run --locked -p auctioneer-cli -- bidders resources --check --all-adapters --audit "$RUNNER_TEMP/resource-audit-before.toml" > "$RUNNER_TEMP/resource-audit-before.md"
```

Expected: actual nonzero dispatch, exact report/source correspondence and a
complete source audit. These commands do not prove guest physical fit. Source
selection still using historical projections must be identified, not silently
replaced or relabeled as full executable discovery.

## Task 2: Complete The SDK Fact And Funding Correspondence

**Inspect:** Four adapter envelope/profile/entrypoint owners,
`crates/auctioneer-core/src/host_primitives/{gates,resource_profile}.rs`,
`src/outbound/`, CLI `src/bidders/resources_platform.rs`, and the pinned SDK
`crates/edgezero-{core,adapter-axum,adapter-cloudflare,adapter-fastly,adapter-spin}`.

- [ ] For each adapter, join published facts and actual enforcement with the
  configured ceiling, scope, populations, dispatch and artifact requirement.
  Record Axum's existing connection gate, Fastly's published separate stack
  and Spin's actual hosted dispatch; do not invent missing implementations.
- [ ] Trace prepared contexts, scopes, tasks, queued/completed results, boxed
  receipts and teardown scratch into an existing funded term/population, or
  mark the correspondence unproven. Include Fastly's recorder/configuration
  keys/backend caches and their callback/instance lifetimes.
- [ ] Separate host ingress/framing from admitted application work and
  application headers from provider-added fields. Preserve Cloudflare's unknown
  inbound population, native operator facts, hosted Spin provenance and codec
  certificate work. Assign facts/actions to #72-#75 and codec coordination #49;
  no external service is contacted or deployment mutated in this task.
- [ ] Verify configured overflow and external absence remain independent in
  the existing platform evaluator. Run its existing controls; if a concrete
  missing case is found, add an inline failing case first and correct only the
  demonstrated mismatch. Do not add code solely to restate a passing table.
- [ ] Record the exact ownership/action ledger and its remaining unknowns.
  Mark #50's coordination outcome separately from provider fulfillment; the
  final handoff must not turn a missing physical correspondence into `known`.

```sh
cargo test --locked -p auctioneer-cli --bin auctioneer-cli bidders::resources_platform::
cargo test --locked -p auctioneer-cli --test bidders_resources
cargo test --locked -p auctioneer-core --lib host_primitives::resource_profile::
```

Expected: unknowns stay nonpassing and local demand failure is still reported.
The index is the deliverable where current behavior already conforms; it is
not a reason to change adapter configuration or the evaluator.

## Task 3: Bound Sync Final Backing Without Changing Its Wire Output

- [ ] Add inline controls beside `bounded_json_line` for empty, small,
  intermediate, exact and over-limit bodies, varying payload and supplied cap
  independently. Include all escaped characters and the final newline. Assert
  literal expected bytes and absence of a partial response on failure.
- [ ] Extend `serialize_allocation_bound`/`usersync_handler_alloc` with two
  unequal sizes, retained-capacity and no-growth observations, plus a positive
  growable-writer control. Run the new cases before editing; record the actual
  red assertions and dispatch, not an expected missing-test failure.
- [ ] Factor the current counted-size/capped-write implementation only as
  needed, keeping the existing sync output cap. Use a cap-aware, nonallocating
  counting writer that stops serialization on the first over-cap write; do not
  use the uncapped `serialized_json_bytes` helper alone on this path. Refuse
  `max < 1` first, count at most `max - 1` raw JSON bytes, and check the count
  before any output-buffer reservation. For the unescaped path reserve that
  count plus newline and write without growth. This adds no cap to info/health.
- [ ] For the escaped path, checked-count HTML expansion from the validated
  raw UTF-8, stop on `max - 1` overflow before reserving the escaped buffer,
  and reserve its full output plus newline. Fund both backing buffers
  simultaneously and write through the same capped writer.
- [ ] Preserve the existing error mapping. Over-cap remains `BodyLimit`,
  serializer failure remains `Serialization`, and reservation failure is a
  bounded terminal internal response, with no retry or partial body. Test a
  deliberately inconsistent custom serializer in both directions: growth must
  fail at the reservation and shrinkage must fail the final length equality
  check. No counted-size mismatch may produce a response. Do not equate
  requested reserve size with a physical proof.
- [ ] Add a serializable sequence with an instrumented visited-entry counter:
  over-cap counting must stop before the remaining entries are visited, while
  a fitting positive control visits all entries. Observe that raw overflow
  reserves neither output buffer and escaped overflow reserves no escaped
  buffer. Independently vary input length, cap and escape expansion; retain
  existing refusal boundaries and avoid an elapsed-time-only work assertion.
- [ ] Run full module/allocation and HTTP reference controls, then commit the
  green bounded-backing change with unchanged isolate charges.

```sh
cargo test --locked -p auctioneer-core --lib handlers::sync_http::
cargo test --locked -p auctioneer-core --lib serialize::
cargo test --locked -p auctioneer-core --lib handlers::tests::
cargo test --locked -p auctioneer-core --test serialize_allocation_bound --test usersync_handler_alloc
cargo test --locked -p auctioneer-core --test usersync_http --test usersync_wire_reference --test usersync_template --test usersync_integrity
```

Expected: byte-identical successful output, unchanged refusal boundaries,
nonzero positive controls and bounded construction/final backing. A change in
measured allocations is reported separately from an accounted reduction.
The inline `handlers::tests::` cases drive production responses against the
frozen wire oracle. `usersync_wire_reference` checks fixture metadata only;
it is retained but cannot substitute for that production oracle.

## Task 4: Establish Header And Complete-Cookie Ownership

- [ ] Enumerate the actual final header shapes in `sync_http`, info, health,
  fallback/error and probe owners. Record map capacities and all duplicate-value
  allocations, name/value backing and cookie/CORS formatting transitions for
  the locked `http`, `bytes`, cookie and SDK versions. Do not derive retained
  bytes merely from `headers.len()` or serialized field size.
- [ ] Add owner-local inline cases and allocation cases varying unique fields,
  duplicate values, name/value length, CORS backing size and cookie length
  independently. Include an oversized backing with a small surviving slice,
  insertion then removal, final failure, and a positive allocation control.
  Prove before-state presence and backing sharing before asserting detachment.
- [ ] Where the source proof requires a change, build the reviewed final
  application header set through its current helper. Copy only bounded final
  names/values when detachment is required; a fresh map may replace retained
  oversized capacity only while old/new map and value backing are both funded.
  Preserve valid duplicates, `Vary`, CORS and complete cookie bytes. Do not add
  a blanket header ceiling to an endpoint with a different existing contract.
- [ ] Account the existing cookie preflight, formatter, field string and
  `HeaderValue` conversion peak. Replace a growable stage only with an existing
  compatible bounded mechanism and literal byte/reference checks; do not add
  a cookie encoder or hand-written replacement grammar for this work.
- [ ] Write the source/ABI/container bound and its actual allocator scope.
  Validate native observations and applicable guest layouts independently.
  If a locked library contract cannot justify a necessary bound, record its
  exact missing fact/owner, retain charges and #48's unmet criterion, and stop
  the affected accounting cutover. A new SDK/layout API or vendor patch goes
  back to design review; do not invent a private layout or grow an allowance.
- [ ] Run all existing header, cookie, route and allocation owners. Commit only
  green independently useful ownership changes; do not mark #48 complete from
  a source-only table or assigning the remaining proof to #50.

```sh
cargo test --locked -p auctioneer-core --lib usersync::cookie::
cargo test --locked -p auctioneer-core --lib handlers::sync_http::
cargo test --locked -p auctioneer-core --lib handlers::tests::
cargo test --locked -p auctioneer-core --lib handlers::info_bidders::
cargo test --locked -p auctioneer-core --lib handlers::health_gvl::
cargo test --locked -p auctioneer-core --test usersync_handler_alloc --test serialize_allocation_bound
cargo test --locked -p auctioneer-core --test usersync_http --test usersync_routes --test usersync_integrity
```

Expected: existing public behavior and application limits stay intact; a
capacity/ownership regression makes a named new case fail. Native requested
heap is not presented as guest physical memory or a whole-wire header cap.

## Task 5: Verify Terminal Ownership Across All Contributing Routes

- [ ] Extend the existing route/wrapper coverage join so every contributing
  auxiliary/fallback response and distinct probe path is accounted for. Reuse
  registered-route parsing; do not write a second manual route list that can
  omit new registration silently.
- [ ] Use the existing real SDK admission/response envelope to suspend egress
  after handler return. Observe the original auxiliary route gate and work
  ledger remaining occupied, then separately exercise completion success and
  completion drop/abandonment. Each must release exactly once.
- [ ] Cover saturation/recovery, header/body construction refusal, config/store
  failure, body-read timeout, cancellation during construction, cancellation
  after handoff and framework-generated fallback after the grant is dropped.
  Do not add a second auxiliary acquisition or new public fault hook.
- [ ] Keep probe-config work, output permits/cache, static refusal and auction
  work/egress counters independent. Exercise zero auxiliary permits and unequal
  nonzero populations. If an existing lifetime control already pins a property,
  extend it for the missing join rather than cloning its whole fixture.
- [ ] Fix only a reproduced ownership/deadline gap. Otherwise retain the current
  completion wiring and commit the proof controls; no gratuitous lifetime split.

```sh
cargo test --locked -p auctioneer-core --test egress_binding --test ingress_admission
cargo test --locked -p auctioneer-core --test aux_routes_wrapped --test aux_route_write_deadline --test aux_request_body_bounds
cargo test --locked -p auctioneer-core --test probe_output_before_boot --test probe_pending_reads --test probe_before_boot
```

Expected: actual positive held counters, exhausted admissions, original
deadlines and complete recovery. Handler return alone must not free terminal
output storage. Filtered or zero-route dispatch is not coverage.

## Task 6: Freeze Any Exact Accounting Correction Before Its Cutover

This task is conditional on Tasks 3-5 supplying the complete required bounds.
If they do not, preserve production accounting and report the blocked part of
#48. Do not add a write-only descriptor, zero placeholder or unused helper.

- [ ] Record `A` (auxiliary population), `P = max(A, probe minimum)`, `D`
  (unchanged declared per-slot work peak), `H` (applicable handoff multiplicity),
  `R` (proved application response-retained storage) and `T` (transmit state).
  Show the exact owners behind each symbol and whether `D` already includes
  one `R`. Include all output/header/cookie/refusal routes and actual target
  copy contracts, not just a cookie-sync maximum.
- [ ] The current charged expression is `(A + P) * (H * D + T)`. A possible
  correction, **only if the proofs establish its premises**, is
  `A * (D + (H - 1) * R + T) + P * D`: retain all declared work slack, replace
  only additional response-copy work, and charge probe output/cache separately
  as already done. Prove that probe configuration creates no additional
  uncharged transmit/copy domain. If any premise fails, retain that existing
  component; do not apply the expression as a default saving.
- [ ] Freeze the actual old/new checked derivation, storage source/ABI witness,
  affected adapter rows, exact delta and residual gap in the current index
  before changing `budget.rs`. `R <= D`, `H >= 1` and every required source
  bound must be established. A different ownership/equation proposal outside
  the approved additive design returns to review.
- [ ] Add failing inline runtime/gate cases varying `A`, probe minimum/output
  population, `D`, `R`, `H` and `T` independently, including zero, one, unequal
  nonzero, overflow and one-byte boundaries. Expected deltas use independent
  values; removing the changed summand or changing the wrong population must
  fail a specific case. Preserve every existing unrelated term assertion.
- [ ] Implement the minimal derivation in runtime and pass owning bound data
  through the existing profile/bootstrap/CLI consumers. Update exhaustive
  destructuring, capture component receipts and actual boot/CLI projections in
  the same checkpoint; the CLI must not calculate its own substitute sum.
- [ ] Before changing current receipt arithmetic or shape, add cutover controls
  in the existing records owner for both `catalog-baseline.toml` and
  `catalog-baseline-patched-lock.toml`. Their original bytes/pins, component
  arithmetic and existing continuity checks must remain valid. Today
  `validate_snapshot` calls `validate_components` before its `Mode` branch;
  changing `component_values` alone would apply new arithmetic to frozen
  records. Separate explicit supported historical validation from current
  validation before that shared call. Do not skip arithmetic for all frozen
  inputs or default missing current bound fields to zero. Test altered old
  receipts and incomplete current receipts are still refused; extending DTOs
  must not make either archived format unreadable.
- [ ] Trace `catalog_comparison::{check_capture,compare,growth_between}` and
  `catalog_continuity` through the same cutover. They currently rederive
  captures and allow only matched-input, nonnegative catalogue growth. Keep
  supported historical validation/continuity and same-model comparisons
  working; refuse unsupported mixed-accounting-model comparisons explicitly.
  Do not reinterpret a handoff saving as catalogue growth, broaden the growth
  allowance, drop a capture check or rewrite a pinned baseline. New accounting
  evidence remains a separate current capture, not a recertified old record.
- [ ] Run the real closed-graph parity target and existing current-resource
  report/refusal controls. Refresh new evidence, never old pinned records.
  Report observed allocation differences and accounted savings separately;
  missing SDK facts and target overflow must remain nonpassing.

```sh
cargo test --locked -p auctioneer-runtime --lib
cargo test --locked -p auctioneer-core --lib host_primitives::gates::
cargo test --locked -p auctioneer-cli --bin auctioneer-cli bidders::resources
cargo test --locked -p auctioneer-cli --bin auctioneer-cli bidders::resources::records::tests::pre_catalog_snapshot_pins_source_terms_and_unqualified_targets -- --exact
cargo test --locked -p auctioneer-cli --bin auctioneer-cli bidders::resources::records::tests::catalog_comparison_loads_only_the_frozen_reviewed_baseline_bytes -- --exact
cargo test --locked -p auctioneer-cli --bin auctioneer-cli bidders::resources::records::tests::patched_lock_catalog_baseline_retains_origin_and_chooser_growth -- --exact
cargo test --locked -p auctioneer-cli --test bidders_resources
cargo test --locked -p auctioneer-cli --test resource_boot_parity
cargo run --locked -p auctioneer-cli -- bidders resources --check --all-adapters --capture-audit > "$RUNNER_TEMP/resource-audit-after.toml"
cargo run --locked -p auctioneer-cli -- bidders resources --check --all-adapters --audit "$RUNNER_TEMP/resource-audit-after.toml" > "$RUNNER_TEMP/resource-audit-after.md"
bash .github/workflows/scripts/test/reconcile-resource-fit.sh
```

Expected: exact production/CLI parity and truthful residual refusal. The last
script permits the CLI's documented exit 1 only when its JSON fit verdicts
agree; it is not a target-fit pass. Current/future new evidence binds its own
source/SDK/artifact, not this planning baseline by substitution.
Each exact historical regression above must execute one case, not zero. Run
the new cutover/refusal cases too; the existing frozen-coefficient test alone
does not cover changing auxiliary component arithmetic.

## Task 7: Complete Verification And The Review Handoff

- [ ] Execute Appendix A and the applicable complete workflows at the final
  implementation head. Run every new named case with nonzero dispatch. Keep
  all unrelated test assertions, full-source checks and feature graphs intact.
- [ ] Demonstrate guard sensitivity with isolated temporary mutations for each
  consequential new storage/lifetime/accounting check. Retain exact failing
  output and restore each mutation before the complete green run; do not mutate
  another reviewer/user checkout or leave a production test hook.
- [ ] Record commands, exit codes, actual result lines, commit, artifact/ABI,
  features/SDK/lock, observations versus bounds, accounted delta, residual
  demand and blocked checks. Do not report guest execution from `--no-run`.
- [ ] Update the current index, keeping #33, #50 and #48 outcomes distinct. If
  #48 is partial, state the exact unmet criterion; do not claim the package or
  whole resource epic complete merely because the index and body fix landed.
- [ ] Request independent review on the separate implementation PR. Use
  closing keywords only for actually delivered issue acceptance; leave provider,
  schema, target certification and host handoff issues open. Never resolve
  reviewer threads as Creator or merge without the user's decision.

## Appendix A: Acceptance Commands And Prerequisites

Use `.tool-versions` and the source/SDK locator in
[development/test ownership](../../testing/development.md). Supply one pinned
bidder source repository with both required immutable revisions,
`PREBID_SERVER_DIR`, `EDGEZERO_SDK_ROOT` and a disposable `RUNNER_TEMP`. No
supplemental checkout variable or untracked private provider config is needed.
Missing source, tool, provider or execution prerequisites are blockers, not
permission to ignore, relabel or weaken a case. Current CI is authoritative
for tool installation and the complete matrix; do not copy workflow bodies
into a Rust snapshot test.

### Native Graphs And Focused Conformance

```sh
cargo test --locked -p auctioneer-config-data --lib --test codegen --test disclosure_codegen --test gvl_archive --test smoke --test url_safety --test activity --test product_defaults_snapshot --test simple_fold --test usersync_config --test usersync_callback_validation --test bidder_bindings --test bidder_storage
cargo test --locked -p auctioneer-config --lib --test root_shape --features build-support
cargo test --locked -p auctioneer-bidder -p auctioneer-exchange -p auctioneer-core --all-targets
cargo test --locked --workspace --all-targets --no-fail-fast -- --skip tests::selected_schema_source_producer_records_exact_real_dispatch_without_qualification --skip tests::controlled_native_acceptance_requires_all_producers_but_never_completes_the_full_catalog
cargo test --locked --workspace --all-targets --features insecure-bidders --exclude auctioneer-config-data --no-fail-fast -- --skip tests::selected_schema_source_producer_records_exact_real_dispatch_without_qualification --skip tests::controlled_native_acceptance_requires_all_producers_but_never_completes_the_full_catalog
cargo test --locked -p auctioneer-config-data --features "insecure-bidders test-harness-override" --lib --test codegen --test gvl_archive --test smoke --test url_safety --test bidder_bindings --test bidder_storage
cargo test --locked -p auctioneer-cli --all-targets -- --skip tests::selected_schema_source_producer_records_exact_real_dispatch_without_qualification --skip tests::controlled_native_acceptance_requires_all_producers_but_never_completes_the_full_catalog
cargo test --locked -p auctioneer-cli --no-default-features --all-targets
cargo test --locked -p auctioneer-core -p auctioneer-adapter-cloudflare -p auctioneer-adapter-fastly -p auctioneer-adapter-spin --all-targets --features conformance-suite
cargo test --locked -p auctioneer-adapter-axum --test cancellation --test wave_transport --test privacy_wire --features insecure-bidders
```

The two libtest skips above retain their existing mandatory once-owned
producer command below. `--all-targets` alone skips `test = false` and
unsatisfied `required-features`; never describe it as the complete matrix.

### Mandatory Closed-Graph Owners And Gates

```sh
bash .github/workflows/scripts/test/verify-schema-compiler-conformance.sh schema
bash .github/workflows/scripts/test/verify-schema-compiler-conformance.sh settings
bash .github/workflows/scripts/test/verify-schema-compiler-conformance.sh exports
bash .github/workflows/scripts/test/verify-schema-compiler-conformance.sh discovery
bash .github/workflows/scripts/test/verify-config-build-failures.sh
bash .github/workflows/scripts/test/verify-evidence-producers.sh
bash .github/workflows/scripts/test/verify-schema-core-secure.sh
bash .github/workflows/scripts/test/verify-schema-core-development.sh
bash .github/workflows/scripts/test/verify-schema-metadata-secure.sh
bash .github/workflows/scripts/test/verify-schema-metadata-development.sh
bash .github/workflows/scripts/test/execute-catalog-publication.sh
cargo test --locked -p auctioneer-cli --test resource_boot_parity
cargo test --locked -p auctioneer-bidder --test test_isolation
./scripts/check-batch-rebase.sh
./scripts/check-roster.sh
./scripts/check-roster-fixtures.sh
./scripts/check-secure-usersync.sh
./scripts/check-raw-json-fixtures.sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo clippy --locked --workspace --all-targets --features insecure-bidders -- -D warnings
cargo clippy --locked -p auctioneer-core -p auctioneer-adapter-cloudflare -p auctioneer-adapter-fastly -p auctioneer-adapter-spin --all-targets --features conformance-suite -- -D warnings
```

Also run the unchanged feature-binding/source-selection, bidder provenance,
browser-shutdown, guest check/lint/test-link/runtime and provider packaging
steps from `.github/workflows/test.yml`, `format.yml`, `ci-cloudflare.yml`,
`ci-fastly.yml` and `ci-spin.yml`. Those scripts/workflows remain the single
complete executable authority; Appendix A does not exclude omitted rows or
replace actual hosted job results. No credentialed deployment test is implied
by a local vendor build or dry run.

### Actual Guest Evidence

Application storage proofs require their applicable guest ABI/execution
evidence, not only packaging. Reuse current guest runners and bound artifacts
where supported. If a provider-only allocation domain cannot be measured or
proved locally, preserve the unknown, exact owner and next verification; it
blocks that claimed bound or target readiness. Do not create a new harness,
skip a target, or claim #48's outstanding criterion completed to finish a PR.

## Planning Publication

This phase changes only the specification and plan. Publish them together
after document review and authorized publication; include `PRfection phase:
plan`, `Refs #33`, `Refs #50`, `Refs #48`, actual executed checks and remaining
validation blockers. Keep all three issues open and preserve project fields.
Planning approval is neither implementation approval nor merge authorization.

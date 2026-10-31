# WASM Resource Envelope Remediation Implementation Plan

**2026-10-30 CI simplification:** AG removed the Rust workflow-configuration
snapshot suite. Completed configuration-mutation checks recorded below describe
historical verification, not a target to recreate. Actual provider workflows and
their colocated build, test and resource-check scripts remain the CI owners.

**2026-10-30 additional owner:** complete worst-case schema resource
certification deferred by the approved
[controlled-native amendment](../specs/2026-10-30-m7-controlled-native-completion-design.md)
belongs to R0, separately from the existing target-fit work below. Preserve
shipped-closure selection and the reviewed engine/patches; join real compiler
ABI facts to reachable allocation/work/stack owners before calling costs proved.
Synthetic target-layout model tests, artifact inspection and native observations
are supporting evidence, not certification. Write the concrete remaining proof
steps against the then-current shipped closure; do not resume unneeded all-source
private-engine expansion as an M7 completion prerequisite.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Repair the declared WASM resource envelopes with justified ownership and population bounds, explicit workload decisions where necessary, and the same absolute-fit accounting at boot and strict acceptance.

**Architecture:** Reuse the portable runtime receipts/gates, core input resolver and M7's resource-report/checker ownership. Audit before selecting changes; verified unchanged-workload improvements may proceed under AG's authorization below. The feasibility checkpoint freezes any proposed limit/model changes for specific approval. Do not create a second calculator, private qualification equation or provider stack.

**Tech Stack:** Rust 1.98.1, existing Cargo workspace, EdgeZero PR #275 at `9fe6c56c648f0a601b1c6c051ce71c09d3733ea9`, existing Rust CLI, target builds and qualification harnesses. The [October 30 outbound adoption](2026-10-30-edgezero-outbound-adoption.md) includes API and semantic changes, not just documentation. Historical captures retain their recorded revision; the October 18 update from `f6c708cc` to `bc300ace` changed seven documentation files only. See the [subsequent SDK update](../../testing/evidence/edgezero-pr275-update-20261030.md) for the current source delta and verification; earlier captures remain historical.

---

## Scope And Checkpoints

- Design: [M7-R0 specification](../specs/2026-10-17-m7-r0-wasm-resource-envelope-design.md).
- Same M7 branch/PR #21; no separate branch or milestone-renaming cutover.
- Planning baseline `871c72e2`; historical evidence stays byte-identical.
- This is a staged remediation plan, not a claim that unchanged declared caps can fit. On 2026-10-18 AG authorized verified unchanged-workload improvements, then explicitly approved proof-backed accounting corrections, including config-node units and response-handoff versus config/privacy storage. Preserve workload limits and the single shared additive total. Independently bound each corrected allocation/population before crediting it; ask before workload or shared-equation changes. These replies supersede Task 3's frozen approval prerequisite for this authorized work. Frozen scope checks remain mandatory for a separately approved package outside that authorization.
- At the pinned SDK revision no target publishes host-ingress accounting; Axum also lacks ceiling/population facts, and Cloudflare lacks an inbound bound. Aggregate `--require-fit` cannot reach exit zero through Auctioneer-only patches. Track completed local packages separately from the external prerequisites in spec section 5.3; do not weaken the gate or declare R0 complete while those facts remain unknown.
- On 2026-10-19 AG approved proceeding with M7 catalog implementation while these R0 readiness blockers remain explicit. Keep completing independently justified local R0 fixes; do not make catalog development wait on unpublished SDK facts, or treat that sequencing decision as proof of absolute fit. Workload limits, the shared additive equation and M7's own acceptance gates remain unchanged.
- The numeric-only planning gaps already require approximately 99.47-99.91% demand reduction. Keep workload/profile proposals explicit; an approved partial limit/model package need not close the whole gap. AG has now chosen to proceed with verified unchanged-workload improvements first. Report their actual effect without assuming they are either sufficient or mathematically impossible before proof.
- M7 Task 4 owns shared catalog accounting, the CLI strict checker, pure profile and isolated boot-parity targets. Reuse those; do not duplicate them under R0 names. Mandatory catalog descriptors, core normalization and schema-2 exports are implemented. The strict CLI and real boot/CLI parity control are now implemented; matched-baseline catalog allowance gates and production memory remediation remain open.
- Unit tests stay beside their owner; allocator/handler/CLI/child-build tests belong in its `tests/` directory. Test names describe behavior, not milestones.
- No new crates, scripts in another language, ignores, blanket lint allows, compatibility defaults or authorship trailers. Commit and push each verified task/repair round.

## File Ownership

| Files                                                                                                                         | Role                                                                                                                                                       |
| ----------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/auctioneer-runtime/src/{budget,auction_budget,admission,aux_gate,aux_work,egress,static_egress}.rs`                       | Checked equation, pure shared minimum-headroom policy, receipts and enforced populations; edit only approved owners.                                       |
| `crates/auctioneer-core/src/host_primitives/{gates,bootstrap}.rs`, `src/configure.rs`                                             | Effective input resolution, boot retention and config lifetime audit.                                                                                      |
| `crates/auctioneer-core/src/handlers/{auction,request_disclosure_audit}.rs`, `src/outbound/wave.rs`                               | Real admission/response lifetimes and construction costs.                                                                                                  |
| `crates/auctioneer-exchange/src/run/mod.rs`, `crates/auctioneer-bidder/src/types.rs`                                                  | Response parsing/retention and prepaid work ownership.                                                                                                     |
| Four `crates/auctioneer-adapter-*/src/envelope.rs`; Axum `src/resource_profile.rs`                                                | Target profiles/exports, never a second formula.                                                                                                           |
| `crates/auctioneer-cli/src/bidders/resources/{mod,review}.rs`, `resources/records/{mod,capture}.rs`, `tests/bidders_resources.rs` | One M7-owned resource command: bounded reports, typed records/capture/tables, strict checking, SDK mapping and reviewed-scope checking; no new executable. |
| CLI `build.rs`, `resource_identity.rs`, `src/bidders/resources_platform.rs`; Axum library envelope/feature exports            | Shared source snapshot and actual compiler identity, SDK adaptation and reuse of shipped declarations; no second accounting formula.                       |
| `crates/auctioneer-cli/src/qualification/{profile,report,evaluate}.rs`, `tests/qualification_cli.rs` and existing `harness/`      | Existing evidence/verdict ownership and shared-floor enforcement; no new qualification engine.                                                             |
| Planned core `tests/{catalog_resource_profile,boot_strict_parity}.rs`                                                         | Pure math versus actual consumers in fresh processes.                                                                                                      |
| New core `tests/fixtures/resource_envelopes/{audit,remediation-plan}.toml` plus baseline/candidate reports in that directory  | Typed source-bound audit and frozen exact-path change packages, not prose-only approval.                                                                   |
| `docs/testing/adapter-resource-envelopes.md`                                                                                  | Human audit, approved patch decisions, evidence index and target limitations.                                                                              |
| `.github/workflows/{test,ci-cloudflare,ci-fastly,ci-spin}.yml` and their colocated scripts                                  | Required target builds/lints/checks. Workflow configuration snapshot tests were removed by AG on 2026-10-30.                                               |

## Task 1: Reproduce And Own The Baseline

Progress (2026-10-17): the [first foundation verification record](../../testing/evidence/resource-policy-foundations-20261017.md)
records baseline/red counts and the bounded implementation. The complete task
is still open; reporting/audit/scope/parity controls below are not implied by
the foundation checks.

- [x] Record complete baseline and new-policy/old-fixture counts before shared-fixture migration.
- [x] Introduce runtime-owned named summands without changing the additive equation.
- [x] Add the pure headroom policy and enforce it through qualification validation and actual CLI commands.
- [x] Migrate shared/current synthetic fixtures and the four independent ceiling controls, retaining original captures.
- [x] Compile the real runtime owner in an isolated scratch workspace; a new unclassified field must fail in both the checked fold and named export, with diagnostics anchored to those functions.
- [x] Add a strictly non-installing core profile target covering independent zero/default config caps, actual baked tuple size and catalog scaling at unequal auction populations.
- [x] Add the four-target source-bound resource report and SDK adapter, including configured failures independent of external unknowns, separate-stack errors, exact fit-mask controls and actual compiler identity.
- [x] Record the initial source/lifetime audit and a nonfitting reduced-workload counterexample without changing production caps.
- [x] Validate bounded typed audit/package records, including ordered projections, strict empty states and separate current-source versus frozen arithmetic.
- [x] Enforce frozen Git review scope with source metadata binding, both diff selectors, regular-file modes, staged-revert controls and bounded subprocess I/O.
- [x] Refresh compiled provenance on Git metadata changes and retain HEAD-only staged deletions in source snapshots.
- [x] Capture the canonical audit and generate its human tables from clean code baseline `ac6d54112638e0576cf57cc6dde2333188e60530` using `--check --all-adapters --capture-audit` and `--check --all-adapters --audit <path>`; standalone rendering validates all four source profile digests at that baseline. This audit snapshot is not the immutable package review checkpoint D.
- [x] Use adapter-owned typed `ResourceProfile` exports in bootstrap and CLI aggregation; compare actual SDK metadata instead of provider-name exceptions, preserving all caps and blockers. AG approved this refactor on 2026-10-18. Native schema-2 export consumes the same profile without a wire cutover; frozen source records retain their own accounting scope.
- [x] Add the one-test fresh-process boot control: ten counted children cover independent defaults, baked bytes, retained terms and warning/strict-refusal semantics.
- [x] Compare that existing probe against actual CLI subprocesses built from the same isolated source and matched owner-library features. Ten synthetic profiles cover the eight independent config-default combinations and two warn/install versus strict-refusal cases; two scratch-only CLI corruptions must refuse before export. This is native consumer parity, not physical deployment fit.
- [ ] Complete resource-command consumption, exhaustive report/added-field controls and the remaining baseline/audit work below.

The [October 18 audit notes](../../testing/evidence/resource-envelope-audit-20261018.md)
record this reporting slice. `bidders resources --check --all-adapters` currently establishes
accounting/source completeness, not the unfinished matched-baseline/catalog
allowance gates. `--require-fit` additionally refuses the current absolute
failures and unknowns. The [checkpoint verification record](../../testing/evidence/resource-review-controls-20261018.md)
adds current-source audit validation and frozen Git scope enforcement.
The [canonical audit](../../../crates/auctioneer-core/tests/fixtures/resource_envelopes/audit.toml)
and [generated human tables](../../testing/adapter-resource-envelopes.md) now capture
the clean code baseline; [this round's verification](../../testing/evidence/adapter-owned-resource-audit-20261018.md)
also records adapter-owned profiles and fresh-process boot controls. A concrete
limit/model change package and any required specific approval/CI binding,
production remediation, matched-baseline allowances and target proofs remain pending. The
[ownership-control record](../../testing/evidence/resource-ownership-controls-20261018.md)
adds actual CLI parity, response retention and App snapshot lifecycle evidence.
This progress is not
Task 1 or R0 completion.

Adapter-selection correction (2026-10-18): resource inspection uses the same
`--adapter axum|cloudflare|fastly|spin` vocabulary as build/deploy/serve. Require
exactly one adapter or `--all-adapters`; reject compiler triples as selectors.
Reports explicitly declare the selected scope and use `adapters`/`adapter`
identities. Canonical audit capture/validation and review remain complete
four-adapter operations requiring `--all-adapters`. Compiler ABI stays derived
metadata; no historical qualification capture or resource equation changes.

**Files:** M7 CLI resource checker/report owner; four provider envelope modules;
core pure profile controls; new fixture directory and human audit named above.

Qualification fixture migration is a **shared dependency change**, not four
isolated test edits. `profile.rs::tests::candidate` feeds roughly 70 source
references across `profile.rs`, `report.rs` and `evaluate.rs`, including derived
fixture helpers; `parse` always calls `validate`. The separate
`tests/qualification_cli.rs::tests::fixture` duplicates the same trillion-byte
ceiling and 1,000,000-byte headroom. Their new native floor is
**62,500,000,000 bytes**, not 256 MiB, so wiring the floor before fixture
migration prevents many tests from reaching their own assertions.

First inventory the shared and derived consumers and record the actual baseline
test count. After adding the floor and its failing controls, run both complete
qualification targets below **before editing either fixture**. Record actual
passing/failing counts and names, distinguishing new policy refusals from
unrelated failures; source-reference counts are not failing-test counts. Then
change each shared fixture once to a realistic independently fitting ceiling
and policy-compliant headroom, justified against its unchanged charged
workload. Reconcile every dependent test, including report/identity/debug
derivations, rather than patching just the names below. Do not mechanically
select 62.5-GB headroom or edit historical captures. Retain a trillion-byte
declaration only in an intentional large-value control.

The shared fixture's replacement configured ceiling must be **at least
4,000,000,000 bytes**, not merely the independently fitting minimum. Its
platform ceiling must be **at least 5,000,000,000 bytes**: Row 4 installs a
4,000,000,000-byte cap and retains a 5,000,000,000-byte scenario peak limit.
Preserve `configured <= platform <= hardware` and policy-compliant headroom
for the baseline and full-evaluator/CLI overrides; the defensive helper sweeps
below remain separate. A smaller platform declaration
would reject at parse before Row 4 reaches accounting, even if the shared
baseline otherwise fits.

Handle these four ceiling-overriding `evaluate.rs` tests separately:

| Existing test                                                                  | Required migration                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| ------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `qualification_accounting_uses_the_smaller_of_independent_memory_ceilings`     | Recompute the required boundary after the shared fixture changes; keep unequal ceilings in both orders and exact/under/over accounting assertions.                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `qualification_measured_peak_and_headroom_use_both_independent_ceilings`       | Its 1,000,100/2,000,000-byte ceilings cannot fit native's 268,435,456-byte floor. Replace this geometry, not just its headroom literal. Use native ceilings 4,294,967,296 and 8,589,934,592 with headroom 268,435,456; the smaller ceiling leaves exactly 4,026,531,840 bytes for measured peak. Sweep 4,026,531,839/4,026,531,840/4,026,531,841 and overflow with unrelated peak/growth limits nonbinding.                                                                                                                                                                                           |
| `qualification_accounted_ceiling_and_headroom_are_independent_of_measurements` | In the 500,000,000-byte cap case select 268,435,456-byte headroom to satisfy the new policy and reach accounting. Demand already exceeds this cap on total alone; the headroom choice does not determine that failure. Keep the separate enlarged-headroom case independently failing with otherwise fitting accounting/evidence. Both must reach accounting rather than failing at parse.                                                                                                                                                                                                            |
| `qualification_measured_peaks_cannot_bypass_a_smaller_installed_ceiling`       | Unconditionally rebase this boundary: installed cap 4,000,000,000, selected headroom exactly 268,435,456, inherited platform at least 5,000,000,000, and nonbinding scenario peak limit 5,000,000,000. Assert accounted fit first; use measured peaks 3,731,564,543/3,731,564,544/3,731,564,545 for pass/pass/fail, with the failure reporting `peak_exceeded`. Their sums with headroom are cap-minus-one/cap/cap-plus-one. This repair is required even when the fixture's accounted demand already fits; a later cap/headroom rebase must update all three independent boundary literals together. |

For Row 4's full-evaluator controls, put the elevated peak at the final
scheduled scenario/repetition of its process, not the first evidence row.
Otherwise later 1,000,000-byte observations violate the process-lifetime
high-water contract and hide the intended boundary. Refresh the changed
profile/debug references and observation bindings; require otherwise passing
complete evidence, accounted fit in all three cases, and the exact selected
scenario result as well as overall local acceptance.

Low-level `account`/`check_memory` sweeps may construct both ceiling orders
directly, including the order prohibited by `Profile::validate`'s
configured-not-above-platform rule. These are defensive helper controls, not
accepted-profile evidence; preserve that validator. Actual command/evaluator
controls must validate their profile and reach their original assertion with
exact outcomes/reasons. In particular a new parse rejection cannot substitute
for a measured or accounted failure. Historical captures remain byte-identical;
handle their current-policy rejection explicitly, without a legacy exception.

Run each command separately at baseline, at the new-policy/old-fixture red
checkpoint, and after migration, retaining each complete result:

```bash
cargo test --locked -p auctioneer-cli --bin auctioneer-cli --no-fail-fast qualification::
cargo test --locked -p auctioneer-cli --test qualification_cli --no-fail-fast
```

- [ ] Read the current raw declarations and actual derivation; do not copy the older golden subtotals as exact totals. Record `git status`, source/lock/SDK/toolchain/features and target ABI. Keep unrelated edits untouched.
- [ ] Add failing report controls for missing target rows, omitted positive terms, raw zero caps, zero baked bytes and inconsistent source/profile identity. Use independent expectations for the three effective caps and baked tuple count, not a report comparing itself to itself.
- [ ] Replace the private array-only summation boundary with runtime-owned `IsolateTerms`, preserving every operand and checked addition. Its checked fold and report adapters destructure exhaustively without `..`; audit term IDs/count come from this owner, not another literal eleven. An isolated added-field compile control must fail until the fold/export/audit mapping is updated. Keep zero C/I rows and existing independent omission/overflow controls.
- [ ] Add failing runtime-policy, qualification unit and actual-command controls for spec sections 3.1-3.2 before implementing them. Isolate each condition: a below-floor profile must refuse through both `qualify validate` and `qualify evaluate` even with otherwise passing observations; missing/excessive stack must remain nonpassing with primary demand fitting and all other SDK facts known in the adapter's test fixture.
- [ ] Add the one pure `minimum_headroom_bytes`/`HeadroomClass` policy in runtime `budget.rs`. Resource checking and qualification `Profile::validate` consume it. Reject below-floor declarations rather than clamping; both accounted and measured qualification checks retain an accepted larger declaration. Run and record the complete new-policy/old-fixture red checkpoint before changing shared fixtures, then perform the dependency-wide migration and four explicit ceiling-test repairs above. Preserve their independent assertions, historical captures and strict policy; no legacy bypasses.
- [ ] Complete/reuse M7 Task 4's bounded Rust report/checker. Add `--require-fit` to the existing planned `bidders resources --check --all-adapters` command, with the same inputs/report and no second calculator. Ordinary relative checks retain blocked absolute results; required-fit mode refuses any failed/indeterminate platform or stack validation, failed primary/configured-ceiling/headroom check, or missing target row. Capture raw/normalized rows, the complete runtime-owned term set, six auction receipts, V/K ownership, construction/catalog descriptors, effective ceiling, floor/selected headroom, stack allowance/requirement/proof identity and SDK validation status/reasons for four targets. Separately report local numeric failure and external-fact blockers, including both when SDK `Exceeds` short-circuits reason gathering. Unknown platform fit/headroom/gap remains pending; the independent configured-cap comparison must still report any detected local overflow. Keep metadata safe and bounded.
- [ ] Implement the spec's Auctioneer-to-`PlatformMetadata::validate_memory_envelope` mapping in the CLI resource owner. First verify the budget's pre-admission population covers the selected SDK bound at the same scope. Pass checked `total + selected_headroom` once as fixed bytes, zero as per-live-request bytes and the reviewed scope; retain the independent configured-ceiling comparison. Require justified nonzero stack bounds where separately capped. Preserve `Exceeds`, every `Indeterminate` reason and errors as nonpassing; a primary pass cannot erase a stack failure. No new SDK API or alternative calculator is needed.
- [ ] Reconcile spec section 2's complete per-target expressions: Cloudflare `138,047,236,632 + 6*L_t + 768*b`, Fastly/Spin `23,589,436,420 + L_t + 128*b`. Measure the actual ABI-bound construction descriptor and count effective baked bytes; do not present older subtotals or native layouts as exact WASM totals. Preserve the smaller config/parse controls as component checks only. Retain over-ceiling expectations until an approved replacement fits.
- [ ] Write bounded typed `audit.toml` and implement spec section 4.1's required-field/term/target/owner completeness checks in the existing CLI owner. Generate `docs/testing/adapter-resource-envelopes.md` tables from it. Native's existing headroom evidence is a test assertion; the shared strict policy is new, not an existing production refusal.
- [ ] Add failing review-mode controls in `bidders_resources`, then implement `--check --review-commit <sha>` in `resources/review.rs`. Read both frozen records from the selected Git tree, check audit digest/source continuity, and bound input/output through the existing CLI policy. `--staged` covers committed review-to-HEAD changes plus the index; `--head <sha>` covers the complete committed diff. The checker validates records/scope, not human authorization, and does not execute recorded test commands.
- [ ] Require negative controls for omitted Q/A or a zero-valued term, a greater-than-5% term with no decision, missing owner/ABI evidence, double-counted overlapping savings, unjustified residual gap, an undeclared file, added/deleted/renamed paths, changed frozen records, stale baseline and an invalid/non-ancestor review commit. Independently test exact 5% versus greater-than-5% selection. A small unrelated saving cannot satisfy a full-package decision.
- [ ] Run the commands below; verify expected target/case counts and separate relative/absolute dispositions. Also run `cargo run --locked -p auctioneer-cli -- bidders resources --check --all-adapters --require-fit`; require nonzero on the unchanged over-ceiling baseline and assert each failed/unknown row rather than accepting any unrelated parser error. Commit baseline/tooling and strict-policy validation only; no allocation, workload, admission or boot changes.

```bash
cargo test --locked -p auctioneer-runtime --lib
cargo test --locked -p auctioneer-core --test catalog_resource_profile
cargo test --locked -p auctioneer-cli --test bidders_resources
cargo test --locked -p auctioneer-cli --bin auctioneer-cli qualification::
cargo test --locked -p auctioneer-cli --test qualification_cli
cargo test --locked -p auctioneer-adapter-axum --test boot_envelope
cargo test --locked -p auctioneer-adapter-cloudflare --test boot_envelope
cargo test --locked -p auctioneer-adapter-fastly --test boot_envelope
cargo test --locked -p auctioneer-adapter-spin --test boot_envelope
```

The new target names above are planned M7 targets, not existing passing tests.
Their initial red state must be an exercised assertion, not a zero-test filter.
Provider `boot_envelope` targets execute host-side declarations, not WASM code.

## Task 2: Audit Allocation Lifetimes And External Facts

Deployment decision (AG, explicit user reply, 2026-10-18): target **Akamai
Functions hosted Spin**, not generic self-hosted Spin. This selects the
intended contract only. Shared-profile dispatch metadata now selects
`AKAMAI_FUNCTIONS_PLATFORM`, removing the generic/hosted mismatch. Artifact
proof and missing provider facts remain unresolved; reporting still refuses
fit and does not treat that metadata selection as a hosted observation.

**Files:** runtime/core/exchange/bidder owners in the table; privacy
`tests/{privacy_resource_limits,gvl_retained_charge}.rs`; provider entrypoints,
envelopes and pinned SDK sources; human audit.

- [ ] Trace each term from acquisition to final drop, including asynchronous suspension, config failure/recovery, cancellation and egress handoff. Record the actual allocator/storage owner and memory scope; distinguish distinct snapshots/parsed lists from shared handles.
- [ ] Audit config extraction raw/value/tree/baked copies, auxiliary/probe extraction overlap, response parse scratch versus retained bids/debug output, privacy projections and registry initialization. Record independent byte/node bounds and all live populations before suggesting a smaller coefficient.
- [ ] Trace `AdmissionGate`, `StreamGate`, auxiliary/probe-config and terminal/static/probe-output gates to their real acquisition/release points. Explicitly test the claim that stream width does not bound retained completed results.
- [x] Add the non-installing `response_retention` allocation control: three unequal stream-width/call-count/body-size rows, each through normal completion and second-wave cancellation, retain earlier response bytes across waves and release every body at the owning drop. This proves an ownership boundary; it authorizes no smaller charge.
- [x] Exercise actual SDK extraction before boot, on the same App after boot and on a new postboot App. Three bounded-read receipts and a payload between independent node caps prove that the original App retains defaults and the new App enforces installed caps. This proves the snapshot lifecycle, not a typed-conversion/merge allocation upper bound or a repaired preboot allowance.
- [ ] Classify SDK memory facts as enforced, externally declared or unknown. For Cloudflare, do not use six outbound connections as an ingress population. For Spin trace the current hosted envelope versus generic `run_app` metadata and freeze the explicit dispatch/envelope/deployment alignment required at the checkpoint. Do not relabel a generic deployment hosted. Record native substitutions, Fastly's separate stack and unresolved host/parser/handoff memory separately.
- [ ] Record the current external blockers explicitly: host-ingress/framing accounting on all four targets, Axum ceiling/scope/population, Cloudflare inbound population, and generic Spin facts if selected. EdgeZero adapter maintainers own the publishing/enforcement contract; AG owns actual operator declarations. Give each a next verification action. No Auctioneer-only package promises a passing aggregate command at this SDK pin; a later dependency update must exercise all four mappings again.
- [ ] Produce explicit native-versus-WASM ABI rows for all affected layouts/owners, separating guest allocations from host storage. Record the actual target artifact/toolchain and proof identity; missing guest evidence is pending, never filled from a native `size_of` run.
- [ ] Assign AG the Spin deployment-profile decision and the Auctioneer Spin maintainer its dispatch/proof patch. Task 3 must retain the Spin row and name either the verified hosted selection or an explicitly configured generic profile, with owner/next evidence action if blocked. Generic/unknown does not remove the obligation.
- [ ] Establish a justified Fastly separate-stack upper bound covering cold initialization, admitted parser depth, validation, error and cancellation paths. Record artifact/ABI/toolchain and proof identity. Missing proof remains pending; a native stack observation or setting the requirement to the published 1,000,000-byte allowance is not a derivation. Keep targets with no separately published allowance at `None` while auditing their stack's primary-memory ownership.
- [ ] Define a bounded target-execution/measurement method for each proposed reduced bound. It must identify guest ABI, artifact, meter scope and overlap. Missing hosted meters stay pending and are not replaced by native RSS. Do not contact or mutate a live deployment in this task.
- [ ] Write separate Auctioneer changes and exact EdgeZero/provider requests, each with enforcement point, expected contract and negative acceptance control. No new SDK interface is assumed implemented merely because requested.
- [x] Specify the current external contract acceptance controls in spec section 5.4, separating EdgeZero fact publication/enforcement from Auctioneer's hosted Spin alignment and Fastly application stack proof. Rechecked PR #275: the head still matches the pinned `f6c708cc`; no dependency update is available to resolve these facts.
- [ ] Review the audit for double charging and undercharging. Preserve all current allowances until the owning proof/change is approved. Commit the source-grounded audit.

## Task 3: Freeze A Feasible Patch List

The frozen approval/window steps in this task apply to changes outside the
execution authorization. AG's 2026-10-18 replies allow verified unchanged-workload
implementation improvements and independently proved accounting corrections
to proceed directly through Task 4's scoped TDD
and review sequence; do not manufacture an approved review SHA for them.

**Files:** this spec/plan; human audit; frozen `remediation-plan.toml` and its
referenced `audit.toml`; candidate declarations only after approval.

- [ ] Generate the per-target decision table from the audit and classify each row: implementation improvement, demonstrated accounting correction, workload/profile proposal or external prerequisite. Cover every term and Q/A subcomponent above 5% of exact normalized isolate demand, and every smaller term's disposition. Name exact functions/files, old/new lifetimes/populations, proof controls and remaining gap; Q/A cannot disappear behind smaller improvements.
- [ ] Reconcile the declared-ceiling numeric lower-bound gaps first: Cloudflare 137,927,236,632 bytes, Fastly 23,469,436,420 and Spin hosted 23,463,607,300, before positive layout/baked terms. Present explicit class-3 old/new workload/profile proposals for all three targets now, alongside any evidenced unchanged-workload alternative. No such alternative is justified at the planning baseline; do not postpone the workload discussion until after failed optimization attempts.
- [ ] Freeze ordered before/after changes and aggregate them against each target's full checked fit gap. Reject overlapping savings counted twice. A full numeric package must project a closed gap, without implying external facts are supplied. Partial work is the expected first disposition and still requires AG's explicit approval with residual bytes, dominant unresolved terms, owner and next action. Projected fit is not measured acceptance.
- [ ] For each concrete implementation improvement, append an ordered subtask to Task 4 with exact files and red test/command before runtime work begins. Do not treat a broad instruction to optimize as an executable patch plan.
- [ ] Obtain the user's decision on any workload proposals, including explicit old/new numeric limits, impacted accepted requests and deployment scope. Wait for approval before any class-3 implementation; amend the spec, profile and downstream acceptance together. Unchanged-workload improvements already have execution authorization, but retain any residual gap rather than assuming they close it. No automatic cap selection or lowered bidder/call/node workload.
- [ ] Keep the additive equation. A proposed phase-aware alternative requires a separate reviewed amendment with enforced separation and changes to all consumers, not just strict checking.
- [ ] Commit the concrete spec/plan amendments first, then select code-base B and bind/rebind the audit to it. Only afterward create the records-only review commit D; do not edit the plan inside the B-to-D interval or silently retain a stale source identity.
- [ ] Commit both typed records as a review checkpoint containing no allocation/workload/admission changes. Freeze exact repository-relative paths for implementation, tests, workflows, human-audit and evidence updates, plus proof commands, code-base commit and audit digest; broad ownership paths are not grants. Ask AG to approve this exact immutable review commit through a PR review or explicit user message. Do not invent approval or use a self-recorded boolean.
- [ ] Record AG's actual author/date/external approval reference in the human audit. Verify that external record before Task 4; no self-approval or automatic scope expansion. A rejected/unresolved target stays blocked. The scope checker alone does not authenticate AG or authorize implementation.
- [ ] Bind the required CI head-diff check to the AG-approved review commit before pushing Task 4 work. Its workflow path must itself appear in the frozen list. Keep the audit and plan immutable during the implementation window; scope/baseline changes require a new checkpoint and AG approval. The milestone is not complete at this checkpoint.
- [ ] Mark the R0 package window active and serialize its commits on the shared M7 branch. Do not interleave unrelated M7 files and then filter them out of D-to-HEAD. Define the planned closing workflow/human-audit update in the exact-path list; later M7 work resumes after AG approves the package endpoint and closing binding.

## Task 4: Implement Verified Owner Changes

**Files:** explicitly named owner paths and matching existing allocator,
handler/receipt and concurrency integration tests. Use Task 3's frozen paths
when implementing a separately approved limit/model package.

Repeat this small TDD sequence for each authorized patch, never in one bulk
rewrite. Append the concrete patch/test scope before production edits.

Current unchanged-workload scope:

- [x] Make info/health JSON output storage non-growing in
      `auctioneer-core/src/serialize.rs` and `handlers/{info_bidders,health_gvl}.rs`.
      Count exact borrowed serialized size, convert fallibly, and reuse the existing
      reserved `CappedWriter` path; impose no new product cap. Preserve successful
      wire bytes and existing error dispositions. Add independent allocation and
      escaped-value controls, including a serializer whose second pass grows or
      fails, with unit coverage beside the owner and the existing allocation test
      target where applicable. This prerequisite alone neither proves every route's
      response/header backing nor authorizes any smaller isolate term.
- [x] Detach accepted CORS origins from inbound shared backing in
      `auctioneer-core/src/handlers/sync_http.rs`. Copy the exact matched configured
      origin into response-owned `HeaderValue` storage rather than cloning the
      request's possibly sliced `Bytes`. First require a real shared-header fixture
      to retain its original buffer, then assert normal/setuid/preflight responses
      preserve their wire value without sharing either small or large inbound
      backing. Keep duplicate/invalid/denied origins refused. This changes ownership,
      not allowed origins, cookie policy, output caps or the runtime equation.
- [x] Correct auxiliary config-node units in `auctioneer-runtime/src/{budget,aux_work}.rs`
      and the directly affected core/runtime tests. First independently vary SDK
      value-node count, config byte caps, body nodes and cookie nodes. Require the
      auxiliary config allowance to use the same 623-byte-per-SDK-value bound as
      auction config, with real extraction and allocation controls retained.
      Represent config values explicitly rather than dividing that allowance by
      the unrelated 384-byte syntactic-node cost. Preserve byte/baked conversion
      terms and all input limits. Reproduce the receipt delta separately: no
      isolate savings while the declared auxiliary slot remains unchanged.
- [ ] Before crediting any auxiliary-handoff split, close or explicitly refuse
      unfunded preboot/stale-App extraction contexts and prove an all-route output
      bound, including liveness, recovery, cancellation and errors. Record any
      behavior-contract decision separately; do not infer approval from low usage.
- [x] Enforce AG's October 18 unfunded-context decision in
      `auctioneer-core/src/config/runtime.rs`, with unit tests alongside it and a new
      process-isolated `tests/config_funding.rs` real-App integration target.
      Require installed `BootGates` and compare the three captured SDK byte/value
      caps with its normalized limits before `AppConfig::from_store`. No store
      read may occur on refusal; keep the no-binding policy distinct. Add the
      fixed `unfunded_extraction` diagnostic and bounded 500/no-retry response;
      `/status` remains live and degraded. Independently exceed backend, blob and
      value caps, include equality and narrower contexts, and exercise old-App
      refusal after boot plus new-App recovery, cancellation and repeated liveness.
      Record the baseline/failing test counts before updating existing test-only
      contexts to install their actual funding. Such fixture migration must not
      bypass the resolver guard, change production caps, or alter raw SDK lifecycle
      controls that deliberately test `AppConfig` directly. Re-run all core targets
      and the four adapter consumers; document remaining output/ABI proofs.
- [x] Complete the existing pure/consumer proof coverage in
      `auctioneer-core/tests/catalog_resource_profile.rs` and CLI resource-owner unit
      tests. Exercise dominant auction demand with nonzero initialization,
      auxiliary zero/minimum and unequal probe populations, unequal call/stream
      populations, checked overflow and exact ceiling boundaries without installing
      gates. Mutate every named term out of reports, including zero-valued catalog
      entries, and require refusal. These are guards on the one existing owner,
      not a second calculator or physical measurement.
- [x] Add three explicit source-resource commands to `test.yml`: clean-source
      audit capture into runner-temporary storage, current-source validation of
      that capture, and strict report generation with exit status reconciled to
      its `required_fit` rows. Pin exact commands, order and unconditional execution
      in the existing semantic `ci_workflows` guard; demonstrate deletion,
      reordering, `if: false`, failure suppression and status-check weakening are
      rejected. Do not require a permanent refusal after a future legitimate fit,
      rewrite committed baseline captures, or label a catalog-relative gate as
      implemented by these source-completeness checks.
- [x] Repair and verify the resource-command review findings before that slice
      is pushed: ignore only the two CI reference checkout trees in `.gitignore`,
      exercise their real nested-Git layout in `auctioneer-cli/resource_identity.rs`,
      and retain refusal of unknown nonfile source inputs. In `test.yml` and its
      `ci_workflows` controls, require exactly one top-level strict report and
      execute positive/negative shell cases, including an invalid report followed
      by a valid one. Do not weaken source ownership or status/fit reconciliation.

- [x] Hosted Spin dispatch alignment: `auctioneer-adapter-spin/src/envelope.rs`
      and directly affected adapter/CLI tests. First require the actual dispatch
      metadata to equal `AKAMAI_FUNCTIONS_PLATFORM`; then align the exported profile
      used by the existing entrypoint. Preserve all limits and accounting terms,
      test that substituted generic metadata still fails, and retain unknown
      host-ingress/fit states. A matching constant is not hosted execution evidence.
- [x] Release the extracted runtime overlay after account/host/plan resolution:
      `auctioneer-core/src/handlers/auction.rs` and
      `auctioneer-core/tests/auction_config_retention.rs`. First measure live allocations
      at a suspended generated-App fanout while independently varying an unselected
      account's vendor list. Require parsing work to grow as a positive control,
      but not the held typed overlay; retain one config read, selected-account
      outputs and normal/cancel cleanup. Then drop only the no-longer-used
      `app_cfg` before constructing the auction context. Keep the full sanitized
      bidder request for `make_bids`, all config caps and every accounting charge.

- [ ] Add the named failing owner test and execute its exact target. Require actual work/allocations, independent expected bounds, at least two sizes and unequal populations. Include the failing-refresh/cancellation/retention overlap that motivates the change.
- [ ] Implement the smallest real ownership or enforcement change. Keep request/config/secret consistency, privacy, full bid/media/error semantics and all refusal/probe paths. Do not lower a coefficient until the changed path's bound is established.
- [ ] Add target-applicable layout/work proof and execute the actual WASM path where available. Native allocator tests alone do not validate 32-bit guest or provider-owned storage. Mark unavailable observations pending.
- [ ] Apply only independently proved accounting corrections within AG's authorization to the shared accounting owner; otherwise obtain specific approval first. Re-run relevant receipt/stream/aux/egress controls; do not replace call count by stream width for retained results, erase evicted pinned GVL versions or combine independent auxiliary/probe permits.
- [ ] Remove the new guard/change temporarily in an isolated scratch copy, verify the intended assertion fails and restore it. Never run mutations against another review's active worktree.
- [ ] Review exact staged paths before each commit, preserving unrelated user edits. For a frozen limit/model package, additionally run the staged scope check including added/deleted paths and both rename sides; refuse undeclared changes or an edited review record.
- [ ] Reproduce unchanged terms or the specifically approved term delta and residual gap; check native fit and M7 catalog allowances independently. For a frozen package run the review-to-HEAD scope check before pushing. Push each verified patch and preserve earlier baselines and partial/blocked dispositions.

Frozen-package scope commands use the review commit actually approved by AG, not an invented
placeholder approval or a commit chosen by the implementer:

```bash
cargo run --locked -p auctioneer-cli -- bidders resources --check --all-adapters --review-commit "$AG_APPROVED_REVIEW_COMMIT" --staged
cargo run --locked -p auctioneer-cli -- bidders resources --check --all-adapters --review-commit "$AG_APPROVED_REVIEW_COMMIT" --head HEAD
```

Relevant existing integration owners include runtime `tests/auction_budget.rs`;
core `tests/{config_peak_charge,config_envelope_value_factor,config_parse_factor,auction_budget_concurrency,usersync_handler_alloc,usersync_resources,response_serialization}.rs`;
exchange `tests/{response_charge,response_factors,node_charge,projection_charge,projection_scrub_bound,finalize_charge,bidder_response_node_budget,debug_allocation_bound}.rs`.
Select the exact affected targets in the checkpoint; do not claim running this
list without executing it. Keep unrelated refactors out.

## Task 5: Prove Real Boot/Strict Parity And Fit

**Files:** runtime `budget.rs`; core `host_primitives/gates.rs`; M7-owned pure
and child-process targets; CLI resource checker, qualification
`{profile,evaluate,report}.rs` and `tests/{bidders_resources,qualification_cli}.rs`;
four provider profiles.

- [x] Reuse M7's single-test `boot_strict_parity` process contract. Count bounded children, assert pre-install empty state and exercise actual boot installation once per process. Never reset or serially reuse `BOOT_GATES`, `BAKED_CONFIG_BYTES`, `PROBE_EGRESS_GATE` or `OVER_CEILING_WARNED`.
- [x] Compare retained boot inputs/terms with the real CLI strict command for raw all-zero caps, each independently zero, unequal explicit caps and independent nonzero baked bytes. The complete baked tuple, eleven terms, six receipts and full derivation must match; scratch-only normalization/baked-count corruptions refuse. Production acceptance still needs the approved replacement profiles and target evidence below.
- [ ] Keep `catalog_resource_profile` non-installing. Independently vary the complete runtime-owned term set, V/K, dominant auction work with nonzero I, auxiliary/probe minima, stream width and call count. Confirm audit/export IDs match the owner and the added-field control rejects an omitted new term. Check overflow and missing scope facts without substituting zero or synthetic platform ceilings.
- [ ] Pin WASM headroom at `8,000,000/8,000,000/8,388,608` for current ceilings; native at least `268,435,456`. Independently pin `Wasm, c = 100,000,001 -> 6,250,001` in the helper and both CLI owners using WASM-target profiles; native's fixed floor would hide rounding at that ceiling. Also pin `Native, c = 4,294,967,297 -> 268,435,457`. Floor-minus-one rejects, floor accepts when demand fits, and larger declared headroom remains binding in both accounted and measured checks. Include unequal configured/platform ceilings, checked-add overflow and exact/under/over primary-fit boundaries. Independent expected literals must detect removal of rounding or bypassing the policy at either caller; real divisible provider ceilings alone are insufficient.
- [ ] Exercise the actual SDK adapter with known, source-scoped test facts and an independently fitting primary total: missing/zero/unproved stack requirements, proved bounds at `999,999/1,000,000/1,000,001`, unexpected stack shape, scope mismatch and each unknown host/population fact. A per-instance population above one must preserve the already-derived total exactly once; an under-declared pre-admission population must refuse independently. Assert exact result/reason, and prove removal of the SDK call or stack-result handling fails its owning control. No test fixture grants real platform qualification.
- [ ] Independently exercise/report numeric failure alone, external-fact blockers alone, and both together, plus an all-known fitting control. With an unknown platform ceiling and fixed known declared headroom, place demand-plus-headroom below, at and above the configured cap: overflow is still a Auctioneer failure while the SDK remains `Indeterminate`; under-cap arithmetic cannot prove platform fit. Keep that configured comparison separate from pending effective-platform headroom/gap. Known-ceiling demand fit with unknown ingress remains nonpassing. Actual native unknown ceiling/population stays pending, not filled from configured limits; SDK `Exceeds` must not erase the separately inventoried external blockers.
- [ ] Keep strict acceptance separate from boot's existing warn-and-install policy. Update each provider's over-ceiling golden test only when its actual normalized replacement row fits; preserve historical failure records.
- [ ] Rebase M7's baseline/candidate reports together under each approved R0 accounting/profile change, preserving old reports and exact catalog deltas. Verify neither unrelated savings nor an `UNCHANGED` catalog descriptor hides catalog allocations.
- [ ] Repeat Task 1's runtime/qualification/CLI/four provider commands, then run `cargo test --locked -p auctioneer-core --test boot_strict_parity` and `cargo test --locked -p auctioneer-core --test catalog_resource_profile` separately. Run `cargo run --locked -p auctioneer-cli -- bidders resources --check --all-adapters --require-fit`; only four complete proved/fitting rows, including required stacks and headroom, permit exit zero. At the current pin the unpublished external facts preclude that outcome regardless of local numeric repair. Record completed Auctioneer work, numeric fit and each external blocker separately, retaining the aggregate nonzero exit without calling it a passing gate or global R0 completion.

## Task 6: Target Execution, CI And Handoff

**Files:** existing provider workflows and their colocated scripts;
approved target-observation records; human audit; `ISSUES.md`, roadmap, M7 docs.

- [x] Execute the unchanged actual CI toolchain/build/lint matrix: Cloudflare worker build/dry-run, Fastly wasm32-wasip1 guest/package build, Spin wasm32-wasip2 component build. Retain existing upload/package/component ceilings and profile settings. Host-side envelope tests do not substitute for this matrix. The [storage verification](../../testing/evidence/auxiliary-storage-controls-20261018.md) records all three real package builds, all-target WASM Clippy rows, guest conformance links and unchanged size ceilings. This is not execution of every workflow job, runtime observation or physical qualification.
- [ ] Wire new required pure/process/CLI controls into existing workflows. For a frozen limit/model package also wire its approved-review-to-HEAD check. Extend semantic gates to pin commands/features, applicable approved review bindings, non-disableable measurement steps, shell failure propagation and triggers; demonstrate deletion/`if: false`/raised-budget mutations fail. A missing binding refuses that package, not unrelated authorized unchanged-workload improvements.
- [ ] Execute the approved local target-applicable observation method with declared workload/overlap and artifact/meter identity. Inspect guest/host scope and bound exceedances. Unknown population, missing meter or unexecuted target path remains pending; do not fabricate or substitute a native result.
- [ ] Preserve resource identity and evaluator fail-closed behavior, including the shared headroom floor in qualification validation and its accepted value in accounted/measured checks. SDK platform/stack proof fields belong to the new resource report; do not silently default them into schema-2 historical captures. If any existing wire contract changes, perform the spec's explicit version cutover. Add boundary tests for missing/stale evidence and a failing row that successful siblings cannot promote. Leave historical captures unchanged.
- [ ] Run final formatting and strict workspace tests/lints under secure defaults and the existing explicit insecure/conformance lanes; use the repository's current dependency/archive prerequisites. Record infrastructure/disk/credential failures as such, not passing tests.
- [ ] Review the complete diff, applicable frozen-package scope result and actual versus projected terms/full gap. Update accounted fit, allocation bounds and observations separately. Verified Auctioneer work may be complete with external prerequisites pending; that does not close its target or R0. R0 completion needs all three WASM targets, and aggregate exit zero also needs all four SDK fact sets including native; otherwise record the exact nonzero reason, owner and next action.
- [ ] Close any opened frozen limit/model package window, even if target readiness remains partial: obtain AG's approval of endpoint H and the closing CI-binding change after D-to-H passes. That change updates only the approved workflow binding and human audit. Keep CI checking explicit frozen D-to-H thereafter, not every future M7 HEAD. Authorized implementation-only work creates no such window or closing-approval requirement. Window closure grants no fit claim for later artifacts.
- [ ] Link the approved profiles/artifacts/reports and unresolved facts into M7.6's existing runbooks. Credentialed hosted-handler execution, corrupt-config injection/recovery, actual deployment Q2-D and publisher readiness stay unexecuted until separately authorized.
- [ ] Update `ISSUES.md`, roadmap and M7 cross-references. Commit and push verified changes to the same M7 PR; no claim that documentation alone repaired WASM fit.

## Completion Record

Check boxes record executed work, not completion by documentation. Concrete
owner patches require their own red/green evidence; limit/model changes also
require the specific review checkpoint above. Report
blocked targets honestly; never close the program by deleting a charge, weakening
a workload or promoting unavailable measurements into evidence.

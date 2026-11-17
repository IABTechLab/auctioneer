# Recipient-Scoped Bidder Roots And Measurements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix #176 by selecting recipient-specific root extensions before copying, then close #182 with measurements of that corrected production path and an explicit optimization recommendation.

**Architecture:** A private exchange root builder borrows the shared grouped request and accepted recipient identity; existing survivor selection, impression supplementation, validation, scrubbing and resource admission stay authoritative. Native measurement uses the same production stages through the existing test-support feature and the shared fake bidder. The privacy correction is independently deliverable; measurement is not a gate on its landing.

**Tech Stack:** Existing Rust workspace, Serde/serde_json, validator-backed request validation, bounded bidder sink, runtime test-support counting allocator, harness-less Cargo benchmark and the existing Linux GitHub Actions test workflow. No new dependency, crate, feature, allocator implementation or configuration interface.

---

## Contract And Execution Gates

- Approved [specification](../specs/2026-11-16-bidder-root-projection-design.md), issues [#176](https://github.com/IABTechLab/auctioneer/issues/176) and [#182](https://github.com/IABTechLab/auctioneer/issues/182). Inspected source base: `86fe367b9fb91ed2123fdaedb512cd19a5ca65b6`. Recheck changed owners after the planning PR merges; do not execute against a stale source tree.
- This is a plan, not implementation authorization. Publish the spec/plan together only after user authorization. Implementation starts in a separate owned worktree/PR after planning review, merge and implementation approval. No issue closes on the planning PR.
- Per the user's instruction, **do not run Cargo, benchmarks, formatter, Clippy or CI scripts locally**. Commands below are prospective **hosted** commands, not executed evidence. The authoritative full workflow matrix remains mandatory. Read-only inspection and Git diff hygiene are permitted locally.
- Use @superpowers:test-driven-development for implementation and @superpowers:verification-before-completion before claiming a fix. For tests-first evidence, publish an authorized tests-only implementation checkpoint, observe the intended hosted assertion failure, then apply the production fix. A missing prerequisite, zero selected tests, compile error or unrelated failure is not the required red.
- Hosted mutation execution is defined below; writing this plan does not authorize experimental branches/PRs. Obtain one explicit batch authorization for the twelve listed mutants before running them. Local mutation runs remain prohibited unless the user separately changes that instruction.
- Tasks 1-4 deliver the security correction. They can merge as an accepted implementation PR while Tasks 5-9 remain open under #182. Do not wait for a benchmark, release build or optimization decision to land the accepted #176 correction. Every code delivery still needs its own complete hosted matrix and review.
- Leave `projection_charge`, `TYPED_CLONE_FACTOR`, workload limits, resource descriptors, additive accounting, SDK pins and guest eligibility unchanged. A measured reduction is not a new production bound. Any required equation, allowance, dependency or workload change returns to review.

## File And Test Ownership

| Path | Responsibility/change |
| --- | --- |
| `crates/auctioneer-exchange/src/split/mod.rs` | Wire root projection; preserve grouping/impression semantics; maintain independent inline oracles; later extract the exact existing stages for measurement. |
| `crates/auctioneer-exchange/src/split/root.rs` (new) | Private select-before-copy root builder and inline field/presence/allocation units. No bidder-specific switch. |
| `crates/auctioneer-exchange/src/split/measurement.rs` (new, native test-support only) | Observe shared production stages, not a second projection implementation. |
| `crates/auctioneer-exchange/tests/privacy/grouped.rs` | Existing cross-module recipient, selection, parameter-shape and GDPR/activity wire cases through the shared fake. |
| `crates/auctioneer-exchange/tests/privacy/grouped_snapshot.json` | Only the approved outbound root changes; response and other paths remain unchanged. |
| `crates/auctioneer-exchange/tests/auction_support/mod.rs` | Reuse the existing harness; extend only reusable fixture/observation support if needed. |
| `crates/auctioneer-bidder/src/bidders/pubmatic/{tests/conformance.rs,parameters.rs}` | Reuse existing real wrapper capture, valid outgoing acat unit and malformed-root units through their current driver. No duplicate consumer test or real bidder fixture/factory in exchange tests. |
| `crates/auctioneer-openrtb/src/ext/request_prebid.rs` | Clarify inbound representation versus bidder forwarding; no model/schema change. |
| `docs/superpowers/specs/2026-05-21-auctioneer-openrtb-server-design.md` | Update the root extension row, split steps 7/9, deferred-FPD prose and divergence row to match the approved correction. |
| `crates/auctioneer-exchange/src/test_support/mod.rs` | Native, feature-gated measurement API reexport and allocator calibration units. |
| `crates/auctioneer-exchange/src/test_support/materialization/{mod.rs,fixtures.rs,report.rs}` (new) | Runner setup/modes, fictional matrix and typed reports respectively; inline unit tests at each owner. |
| `crates/auctioneer-runtime/src/test_support/alloc_track.rs` | Add monotonic allocation/reallocation call counters without changing existing byte/live/peak semantics. |
| `crates/auctioneer-exchange/benches/request_materialization.rs` (new), `crates/auctioneer-exchange/Cargo.toml` | Small harness-less entrypoint using existing dev-dependencies/features and the existing counting allocator. |
| `.github/workflows/test.yml`, `.github/workflows/scripts/test/measure-materialization.sh` (new) | Bounded smoke in the existing secure lane; explicitly requested full measurement in that same lane only. No new runner/job/compiler lane. |
| `docs/testing/development.md` | Exact smoke/full-mode invocation, ownership, reporting limitations and follow-up procedure. |
| `docs/testing/evidence/bidder-materialization/` (new, Task 9 only) | Raw hosted timing/allocation reports, source/checksum manifest and recommendation; never fabricated during implementation. |

Keep ordinary module declarations. Unit tests stay inline; conformance stays with the existing domain target. Do not introduce handwritten `#[path]`/`include!`, source-file test targets, another counting allocator or a second integration harness. Existing allocator/socket/process-isolated binaries stay isolated. Shared tests use `fake_` names; PubMatic remains solely its real consumer's owner.

### Hosted Mutation Execution

Use existing `pull_request` triggers, not the later Task 8 dispatch. Under the batch authorization, first create an owned frozen base branch at the exact clean implementation checkpoint being challenged; PR bases are branch names, not arbitrary SHAs. Create one disposable owned branch and draft PR per mutant targeting that frozen base branch, and verify each PR's `base.sha` remains the recorded checkpoint. Each head changes only its recorded mutant; never move the base, stack mutants or push them onto the delivery PR. Distinct PR refs isolate their existing concurrency groups. Record base/head SHA, patch, PR/run/job URL, exact command, selected test names/count and actual failure; a pending, canceled or unrelated failing job is inconclusive. No new workflow, job, trigger or local execution is needed for these experiments.

| Mutants | Required hosted owner/result |
| --- | --- |
| Old root clone; full alias map; full parameter map; forward `data`; forward `bidderconfig`; alias falls back to base parameters | Existing exchange units/privacy targets: Task 1's recipient-root assertions, plus Task 4's `omitted_root_payloads_are_not_cloned` for the old clone. Require the intended executed assertion failure, not a compiler error. |
| Overwrite supplements; skip final validation | Existing split units: independently expected supplement precedence and the existing survivor-validation assertion respectively. Require an executed assertion failure at the affected boundary. |
| Remove bidder feature forward; remove runtime feature forward | Existing secure job's isolated `cargo check --locked -p auctioneer-exchange --lib --features test-support` from Task 8. After measurement code is wired, each removal must fail for its own unavailable dependency API, without dev-feature rescue. This pair intentionally tests compilation, unlike the eight runtime mutants. |
| Disable allocation counting; emit zero serializations | Existing secure job's Task 6 calibration/benchmark smoke commands. Require the respective positive-control/dispatch assertion failure, with nonzero selected controls. |

Tasks 1-4 need the first eight mutants only. The remaining four belong to #182 after the mandatory secure-lane commands exist; they cannot delay the security delivery. Do not replace any ordinary CI checks or suppress the expected red checks to call a mutant green. Preserve run logs, close each experimental PR unmerged, and delete only its owned temporary branch after its run finishes; remove a frozen base branch only after no experiment still uses it. These twelve full-matrix draft PRs have a real hosted cost; any proposal to replace them with local or narrower execution needs explicit user approval rather than an unrecorded policy exception. The clean delivery checkpoint still needs its own complete passing matrix and review.

## Task 1: Establish A Genuine Disclosure Regression

**Files:** `src/split/mod.rs` and `tests/privacy/grouped.rs` under `crates/auctioneer-exchange`; inspect but do not replace the existing `tests/auction_support/mod.rs` harness.

- [ ] Record implementation base SHA, current working-tree state, hosted baseline check conclusions and existing relevant test names before editing. The planning base's passing checks are not evidence for the implementation head. Read `materialize_per_bidder`, `split/resource.rs`, `run/mod.rs`, `alias.rs` and `ExtRequestPrebid` at that base.
- [ ] Add inline `recipient_root_selects_only_own_parameters_and_request_alias` in the existing split unit module, initially exercising **current `materialize_per_bidder`**, not a nonexistent helper. Use two participating `fake_` recipients with distinct `wrapper`/`acat` and nested supplement values, plus a nonparticipating competitor. One recipient is a request alias containing `/~`; another row requests the base. Resolve aliases through the existing resolver. Assert all decoys are present before projection and record the input/grouped bytes.
- [ ] Hand-write the complete expected outbound `prebid` using `json!`: selected params are the direct object, selected accepted inbound alias only, and exactly the nine retained fields from spec section 3.2. Populate every forbidden family, an unknown future field, all nine retained fields and an ordinary root-ext sibling. Assert complete root equality and unchanged input/grouped bytes. Do not derive expected keys from a production list or clone the old root ext as the oracle.
- [ ] Add `recipient_root_wire_shape_is_scoped_before_fake_construction` to `tests/privacy/grouped.rs`: send both allowed recipients through `Harness`, assert exact constructed identities/call counts, direct own params, absence of competitor aliases/params/raw FPD and presence of retained controls. The fake's actual serialized body is the observation; a manually constructed body is not the exchange result.
- [ ] Obtain the intended red from the tests-only implementation checkpoint in existing hosted jobs. Discovery and focused reproduction commands are:

  ```sh
  cargo test --locked -p auctioneer-exchange --lib -- --list
  cargo test --locked -p auctioneer-exchange --test privacy_policy_composition -- --list
  cargo test --locked -p auctioneer-exchange --lib recipient_root_selects_only_own_parameters_and_request_alias -- --nocapture
  cargo test --locked -p auctioneer-exchange --test privacy_policy_composition recipient_root_wire_shape_is_scoped_before_fake_construction -- --nocapture
  ```

  Expected: each focused selector executes one test and fails the outbound root assertion on the old full-map forwarding. Record actual result lines/head SHA; do not predict pass totals for the full target.
- [ ] Commit the regression checkpoint: `Expose recipient root parameter and FPD disclosure`. No production change or expected-snapshot regeneration in this commit.

## Task 2: Select Root Fields Before Copying

**Files:** new `src/split/root.rs`, `src/split/mod.rs`, model comments and master spec named above.

- [ ] Add ordinary private `mod root;`. Give the root owner one narrow entrypoint borrowing the canonical `Value`, requested `BidderName` and accepted `AliasMap`, returning a root map **without** `imp` or the existing fixed internal error for a non-object grouping invariant. It neither groups nor invokes privacy/selection. Keep the impression list insertion in the materializer.
- [ ] Iterate root members once, handling `imp` and `ext` before cloning a value. For `ext`, copy ordinary siblings individually, and build `prebid` from the finite dispositions. Never call `clone()` on the original entire ext/prebid and delete afterward. Use literal keys; look up recipient strings with `.get(name)`, never an interpolated JSON Pointer.
- [ ] Implement exact recipient `bidderparams` selection and object unwrapping. Missing/null/scalar/array entries produce no slot; empty objects retain the current empty-map Serde omission. Do not use the impression-merged params, a base fallback or an invented wrapper around the selected object. Leave `merge_supplement` untouched.
- [ ] For `aliases`, require an inbound request entry **and** an accepted matching alias mapping. Preserve only that entry. Host-only aliases must not manufacture request ext/prebid/aliases. An invalid/dropped inbound alias cannot reappear by falling back to a registered same-named base.
- [ ] Copy only `alternatebiddercodes`, `channel`, `currency`, `debug`, `integration`, `multibid`, `sdk`, `server`, `targeting`. Drop all other root-Prebid fields, including `aliasgvlids`, `nosale`, `returnallbidstatus`, `data`, `bidderconfig`, `schains` and future unknowns. Preserve standard `source.schain`. This does not implement PBS's narrower transforms for the retained controls.
- [ ] Replace only the old root-copy block in `materialize_per_bidder`; retain the original per-imp loop, base resolution, typed decode, `.validate()` and code-15/error mapping. Do not touch run ordering, charges or receipts.
- [ ] Extend inline root tests with the presence cross-product: no ext; ext without prebid; empty ext/prebid; only removed fields; each selected parameter shape (absent/null/scalar/array/empty/nonempty object); integer/bool debug; base/request/host-only aliases; `/` and `~`; accepted and dropped mappings. Assert both typed and reserialized canonical outcomes, not byte preservation for raw ingress spelling.
- [ ] Amend `scoped_materialization_matches_clone_first_params_precedence_and_supplements`: keep its independent clone-first **impression** oracle, but construct the expected root with a hand-written literal per row. Do not call the new root helper from expected-value code. Preserve existing hoisted/typed/legacy/null/merge/identity assertions.
- [ ] Update model comments and all five master contract passages: the `BidRequest.ext.prebid` field row (inspected base line 482), section 5.1 step 7's competitor contract (1209-1214), step 9's materialization contract (1239), the deferred-feature prose naming request-level FPD (6806-6808), and section 15's request-level-FPD divergence row (7434). Locate by heading/content after other edits, not stale offsets. Request-level FPD is represented inbound but omitted outbound until its resolver exists; retain the divergence from full PBS resolution. Preserve external captures and historical measured records.
- [ ] Observe the two Task 1 selectors turn green and the complete split units run in hosted CI. Expected focused result: `1 passed; 0 failed` for each selected test, with no ignored/zero-test success. Commit: `Project recipient roots before copying request extensions`.

## Task 3: Pin Privacy, Selection And Real Parameter Consumption

**Files:** `tests/privacy/grouped.rs`, `tests/privacy/grouped_snapshot.json`, reusable `tests/auction_support/mod.rs` support if necessary; PubMatic-local conformance.

- [ ] Before claiming #176 acceptance, read back its third criterion and the user-authorized two-owner substitution recorded during PR #202 review: exchange fake proves the direct projected `wrapper`/`acat` shape, while PubMatic-owned conformance proves the real outgoing consumer. The user explicitly authorized that issue edit; preserve the other criteria and keep the issue open. If the recorded criterion is absent or differs, reconcile it with the user rather than silently treating the original exchange-level real-PubMatic test as waived.
- [ ] Add `recipient_root_privacy_varies_pass_id_and_ufpd_independently`. Use a valid existing full-consent fixture and the harness's deterministic vendor-8 GVL, not malformed TCF or a privacy exception that also rescues IDs. Enable GDPR; disable only configured Purpose-2 enforcement flags so fetch remains permitted for vendor 0, while `pass_id` still uses forced enforcement. Keep COPPA/LMT/CCPA and all other activities allowing. Independently vary selected recipient (two aliases), its resolved vendor (8 versus 0, producing real PassID true/false) and `transmitUfpd` (true/false): **8 rows**, with the sibling always vendor 8/activity allowed.
- [ ] Prove the fixture's actual real-resolver PassID result for each vendor row, rather than injecting a decision. Require exactly two constructed recipients and two fake wire bodies in every row; a denied fetch or sampled/capped bidder cannot mask ID disclosure. Assert inbound presence and exact preservation/removal of **both** top-level `user.eids` and `user.ext.eids`; raw root `data`/`bidderconfig` are absent even in the all-allow row. Keep distinct recipient marker values so cross-recipient reuse fails.
- [ ] Add selection rows varying disabled, privacy-denied, sampled-out and capped-out identities **one cause at a time**, each with an allowed sibling. Require absence from construction and outbound alias/params, plus exact existing statuses/warnings and RNG draw ordering. Keep existing sampled-out/cap-dropped malformed FPD and survivor typed-roundtrip/validation/imp-ID cases intact.
- [ ] Pin supplement precedence across hoisted/typed/legacy imp slots, explicit null, nested fill-only merge, alias with distinct base supplement and nonparticipating root supplement. Assert expected full selected params, original/grouped immutability and no created participation.
- [ ] Reuse three existing PubMatic owners: `captured_root_wrapper_precedence` conformance (`tests/conformance.rs`, driving `supplemental/reqBidderParams.json`), `parameters.rs::request_acat_trims_each_string_without_inventing_filtering` (valid direct root params through the real driver, asserting outgoing `ext.acat`), and `parameters.rs::request_root_wrapper_acat_and_alternate_codes_fail_before_impressions` (malformed root rejection). These already cover the real wrapper/acat consumer and remain unchanged; do not add the previously proposed duplicate `projected_root_wrapper_and_acat_reach_the_real_consumer`. Task 1's new exchange fake producer-shape assertion plus these executed existing consumers deliver the amended #176 criterion without a real factory in shared tests, changes to pinned fixtures or regenerated digests.
- [ ] Review the existing grouped snapshot diff path by path. Change only recipient outbound `ext.prebid` disposition paths. Require unchanged response/warnings/statuses and unchanged non-root request fields; do not approve a broad snapshot refresh.
- [ ] Hosted focused/full owner commands:

  ```sh
  cargo test --locked -p auctioneer-exchange --test privacy_policy_composition
  cargo test --locked -p auctioneer-exchange --test privacy_enforcement
  cargo test --locked -p auctioneer-bidder --lib bidders::pubmatic::
  ```

  Expected: real nonzero dispatch; the new independent privacy test runs all 8 rows and fails on either ID location's leak. Read actual test names/result lines; the PubMatic filter must execute all three existing named consumer controls. Commit: `Cover root scoping through privacy and bidder consumers`.

## Task 4: Preserve Funding And Prove Omitted Roots Are Not Cloned

**Files:** inline `src/split/root.rs` tests. Existing owners `tests/projection_charge.rs`, `tests/projection_scrub_bound.rs`, `tests/sparse_request_copies/main.rs`, `tests/construction_bound.rs`, `tests/disclosure_policy_diff.rs` are acceptance, not replacement candidates.

- [ ] Add native-only `omitted_root_payloads_are_not_cloned` using the runtime test-support counters and the exchange unit binary's **existing** allocator delegate. Construct/group fixtures, warm counters and reserve observation storage before the window. Hold retained root output identical while varying omitted payload lengths **128/1,024/8,192 bytes**, separately for competitor params and raw `bidderconfig`. Call the actual root helper, retain its result during peak observation and compare complete output equality. Keep portable root semantics tests compiled for guests; do not gate the entire test module to hide native measurement imports.
- [ ] Require equal incremental requested bytes/peak across those omitted-size rows; the number of keys and retained content are fixed. Add a positive control with the same size sweep in a retained non-Prebid ext string; require observed requested-byte growth at least the independently known payload increase. This pins selection-before-copy without letting a later typed-decoder peak hide a temporary competitor clone. No new production allowance or ad-hoc 16x measurement slack.
- [ ] Preserve the existing full-path amplification, zero-survivor, overflow, underfunded-before-allocation, impression/survivor differential and retained-output lifetime controls. If a direct root expectation changes, review only that expectation; never recalculate a bound from the production charge being tested.
- [ ] Run in the existing hosted owners:

  ```sh
  cargo test --locked -p auctioneer-exchange --lib split::
  cargo test --locked -p auctioneer-exchange --test projection_charge --test projection_scrub_bound --test sparse_request_copies --test construction_bound --test disclosure_policy_diff
  cargo test --locked -p auctioneer-bidder --test test_isolation
  ```

- [ ] Use the authorized Hosted Mutation Execution procedure above for the eight root/supplement/validation mutants, each in its own draft PR and existing owner. Require executed assertions and retain patch, command, actual failure and head, not just a claimed kill count. Do not use the Task 8 dispatch, leave mutations in a delivery branch or touch another reviewer's tree.
- [ ] Obtain complete authoritative hosted CI for the privacy delivery head, including all existing feature/compiler/producer/adapter lanes, and independent review. Keep #182 open. Commit: `Guard recipient projection allocation and admission`. Ask for merge approval separately; accepted #176 must not wait for Tasks 5-9.

## Task 5: Expose The Same Production Stages For Measurement

**Files:** `src/split/mod.rs`, new native/test-support `src/split/measurement.rs`, `src/test_support/mod.rs`, and `Cargo.toml` under `crates/auctioneer-exchange`; inline tests only.

- [ ] After the privacy checkpoint, extract the existing per-recipient scoped-impression loop into a private helper; retain parameter precedence, ID comparison and error strings verbatim. Extract typed decoding and request validation into separate private helpers with the same code-15 mapping. Production calls these helpers and the Task 2 root helper in the same order. Preserve base resolution, result-vector reservation, survivor order and retained output lifetime.
- [ ] Add an ordinary native-only test-support child module under `split`. It may call those private helpers and the public grouping function; reexport its deliberate observation API through `test_support`, not the private helpers or grouping internals. The observation wrapper owns only stage boundaries/records; no duplicate field-disposition, merge, filtering or serialization logic. No generic serving observer framework, callbacks in the public serving API or root-model reuse optimization.
- [ ] Make the existing opt-in library feature own its dependencies explicitly. Replace exchange's empty `test-support` feature with the following forwarding, retaining the existing workspace dependencies/dev-dependencies. Do not enable these dependency features unconditionally on normal dependencies or add them to defaults:

  ```toml
  test-support = [
      "auctioneer-bidder/test-support",
      "auctioneer-runtime/test-support",
  ]
  ```

  This uses existing features; it adds no dependency or feature name. Native measurement modules remain native-gated. An isolated normal library build must work without dev-dependency feature unification; passing only `cargo test` or an all-target workspace build cannot establish that boundary.
- [ ] Define six observation phases: grouping (once), scoped impressions (per recipient), root projection (per recipient), typed decode (per recipient), typed validation (per recipient), and actual fake-bidder bounded serialization (per recipient). Full operation means grouping through serialization of all survivors; clearly exclude ingress/privacy evaluation/network/response processing from these measurements.
- [ ] Separate phased and **unphased** complete-operation runs. Peak resets for individual phases must not corrupt an outer peak; never claim a sum of per-phase peaks is the whole peak. The unphased control uses ordinary production grouping/materialization plus the same bounded serialization, without stage callbacks/resets.
- [ ] Add `measured_materialization_matches_ordinary_outputs_and_errors`: compare full typed/wire outputs and structured errors for all matrix shapes plus missing grouped recipient, FPD-rewritten ID, typed FPD failure and validation failure. Each failing row must reach the same error stage, not be rejected by an earlier fixture setup. Retain existing materialization tests rather than replacing them with observer tests.
- [ ] Hosted commands, as separate invocations selecting only this package:

  ```sh
  cargo check --locked -p auctioneer-exchange --lib
  cargo check --locked -p auctioneer-exchange --lib --features test-support
  cargo test --locked -p auctioneer-exchange --lib --features test-support
  ```

  Expected: both normal library checks exit 0; the new equivalence test and all existing units execute successfully. Do not add `--all-targets` or another selected package to the checks, which would mask the dependency-feature boundary. Once Task 6's native runner is wired, independently remove each dependency-feature forward in disposable hosted controls and require the isolated feature-enabled library check to reject its unresolved API import; no surviving dev-feature rescue. Commit: `Expose real materialization stages for native measurement`.

## Task 6: Add Calibrated Counters And A Bounded Benchmark Entrypoint

**Files:** runtime `src/test_support/alloc_track.rs`; exchange `src/test_support/mod.rs`, new `src/test_support/materialization/{mod.rs,fixtures.rs,report.rs}`, `benches/request_materialization.rs`, exchange manifest.

- [ ] Extend the existing TLS helper with monotonic **allocation calls** (alloc plus alloc_zeroed) and separate **reallocation calls**. Keep `allocated()`, `live()`, `reset_peak()` and `peak_since()` semantics unchanged, including full new requested realloc size before refunding the old size. Report requested, not allocator-resident, memory. Snapshot/subtract counters outside windows with checked arithmetic; invalid/overflowed observations refuse a sample.
- [ ] Add independent calibration units beside exchange `test_support`, whose unit global allocator already delegates to this helper: one allocation, deallocation without a cumulative/call refund, alloc_zeroed behavior, a forced nonempty Vec growth/reallocation and a retained/peak positive control. Warm TLS before snapshots; keep assertions/formatting outside windows. Do not put these controls in a binary whose installed allocator does not feed the helper.
- [ ] Register `[[bench]] name = "request_materialization"`, `harness = false`, `required-features = ["test-support"]`. Use only existing dev-dependencies. The native entrypoint installs the **existing** `CountingAllocator`, parses modes, runs controls and returns nonzero on any invalid/incomplete run. Guest compilation must still pass the existing all-target Clippy/build lanes; a guest execution of this native-only benchmark returns an explicit unsupported error, never a successful zero-work report. Gate native measurement modules/allocator imports consistently.
- [ ] Default invocation and Cargo `--test` run bounded smoke only. Accept Cargo's inert `--bench` marker. Handle the current broad CI pass-through flags explicitly: repeated `--skip VALUE`, `--nocapture`, `--show-output`, and `--test-threads VALUE`/`--test-threads=VALUE` do not filter this single smoke entrypoint or turn on repeated measurement. Test that the actual workspace `--skip` argument shape still performs nonzero smoke. Reject unknown flags, missing values, zero counts, conflicting modes and any full-measure request combined with `--test`.
- [ ] Full modes are explicit: `--measure timing` or `--measure allocations`, each with `--metadata PATH --output PATH`. Do not expose iteration/cap tuning flags. Full mode refuses missing/inconsistent source metadata and empty/preexisting output destinations. Typed parsing/report controls live in the native test-support owner with inline tests; do not hide unexecuted `#[test]` modules in a harness-less benchmark.
- [ ] Smoke executes every one of the frozen 17 cases once with exact phase/dispatch assertions and one unphased equivalence control per case, not repeated statistical sampling. Add negative controls for zero/partial projections, skipped validation, missing/extra serialization, construction error and disabled counters. Report generation cannot turn an error into a successful sample.
- [ ] Hosted commands:

  ```sh
  cargo test --locked -p auctioneer-exchange --lib --features test-support
  cargo test --locked -p auctioneer-exchange --bench request_materialization --features test-support
  cargo test --locked -p auctioneer-exchange --bench request_materialization --features test-support -- --skip unrelated_a --skip unrelated_b
  ```

  Expected: inline calibrations/mode tests execute; each benchmark invocation prints a machine-checkable summary with **17 cases, 0 failures**, nonzero work, and no full measurement. Require a disabled-counter mutant and a zero-serialization mutant to fail the positive controls. Commit: `Add a calibrated materialization benchmark smoke`.

## Task 7: Freeze The Matrix, Lifetimes And Report Contract

**Files:** native test-support `materialization` owners; `docs/testing/development.md`. Freeze these literals before the first hosted timing run.

Baseline: **4 survivors, 4 impressions per recipient, 64-byte retained root-ext/user/device strings, supplements absent, per-imp FPD absent**. Use one shared fake implementation and a fixed roster of 16 accepted inbound `fake_` aliases. Keep four nonparticipating competitors with 128-byte root parameter payloads fixed across survivor sweeps. No real bidder/consumer names in fixtures.

| Axis | Rows, all other declared axes at baseline | Additional unique cases |
| --- | --- | --- |
| Survivors | 1, 4, 16 | 2 |
| Impressions per recipient | 1, 4, 8 | 2 |
| Retained non-Prebid root ext string | 64, 1,024, 8,192 bytes | 2 |
| `user.keywords` string | 64, 1,024, 8,192 bytes | 2 |
| `device.ua` string | 64, 1,024, 8,192 bytes | 2 |
| Supplements | absent; present with 64, 256, 1,024-byte string | 3 |
| Existing per-imp FPD | absent; present with 64, 256, 1,024-byte `tagid` string | 3 |

**Exactly 17 unique cases:** baseline + five independent two-case additions + two three-case additions. This is not a cross product or guest workload declaration. Presence is independently observed; "present empty" is a correctness control, not substituted for a measured size. Keep independent ordinal/case-membership expectations in tests rather than deriving them from the matrix generator alone.

The reference workload is Axum's source-bound resource profile, not the more permissive exchange harness defaults. At the inspected base, `crates/auctioneer-adapter-axum/src/envelope.rs` declares a 32,768-byte input ceiling, 2,048 input nodes, 16 bidders, 16 calls, **8 impressions** and a 32,768-byte raw outbound body ceiling. Core admission uses `max_impressions_per_auction` in `auction/prepare.rs::normalization_limits`; 16 impressions is not an admitted native row. The revised impression sweep keeps three sizes and all 17 cases without changing any production limit.

- [ ] Generate valid fixtures with `json!`, use object supplements such as `{"own":99,"fill":"..."}` against explicit imp `own=1`, and FPD patches that preserve original imp IDs/media. Validate original and materialized requests. Pin the smoke's reference values above independently from the matrix generator; assert the exact impression sweep `[1, 4, 8]`. The benchmark's **fixture-admission guard** admits 8 and refuses a separate 9-impression negative control before measured grouping; it is not production admission and not an eighteenth successful matrix case. Production funded-cap enforcement remains owned by core's existing `boot_funded_impression_limit_is_enforced_on_the_auction_path` in `tests/impression_limit_on_the_auction_path.rs`, executed by the ordinary hosted matrix. Do not introduce a core dependency or private admission import into the exchange benchmark.
- [ ] Check actual input bytes/nodes/depth/imp count and selected call/body/meta usage against that reference profile before measuring. Full-mode setup must also check the cases against the existing secure job's source-bound `resource-fit.json` Axum `effective_limits` row and include those limits in report metadata. Use exported data outside measurement windows, not a new adapter/core dependency or a private source import in exchange. Do not substitute the harness's 100-impression fixture allowance. If a declared row no longer admits or the frozen reference differs from current exported limits, stop and review that drift; never enlarge a production cap or silently drop a case.
- [ ] Outside windows, record each case's actual canonical bytes, node/depth counts, declared/selected names, selected imp IDs, supplement/FPD sizes and expected output size/content. Record unavoidable structural overhead when membership changes. Before every counted iteration, grouping must perform one canonical conversion; do not reuse a precomputed grouped object and call it grouping time.
- [ ] Freeze **8 warmups and 80 measured iterations for each of phased and unphased runs, per case and mode**. Thus each process reports 1,360 phased and 1,360 unphased samples, plus separately identified warmups. No outlier removal, retry-until-fast or postmeasurement threshold. Record raw nanosecond timings and summary method; do not round short phases into missing/zero-work rows.
- [ ] In each complete operation require 1 grouping, exactly S projections/decodes/validations/accepted fake calls/body writes and S*M sent impression associations. Include real sink outcome errors/rejected/suppressed counts, not just writer attempts. Use `FakeBidder` aggregate mode, `OutboundRequestSink`, `CallBudget`, existing `SinkLimits`, endpoint validator and real bounded JSON serialization. A deliberately underfunded sink is a negative control, not an accepted zero-bid sample.
- [ ] Preallocate fixture/registry/expected/report/sample storage and reset reusable fake counters outside the window. Retain all per-recipient typed results as production does; consume/drop temporary root Values at decode. Drop grouped source at the production handoff before bidder serialization, retain accepted bodies across survivor construction and release results at matching boundaries. Document what any benchmark-only observation storage retains. Use `black_box` on real inputs/results, verify content outside windows and never use allocator statistics computed from a separately reconstructed request.
- [ ] Use fresh OS processes for timing and allocations; **both use the disclosed counting allocator**, with no new pause/reset semantics. Timing is allocator-instrumented native observation, not uninstrumented production latency. Allocation mode reports cumulative requested bytes, allocation calls, realloc calls, retained growth and peak growth separately. The unphased run owns the whole-operation peak; per-phase metrics keep their own baselines and are not summed into a false total. Do not subtract observer overhead to manufacture smaller/negative costs.
- [ ] The report requires source commit/tree, dirty/source-tree match state, lock SHA-256, toolchain, effective release settings/features, runner/CPU identity, mode, exact case and iteration manifest, instrumentation/lifetimes, raw samples and checksums. Reuse existing tools outside windows to collect identity; no new hashing dependency. Dirty/incomplete reports may be diagnostics but are not final source-bound evidence.
- [ ] Assert report row identities/counts, sample lengths, exact phase coverage and dispatches independently, including malformed/missing metadata, unknown cases, duplicate rows and an operation doing no work. Use literal expected shapes, not acceptance based solely on successful serialization of the report type. Hosted owner remains exchange units plus bounded benchmark smoke. Commit: `Freeze materialization cases and source-bound reports`.

## Task 8: Add One Hosted Measurement Owner Without Repeating Release Builds

**Files:** `.github/workflows/test.yml`, colocated `.github/workflows/scripts/test/measure-materialization.sh`, `docs/testing/development.md`.

- [ ] Add an explicit **bounded benchmark smoke command** to the existing secure test lane using Task 6's `cargo test ... --bench ... --features test-support`. Do not rely on implicit dev-feature unification or Cargo silently selecting a `required-features` target. Other `--all-targets` lanes may also execute its bounded smoke, but no ordinary PR/push runs repeated release measurements.
- [ ] Give Task 5's two isolated `cargo check --lib` commands mandatory ownership in that same secure lane, as separate package-only invocations before the smoke. Keep the existing full workspace/guest checks too; they do not replace the isolated normal-dependency checks. This adds no job/compiler lane and enables no test-support feature in ordinary serving builds.
- [ ] Extend the existing workflow with `workflow_dispatch` inputs: boolean `measure_materialization`, default false, and string `expected_commit`, required by a full measurement to be a full 40-character commit SHA. Preserve `push: branches: [main]`, `pull_request:`, all jobs, lanes, assertions, timeouts, permissions and caches. Preserve the ordinary push/PR concurrency group and `cancel-in-progress: true`; prefix **dispatch-only** groups with `measurement-` so even an invalid dispatch cannot cancel main/PR CI before its script refuses. The one full-measure step runs only for a manually requested dispatch with the boolean true in **the existing secure lane**. No new paid/macOS runner, matrix/compiler lane or skipped branch checks. Keep shell logic in the colocated script, not a large YAML block.
- [ ] Require a dedicated frozen branch `measurement/bidder-materialization/<expected_commit>` created at that exact reviewed, merged implementation commit. Do not dispatch a measurement on `main` or a moving delivery branch, move/force-push the measurement ref, or start overlapping runs for it. Dispatch refs are branch/tag names, not arbitrary SHAs. In the script, require `github.ref` to name that frozen branch and `github.sha`, `git rev-parse HEAD` and the branch's resolved object to equal `expected_commit`; also reject a dirty checkout. Check the source identity against the same commit. A ref/source mismatch or canceled run is incomplete evidence, not a timeout/fit result or permission to retry against newer code. The dedicated ref and dispatch-only concurrency namespace preserve ordinary main/PR CI. Keep the ref until evidence/run identity is recorded, then remove only that owned temporary ref with authorization.
- [ ] Create a fresh output directory under runner temp, record exact source/tree/lock/toolchain/CPU/workflow-run metadata and the Axum `effective_limits` from the existing current-source `resource-fit.json`. Require exactly one Axum row, source identity matching `expected_commit` and the measured checkout, and agreement with the frozen reference values in Task 7; a missing/stale profile is a refusal, not permission to use larger harness defaults. Then run:

  ```sh
  cargo bench --locked -p auctioneer-exchange --bench request_materialization --features test-support -- --measure timing --metadata "$metadata" --output "$output/timing.json"
  cargo bench --locked -p auctioneer-exchange --bench request_materialization --features test-support -- --measure allocations --metadata "$metadata" --output "$output/allocations.json"
  ```

  Each command launches a fresh measured process. Cargo may reuse its release binary within the same clean job; never reuse sample results. Validate both reports and their exact source/matrix/sample agreement before successful completion. Collect SHA-256 digests and invocation logs without touching source while measuring. Fail on incomplete files or an unavailable prerequisite; do not increase the secure job timeout to hide the problem.
- [ ] Upload those reports/manifest/logs using the repository's provider-workflow precedent: `actions/upload-artifact@v7` in `ci-cloudflare.yml`, `ci-fastly.yml` and `ci-spin.yml` at the inspected base. `test.yml` has no existing upload step. Error on missing outputs; document the run URL and exact commands. This is an evidence artifact, not a compiled/test-result cache or a qualification verdict.
- [ ] **Schedule honestly:** a new dispatch trigger cannot be assumed usable before its workflow exists on the default branch. Land accepted #176 independently; then merge reviewed benchmark/dispatch plumbing with full ordinary CI. Only after that, obtain authorization to create the frozen measurement branch and dispatch its exact commit. GitHub's [manual-run contract](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/manually-run-a-workflow) and [concurrency behavior](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/control-workflow-concurrency) govern this sequence. #182 stays open through these merges. If an uncanceled full release measurement cannot finish within the existing owner, report #182 blocked and return to review; do not delay or relabel the privacy fix.
- [ ] Observe complete hosted matrix on the benchmark-plumbing PR, with smoke summaries and guest all-target compilation included. No performance claim from compilation or smoke. Commit: `Own materialization measurements in the existing test workflow`.

## Task 9: Capture, Review And Commit The Measured Outcome

**Files:** new `docs/testing/evidence/bidder-materialization/{timing.json,allocations.json,README.md}` plus an explicit SHA-256 manifest; update issue/PR records only when authorized.

- [ ] With user authorization, create Task 8's frozen branch at the exact reviewed, clean merged implementation SHA and dispatch `gh workflow run test.yml --repo IABTechLab/auctioneer --ref "measurement/bidder-materialization/$measured_sha" -f measure_materialization=true -f expected_commit="$measured_sha"`, with `measured_sha` set to that recorded full SHA, not freshly resolved from moving `main`. Read back run `head_sha` and its ref; require the script's checkout/source checks to agree before accepting outputs. Preserve raw successful reports, logs/run URL, source/matrix identity, declared warmups and every sample. Verify both outputs agree on source/toolchain/features and case manifest; do not splice modes from different source trees or quietly recapture only favorable rows. No temporary branch or dispatch is authorized by publishing this plan.
- [ ] Validate artifact digests and typed content before copying the raw reports into the evidence directory. Keep captured report bytes unchanged; write interpretation separately. Record that the later evidence-only commit differs from the measured code commit and identify both; do not claim the evidence commit itself was the measured binary. Do not rewrite historical resource bundles or external captures.
- [ ] Write the recommendation with actual phase/allocation/end-to-end results, variation across independent axes, observer overhead comparison and limitations (native, allocator-instrumented, no guest/production-latency proof). Distinguish a measured bottleneck from an optimization proposal. Choose either **keep the corrected implementation** or link a separate reviewed optimization design; this plan never authorizes typed-root reuse or reducing a charge.
- [ ] Run the unchanged full hosted matrix for the evidence delivery and verify recorded checksums/report counts there using the same report-validation owner. Attach actual result lines, source SHA and run links. Missing or failed evidence remains explicit; no invented totals, speedups or "qualified" outcome.
- [ ] Commit: `Record source-bound bidder materialization measurements`. Obtain independent review and explicit merge authorization. Close #176 only after the actual scoped projection implementation and full acceptance are merged; close #182 only after these committed measurements and recommendation are merged. Keep separately tracked FPD, schema-resource and guest-readiness issues open.

## Review And Handoff Checklist

- [ ] Independently review this entire plan against the approved spec, current source owners and CI selection; repair material contradictions before publication. Record actual document-review outcome separately from GitHub REVIEWER approval.
- [ ] Confirm only the spec approval annotation and this plan change in the planning branch. `git diff --check`/staged diff checks are hygiene, not executed Rust tests.
- [ ] After user authorization, publish a **planning-only** PR linking both issues without closing them. Report its exact head and hosted checks; do not reuse base check results as new-head verification.
- [ ] Implementation, each delivery PR, remote experiment/dispatch and merging remain separately authorized actions. The next actor reviews the spec and plan together; no production task above has been executed by writing this document.

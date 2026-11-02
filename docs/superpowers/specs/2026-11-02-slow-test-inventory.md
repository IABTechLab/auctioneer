# Slow Test Inventory

Status: findings incorporated into the approved 2026-11-02 repair.
Implementation and verification: [repair plan](../plans/2026-11-02-test-and-runtime-performance.md).
Measured: run 41459435846, logs pulled per job on 2026-11-02.
Method: all 18 completed job logs downloaded via
`actions/jobs/<id>/logs`, ANSI stripped, libtest `test result:` lines joined
to their owning binary. Attribution must use the `running N tests` boundary:
stdout/stderr interleaving can put the next Cargo `Running` line before the
previous binary's summary. Nested child reports must not be double-counted as
independent parent wall time.

## What the measurement can and cannot say

Rust's libtest does not print per-test durations. It prints one
`finished in <t>s` per **suite** (test binary). So this inventory is
suite-level, except where a suite runs exactly one test — which, as it turns
out, is where most of the time is.

Run 41459435846 was still `in_progress` when queried, so `gh run view --log`
refused the whole-run download. The 18 finished jobs were fetched
individually; only `cargo test (cli)` is missing, and it was still inside
"Test Rust resource qualification and provider harnesses" after ~40 minutes.
Its contribution is unmeasured and is not counted below.

Completion update: that run subsequently finished. Its Rust steps passed;
the browser-shutdown step failed before reaching the pending-browser marker
because the isolated CLI lane had not built its Axum/config-generator
prerequisites. The explicit prerequisite build now passes both Node cases
locally. The table below remains the original 18-job measurement, not a
retroactively complete or passing hosted baseline.

725 suites reported a duration. 110 took at least one second. Those 110 total
**8,339 reported seconds (139 minutes)** in the original extraction. This is a
reported-duration sum, not a reconciled non-overlapping runner-time total when
parent tests forward child reports. The remaining 615 reports are under a
second each; they are not the primary critical-path target.

## Suites over 60 seconds

| Seconds | Job | Suite | Tests run |
| --- | --- | --- | --- |
| 1238.5 | workspace-default | `tests/bidders_evidence.rs` | 19 |
| 1201.1 | workspace-insecure | `tests/bidders_evidence.rs` | 19 |
| 722.1 | schema source selected-schema | `tests/schema_source_conformance.rs` | **1** |
| 470.7 | config build failures | `tests/build_failure_harness.rs` | — |
| 427.8 | config build failures | `tests/disclosure_model_coverage.rs` | — |
| 382.7 | compiler conformance schema | `tests/schema_build_controls.rs` | — |
| 379.2 | application artifact axum | `tests/schema_source_conformance.rs` | **1** |
| 369.4 | schema source private-layout | `tests/schema_source_conformance.rs` | **1** |
| 317.8 | compiler conformance schema | `tests/schema_source_conformance.rs` | 684 |
| 223.4 | application artifact cloudflare | `tests/schema_source_conformance.rs` | **1** |
| 220.5 | application artifact spin | `tests/schema_source_conformance.rs` | **1** |
| 200.0 | workspace-default | `tests/resource_boot_parity/main.rs` | — |
| 193.0 | workspace-insecure | `tests/resource_boot_parity/main.rs` | — |
| 190.7 | application artifact fastly | `tests/schema_source_conformance.rs` | **1** |
| 154.8 | workspace-default | `auctioneer_cli` unittests `src/main.rs` | — |
| 151.4 | workspace-insecure | `auctioneer_cli` unittests `src/main.rs` | — |
| 151.0 | compiler conformance exports | `tests/insecure_bidders_snapshot.rs` | — |
| 133.3 | compiler conformance settings | `tests/generated_settings.rs` | — |
| 122.8 | compiler conformance discovery | `tests/registry_build.rs` | — |
| 111.3 | compiler conformance discovery | `tests/factory_binding_build.rs` | — |
| 74.9 | secure | `tests/response_body_is_sealed.rs` | — |

Suites between 25 and 60 seconds: `maintenance_build` (51.9/49.1),
`feature_exports_source_conformance` (49.5/36.7/36.6),
`bidder_request_allocation` (42.1/28.8/28.4), `response_body_is_sealed`
(60.0/59.4). Below 25 seconds the tail is flat and uninteresting:
`bidders_resources` 24.2, `bidders_import` 18.8, `projection_scrub_bound`
17.1, `bidders_wave` 16.4, down to 1.0.

## Finding 1: six jobs spend 190–722 seconds to run one test each

`schema_source_conformance` appears 14 times across the run for 2,466
runner-seconds. The breakdown by test count is the story:

| Job | Seconds | Passed | Filtered out |
| --- | --- | --- | --- |
| schema source selected-schema | 722.1 | **1** | 689 |
| application artifact axum | 379.2 | **1** | 689 |
| schema source private-layout | 369.4 | **1** | 689 |
| compiler conformance schema | 317.8 | 684 | 6 |
| application artifact cloudflare | 223.4 | **1** | 689 |
| application artifact spin | 220.5 | **1** | 689 |
| application artifact fastly | 190.7 | **1** | 689 |
| secure | 15.0 | 34 | 656 |
| workspace-default | 10.8 | 34 | 656 |
| workspace-insecure | 10.6 | 34 | 656 |
| compiler conformance schema (x4) | 1.1–2.5 | 2 each | 688 |

Six jobs run a single test for a combined **2,105 seconds (35 minutes)**.
The tests:

- `tests::selected_schema_probe_links_and_executes_the_source_bound_cohort`
- `tests::compiler_private_layout_facts_cover_native_and_every_guest_abi`
- `tests::actual_application_artifacts_{axum,cloudflare,fastly,spin}_retain_every_selected_validator_in_both_feature_lanes`

**This partition is deliberate and correct.** The six are `--skip`ped from the
bulk run in `verify-schema-compiler-conformance.sh:53-58` and given dedicated
jobs via the matrix at `test.yml:611,623,625`. Someone already identified
these as the heavy cases and isolated them. The structure is not the bug.

The cost is intrinsic to what the tests do. `assert_actual_application_artifacts`
(`schema_source_conformance.rs:610`) loops over two feature lanes —
`(Lane::Secure, false)` and `(Lane::Development, true)` — and calls
`application::inspect`, which routes to `application_cargo`
(`schema_build/application.rs:867`) and spawns a **child cargo build**. The
shared `schema_build/mod.rs:45` declares `const TARGETS: [&str; 4]`.

So each of these "one test" jobs is really a multi-target Rust build wearing a
test's name. Measured on the axum job:

```
first outer Compiling   14:11:25
suite starts Running    14:12:09   → 44s outer compile
test result line        14:18:28   → 379s inside the test
```

Outer compilation is 44 seconds. The remaining 379 seconds is child builds
launched by the test body. Any plan that treats this as execution time to be
parallelized at the libtest level will not move it.

## Finding 2: `bidders_evidence` is the critical path, at 65s per test

| Job | Seconds | Tests | Per test |
| --- | --- | --- | --- |
| workspace-default | 1238.5 | 19 | 65.2s |
| workspace-insecure | 1201.1 | 19 | 63.2s |

This one suite is 2,440 runner-seconds — 29% of all measured slow-suite time —
and it is why `workspace-default` and `workspace-insecure` are now the two
slowest jobs. It runs twice because the two lanes differ by the
`insecure-bidders` feature, which is a real difference in what gets built.

The quotient of suite duration and case count does not establish uniform
per-test cost. The completed workspace-default log gives a more useful
timeline: the suite starts at 14:18:30Z; the native producer case finishes at
14:36:20Z and the schema producer case at 14:39:09Z. Both acquire the same
test-only mutex, so the latter's elapsed time can include waiting, not just CPU
or builds. These are the final two cases running, not evidence that all 19
cost 65 seconds each.

Per-action or isolated-case measurement is appropriate for further attribution.
Do not enable unstable libtest options on the pinned stable toolchain. Any
instrumentation belongs in test code and must not log source/config contents.

## Finding 3: stdout volume from a passing test

`selected_schema_probe_links_and_executes_the_source_bound_cohort` prints a
large single-line JSON document — schema digests, per-bidder selection
records, trapped import lists — directly into the log on a **passing** run.
One job log reached 632 KB largely from this.

This is not a major time cost but it has two real effects: log retrieval and
grep over these jobs is slow, and a reader looking for a failure scrolls past
kilobytes of success output. `assert_actual_application_artifacts` imports
`std::io::{Write as _, stderr}` and writes diagnostics unconditionally; gating
that on failure, or on an env flag, would make these logs readable.

## Recommendations

Ordered by measured bottleneck and implementation risk. Implementation is now
authorized through the repair plan; do not treat unfinished checks as passing.

### 1. Isolate the expensive evidence producers without losing feature coverage

It is the largest measured suite across the two workspace lanes. The timestamp
timeline isolates the two fresh-build producers as the long tail. Their child
plans explicitly select secure/development graphs and must still execute real
test processes with exact dispatch receipts. Lightweight receipt/current-input
checks should remain under the existing parent feature selections.

AG approved one mandatory owner for the heavy producer cases. The implementation
changes only libtest skip arguments in broad runs, not Cargo graphs, and must
not drop feature-binding assertions or cache results.
Preserve isolated reproductions and report build time separately from test time.

### 2. Treat the six heavy jobs as a build-reuse problem, not a test problem

Their 2,105 seconds is child `cargo build` across four targets and two feature
lanes, confirmed in source and in the 44s-vs-379s split above. Options worth
evaluating, in the order they should be tried:

- **Share one child target directory across the six jobs.** They build
  overlapping graphs into separate scratch directories today. This is the
  deferred Phase 2 of the existing CI-partition design, and that document's
  warning applies unchanged: `verify_probe_workspace` writes `Cargo.lock` and
  runs `cargo metadata`, so it is a build step, not a cache validator, and
  must not be called before validating a candidate cache hit.
- **Collapse the four `application_artifact` adapter jobs into one job** that
  builds the shared dependency graph once and then checks four adapters.
  Today each adapter job re-pays the common compile. Whether the graphs
  overlap enough to win needs measuring, not assuming.

Explicitly do **not** respond to this by raising `--test-threads`. The work is
in child processes; more libtest threads would contend for
the same cores and can make it slower.

This is a public repository: the documented standard Linux tier has four
vCPUs, not two. Cross-job child-artifact reuse remains deferred; the current
repair does not call a mutating build helper to validate a cache hit or merge
the four independently owned adapter checks.

### 3. Gate the success-path diagnostics

Make the `stderr` writes in `assert_actual_application_artifacts` and the JSON
dump in the selected-schema probe conditional on failure or on an env flag.
Cheap, no effect on what is asserted, and it makes the six slowest jobs'
logs usable.

### 4. Leave the tail alone

Below 25 seconds there are 90 suites totaling a few minutes, spread evenly.
There is no win there, and touching working tests to shave seconds adds risk
for no wall-clock gain. Note in particular that `schema_source_conformance`
running 684 tests in 317.8s — 0.46s per test — is *healthy*; it looks like a
slow suite only because it sits next to the single-test jobs.

## What was not measured

- `cargo test (cli)` never finished, so "Test Rust resource qualification and
  provider harnesses" is absent from every total here. It had been running
  ~40 minutes at last check and may be the largest cost in the run.
- Exact per-test CPU/build durations within multi-test suites. Completion
  timestamps identify a long tail, but not time spent waiting on the mutex.
- The compile/execute split inside `bidders_evidence`,
  `build_failure_harness`, `disclosure_model_coverage` and
  `resource_boot_parity`. Each may also spawn child builds; this was confirmed
  only for the `schema_build` family.
- No speedup figure is given. The run is incomplete, one job is missing, and
  the dominant suite's internal profile is unknown. Estimating a total saving
  from these numbers would repeat the error the CI-partition review corrected.

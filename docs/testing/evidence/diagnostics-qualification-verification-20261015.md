# diagnostic qualification Integration Verification

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

Executed 2026-10-15 in `diagnostic-hardening`. The original integration matrix below
records source `82075f11abded7265aa4245e01cb0191523c85bc`; it is not a full
workspace rerun of later self-review repairs. The follow-up sections separately
record those changes and fresh scopes. This original matrix used SDK pin
`65312643f4fd969bb1651e3f7dbdabd1479b2e74`. The later Rust 1.98.1/SDK
upgrade uses `596f9f4e23e43c8952a51ce6d92961d461032004`; its scoped
verification below does not retroactively change the original matrix.

## Results

All successful verification commands completed with exit 0. Counts below are
per invocation, not a sum of distinct tests; repeated coverage is intentional.
Cargo commands use `--locked --offline -j2`. Full commands are the Task 16
checklist in the [implementation plan](../../superpowers/plans/2026-10-07-diagnostics-and-qualification.md#task-16-integrated-verification-and-acceptance-reconciliation).

| Invocation                                                                             | Result                                           | Private log suffix                                             |
| -------------------------------------------------------------------------------------- | ------------------------------------------------ | -------------------------------------------------------------- |
| `cargo test --workspace --all-targets`                                                 | 212 binaries; 3,429 passed, 0 failed, 10 ignored | `workspace-repaired`                                           |
| Separate secure-default bidder/exchange/core all-targets                               | 131 binaries; 1,797 passed, 0 failed, 2 ignored  | `secure-foundations-repaired`                                  |
| Secure-default config: lib and all ten non-harness integration targets                 | 11 binaries; 389 passed, 0 failed                | `secure-config-final`                                          |
| Feature workspace, config excluded                                                     | 200 binaries; 2,955 passed, 0 failed, 10 ignored | `feature-workspace-final`                                      |
| Feature config: lib/codegen/GVL/smoke/URL                                              | 5 binaries; 276 passed, 0 failed                 | `feature-config-final`                                         |
| Native four-binary fanout conformance                                                  | 77 passed, 0 failed                              | `conformance-final`                                            |
| Native cancellation and wave TCP probes                                                | 17 passed, 0 failed                              | `tcp-final-repaired`                                           |
| Actual native export with dependency-only core conformance                             | 33 passed, 0 failed                              | `native-conformance-profile-unsandboxed-final`                 |
| Separate default and conformance core feature indicators                               | 1 passed each                                    | `core-default-feature-final`, `core-conformance-feature-final` |
| Separate secure production usersync binary                                             | Explicit ignored probe executed; 1 passed        | `secure-usersync-final`                                        |
| Strict workspace Clippy, all targets, `-D warnings`                                    | Passed                                           | `clippy-workspace-final`                                       |
| Cloudflare/Fastly/Spin foundation checks, strict lint and WASM conformance compilation | All nine commands passed                         | `wasm-{adapter}-{foundation,clippy,conformance}-ci`            |
| Qualification Node suite, including frozen-native probe                                | 68 passed, 0 failed, 0 skipped                   | `resource-geometry-native`                                     |
| Offline provider suite with explicit pinned SDK root                                   | 117 passed, 0 failed, 0 skipped                  | `provider-offline-final`                                       |
| Browser shutdown controls                                                              | 2 passed, 0 failed                               | `browser-shutdown-final`                                       |
| Actual installed Chromium 153.0.8010.12                                                | Seven browser flow checks passed                 | `chromium-final`                                               |

Logs remain outside the repository at `/private/tmp/auctioneer-diagnostic-qualification-{suffix}-20261015.log`.
The secure config invocation explicitly names lib, codegen, disclosure_codegen,
gvl_archive, smoke, url_safety, activity, baked_defaults_snapshot, simple_fold,
usersync_config and usersync_callback_validation. The isolated build-failure
harness completed all 84 tests in the full workspace invocation. Ignored tests
are not counted as executed; the production usersync exception is explicit.
Workspace feature unification is not secure-default evidence.

WASM rows use Cloudflare `wasm32-unknown-unknown`, Fastly `wasm32-wasip1`, and
Spin `wasm32-wasip2`, with the exact package/feature scopes in current CI.
Compilation is not hosted execution. A sandboxed native socket attempt failed
with EPERM; the authorized rerun passed 33/33. An extra, non-CI invocation that
selected core's native-only dev/test graph for WASM failed in `mio`; the exact
CI scopes passed without changing source, suppressing lint or weakening tests.

Formatting, whitespace, batch-rebase, roster and rejection-fixture guards
passed. The raw-JSON guard reports 153 budgeted fixtures across 47 files; all
rejection controls passed. Actual Chromium uses installed Playwright 1.63.0,
ephemeral loopback origins and controlled redirects. Its seven checks cover
CORS, JSON preflight, emitted callbacks, signed Secure/SameSite=None cookie
acceptance, credentialed readback, denied origin, deletion and empty readback.
This remains cross-origin same-site evidence, not third-party-cookie or hosted
certification. No real bidder/provider was contacted.

## Acceptance Boundary

[Native local Q2-C](native-axum-82075f1-local-pass/README.md) passed all 45 rows;
safe files and hashes are retained alongside both failed attempts. Overall
qualification remains unsupported, SDK attestations pending, publisher ready
false, and the harness exits 1 by design. Per-thread allocation and whole-process
high-water evidence remain distinct. Independent spec and quality reviews
approved the implementation repairs, offline harnesses and actual local evidence.

The reported `usersync_routes` intermittent failure remains open/not reproduced;
passing suites do not establish its cause. Application header bounds still
exclude provider-added framing fields. publisher pilot owns live provider/corrupt-config
execution, actual-deployment Q2-D and publisher rollout after Trusted Server embedding and the
selected bidder wave/host setup, not full catalog or video completion.

Remote CI is **not green**. The
[observed job](https://github.com/IABTechLab/auctioneer/actions/runs/38725558020/job/116390396542)
has no assigned runner and no steps; its annotation reports failed account
payments or an insufficient spending limit. Local results do not override this
external refusal, and no workflow gate was relaxed to hide it.

## Self-Review Follow-Up

The eight approved repairs were committed as `35ef524`, following the separate
approved reference-data refresh `a28887e`. Fresh secure-default
bidder/exchange/core all-targets passed 1,813 tests across 131 binaries, with
two ignored. CLI passed 54 unit and eight integration tests, with two unrelated
unit tests ignored. The actual dependency-only native resource export passed
35 tests with the explicit enabled expectation. Default and conformance core
indicator groups passed five each; all 70 workflow guards passed. Scoped strict
all-target Clippy and each of the three exact-CI WASM lint commands passed.
Reference refresh checks passed 42 config library and three archive tests;
offline provider tests passed 117 with zero skips. Formatting, whitespace,
roster, batch, raw-JSON and rejection-fixture checks passed. The raw-JSON count
is now 154 across the same 47 files; the extra budgeted fixture preserves a raw
numeric price token that a JSON value would normalize.

Private follow-up logs are `/private/tmp/auctioneer-diagnostic-qualification-fixes-*.log` and
`/private/tmp/auctioneer-diagnostic-qualification-reference-refresh-check.log`. Counts are per invocation,
not additive distinct-test totals. No new full workspace, browser or hosted
provider sweep is claimed for these repairs.

The stricter 35ef524 native run measured all 45 rows but rejected four underload
rows. Its [failed capture](native-axum-35ef524-failed/README.md) is immutable.
The final-wave fixture correction `e0a540f` changed no Rust or criteria and passed
independent source review. Its new predeclaration was committed and pushed as
`ed27119` before traffic. The [corrected capture](native-axum-e0a540f-local-pass/README.md)
passes 45/45 rows with zero runner failures, actual ordinary dispatch/decode
geometry and witnessed GVL overlap. The final resource-harness suite passed
74 tests with its one explicit native opt-in skipped; that opt-in was then run
separately against the new frozen artifact and passed, exercising fourteen real
archived versions. Overall qualification remains unsupported,
publisher readiness false and exit code 1. publisher pilot and pending SDK attestations are
not closed by this local result. Earlier passing and failed captures are retained
without relabeling them as execution of the repaired harness.
Independent evidence review approved source/artifact/profile identities,
byte-for-byte capture/digest parity, all rows and unchanged criteria, ordinary
dispatch/decode isolation, GVL overlap and the stated acceptance limitations.

## Qualification Guard Review

At base `1ecb4f0`, the reported fail-to-qualified mutation already fails the
existing real CLI lifetime-high-water regression: expected exit 1 becomes 0.
The reported 48-unit/six-integration count predates that fix. This follow-up
adds explicit threshold and zero-reading CLI cases, asserting local/overall
verdicts, exit code, readiness and the scenario reason; all lifetime samples
agree so monotonicity cannot mask the intended refusal. Direct helper tests
separately isolate every meter property, both window boundaries, peak/growth
limits, checked-add overflow and unequal platform/installed ceilings. Four
debug-clamp tests use literal below/at/above values and independently vary each
axis. No production evaluator or clamp needed a behavior change.

Seventeen single guard mutations each fail the intended assertion: four debug
clamps, nine meter properties/bounds, verdict mapping, both ceiling comparisons
and headroom overflow. Final red logs remain at
`/private/tmp/auctioneer-diagnostic-qualification-review-mutation-*.log`. The source-mismatch case sets the
matching allocator limitation; an otherwise invalid limitation cannot hide
removal of the source check.

| Fresh scope                                              | Result                                                        | Private log                                             |
| -------------------------------------------------------- | ------------------------------------------------------------- | ------------------------------------------------------- |
| CLI/bidder/exchange, all targets                         | 44 binaries; 824 passed, 0 failed, 3 ignored                  | auctioneer-diagnostic-qualification-review-qualification-and-projection-final.log |
| Final CLI after numeric lint annotations                 | 63 unit, one help and nine integration passed; 2 unit ignored | auctioneer-diagnostic-qualification-review-cli-final-pass.log                     |
| Final bidder library after test lint edits               | 118 passed, 0 failed                                          | auctioneer-diagnostic-qualification-review-projection-final-pass.log              |
| Final exchange library after literal truth-table cleanup | 267 passed, 0 failed                                          | auctioneer-diagnostic-qualification-review-alias-final.log                        |
| Strict scoped all-target Clippy                          | Passed, no lint suppression                                   | auctioneer-diagnostic-qualification-review-clippy-verified.log                    |

Logs are under `/private/tmp`; repeated counts are not distinct-test totals.
Formatting, whitespace, batch, roster and raw-JSON guards plus their rejection
fixtures passed; raw-JSON remains 154 across 47 files with no exemption change.
Independent review approved the final test design, fixed the source-test masking
case before closure, and verified all mutation failures were assertions.
Duplicate wire occurrences and alias permission expectations are now explicit
test contracts. The runbook records the roughly 83x observed-peak margin rather
than claiming tight regression protection. Criteria and all historical captures
are unchanged; Rust edits are confined to tests. This is not a new native
qualification run, full workspace/WASM/browser sweep or hosted execution.

## Rust Harness Migration

Executed in `diagnostic-hardening` on 2026-10-15. The user-approved migration replaces
both non-browser JavaScript harnesses with modules in the existing `auctioneer-cli`;
no new workspace crate or guest dependency is added. Implementation/capture
source is `d68ba170f0b25b40fe4890d9d56bdd9d5048251c`; later `ab9a872` changes
only a `cfg(test)` function name and records native verification. SDK pin is
still `65312643f4fd969bb1651e3f7dbdabd1479b2e74`.

All commands below completed with exit 0. Cargo invocations use `--locked
--offline -j2`; the production usersync script uses equivalent offline/job
environment settings. Counts are per invocation, not distinct-test totals.
The broad test matrix preceded the name-only repair; its unchanged assertion
was then run exactly once in each feature state, followed by strict lint and
cross-target/emulator verification. No production/harness behavior changed.

| Scope                                                                               | Result                                                                                              | Private log suffix                                                         |
| ----------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| `cargo test --workspace --all-targets`                                              | 214 binaries; 3,764 passed, 0 failed, 10 ignored                                                    | `workspace-rerun`                                                          |
| Separate secure-default bidder/exchange/core, all targets                           | 131 binaries; 1,819 passed, 0 failed, 2 ignored                                                     | `secure-foundations`                                                       |
| Secure config: lib and all ten non-harness integration targets                      | 11 binaries; 390 passed, 0 failed                                                                   | `secure-config`                                                            |
| Workspace all targets with `--features insecure-bidders --exclude auctioneer-config`    | 202 binaries; 3,289 passed, 0 failed, 10 ignored                                                    | `feature-workspace`                                                        |
| Feature config: lib/codegen/GVL/smoke/URL, `insecure-bidders test-harness-override` | 5 binaries; 277 passed, 0 failed                                                                    | `feature-config`                                                           |
| CLI unit/help/qualification/protocol/ownership targets, included in workspace       | 345 unit and 20 integration passed; 2 parent-driven unit markers ignored in the ordinary invocation | `workspace-rerun`                                                          |
| Core default/conformance indicators, separate external expectations                 | 5 passed each                                                                                       | `core-default`, `core-conformance`                                         |
| Native resource export with dependency-only core conformance/config override        | 35 passed each                                                                                      | `resource-core-conformance`, `resource-config-override`                    |
| Four native fanout conformance binaries                                             | 77 passed, 0 failed                                                                                 | `native-conformance`                                                       |
| Separate production usersync binary/ignored probe                                   | 1 passed, 0 ignored                                                                                 | `secure-usersync`                                                          |
| Explicit ignored disclosure-model coverage                                          | 1 passed, 0 ignored                                                                                 | `model-coverage`                                                           |
| Explicit ignored Spin 4.1.0 loader                                                  | 1 passed, 0 ignored                                                                                 | `spin-loader`                                                              |
| Renamed override-state test, default and enabled feature                            | 1 passed each, unchanged assertions                                                                 | `config-test-name`, `config-feature-test-name`                             |
| Strict workspace default/insecure and conformance Clippy                            | All three passed with `-D warnings`                                                                 | `clippy-workspace-green`, `clippy-feature-workspace`, `clippy-conformance` |
| Three WASM foundation checks, strict lint and conformance builds                    | All nine passed; no WASM execution inferred                                                         | `wasm-{adapter}-{foundation,clippy,conformance}`                           |
| Actual Fastly library-test WASM guest build                                         | Passed; exact guest selected from Cargo JSON                                                        | `fastly-guest-build`                                                       |
| Explicit local Viceroy 0.21.0 logging/health-store controls                         | 2 passed each, 0 ignored                                                                            | `fastly-boot_logging`, `fastly-manifest_health_stores`                     |
| Preserved browser shutdown controls                                                 | 2 passed, 0 failed, 0 skipped                                                                       | `browser-shutdown`                                                         |

Private logs are `/private/tmp/auctioneer-rust-final-{suffix}.log`. Exact feature,
package, target and environment scopes follow current `test.yml`, `format.yml`
and the vendor workflows. Secure config explicitly includes `activity`,
`baked_defaults_snapshot`, `simple_fold`, `usersync_config` and
`usersync_callback_validation` as well as the six earlier targets. All 84
hermetic build-failure tests completed in the workspace pass; the feature pass
does not duplicate them. All ten ignored marker rows have explicit drivers:
the three parent/child config probes, production usersync, model coverage,
Spin loader and four local Fastly guest controls. The table reports their
executions separately, not as ordinary unignored workspace tests.

Formatting, whitespace, batch rebase, roster, raw-JSON and rejection-fixture
checks passed. Raw JSON remains 157 budgeted fixtures across 49 files. No new
real Chromium flow run is claimed by migration; the earlier seven-flow result
above remains historical and browser-facing application code is unchanged.

### Failures Retained

The first workspace attempt (`workspace.log`, exit 101) saw a zero-byte
connection reset in the slow-response usersync disconnect test. Its exact
case passed standalone and under the original workspace feature graph; six
unchanged concurrent-sibling runs passed 72 tests. The diagnostic workspace
filter was stopped with SIGINT after its intended test passed, not recorded
as a completed workspace pass. The complete unfiltered rerun is in the table.
No assertion, retry or production fix was added. Independent source analysis
found TCP-only readiness can misattribute a released port, but did not prove
that caused the observed reset. `ISSUES.md` retains this separate open case
and the required child/port/caller diagnostics; the older route flake also
remains unresolved.

Initial strict workspace lint (`clippy-workspace.log`, exit 101) rejected the
override test's redundant `test_` prefix. The name-only repair preserved its
assertion and test target; both feature states and all strict reruns pass.
No lint suppression, historical log rewrite or acceptance weakening occurred.

### Measured And External Scope

The [new Rust capture](native-axum-rust-d68ba17-local-pass/README.md) passed all
45 local rows plus the separate frozen control: fourteen archived versions,
three invalid-consent inputs with zero parse credit, sixteen ordinary and
sixteen recovery bids, and unchanged evidence. Predeclaration `7a1b246` preceded
traffic; exact JSON/hashes are retained. The first Rust capture's separate
control failure remains unchanged. Independent read-only source and evidence
reviews approved the repaired capture and stated limitations.

Overall qualification is unsupported, all six SDK attestations pending and
publisher readiness false. Local Viceroy/Spin checks do not execute the
credentialed deployed-handler cases deferred to publisher pilot. No live provider binding,
real bidder, production mutation, pilot or rollout was exercised.

The [latest inspected CI job](https://github.com/IABTechLab/auctioneer/actions/runs/38822023961/job/116720603443)
has no assigned runner or executed step; its annotation reports failed account
payments or an insufficient spending limit. Remote CI is not green and no gate
was weakened to hide this external refusal. Local migration completion remains
separate from PR approval/merge and publisher pilot actual-deployment acceptance.

Final independent read-only review verified counts, links and revision/scope
boundaries and found no actionable reconciliation issue.

## Qualification Guard Follow-Up

The next review identified missing negative controls, not a new measured
qualification. This repair changes only tests and documentation. Evaluator,
harness, SDK and resource criteria remain unchanged; no native traffic was
recaptured and no historical payload was rewritten.

Independent tests now vary consistent histogram percentiles below, at and above
the declared latency threshold, separately from an incorrect reported value.
Unexpected scenario/repetition and startup-variant rows, failed capabilities,
verified capabilities without a digest, and achieved calls above the installed
maximum require exact refusal reasons and overall outcomes. The actual CLI
also rejects altered copies of the committed 45-row Rust observations without
mistaking its existing unsupported result for a successful refusal.

The reported JavaScript equality mutation applies to deleted tooling. Rust's
per-ID fixture coverage and ordinary-only width reconciliation already require
exact counts. Successful and timed-out populations now explicitly reject both
2-of-16 and 17-of-16 alongside the existing 15-of-16 case.

Three development features are intentional: `test_harness_override` supplements
`insecure_bidders` and `conformance_suite`. All eight unique profile subsets are
preserved, duplicate/unknown modes are rejected, and a fourth observation entry
is refused. The profile length guard overlaps finite-enum/uniqueness validation;
a fourth unique valid mode is not independently constructible. Development
identities still cannot become production-qualified. Config's linked override
indicator now has literal, separately cfg-selected expectations, not another
copy of its defining cfg expression. Runtime opacity keeps these assertions
executable without a constant-assertion lint suppression.

The Rust evidence integration target owns all seven retained capture directories,
all 25 indexed raw-byte digests, exact index/file membership and profile bindings.
It reads historical README tables and both Rust checksum manifests. Each payload
and index has negative controls; wrong observation/report profile hashes and
snapshot bytes are deliberately reindexed first, so digest refusal cannot mask
a missing binding check. The original `7106af6` runner/report discrepancy remains
unchanged and is explicitly tracked in `ISSUES.md`. Index consistency does not
establish independent authenticity or qualify a historical attempt.

### Executed Checks

All following final invocations exited 0, using `--locked --offline -j2`:

| Scope                                                                       | Result                                                                                             | Private log                                            |
| --------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- | ------------------------------------------------------ |
| CLI `--all-targets`                                                         | 352 unit plus 26 integration tests passed; 0 failed, 2 existing parent-driven unit markers ignored | `auctioneer-qualification-review-final-cli.log`            |
| Config `--lib`, secure defaults                                             | 42 passed, 0 failed                                                                                | `auctioneer-qualification-review-final-config-default.log` |
| Config `--lib --features test-harness-override`                             | 42 passed, 0 failed                                                                                | `auctioneer-qualification-review-final-config-enabled.log` |
| Strict CLI/config Clippy, both crates `--all-targets`                       | Passed with `-D warnings`                                                                          | `auctioneer-qualification-review-clippy-published.log`     |
| Same strict lint with dependency-only `auctioneer-config/test-harness-override` | Passed with `-D warnings`                                                                          | `auctioneer-qualification-review-final-clippy-enabled.log` |
| Core `--test ci_workflows`                                                  | 75 passed, 0 failed                                                                                | `auctioneer-qualification-review-final-ci-guards.log`      |

Formatting, whitespace and both raw-JSON policy/negative-fixture scripts passed;
the existing 157-fixture/49-file budget is unchanged. The final integrity-only
rerun passed all five tests (`auctioneer-qualification-evidence-published.log`).

Logs are under `/private/tmp`. Seventeen distinct single-guard mutations were
tested in an isolated archived source copy: latency threshold and reported
value, both unexpected-evidence sites, failed capability, verified-without-digest,
call ceiling, observation feature cap, duplicate profile features, per-ID call
coverage, both width weakenings, both false override states, payload digest,
profile binding and snapshot binding. Every mutation compiled and failed its
intended behavior test; restored evaluator controls passed. Detailed logs are
`auctioneer-qualification-mutation-*.log`. An isolated restoration-script setup error
produced a compile failure before the observation-feature mutation; that attempt
is retained as `observation-feature-cap-setup-error.log`, excluded from the
seventeen, and followed by the successful behavioral check.

Earlier lint attempts caught new test-style violations, including constant
assertions, Result-state assertions, shadowed bindings and import/numeric style.
They were corrected without suppression; failed logs remain retained. Independent
review also caught the initially missing negative profile-binding controls; those
were added, independently reindexed, mutation-tested and re-reviewed before this
final matrix. The later unused-extra-file fixture cleanup is verified separately
by the five-test integrity target; it carries no JSON payload or new exemption.

This scoped test-only round does not rerun or replace the earlier full-workspace,
WASM, emulator, measured-native or frozen-control evidence. Live deployment
acceptance remains publisher pilot work, SDK attestations remain pending, and no remote-CI
success is inferred from local results.

## Exact JSON MIME Repair

The next self-review reproduced an exact-policy defect in the locked MIME
parser's API use: `subtype()` returns `json` for `application/json+secret` and
`application/json+json`. The previous predicate therefore authorized sanitized
body capture and normalized debug headers for types outside the fixed
`application/json` policy. No raw-data leak was demonstrated; the projection
still applied its fixed field rules.

The shared predicate now also requires `suffix().is_none()`. Independently
exercised request projection, sealed response projection and header sanitization
reject those suffix-bearing types and mixed-case suffixed JSON. Positive controls
retain case-insensitive JSON and valid parameter values containing `+`; negative
controls retain `empty` precedence for zero-length bodies. The spec now states
the exact MIME essence requirement explicitly.

### Red And Green Checks

Cargo test and Clippy commands use `--locked --offline -j2`, from the diagnostic qualification worktree:

| Command scope                                                                                                          | Observed result                                                                          | Private log                          |
| ---------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- | ------------------------------------ |
| `cargo test -p auctioneer-bidder --lib mime`, tests added before the predicate repair                                      | Exit 101: 1 passed, 3 behavioral failures                                                | `auctioneer-mime-suffix-bidder-red.log`  |
| `cargo test -p auctioneer-exchange --lib mime_suffixes_are_not_normalized_as_json_headers`, before the repair              | Exit 101: 1 behavioral failure                                                           | `auctioneer-mime-suffix-header-red.log`  |
| `cargo test -p auctioneer-bidder -p auctioneer-exchange --lib mime`, after the repair                                          | Exit 0: 5 passed, 0 failed                                                               | `auctioneer-mime-suffix-green.log`       |
| `cargo test -p auctioneer-bidder -p auctioneer-exchange --all-targets`                                                         | Exit 0: 754 passed, 0 failed, 1 existing host-driven model-coverage probe ignored        | `auctioneer-mime-suffix-all-targets.log` |
| `cargo test -p auctioneer-core --test auction_debug_http --test debug_capture_resources --test serialize_allocation_bound` | Exit 0: 8 routed-debug, 4 ownership/cancellation and 6 allocation tests passed; 0 failed | `auctioneer-mime-suffix-core-debug.log`  |
| `cargo clippy -p auctioneer-bidder -p auctioneer-exchange -p auctioneer-core --all-targets -- -D warnings`                         | Exit 0                                                                                   | `auctioneer-mime-suffix-clippy.log`      |

`cargo fmt --all -- --check`, `git diff --check` and both raw-JSON policy and
rejection-fixture scripts passed. The fixture budget remains 157 across 49 files.
The ignored model-coverage probe's earlier explicit execution is historical;
this round does not rerun it or count it as a pass.

The independent read-only reviewer found no actionable issue with the predicate,
its consumers, parameter handling, or positive/negative/empty controls. That
review did not run Cargo; the executions above are separate evidence. Logs are
under `/private/tmp`.

This round changes runtime debug MIME eligibility, not resource criteria or the
SDK pin. No native measurement or frozen-probe run was repeated. Existing
captures remain unchanged and are not represented as captures of this source;
live provider and publisher acceptance remain deferred to publisher pilot.

## SDK And Toolchain Alignment

The user requested current EdgeZero PR #275 and matching tool configuration.
The reviewed head is `596f9f4e23e43c8952a51ce6d92961d461032004`, two commits
beyond `65312643`: the Rust 1.98.1/dependency upgrade and its outbound-branch
integration. All six direct SDK dependencies and eight locked SDK packages
now use that exact revision. Original Content-Type provenance is unchanged;
the SDK also repairs an expired-entry read-guard lifetime in its native KV
store. No Cargo SDK checkout was edited.

Auctioneer's `validator` requirement moves to 0.21 with the SDK's public trait.
Existing audited bytes/Fastly/Spin/Worker versions are exact requirements.
Only SDK-required dependency changes were resolved; Auctioneer's cookie HMAC and
SHA-256 dependencies retain their existing versions. The SDK's newer SHA-256
implementation remains a separate dependency, not a replacement for those
cookie-integrity dependencies.

`.tool-versions` now matches the SDK's six declarations: Fastly 16.0.0,
Node.js 24.20.0, Rust 1.98.1, Spin 4.1.0, Viceroy 0.21.0 and Wasmtime 48.0.2.
Every Rust CI setup already reads that file. Both Node setups now use its
supported `node-version-file` input without a higher-precedence version
override. Provider installers read the same file. Fastly CI deliberately
moves from its separate 16.1.0 literal to the SDK's reviewed 16.0.0; its Linux
archive checksum was cross-checked against the official release checksum
file and the SDK's installer manifest. Viceroy retains its existing checksum.
The Rust workflow guards require complete active installer commands, reject
Node version/source overrides and pin all six tool declarations independently.

The 1.98.1 lint migration preserves existing code conventions centrally:
inline modules, inline bounds and lazy async trait implementations remain
permitted. Stable Rust cannot implement `Drop::pin_drop` alongside `drop`,
so that provided-method restriction is handled by the same central style
allowlist. Eight stale module-level allowances were removed. No new per-site
`expect` was introduced; non-trait `unused_async`, strict warning enforcement
and the other correctness restrictions remain unchanged. A redundant test
import and byte-array fixture spellings were also corrected without changing
fixture bytes or assertions. The egress unit-test poll helper now uses the
standard `Waker::noop()` instead of an allocating no-op implementation.

### Checks Recorded

The toolchain reports rustc 1.98.1 (`48a229cea`, LLVM 22.1.8). Cargo checks
use `--locked --offline -j2`; provider archive controls use the exact verified
SDK checkout through `EDGEZERO_SDK_ROOT`.

| Scope                                                                                                        | Observed result                                                                                         | Private log                                                               |
| ------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------- |
| CI guards before tool-file/workflow alignment                                                                | Exit 101: 24 passed, 53 behavioral failures                                                             | `auctioneer-sdk-toolset-red.log`                                              |
| CI guards after alignment and command-target repair                                                          | Exit 0: 77 passed, 0 failed                                                                             | `auctioneer-sdk-toolset-verified.log`                                         |
| Browser-process shutdown on Node.js 24.20.0                                                                  | Exit 0: 2 passed, 0 failed                                                                              | `auctioneer-sdk-596f9f4-node-tests.log`                                       |
| `cargo test --workspace --all-targets`                                                                       | Exit 0: 3,782 passed, 0 failed, 10 existing markers ignored; all 84 isolated build-failure tests passed | `auctioneer-sdk-596f9f4-workspace-tests.log`                                  |
| Separate secure defaults: bidder, exchange, core, runtime, openrtb and privacy, all targets                  | Exit 0: 2,503 passed, 0 failed, 2 host-driven markers ignored                                           | `auctioneer-sdk-596f9f4-secure-foundations.log`                               |
| Secure config library plus all ten non-harness integration targets                                           | Exit 0: 390 passed, 0 failed                                                                            | `auctioneer-sdk-596f9f4-secure-config.log`                                    |
| Four native `fanout_conformance` binaries with `conformance-suite`                                           | Exit 0: 77 passed, 0 failed                                                                             | `auctioneer-sdk-596f9f4-native-conformance.log`                               |
| Axum `resource_profile`, dependency-only `auctioneer-core/conformance-suite`                                     | Exit 0: 35 passed, 0 failed                                                                             | `auctioneer-sdk-596f9f4-profile-conformance.log`                              |
| Axum `resource_profile`, dependency-only `auctioneer-config/test-harness-override`, explicit enabled expectation | Exit 0: 35 passed, 0 failed                                                                             | `auctioneer-sdk-596f9f4-profile-override.log`                                 |
| Config library with `test-harness-override`                                                                  | Exit 0: 42 passed, 0 failed                                                                             | `auctioneer-sdk-596f9f4-config-override.log`                                  |
| Config library/codegen/archive/smoke/URL safety with `insecure-bidders test-harness-override`                | Exit 0: 277 passed, 0 failed                                                                            | `auctioneer-sdk-596f9f4-config-insecure.log`                                  |
| Strict workspace all-target Clippy                                                                           | Exit 0                                                                                                  | `auctioneer-sdk-596f9f4-clippy-rerun.log`                                     |
| Strict workspace all-target Clippy with `insecure-bidders`                                                   | Exit 0                                                                                                  | `auctioneer-sdk-596f9f4-clippy-insecure.log`                                  |
| Strict core/three-provider all-target Clippy with `conformance-suite`                                        | Exit 0                                                                                                  | `auctioneer-sdk-596f9f4-clippy-conformance.log`                               |
| Cloudflare `wasm32-unknown-unknown`: foundation, strict all-target conformance lint, conformance build       | All 3 commands exit 0                                                                                   | `auctioneer-sdk-596f9f4-cloudflare-{foundation,clippy,conformance-build}.log` |
| Fastly `wasm32-wasip1`: same three commands                                                                  | All 3 commands exit 0                                                                                   | `auctioneer-sdk-596f9f4-fastly-{foundation,clippy,conformance-build}.log`     |
| Spin `wasm32-wasip2`: same three commands                                                                    | All 3 commands exit 0                                                                                   | `auctioneer-sdk-596f9f4-spin-{foundation,clippy,conformance-build}.log`       |
| Explicit ignored disclosure-model compiler-coverage probe                                                    | Exit 0: 1 passed, 0 failed                                                                              | `auctioneer-sdk-596f9f4-model-coverage.log`                                   |
| Explicit ignored Spin 4.1.0 manifest-loader probe                                                            | Exit 0: 1 passed, 0 failed                                                                              | `auctioneer-sdk-596f9f4-spin-loader.log`                                      |
| Final `ci_workflows` and `codec_pins` regression run                                                         | Exit 0: 77 plus 2 passed, 0 failed                                                                      | `auctioneer-sdk-596f9f4-final-pins.log`                                       |

Counts are per invocation, not unique-test totals. Secure defaults run without
the Axum dev-dependency feature unification. The full workspace run preceded
the final test-only USP byte spelling and no-op-waker cleanup; both owners
were subsequently rerun in the secure all-target sweep. The resource-export
feature modes run separately; the override invocation independently sets
`AUCTIONEER_EXPECT_CONFIG_OVERRIDE=enabled`.

The WASM rows each execute the following exact command templates, where
`adapter` is the matching `auctioneer-adapter-*` package and `target` is the
table's target. These compile, lint and link; they do not execute the guest:

```sh
cargo check -p auctioneer-runtime -p auctioneer-openrtb -p auctioneer-config -p "$adapter" --target "$target" --locked --offline -j2
cargo clippy -p "$adapter" -p auctioneer-exchange -p auctioneer-privacy --all-targets --features conformance-suite --target "$target" --locked --offline -j2 -- -D warnings
cargo test --no-run -p "$adapter" --features conformance-suite --test fanout_conformance --target "$target" --locked --offline -j2
```

`cargo fmt --all -- --check`, `git diff --check`, batch/roster/raw-JSON guards
and both rejection-fixture suites passed. Raw-JSON accounting remains 157
fixtures across 49 files; the roster remains 139. Comparing the two actual
`.tool-versions` files against the verified SDK checkout returned no difference.
Independent read-only review approved the tool/CI configuration, including
the Spin mutation probe's explicit command selection. The final SDK-head
recheck still returned `596f9f4e`.

Initial strict Clippy runs rejected the new style restrictions, unsupported
provided Drop method, redundant import, byte-array spelling and manual no-op
waker; the failed logs remain retained. In particular, `clippy-verified.log`
is the retained failed USP/waker attempt; `clippy-rerun.log` is the successful
full-workspace run. The first aligned CI run passed 76 tests but exposed
an existing Spin mutation test's positional command selection. It now names
the manifest-validation command explicitly; no rejection assertion was removed
or weakened. Logs are under `/private/tmp`.

Cargo also retains a transitive future-incompatibility warning for
`validator_derive`'s `proc-macro-error2` 2.0.1: its private `extern crate
proc_macro` is publicly re-exported at `src/lib.rs:494` (E0365). Current
Rust 1.98.1 builds and strict lint succeed. This is recorded as dependency
maintenance in `ISSUES.md`, not suppressed or called a warning-free build.

Rust, Node.js, Fastly, Spin and Viceroy were observed at the declared local
versions. Local Wasmtime remains 45.0.0, not the optional declared 48.0.2;
it is not used as a runner in this verification. Install the declared version
before using that optional SDK conformance tooling. No version declaration
is represented as installation or runtime execution.

This upgrade does not recapture resource measurements, rerun the frozen
probe, rebuild vendor release packages, rerun Fastly guest controls or qualify
a provider deployment. Historical candidate profiles,
captures, digests and their compiler/SDK identities remain unchanged. Their
previous local acceptance is not evidence for this new SDK/compiler source.
Live provider and publisher acceptance remain publisher pilot work.

## Empty-Auction And Abort Guard Follow-Up

This repair changes only colocated CLI tests and their synthetic candidate
helper. Production logic, actual resource profiles, acceptance thresholds,
SDK/tool declarations and retained capture bytes/digests are unchanged.

The empty-success case funds all 48 calls for three completed auctions and
retains two successful siblings' decode credit. It must refuse solely because
the third HTTP-success auction returns zero bids. Replacing that outcome with
one bid passes and credits 33 decoded bids. The existing combined regression
also funds the third auction instead of accidentally failing its call floor.

The native runner test independently classifies interrupted, run_deadline and
cleanup_failed as fatal, both without stage metadata and at the scenario stage.
Three ordinary measurement errors remain nonfatal controls. This pins the
classifier; it is not a new end-to-end signal or cleanup qualification.

A four-call synthetic candidate funds 12,288 assembly bytes and refreshes its
derived resource/debug references. Planned calls independently range from zero
through five. Evaluation holds planned work at two and installed capacity at
four while achieved calls range from zero through five, asserting scenario
reason, local acceptance and overall qualification. Only the test candidate's
assembly funding changes; no measured or deployment profile is relaxed.

### Guard-Removal Controls

An initialized detached checkout of 5bb43b5 received exactly the four test-file
edits. Each of the four named tests first passed individually. Each mutation
then compiled, executed exactly one named test and exited 101 with its intended
assertion failure: zero passed, one failed, not an empty filter or compile error.
Every mutation was individually restored before the next; the disposable
checkout was removed after all nine controls.

| Removed guard                          | Observed result | Private log suffix    |
| -------------------------------------- | --------------- | --------------------- |
| Zero-bid invalid flag                  | Test failed     | empty-success-flag    |
| Invalid-outcome refusal                | Test failed     | empty-success-refusal |
| interrupted fatal classification       | Test failed     | abort-interrupted     |
| run_deadline fatal classification      | Test failed     | abort-deadline        |
| cleanup_failed fatal classification    | Test failed     | abort-cleanup         |
| Planned calls above installed maximum  | Test failed     | planned-call-ceiling  |
| Planned calls equal zero               | Test failed     | planned-nonzero-calls |
| Achieved calls above installed maximum | Test failed     | achieved-call-ceiling |
| Achieved calls below planned work      | Test failed     | achieved-call-floor   |

Mutation logs are /private/tmp/auctioneer-diagnostic-qualification-mutant-SUFFIX.log; four individual
baselines are auctioneer-diagnostic-qualification-call-guards-baseline-0.log through -3.log. The corrected
qualification-name selection also passed 64 tests in both the actual diagnostic qualification
worktree and probe checkout. An independent read-only review approved the
test-only changes, including the corrected four-call assembly funding; the
parent executed all tests and mutations.

### Attempts Retained

An initial --lib invocation selected no target because auctioneer-cli is binary-only.
The first --bin auctioneer-cli run passed 353 tests but failed two new cases: the
four-call candidate still funded only one call's 3,072-byte assembly floor and
was correctly refused as InvalidProfile. Correcting only that synthetic funding
made the 64 qualification-name tests pass; these setup failures are not mutation
evidence. Logs are auctioneer-diagnostic-qualification-call-guards-unit.log, -bin.log and -qualified.log.

The first all-target attempt reused a cached build script containing the deleted
probe's compile-time manifest path and failed before tests. Local Auctioneer package
artifacts were cleared while the dependency cache was retained. The subsequent
sandboxed run passed 307 tests, failed 48 socket/process-dependent cases and
ignored two existing markers. Direct bind errors reported PermissionDenied;
fixture/child startup refusals also prevented their dependent checks. No source
or assertion was changed to bypass these failures; the complete command was
rerun with local socket/process permissions. Both failed logs remain as
auctioneer-diagnostic-qualification-call-guards-all-targets.log and -all-targets-rerun.log.

Strict CLI Clippy initially rejected default integer fallback in the new planned
call table. An explicit u32 suffix fixes its type without changing serialized
inputs, assertions or lint policy. The failed log is
auctioneer-diagnostic-qualification-call-guards-clippy.log; the successful rerun is -clippy-verified.log.

### Final Checks

On Rust 1.98.1 / SDK 596f9f4e, the complete CLI command passed both before and
after the type-only lint correction. The final invocation was:

```sh
env CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 \
  RUST_TEST_THREADS=4 EDGEZERO_SDK_ROOT=/path/to/verified/596f9f4 \
  cargo test -p auctioneer-cli --all-targets --locked --offline -j2
```

The SDK path names the same verified checkout used by the preceding upgrade
record. Final log: /private/tmp/auctioneer-diagnostic-qualification-call-guards-all-targets-final.log.
The earlier passing full run remains in -all-targets-verified.log.

| Target                        | Passed | Failed | Ignored |
| ----------------------------- | ------ | ------ | ------- |
| Binary unit tests             | 355    | 0      | 2       |
| help                          | 1      | 0      | 0       |
| qualification_cli             | 10     | 0      | 0       |
| qualification_evidence        | 5      | 0      | 0       |
| qualification_harness         | 6      | 0      | 0       |
| qualification_migration       | 4      | 0      | 0       |
| Total per complete invocation | 381    | 0      | 2       |

Both ignored markers are existing environment-probe children driven by their
parent tests, not skipped repair cases. Strict all-target CLI Clippy passed with
the same low-disk build settings and -- -D warnings. Workspace formatting,
diff whitespace and both raw-JSON policy/rejection checks passed; accounting
remains 157 fixtures across 49 files. The rebuilt target directory was about
1.3 GiB, with debug symbols and incremental caches disabled.

All four edited Rust files have identical production prefixes before cfg(test)
compared with 5bb43b5. No full workspace, WASM, browser, new resource capture,
frozen-probe or live-provider qualification rerun is claimed for this test-only
repair. The preceding SDK verification remains separate historical evidence;
hosted CI and publisher pilot deployment acceptance are not inferred from these checks.

## Qualification Boundary Repair

User-approved repairs follow base `43cb589`. SDK `596f9f4e` and Rust 1.98.1 are
unchanged. Production auction code, resource criteria and every retained capture
remain unchanged; the new tar/filetime dependency belongs only to `auctioneer-cli`.
This is harness/input-boundary verification, not a new resource measurement.

### Reproductions and Controls

- Final RSS now starts after both work completion and the minimum window. The
  unchanged-source service probe previously reported zero growth after retaining
  32 MiB; repaired source reports 33,619,968 bytes, matching the independent RSS
  endpoint. The slower positive control also reports that growth. Nine paused-
  clock tests independently cover both boundaries, the literal 128-sample limit,
  read failure, post-read cancellation and elapsed time. The initial old-loop
  run failed six tests and passed its positive control.
- The old JSON reader failed six duplicate-key controls. Recursive decoded-key
  validation now precedes Value construction, including escaped/nested/ignored
  keys. The real CLI returns 0/pending/ready-false for valid offline evidence, 2
  with mutation_not_isolated for a single false authorization, and 2 with
  invalid_input for duplicate authorization or artifact digests. Integration
  tests also independently check the single mismatched identity refusal.
- Four initial archive refusals reached sdk_identity instead of release_invalid;
  the exact-cap positive control reached sdk_identity as intended. Preflight now
  bounds raw entries, expansion and all member payloads before SDK lookup. An
  intermediate PAX path/size table failed both refusal expectations; structural
  overrides are now unconditionally rejected, with mtime as a positive control.
  A separate framing red exposed empty/blank-record acceptance and is repaired.
  PAX is intentionally limited to complete single-line metadata records, not
  general multiline POSIX PAX support. GNU names remain bounded and validated.
- Fault-injected writer tests cover sync failure, successful ordered sync and
  body/newline refusal. A copied-source mutation removing the sync call compiled,
  then failed both sync-specific assertions: one passed, two failed, exit 101.
  This does not claim to simulate an actual full filesystem.
- The newline CLI regression first failed because the report ended in `}`.
  It now emits one complete newline-terminated record. Evaluating the retained
  d68ba17 profile/observations returned the expected unsupported exit 1 and plain
  cmp against its committed report returned 0, without appending a newline.

Red/control logs are `/private/tmp/auctioneer-diagnostic-qualification-boundary-{io-red,meter-red,
archive-red,newline-red,pax-both-red,pax-framing-red,sync-mutant}.log`.
No mutation was applied to the shared worktree's production source.

### Final Verification

Cargo used `--locked --offline -j2`, dev/test debug disabled, incremental disabled,
four test threads and an explicit verified EDGEZERO_SDK_ROOT at 596f9f4e.
Counts are per invocation, not distinct-test totals.

| Exact Scope                                                        | Result                                          | Private Log                              |
| ------------------------------------------------------------------ | ----------------------------------------------- | ---------------------------------------- |
| core/exchange/bidder/runtime/privacy/openrtb/cli, all targets      | 160 binaries; 2,923 passed, 0 failed, 4 ignored | auctioneer-diagnostic-qualification-boundary-native-verified.log   |
| core/Cloudflare/Fastly/Spin, conformance-suite, fanout_conformance | 77 passed, 0 failed                             | auctioneer-diagnostic-qualification-boundary-conformance-final.log |
| Axum cancellation/wave_transport/resource_profile                  | 52 passed, 0 failed                             | auctioneer-diagnostic-qualification-boundary-axum-final.log        |
| CLI all-target Clippy, -D warnings                                 | Exit 0                                          | auctioneer-diagnostic-qualification-boundary-clippy-complete.log   |

All logs are under `/private/tmp`. Ignored markers are not counted as executed.
No config build-failure-harness, new full-workspace, WASM, browser or credentialed
provider sweep is claimed. The existing derive-dependency future-incompatibility
warning remains tracked, not suppressed. Formatting, whitespace, batch-rebase,
roster and raw-JSON gates plus rejection controls passed. Raw-JSON accounting is
169 fixtures across 50 files; twelve new object literals specifically preserve
duplicate/escaped keys that json! cannot represent.

The first combined run reported fixture_changed while source was still being
edited. Frozen-source reruns retain every guard. The actual BSD tar control also
exposed hidden AppleDouble/xattr records, then passed with portable archive
creation (COPYFILE_DISABLE and no-xattrs/no-acls), not relaxed member checks.
Those failed attempts remain in the boundary-cli-first, boundary-cli-final,
boundary-sdk-isolated and boundary-sdk-diagnostic private logs. Initial strict
lint failures were resolved through imports/types, checked conversions/arithmetic
and formatting, with no lint suppression. A 2,922-test passing native attempt
preceded the final canonical-framing guard; the final 2,923-test row supersedes
it rather than attributing that earlier run to later source.

### Peer Reconciliation

Abort-code and independent planned/installed-call guards were already repaired
by 43cb589; their real tests execute in the final native sweep. MIME probes cover
all sixteen requested/control variants on both projection paths; only exact JSON
types capture. The SDK bump's ten-line static_egress deletion removed only a lint
attribute, not funding logic. Historical verification sections retain their own
source/SDK identities and are not current-deployment certification.

The runner/evaluator missing-schedule invariant is now explicit in the design
and runbook. Fixture count/ID checks remain intentional runtime defenses on the
deserialized child protocol, with independent forged-count/repeated-ID controls.
Sync failure and newline consistency are tested as above. The CLI integration
case now declares test_harness_override alone; other development features no
longer mask its expectation. ISSUES.md records these dispositions. Hosted CI,
fresh resource qualification and all deferred live acceptance remain unclaimed.

## Archive Guard Ownership: October 16

This repair follows `bf301ee` on `diagnostic-hardening`. Rust 1.98.1, SDK
`596f9f4e23e43c8952a51ce6d92961d461032004`, dependency pins, archive size
ceilings and every retained measurement/capture are unchanged. The
[repair plan](../../superpowers/plans/2026-10-16-archive-verification-guards.md)
records implementation and verification ownership. bidder catalog is not changed.

### Policy and Boundary Changes

`ReleaseMetadata::validate` now owns format, lifecycle protocol, adapter,
source revision, safe/unique member paths, digest syntax and exact member
membership. Both raw-archive preflight and extracted-bundle verification
use it; the copied four-term identity block is gone. Extracted enumeration
is bounded before shared validation and file hashing. The raw pass owns
resource upper bounds, not completeness: its redundant exact-file/minimum
check and the semantic pass's duplicate upper-file check are removed.
The raw file/record caps and independent filesystem file/node caps remain.

New controls accept a complete archive and additional zero padding, but
reject a hidden sixth member after the terminator in both single-gzip and
concatenated-gzip layouts. The zero-only expanded fixture independently
reaches raw resource refusal without a metadata/missing-file backstop.
Direct filesystem tests isolate the regular-file and cumulative-node
limits. Common policy tests supply missing/replaced/extra names and
matching attacker declarations with unsafe paths, duplicate roles or bad
digests, so another layer cannot supply their verdict.

### Isolated Mutation Evidence

A disposable source snapshot received the current archive/artifact files.
The clean tail baseline passed. Each mutation below was applied alone,
compiled successfully and failed the intended test, with no empty filters
or compiler-error credit. Mutations never touched the shared worktree.

| Removed Predicate                 | Failed Tests per Isolated Run | Log Suffix      |
| --------------------------------- | ----------------------------- | --------------- |
| Nonzero trailing tar data         | 1                             | tail            |
| Metadata format                   | 2, one per caller             | format          |
| Lifecycle protocol                | 2, one per caller             | lifecycle       |
| Adapter identity                  | 2, one per caller             | adapter         |
| Source revision                   | 2, one per caller             | source          |
| Raw regular-file ceiling          | 1                             | raw-files       |
| Raw record ceiling                | 1                             | raw-records     |
| Extracted regular-file ceiling    | 1                             | extracted-files |
| Extracted cumulative-node ceiling | 1                             | extracted-nodes |
| Exact declared-member set         | 1                             | member-set      |
| Member path safety                | 1                             | member-path     |
| Digest syntax                     | 1                             | member-digest   |
| Unique member role                | 1                             | member-role     |
| Zero-only expanded-stream refusal | 1                             | expansion       |

All fourteen compiled removals exited 101 with their assertion failures.
Private logs are `/private/tmp/auctioneer-diagnostic-qualification-archive-mutant-SUFFIX.log`;
`-baseline.log` is the passing unmodified tail control. The source snapshot
is not a deployed artifact or new source/resource qualification.

### Executed Checks

Cargo commands used `--locked --offline -j2`, dev/test debug disabled and
incremental disabled. The complete CLI runs used four test threads and
the explicit verified SDK checkout, with local process/socket permissions.
The final complete run follows the last test-only zero-expansion assertion.

| Scope                                                                    | Result                                              | Private Log Suffix |
| ------------------------------------------------------------------------ | --------------------------------------------------- | ------------------ |
| CLI all targets, final                                                   | 430 passed, 0 failed, 2 ignored across six binaries | cli-final          |
| Strict default workspace Clippy, all targets                             | Exit 0                                              | clippy-workspace   |
| Strict insecure-bidders workspace Clippy, all targets                    | Exit 0                                              | clippy-insecure    |
| Strict core/Cloudflare/Fastly/Spin conformance-suite Clippy, all targets | Exit 0                                              | clippy-conformance |
| Cloudflare/shared exchange/privacy CI WASM Clippy                        | Exit 0                                              | clippy-cloudflare  |
| Fastly/shared exchange/privacy CI WASM Clippy                            | Exit 0                                              | clippy-fastly      |
| Spin/shared exchange/privacy CI WASM Clippy                              | Exit 0                                              | clippy-spin        |

Logs use `/private/tmp/auctioneer-diagnostic-qualification-archive-review-SUFFIX.log`. CLI unit results
are 401 passed and two existing ignored environment-probe children; the
integration binaries add 1, 11, 5, 8 and 4 passes. The actual complete-archive
pinned-SDK control executes in this run. Counts are per invocation, not
distinct-test totals. Ignored markers are not counted as executed root tests.
The final CLI-only strict lint follows the last test-only assertion; its
log is `auctioneer-diagnostic-qualification-archive-review-clippy-cli-final.log`.

WASM lint commands select the actual adapter plus exchange/privacy,
`--all-targets --features conformance-suite`, and respectively
`wasm32-unknown-unknown`, `wasm32-wasip1`, `wasm32-wasip2`, with `-D warnings`.
These are compilation/lint checks, not executed WASM or package-size tests.
Formatting, whitespace and both raw-JSON gate/rejection controls pass;
the count remains 169 object-leading source-line matches across 50 files.
`io.rs` has thirteen raw literals, but one starts with an array and is
intentionally outside this existing object-fixture rule. No allowance grew.

### Lint and Evidence Reconciliation

The two functional workspace allowances are removed. Exceptions are local
to the affected impls: stable Drop cannot implement unstable/mutually
exclusive `pin_drop`; the JSON visitor and other external traits retain
their intentional defaults; immediate async controls preserve lazy reads
and side effects. No destructor, future or stream implementation body
was rewritten. `inline_modules` and `inline_trait_bounds` deliberately
remain repository style policies, not claimed as narrowed exceptions.
Existing expectations are preserved; no test-module relocation or
per-function inline-bound annotation was introduced.

The top header now identifies the original matrix's historical SDK pin
instead of calling it current. Its source's Cargo.toml confirms that pin;
later scoped results remain separately attributed. E0365 remains Open in
`ISSUES.md`: the fresh Cargo report still reaches proc-macro-error2 2.0.1
through validator_derive 0.20.0. No transitive warning is suppressed and no
unqualified dependency patch/upgrade is introduced. Detailed report:
`/private/tmp/auctioneer-diagnostic-qualification-archive-review-future.log`.

### Attempts and Limits

The first command used the wrong bin name and selected no target; the
correct target is `auctioneer-cli`. An early artifacts-only invocation ran
while source was being edited and correctly failed one `fixture_changed`
control (13 passed, one failed). Frozen-source complete runs both passed
430 tests with zero failures. Initial strict lint rejected numeric fallback
and soft Result assertions in new tests; explicit types, checked counting
and concrete success assertions fixed them without weakening policy.

Independent read-only review found no actionable issues in the shared
validator, count ownership, test seams or attribute-only lint changes,
and confirmed the final zero-only resource-stage control. No fresh full
workspace test, live provider, browser, package-size, source-bound resource
measurement or publisher qualification is claimed. publisher pilot execution and the
existing hosted-CI caveat are unchanged by these local checks.

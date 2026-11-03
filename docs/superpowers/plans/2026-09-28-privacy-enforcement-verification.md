# privacy enforcement Implementation Verification

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

Date: 2026-09-28
Final checkpoint verification: 2026-09-29
Latest branch-alignment verification: 2026-10-04
Classified-scrub follow-up: [2026-10-11 execution record](../sdd/2026-10-11-gdpr-scrub-classification.md)
Peer-review coverage follow-up: [2026-10-12 repair record](../sdd/2026-10-12-privacy-review-coverage.md)
Worktree: `.claude/worktrees/privacy-enforcement`
Base: `ba0ddd7`
Status: privacy enforcement privacy, auction enforcement, and shared live resource accounting
are locally implemented. Current branch-alignment evidence is recorded below;
earlier sections retain the chronological checkpoints, not outstanding work.
Local acceptance does not establish hosted provider execution, packaging, or
physical-isolate memory safety. bounded fanout has merged; PR #17 targets `main` and
remains draft. The sections below are historical checkpoints; the linked
follow-up record owns current classified-scrub qualification.

## Branch Alignment: 2026-10-04

The remaining privacy enforcement fixes were reconciled onto `e36e7c3`, which includes bounded fanout
through `ec5a0d5`. Approved privacy enforcement privacy scope is unchanged. bounded fanout's bounded
messages, alias validation, header-copy allowance, outbound caps, module
layout, and health bootstrap fixes remain intact. No user sync endpoint or registry
work was added.

- Malformed bidder responses now produce fixed code-14 diagnostics instead
  of third-party parser messages. Unsupported currency still drops bids and
  emits code 10004, but does not echo the upstream currency value. Regression
  tests first failed on leaked response markers and currency, then passed
  through every compiled production bidder plus the insecure test bidder.
  Large and escaped values, malformed/truncated JSON, and USD/default-currency
  success controls are covered. The 1 KiB constructor bound remains in place;
  bounding a message is not a substitute for sanitizing its source.
- Account validation inspects supplied activity replacements without
  materializing a merged activity map per account. Baked rules are already
  build-validated, replacement arrays use the same bounds, and the merge does
  not append rules or add cross-field constraints. The 4,000-account allocation
  regression first measured 1,692,236 bytes against a 708,236-byte direct
  validation baseline; removing the unnecessary merges restores the baseline.
  Rule, condition-entry, and UTF-8 name boundaries and the hermetic custom-baked
  runtime parity test remain green.
- Disabled aliases that collide with registered bidder names have regression
  coverage across request/host aliases and all three impression-extension
  shapes. Tests prove no construction or fanout for the disabled identity,
  preserve its single disabled warning, and retain the declared base for an
  enabled alias. They exercise the existing exclusion guard, not a new routing
  policy. Both configuration and privacy integration tests share
  `tests/support/auction.rs`; the harness retains bounded fanout's preparation defaults.
- README outbound-limit examples now name `[baked.host.resource_limits]` and
  `[runtime.auction]` accurately. Independent read-only review found no blocking
  issue in this patch.

Checks use `CARGO_TARGET_DIR=/private/tmp/auctioneer-privacy-enforcement-resolution-target`,
`RUSTC_WRAPPER=`, `CARGO_INCREMENTAL=0`, `CARGO_NET_OFFLINE=true`, and
`--locked --offline`. Real transport tests run with loopback socket access.

| Check                                                                                       | Result                                                                                    |
| ------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| Default workspace, all targets, excluding `auctioneer-config`                                   | Passed: 1,537 tests, 0 failed, 3 ignored probes across 134 targets                        |
| Workspace with `insecure-bidders conformance-suite`, all targets, excluding `auctioneer-config` | Passed: 1,653 tests, 0 failed, 3 ignored probes across 138 targets                        |
| Focused exchange auction, config overlay, and privacy integration                           | Passed: 76 tests                                                                          |
| Core config allocation and runtime/shared-schema parity                                     | Passed: 6 tests; emitted-baked probe exercised separately by its parent below             |
| Config library, activity, simple-fold, snapshot, codegen, archive, smoke, URL safety        | Passed: 170 tests across 8 targets                                                        |
| Hermetic custom-baked/runtime privacy-config parity                                         | Passed: 1 parent test, which executes the emitted-baked probe                             |
| Workspace all-target strict Clippy, default and insecure/conformance features               | Passed                                                                                    |
| Cloudflare, Fastly, Spin WASM checks                                                        | Passed on `wasm32-unknown-unknown`, `wasm32-wasip1`, `wasm32-wasip2`; compile checks only |
| Formatting, diff, batch-rebase, roster, roster fixtures, raw-JSON fixtures                  | Passed                                                                                    |

The remaining 56 build-failure harness cases were not rerun for this patch;
their earlier evidence is retained below. WASM checks are not hosted execution
or package-size qualification. GitHub's current failed check has no executed
steps: its annotation reports failed account payments or an insufficient
spending limit. Local green results must not be described as green hosted CI.

## bounded fanout Base Refresh: 2026-10-04

privacy enforcement incorporates bounded fanout through `0ac43f2`, including `189ad2f`. Winner selection
borrows bid slices instead of cloning complete responses, and cold probe-config
resolution occupies a gate slot when one is available. privacy enforcement's pre-targeting
resource checks and survivor-only projection remain intact. The auxiliary
budget now funds both independent aux and probe-config populations, retaining
privacy enforcement's larger measured factors and the separate compiled-config allowance. Exact
totals and overshoots were recomputed for all four adapters and the core boot
warning fixture, not replaced with bounded fanout's smaller baseline figures.

Repeated probe tests exposed a process-global gate race: the two tests did not
share the existing isolation guard. Both now acquire it; assertions and
production behavior are unchanged. Independent review confirmed the budget and
projection merge, and identified a Cloudflare logging mismatch: the widened
filter admitted INFO/DEBUG records, but emitted them as WARN. Quiet records now
use `console_log!`, with WARN/ERROR keeping their original console channels.

| Refresh Check                                                                  | Result                                                                                        |
| ------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------- |
| Default workspace, all targets, excluding `auctioneer-config`                      | Passed: 1,539 tests, 0 failed, 3 ignored probes across 134 targets                            |
| Workspace with `insecure-bidders conformance-suite`, excluding `auctioneer-config` | Passed: 1,655 tests, 0 failed, 3 ignored probes across 138 targets                            |
| Axum boot installation, default and alternate features                         | Passed: 1 test in each configuration                                                          |
| Probe gate regression with two test threads                                    | Passed: 100 consecutive runs of both tests                                                    |
| Cloudflare boot-log regression after severity correction                       | Passed: 5 tests; source-channel regression first failed without `console_log!`                |
| Workspace all-target strict Clippy, both feature configurations                | Passed after final logging correction                                                         |
| Cloudflare, Fastly, Spin WASM checks                                           | Passed on their respective deployment targets; Cloudflare rechecked after severity correction |
| Formatting, diff, batch-rebase, roster and raw-JSON fixtures                   | Passed                                                                                        |

The first refreshed alternate-feature workspace invocation passed 1,654 tests
but failed Axum's boot integration test on its stale total. Updating that exact
golden resolved the failure; the complete workspace invocations above were
rerun cleanly. The subsequent console-channel correction has focused regression,
strict lint, and WASM compile evidence; the workspace totals precede that final
WASM-only emission change. These checks use the same offline target directory
and loopback access as the preceding branch-alignment checkpoint. Config
build-failure harnesses were not rerun: this refresh changes no config schema,
code generation, or baked defaults. Hosted execution and provider packaging
remain separate from these local checks.

## Review

The privacy enforcement design and implementation plan were independently reviewed before code
changes. Four design findings were corrected and re-reviewed:

1. Make the bounded fanout live admission/construction/accounting handoff explicit.
2. Accept validated alias identities dropped only for disablement in `nosale`.
3. Record the negative-coordinate rounding divergence from the pinned reference.
4. Carry source/section provenance into fixed warning scopes and ordering.

No blocker to starting independent privacy enforcement work remained. GPP parser qualification
and corrected live bounded fanout resource paths remain integration dependencies. The
master privacy enforcement summary, activity/scrub rules, and GPP error/projection contracts were
aligned without marking the capability complete.

## Baseline

Unless noted otherwise, commands use `--locked --offline` and
`--target-dir /private/tmp/auctioneer-privacy-enforcement-baseline-target` from this worktree.

- `cargo test -p auctioneer-privacy --lib`: 217 passed, 1 ignored.
- `cargo test -p auctioneer-exchange --lib scrub`: 12 passed.
- `cargo fmt --all -- --check`: fails on pre-existing formatting in untouched
  core/runtime/Fastly files. Those files are not reformatted by this change;
  changed Rust files are checked separately below.

## Foundation Checkpoint

- USP parser: implemented; independent spec-compliance and code-quality reviews
  passed. Red run failed with E0432 missing `Usp`/`UspParseError`; after
  implementation the focused `usp` filter passed 12 tests (11 new USP tests
  plus one existing matching test). Fresh full privacy library suite: 228
  passed, one ignored. The full privacy crate suite also passed: 255 tests,
  one ignored (library 228; integration 6 + 19 + 2). Scoped rustfmt passed.
- Complete scrub primitives: implemented; independent spec-compliance and
  code-quality re-reviews passed. Initial red run failed with E0432 for six
  missing APIs. Spec review identified
  a same-field `user.other["geo"]` / `device.other["geo"]` shadow gap. Five new
  COPPA/coarsening regression tests failed before the fix (full exchange library
  run: 92 passed, five failed). The fix clears existing typed/shadow geo objects
  to `{}` for COPPA. Coarsening prefers typed geo over a conflicting shadow;
  shadow-only numeric coordinates are rounded, preserving other context.
  Malformed shadow containers/coordinates are dropped. Absent objects stay
  absent; opaque vendor fields are not recursively scanned.
- Code-quality review also identified direct `user.other["ext"]` and
  `imp.other["ext"]` container shadows bypassing leaf scrubs. Five additional
  regression tests failed before the fix (focused scrub run: 34 passed, five
  failed). The narrow helper clears only the required `eids`/`data`/`tid` leaves,
  preserves unrelated metadata, drops malformed containers, and leaves missing
  extensions absent. Tests inspect both serialized typed and shadow views so
  last-key-wins parsing cannot hide a retained protected field. Spec and quality
  re-reviews confirmed this fix.
- Final full exchange package suite: 165 passed (102 library, 63 integration),
  including 39 scrub tests and all eight privacy-enforcement integration tests.
  The new tests exercise every combination of the eight transformation flags,
  idempotence, sibling isolation, field inventory, same-field shadows, malformed
  IP/geo, and IP/rounding boundaries. Existing GDPR wrapper tests confirm no
  activation of new policy flags from incoming signals.
- `cargo clippy -p auctioneer-privacy -p auctioneer-exchange --all-targets --no-deps
--locked --offline --target-dir /private/tmp/auctioneer-privacy-enforcement-baseline-target --
-D warnings`: passed after the fix. Existing config build-script GVL notices
  remain; there were no Clippy diagnostics.
- `rustfmt --edition 2024 --config skip_children=true --check` on all four
  changed Rust files: passed. `git diff --check`: passed.
- Full workspace tests and portable/provider builds were not run for this
  independent foundation checkpoint; they remain Task 11 acceptance work.

The checkpoint does not activate new CCPA/activity/LMT/COPPA auction policies.
Task 8 connects normalized per-bidder decisions after configuration, parsing,
and policy state are implemented. The existing GDPR wrapper remains the only
live caller of scrub composition until then.

## Remaining Acceptance

Tasks 4-11 in the [implementation plan](2026-09-28-privacy-enforcement.md) track the
qualified GPP module/simple-fold dependency, pinned reference fixtures, typed
configuration, activities, normalization, exchange integration, measured
resource accounting, and full cross-target/E2E acceptance.

No GPP, CCPA, activity, outbound-capture, or production acceptance matrix row
is closed merely because the foundation tests pass. Full privacy enforcement closure still
requires every row in design section 10 and no live auction privacy stubs.

## Disclosed Vendors Checkpoint

Task 3 implementation on 2026-09-29 retains DV membership in `TcfString`
during the existing suffix-validation walk. Range filling is word-wise,
with charges preceding mutation. The bitfield reserves the whole read and
checks truncation before writing any membership bits. Core consent/LI sets,
authorization algorithms, GCL/GVL gates, and Publisher TC behavior are unchanged.
Independent spec and code-quality reviews passed with no outstanding findings.
Each reviewer independently reran the 65 TCF-filtered library tests, 23
hostile-input tests, and three property tests successfully.

Fresh implementer baseline: 255 privacy tests passed, one ignored.
`cargo test -p auctioneer-privacy --lib tcf` first failed with exit 101 for the
missing `disclosed_vendors` field/API; after implementation 65 matching tests
passed. Hostile/property suites passed 23 + 3 tests. Negative-control runs
temporarily moved writes before budget checks and caused both the range and
bitfield charge-before-mutation tests to fail; correct ordering was restored.
Reuse tests include suffix, core, budget, and whole-input-limit failures.

Parent verification: `cargo test -p auctioneer-privacy -p auctioneer-exchange` passed,
including privacy library 230 passed/one ignored plus integrations 6 + 23 + 3,
and all 165 exchange tests. Combined: 427 passed, one ignored.
Strict scoped Clippy passed for both packages (`--all-targets --no-deps`,
`-D warnings`); changed-file rustfmt passed for all nine foundation Rust files.

Native `size_of` measurements taken before/after the change using Rust 1.95.0
on `aarch64-apple-darwin`:

| Type        |   Before |    After | Increase |
| ----------- | -------: | -------: | -------: |
| `TcfParser` | 16,400 B | 24,592 B |  8,192 B |
| `TcfString` | 16,528 B | 24,720 B |  8,192 B |

`VendorBitSet` is 8,192 inline bytes. This adds no per-vendor heap nodes, but
parser/result coexistence adds 16 KiB, and additional result copies must be
accounted for separately. These are object-size measurements, not an entire
auction peak-memory measurement or a portable-layout guarantee. Task 10
must integrate live retained/transient accounting before privacy enforcement activation.

## GPP Candidate Audit

Source inspection and an isolated header probe reject unmodified `iab_gpp`
0.1.2 for privacy enforcement; no dependency was added to Auctioneer. The published crate records
source revision `97b777134250e2c2d7aabbcc518a1787ba364efb`.

Its public parser uses a closed `SectionId` enum, copies payloads into owned
strings, and decodes IDs before a caller can cap expansion. The extraction
path infers `u8` IDs, not the required numeric `1..=65535` domain. It provides
no caller-supplied work/allocation limit or separate terminal-limit result.
See the [published parser](https://docs.rs/iab_gpp/0.1.2/src/iab_gpp/v1/mod.rs.html).

An additional correctness probe decoded the same header in both libraries:

```text
Header: DBADNb (also checked with a trailing zero-padding A)
go-gpp v0.2.0 ReadFibonacciRange: [{2,2}, {6,6}, {7,7}]
iab_gpp 0.1.2 GPPString::parse_str("DBADNb~tcf~usp~usnat"):
  [TcfEuV2, UspV1, TcfCaV1] = [2,6,5]
Control header DBABTA: both decode section 6
```

This is a header-structure probe, not valid TCF/USP payloads or a full upstream
auction fixture. The Go probe used cached `github.com/prebid/go-gpp@v0.2.0`,
`go1.23.12 darwin/arm64`, and its public `util` header reader. Rust used the
published 0.1.2 crate with Rust 1.95.0. Both ran offline in an isolated
temporary harness without changing either project or Auctioneer dependencies.
The Rust singleton-range loop assigns its cursor to the delta rather than
the cumulative ID (`src/core/mod.rs`, `read_fibonacci_range`). Qualification
must cover three-or-more singleton deltas, mixed ranges, and overflow, not
only the library's two-section examples.

Task 4 remains open. On 2026-09-29 the user selected an internal reference-based
envelope module with the ability to extract it into its own crate later. See
the [module design](../specs/2026-09-29-gpp-envelope-module-design.md), whose
detailed contract passed independent spec review and self-review. The user
authorized implementation on 2026-09-29; the subordinate parser plan also
passed independent review.
No alternative codec, dependency pin,
or full reference-parity claim was introduced by this audit.

### Header Padding Probe

The same isolated Go harness was extended on 2026-09-29 to call
`util.NewBitStreamForWrite`, write type 3 and version 1, and call
`WriteIntRange` followed by `Base64Encode`. Despite its name, that writer
uses Fibonacci-coded cumulative range offsets. Actual outputs:

```text
writer [{2,2}]: DBABMA
writer [{2,2}, {6,6}]: DBACNYA
writer [{2,2}, {6,6}, {7,7}]: DBADNbA
reader DBABM and DBABMA: 2
reader DBACNY and DBACNYA: 2,6
reader DBADNb and DBADNbA: 2,6,7
```

This reproduces the writer's byte-aligned zero padding versus the format's
six-bit padding. The module design explicitly accepts both bounded forms,
not arbitrary trailing zero characters. The probe does not establish complete
GPP parsing or policy parity. The writer itself rejects Fibonacci values
at 6765 and above, so it cannot generate every required boundary fixture.

Reference module checksum from the pinned upstream `go.sum`:
`github.com/prebid/go-gpp v0.2.0 h1:41Ssxd4Zxr50WgwG1q/1+6awGU3pFnwV7FR4XCLQSuM=`.
Command: `env GOTOOLCHAIN=local GOPROXY=off GOSUMDB=off
GOCACHE=/private/tmp/auctioneer-privacy-enforcement-gpp-audit/go-cache go run . DBABM DBABMA
DBACNY DBACNYA DBADNb DBADNbA`, in `/private/tmp/auctioneer-privacy-enforcement-gpp-audit`.
The harness is temporary; reproducible committed fixture generation remains
Task 4 acceptance work.

## privacy enforcement Reference Acquisition

The committed generator at
`crates/auctioneer-privacy/tests/data/privacy/reference/main.go` was executed against
an isolated `git archive` of upstream
`d7f8515b86258688304b0d9b6668c6a0e258bc9e`, not the local checkout's newer HEAD.
With its locked go-gpp v0.2.0 and Go 1.23.12, it generated 37 USP, 36 source
selection, 18 no-sale, 864 activity, and nine scrub cases. A second generation
compared byte-identically. Source-selection placeholders are not valid TCF
authorization fixtures. Intentional differences have separate names; Rust
consumption and live outbound assertions remain to be implemented.

The same generator records Go Unicode 15.0.0 simple-fold orbits. An isolated
Rust qualification harness using `unicode-case-mapping` 0.5.0, source revision
`b78edebc40bbec646a75c85463d5e5588b58aea1`, compared every one of the 1,112,064
Unicode scalar values against Go's equivalence classes. It checked both that
no Go orbit splits and that no distinct orbits collapse. Kelvin, sharp-s,
capital sharp-s, and Turkic-I controls passed. This candidate has no runtime
dependencies and uses Unicode 15.0.0 tables. Native compilation and isolated
`cargo check` for `wasm32-unknown-unknown`, `wasm32-wasip1`, and
`wasm32-wasip2` passed with Rust 1.95.0. Workspace pinning, permanent parity
assertions, and activity integration remain the next steps; these probes do
not claim full privacy enforcement or provider acceptance.

## GPP Implementation Checkpoint

The dependency-free `auctioneer-privacy::gpp` module now implements the approved
borrowed envelope API. The implementer's red check first failed for the
missing API, then exposed 17 behavioral failures before implementation;
all 30 isolated parser tests subsequently passed. Full privacy tests before
the new policy-reference integration test passed 292 tests, one ignored.
Scoped strict Clippy and changed-file formatting passed. Independent spec
and code-quality reviews remain pending at this checkpoint.

Twelve header vectors were generated by the pinned Go writer/reader and
regenerated byte-identically. The committed generator, IAB content hash,
license attribution, and native/WASM type-layout measurements are in
`crates/auctioneer-privacy/src/gpp/data/README.md`. A native library check and a
`wasm32-unknown-unknown` package check passed; standalone parser compilation
also passed for both WASI targets. Full package WASI checks and post-bounded fanout
revalidation remain pending. None of these checks activates auction policy
or closes live memory accounting.

## bounded fanout Integration Basis

On 2026-09-29 the committed delta from common ancestor
`d1f1bde25b02169b8eba04ce0dca23061c757203` through bounded fanout commit `4d830f0`
was imported mechanically with `git diff --binary ... | git apply -` after
a clean `git apply --check`. This preserved the privacy enforcement branch's pre-existing
edits, including its documentation; no uncommitted bounded fanout work was copied and
neither the source worktree nor branch history was changed.

The imported basis includes the corrected admission lifetime, bounded-wave
helper fixes, and EdgeZero pin `491df844a0f06924de88000b244329a141debf5a`.
It does not establish completed live bounded construction or corrected
retained-node accounting; the subsequent resource audit below corrects that
earlier characterization. Post-integration privacy/exchange
regressions passed 459 tests, one ignored, including the 37-row USP reference
test and nine-row scrub reference test. This records integration provenance, not an
independent recertification of all bounded fanout provider evidence.

The new `privacy_reference` USP test produced the expected missing-test-dependency
red failure (E0432/E0433 for `serde_json`); JSON support was then added as a
privacy dev-dependency only. The parser's extractable tests still consume
Rust constants without JSON or Auctioneer policy dependencies.

The initial independent parser spec review identified uncharged checked
padding arithmetic under a depleted work budget. Padding, cursor, alphabet,
and shift arithmetic now precharge their work. Eight regression tests failed
against the prior implementation; all 35 isolated parser tests then passed.
Exact budget controls include 85 operations for `DBAA`, 114 for `DBABM~`,
and 134 for `DBABMA~`. Independent spec re-review and code-quality review
approved. Isolated strict Clippy, formatting, and native/three-WASM library
checks passed; full-package cross-target checks remain open.

## Independent Policy Components

`ccpa::NoSale` now validates the exchange-supplied identity namespace without
reading consent or configuration. It accepts only a singleton wildcard,
deduplicates valid exact names without core-name/folding fallback, and emits
a fixed `PrivacyError::InvalidNoSale`. The missing-API red check failed as
expected; all five tests passed, including the 18 pinned reference cases and
the deliberate invalid-USP/error-precedence distinction. Independent spec
and quality reviews approved the pure value component. Live namespace
construction and auction enforcement remain Tasks 7-8.

`sources::normalize` implements borrowed GPP/legacy source selection,
terminal GPP limits, unconditional legacy USP validation, projection
candidates, and ordered fixed warnings. It does not authorize TCF, apply
configuration, mutate requests, or activate auction policy. The red check
failed for the missing module/error variant; all 11 tests subsequently
passed, including 36 reference cases with the explicit inactive-TCF decision
and arbitrary-input coverage. Independent spec and code-quality reviews
approved this component. The live resolver, projection, and resource
integration remain open.

A subsequent privacy Clippy invocation encountered the concurrent Task 5
configuration edit between its type and generated-initializer changes
(`AccountConfig.ccpa` missing from generated output). That run is not a
completed lint gate; rerun after Task 5 is coherent.

## Configuration, Activities, and Resolution Checkpoint

Task 5's implementer recorded expected red tests followed by 78 focused
passing tests, including exhaustive comparison of 1,112,064 Unicode scalars
with Go simple-fold equivalence classes. Configuration uses the exact
`unicode-case-mapping =0.5.0` pin and shared immutable activity maps/rule
arrays. Independent spec and quality reviews approved; full config/core
suite, build-harness, and final lint gates are not yet recorded.

Task 6 produced a missing-API red result, then five passing tests including
all 864 pinned activity cases. Independent reviews approved after removing
an avoidable empty-Arc allocation. A direct pointer-identity regression
covers out-of-scope, absent, malformed, and usable GDPR consent paths.

Task 7's resolver now takes explicit narrow GDPR-exemption and validated
no-sale inputs, normalizes once, and returns owned decisions plus borrowed
projection inputs. The initial missing-API red result was followed by eight
passing normalization tests. A fresh complete privacy run passed 331 tests,
one ignored. Quality review prompted stronger paired legacy/GPP tests for
CMP/vendor deletion boundaries, GVL policy mismatch, publisher restrictions,
warning codes, and actual denial; all ten normalization tests then passed.
Source-specific GVL warnings no longer allocate and discard a legacy message.
Task 7 quality re-review found one weak restriction test: its original
fixture did not consent to the evaluated vendor. It now compares otherwise
identical vendor-8 granting and Purpose-2-restricted strings through both
legacy and GPP. Static re-review approved; the new test and final lint are
included in the pending combined verification run.

## Live privacy enforcement Integration In Progress

The new exchange acceptance suite compiled and produced the expected red
result: three tests passed and 28 failed before live policy integration.
It covers actual adapter construction counts, captured outbound bodies and
headers, sibling/original isolation, aliases and disabled namespaces, policy
independence under GDPR-off/NSP, projection precedence, warning uniqueness,
ATT, multi-request adapters, and terminal input limits. The exchange wiring
and bidder-local projection then passed all 31 tests.

Integration review found that denied-bidder FPD could abort an allowed
sibling, and that `make_bids` received the original request. The new tests
produced 32 passes and exactly three expected failures. Fetch gates now run
before bidder FPD projection, retaining one warning and per-impression
exclusion rows; allowed-bidder malformed FPD still fails. Both adapter hooks
receive the same retained, sanitized, metadata-projected request, including
bidder-visible `tmax`. All 35 focused exchange tests now pass. This removes
one redundant request clone, but does not establish the remaining allocation
budget described below.

The actual core handler suite `privacy_wire_signal_400` passed all six tests,
including fixed no-sale/GPP-limit 400 responses with zero sends, runtime
activity config reaching the live gate, and an allowing control that sends.
`cargo fmt --all -- --check` and `git diff --check` passed at this checkpoint.
Broader crate/target verification remains open.

The unused no-op `PrivacyContext::apply_scrub` and the old broad NSP
allow-all-with-warnings constructor were removed. Request transformation
belongs to the exchange, while all auction traffic uses the resolver even
when GDPR is disabled. The obsolete no-op-only unit test was removed;
actual serialized scrub acceptance replaces that placeholder assertion.

## Handler and Package Verification

The first combined privacy/exchange/core run stopped at the GDPR enforcement-only
`gpp_section_two_never_reads_the_legacy_consent_string` expectation. Its
fixture carries SID 2 but no GPP envelope. privacy enforcement design sections 5 and 11
explicitly replace that temporary behavior with legacy fallback. The test
now checks the legacy granting response plus the structural wire warning.
Two separate handler controls supply a real empty or invalid active section
2 and retain the all-denied response despite granting legacy consent. Other
GDPR enforcement consent, identifier, and geo expectations are unchanged. The local fake
bid responses now use `json!` and serialization instead of JSON interpolation.

Focused command (`auctioneer-core`): `--test response_gdpr_applies --test
privacy_outbound --test privacy_wire_signal_400 --test config_reference_examples`.
All 17 tests passed: six GDPR response rows, two real-handler/shipped-adapter
outbound rows, six privacy input/gate rows, and three executable operator
configuration checks. Generic, AppNexus, and PubMatic each have scrubbed and
allowing outbound controls with exact send counts.

Fresh combined command: `cargo test -p auctioneer-privacy -p auctioneer-exchange
-p auctioneer-core --no-fail-fast --locked --offline --target-dir
/private/tmp/auctioneer-privacy-enforcement-baseline-target -j 1`.
Result: 856 passed, zero failed, one ignored, including the strengthened
publisher-restriction controls through legacy and GPP consent sources.

Task 8/9 spec review approved the live integration. Quality review requested
caching complete decisions once per requested bidder identity and exact
response-hook/outbound coverage correlation. A missing-API red check preceded
the cache helper. Its regression checks 300 lookups produce only three
evaluations, preserving separate alias/base decisions; the focused exchange
library and acceptance run passed 145 tests. Denial map keys allocate only on
the first excluded impression, and the decision cache drops after scrubbing,
before adapter construction. Response hooks now assert exact ordered coverage
against their own captured requests. Both spec and quality re-reviews approved.
The cache follow-up is not included in the earlier 856-test package result;
the full-workspace run will verify the final cache lifetime change.

Strict all-target Clippy first found an avoidable `format!` allocation in
the new configuration build-harness test; it now writes directly to its String.
The second run reached the newly added hostile-input test and found numeric
suffix/import issues. Those are corrected; a completed lint gate remains open.

The normal-feature dependency tree for `auctioneer-privacy` on
`wasm32-unknown-unknown` has no GPP library, async runtime, or HTTP client.
The only new production dependency is `unicode-case-mapping 0.5.0` through
the shared configuration schema. Package/adapter build checks remain separate
from this dependency-tree inspection.

## Workspace Acceptance Checkpoint

The full native command `cargo test --workspace --all-targets --no-fail-fast`
with the locked/offline/target-directory flags above passed **1,480 tests,
zero failed, three ignored**, across 104 test binaries. This verifies the final
decision-cache ownership change and all three new hostile GPP normalization
tests, including the restrictive-TCF 64-to-65-section transition. Both reviewers
approved those hostile-input assertions. All 56 config child-build tests passed.
The three ignored entries are two CLI probes executed separately by their
parent tests with controlled environments, plus the existing optional GVL
latency calibration. No privacy enforcement privacy behavior test was newly ignored.

The first sandboxed run had 1,423 passes and 57 failures in two targets:
transport tests could not bind local ports, and config child builds could not
access `sccache`. The successful rerun used approved unrestricted execution;
no product behavior or test assertions were changed to bypass these failures.
Logs: `/private/tmp/auctioneer-privacy-enforcement-workspace-tests.log` and
`/private/tmp/auctioneer-privacy-enforcement-workspace-tests-unrestricted.log`.

The [coverage map](2026-09-29-privacy-enforcement-coverage.md) traces individual design rows to
their assertions. Additional combined-signal, configuration-parity, and
current-router sync-deferral tests are being added after this checkpoint and
require their own verification. Final strict Clippy, native conformance,
feature-matrix, and package/adapter WASM checks remain in progress. Passing
native tests does not close the resource dependency below.

## Final Policy And Portability Checks

After the last test/style changes, `cargo test -p auctioneer-core -p auctioneer-exchange
-p auctioneer-privacy --no-fail-fast` passed **867 tests, zero failed, one ignored**
across 71 suites (including doc-test targets). This includes four new
shared-schema/runtime parity tests, two conflicting-GPP/no-sale handler tests,
and one real-app route test proving the three user-sync URLs still return 404
with a registered `/status` positive control. The new tests passed independent
review. Shared-DTO parity does not claim execution/value comparison of the
same fixture through generated Rust.

Strict Clippy passed for privacy, exchange, config, and core with
`--all-targets --no-deps --keep-going -- -D warnings`. Earlier runs found only
test-style issues; those were corrected without changing product semantics or
weakening assertions. The final response-hook test extracts fixture setup
while retaining exact per-outbound coverage and sanitized-input comparisons.
Logs: `/private/tmp/auctioneer-privacy-enforcement-final-policy-tests.log` and
`/private/tmp/auctioneer-privacy-enforcement-clippy-final-5.log`.

All three CI foundation/adapter checks passed with `auctioneer-privacy` explicitly
included: Cloudflare `wasm32-unknown-unknown`, Fastly `wasm32-wasip1`, and Spin
`wasm32-wasip2`. The `fanout_conformance` test binary also built successfully
with `--no-run --features conformance-suite` for each of those targets. These
are compile/link checks, not executed WASM tests or provider deployment proof.
The `cargo tree -p auctioneer-privacy --target all --edges normal,features` audit
confirms no new runtime/network dependency; only the qualified Unicode table
crate was added. Native conformance execution passed **76 tests, zero failed**
across four binaries (core plus the three adapters).

The CI insecure-bidders commands also passed: workspace `--all-targets
--features insecure-bidders --exclude auctioneer-config` produced **1,265 passes,
zero failures, three ignored**; config's non-harness selection with
`--features "insecure-bidders test-harness-override" --lib --test codegen
--test gvl_archive --test smoke --test url_safety` produced **162 passes,
zero failures**. The hermetic config child-build harness was already covered
by the default run and was not repeated under the alternate feature set.

The workspace runs used vendored OpenRTB round-trip fixtures. A separate
successful `sample_requests_round_trip` test set `PREBID_SERVER_DIR` to a
temporary `git archive` of CI's exact upstream revision
`0ef82cbb44f00b15290d02883c315e0f5a109536`, covering its full `valid-whole`
directory as well. This is distinct from the privacy enforcement policy-reference pin.
Logs are `/private/tmp/auctioneer-privacy-enforcement-insecure-workspace.log`,
`/private/tmp/auctioneer-privacy-enforcement-insecure-config.log`, and
`/private/tmp/auctioneer-privacy-enforcement-full-openrtb-fixtures.log`.

Final formatting, `git diff --check`, local Markdown file-link validation,
batch-API documentation and roster checks passed. No commit, push, or PR was
created. Passing these gates does not establish RES-1 or the remaining
same-fixture emitted-Rust/runtime comparison called out under ACT-2.

## Live Resource Audit

Read-only inspection of both the privacy enforcement worktree and the clean bounded fanout worktree at
`4d830f0` confirms that there is no newer committed integration basis to import.
The following shared execution gaps remain; passing parser/helper tests do
not close them:

- The splitter retains projected impressions and whole request copies
  for every privacy-allowed identity before sampling and the bidder cap.
  Privacy gating now precedes FPD, but many allowed aliases can still exceed the population used by
  the `max_bidders_per_auction` memory charge.
- The actual exchange construction path still calls `make_requests`, not
  the pre-allocation bounded sink. Outbound caps apply after construction.
- At this audit checkpoint account discovery and typed ingress parsing did
  not use the bounded-node parser; the bounded-ingress follow-up below closes
  that wiring defect. `NODE_RETAINED_BYTES` was 64 and its
  deep-nesting fixture was excluded from the per-node assertion; the
  retained-node correction below supersedes that portion of the finding.
- Admission holds concurrency/terminal permits through the handler, but no
  general per-auction byte-reservation API exists on the inspected live path.
  privacy enforcement must not claim a terminal memory-reservation error contract is wired.

Subsequent work resolves the documented node-charge, ingress-parser, and
final-response-buffer defects in the privacy enforcement integration worktree only. The bounded fanout
worktree is not modified. Task 10, RES-1, and whole-capability completion
remain open until the remaining shared integration is complete.

## Retained-Node Accounting Correction

The privacy enforcement integration worktree now corrects one shared bounded fanout accounting
prerequisite. The sibling bounded fanout worktree remains untouched at `4d830f0`.
The old guard exempted deep nesting because a single depth-limited chain
was small. A containing array can retain many such chains, so that
exemption did not justify a 64-byte per-node charge.

The new guard first failed against 64 bytes/node with these measurements:

| Shape                                   | Syntactic Nodes | Retained Bytes |
| --------------------------------------- | --------------: | -------------: |
| Single deep array chain                 |             121 |         15,360 |
| 128 array chains, depth 100             |          12,929 |      1,642,496 |
| 128 single-key object chains, depth 100 |          25,729 |      8,106,496 |

`NODE_RETAINED_BYTES` is now 384, rounding the binding native measurement
(about 315.073 bytes/node) plus 20% upward. All ten fixtures must satisfy
the same checked 20% margin, including depth/cardinality controls at 1/1,
4/16, and 100/128. No depth exemption remains. Contents of long strings,
typed OpenRTB allocations, transient parse/transform scratch, and additional
simultaneously retained trees are separate charges, not claims of this guard.

The production node cap stays 65,536, so the correction adds 20,971,520
bytes per auction. Checked runtime arithmetic propagates it through the
per-auction peak and concurrency-scaled isolate total; tests explicitly
pin those deltas and unchanged call/egress terms. No cap, ceiling, admission
count, or warn-only boot policy was changed.

| Adapter    | Previous Declared Total | Corrected Declared Total |
| ---------- | ----------------------: | -----------------------: |
| Axum       |          12,924,883,968 |           14,267,061,248 |
| Cloudflare |           1,600,005,120 |            1,725,834,240 |
| Fastly     |             878,308,864 |              899,280,384 |
| Spin       |             878,308,864 |              899,280,384 |

`cargo test -p auctioneer-runtime -p auctioneer-exchange --lib --test node_charge`
passed **189 tests** after the node regression and the three expected
runtime-total failures were corrected. The adapter evidence update first
produced nine expected failures, then passed **26 targeted envelope tests**.
Commands used `--locked --offline -j 1`; runtime/exchange used
`/private/tmp/auctioneer-privacy-enforcement-baseline-target`, adapter evidence used
`/private/tmp/auctioneer-privacy-enforcement-envelope-target`.

`cargo check -p auctioneer-runtime` passed separately for
`wasm32-unknown-unknown`, `wasm32-wasip1`, and `wasm32-wasip2`, using
`/private/tmp/auctioneer-privacy-enforcement-runtime-portability`. The fixed charge is not reduced
for narrower WASM handles. These are compilation checks, not WASM allocator
measurements or provider resource certification.

This closes the retained-node measurement defect only. Live bounded
ingress/construction, simultaneous request representations, general byte
reservations, and measured privacy enforcement-specific charges still need integration and
terminal-exhaustion tests before RES-1 can pass.

Independent spec and quality reviews approved this bounded correction.
Their approval excludes the still-open whole-auction resource contract and
does not turn native requested-allocation measurements into RSS or WASM
allocator evidence.

## Emitted-Config Parity

ACT-2 now executes the two actual lowering paths on one nondefault fixture.
`child_build_executes_privacy_baked_runtime_parity` replaces the former compile-only
privacy enforcement harness case. It writes a TOML fixture into its owned temporary directory,
builds the core `privacy_config_parity` target with that fixture compiled into
`auctioneer-config`, and executes exactly
`tests::emitted_baked_config_matches_runtime_fixture` with `--ignored --exact`.
The parent requires both a successful exit and one actually executed test;
a renamed/missing probe cannot silently pass by matching zero tests.

The child reads the same fixture and resolves its privacy enforcement values through validated
`AuctioneerAppConfig` plus `resolve_host`/`resolve_account`, using neutral defaults
rather than inheriting generated values. It compares complete effective
activity rules and CCPA values plus both host switches against the compiled
`host_default()`/`account_default()`. Coverage includes eight activities, four
component types, missing/true/false defaults, an empty rule array, ordered
rules, duplicate/Unicode/escaped names, every CCPA tristate, and nondefault
host enforcement switches. Existing tests retain partial-overlay and invalid
schema/limit coverage.

Spec review approved the comparison. Quality review found unrelated build
overrides leaking into the child and an unnecessary forced-offline cache
prerequisite. The real poisoned-environment check reproduced failure while
reading the unrelated missing bootstrap path. The helper now clears all four
unrelated path overrides and inherits Cargo's network policy. A fast command
regression pins that isolation without mutating global process environment.

Both harness tests then passed with `CARGO_NET_OFFLINE=true` and invalid
inherited GVL/bootstrap paths (**2 passed**, including the nested value probe).
The initial sandbox attempt failed at sccache permissions; the authorized
outside-sandbox runs established the actual red/green behavior. Independent
spec and quality reviews approved the final helper. The core probe is ignored
in ordinary workspace runs because only its parent can supply the correct
compiled-in fixture; the parent remains a normal non-ignored test.

Strict Clippy passed for runtime, exchange, config, and core using
`--all-targets --no-deps --keep-going -- -D warnings` after test-only style
fixes. No runtime configuration semantics or production APIs changed for
this parity test. Final broad regression results are recorded separately.

## Post-Correction Regression

After the code/style changes, the default-feature workspace selection
`cargo test --workspace --all-targets --no-fail-fast --exclude auctioneer-config`
passed **1,264 tests, zero failures, four ignored across 97 test binaries**.
It ran outside the sandbox for the existing ephemeral-port tests, with
`--locked --offline --target-dir /private/tmp/auctioneer-privacy-enforcement-baseline-target -j 1`.
This includes the full runtime, exchange, core, and native adapter regressions;
the config child-build harness is verified separately rather than rerunning
its unrelated negative-build cases. The extra ignored case is the new core
parity probe, which its config parent executes explicitly.

The final post-style rerun of `cargo test -p auctioneer-config --test
build_failure_harness privacy_ -- --nocapture` also passed **2 tests**, including
the isolated emitted-value/runtime comparison (191.92 seconds in the
harness). This is in addition to the earlier poisoned-environment red/green
run, not an inference from Clippy or a zero-test child invocation.

Captured test output: `/private/tmp/auctioneer-privacy-enforcement-followup-workspace.log`.
Formatting, `git diff --check`, batch-API documentation checks, test-roster
arithmetic, and the changed privacy enforcement/architecture documents' local file links also
passed. No commit, push, or PR was created. privacy enforcement remains incomplete because
live resource reservations and bounded construction are not yet integrated.

## Bounded Ingress and Response

Account lookup and typed auction parsing now share the installed
`max_parsed_nodes`. The first live regression reproduced acceptance of
65,537 nodes with a 65,536-node cap. The corrected dispatch matrix checks
65,535/65,536/65,537 nodes with enabled and disabled accounts, empty and
nested opaque banner extensions, zero sends on rejection, and admission/
terminal-permit release. Budget failure yields exactly
`Invalid request: too many JSON nodes\n`; ordinary syntax errors remain
deferred by account discovery, and disabled-account precedence otherwise
stays unchanged. `resolve_account` now takes the explicit node ceiling.

Independent review identified two defects in the previous generic counting
deserializer: synthetic `RawValue` maps overcounted an empty raw object
(five nodes instead of three for the containing fixture), and numeric
conversion rejected valid opaque `1e400`. Both were reproduced before their
fixes. The implementation uses the plan's permitted bounded token pass,
followed by Serde's non-materializing syntax validation and normal typed
deserialization. This is not a replacement JSON validator. Counting
includes duplicate keys, escaped strings, and all skipped/opaque subtrees;
validation scratch is dropped before constructing the target. A further
tight-budget trailing-root regression preserves `Syntax` deferral rather
than charging a second JSON value against the first root. These extra
scans are an explicit CPU tradeoff, not an allocation-free parsing claim.

Final-response serialization now uses the existing `CappedWriter` after a
fallible exact reservation. The allocation regression first observed an
8,388,622-byte request with a 4,096-byte output cap. It now passes without
oversized temporary output. Seven integration tests cover escaped JSON,
exact/one-over/zero limits, allocator-failure injection, capacity overflow,
and the unchanged no-envelope behavior. Five serialization unit tests
also pass, including preservation of genuine Serde encoding-error details.
Cap and allocation failures remain terminal 500s, with no truncated body or
unbounded fallback. Output storage is bounded; response trees and other
serializer scratch are not covered by this claim.

Both changes passed independent spec and code-quality review after the
identified regressions were fixed. The general per-auction reservation API,
bounded outbound request construction, capped projected-request retention,
and measured privacy enforcement state charges remain open. No PR or commit was created.

Final follow-up verification: `cargo test -p auctioneer-core -p auctioneer-runtime
-p auctioneer-exchange --all-targets --no-fail-fast --locked --offline
--target-dir /private/tmp/auctioneer-privacy-enforcement-baseline-target -j 1` passed **628 tests,
zero failures, one expected ignored parity probe, across 60 binaries**.
The allocator guard and all twelve live node-boundary cases are included.
Captured output: `/private/tmp/auctioneer-privacy-enforcement-bounded-paths.log`.

Strict Clippy passed for core/runtime with `--all-targets --no-deps -- -D
warnings`. Runtime checks passed on all three WASM targets; core checks
passed on `wasm32-unknown-unknown` and `wasm32-wasip1`. This follow-up did not
repeat the full adapter/provider matrix or the config child-build harness.
Formatting, whitespace, batch-rebase/roster checks, and changed-document
local links passed. The bounded fanout worktree remained clean and unchanged.

## Draft Publication

On 2026-09-29 the current checkpoint was committed as `3b34a20` and
published as [draft PR #17](https://github.com/IABTechLab/auctioneer/pull/17),
stacked on `bounded-fanout` / PR #15. Commit `b0554d2` records the already
imported bounded fanout ancestry; its tree is identical to its first parent's tree.
The draft explicitly leaves live outbound construction and whole-auction
resource accounting open. Publication is not privacy enforcement completion or merge approval.

Fresh pre-publication checks: 18 account-resolution and seven bounded-response
integration tests passed, as did workspace formatting and `git diff --check`.
The broader preceding evidence above is not relabeled as a new full run.

The five workflows initially excluded this stacked PR because their PR base
filter named only `main`. Commit `fa682a3` adds `bounded-fanout` to those filters;
push triggers, permissions, jobs, and their commands are unchanged. A parsed
YAML assertion failed before the change and passed for all five afterward.
GitHub then created the checks, but did not start their jobs: the annotation
on [test run 36652549846](https://github.com/IABTechLab/auctioneer/actions/runs/36652549846)
reports failed recent account payments or a spending limit that must be
increased. There are no executed CI test results from that run. Local
verification continues; account billing must be resolved before hosted checks
can provide evidence.

## Survivor-Only Request Materialization

The live exchange now groups bidder identities and impression indices around
one serialized request, then applies privacy decisions, sampling, and the bidder
cap before constructing bidder-local requests. FPD projection and validation run
only for survivors; their recorded decisions are applied afterward and released
before fanout. The original typed request is borrowed rather than cloned.
The unlimited `split_by_bidder` convenience API retains its previous behavior.

The regression first reproduced materialization of invalid FPD for sampled-out
and cap-dropped bidders and allocation growth from 2.60 MB to 19.45 MB when
participant count increased. Fresh parent verification passed all **218 exchange
tests**, including 42 privacy enforcement privacy tests and three allocation/fixture guards.
Strict exchange Clippy, workspace formatting, and whitespace checks passed.
Independent spec and code-quality reviews approved this focused change.
The mixed snapshot compares complete outbound bodies and response
bookkeeping, with slash/tilde aliases, FPD, disabled/privacy-denied identities,
sampling, caps, and scrubbing.

The exact 256 KiB fixture contains 11,633 identities. Grouping plus resolved
alias bookkeeping peaks at **1,960,381 bytes**, below the factor-8 allowance of
2,097,152 bytes. This is not a full-auction measurement. One survivor retains
**1,185,275 bytes** and eight retain **9,482,211 bytes**, exceeding their current
factor-2 allowances of 524,288 and 4,194,304 bytes. The host-overlay fixture uses
64 aliases, not a claimed enforced alias maximum. Population bounding is fixed;
the typed-byte charge, other overlapping state, live sinks, and reservations
remain open. No byte factor, admission capacity, or boot policy changed here.

## Per-Auction Reservation Primitive

`auctioneer-runtime::AuctionBudget` now provides an independent per-auction `u64`
ledger with checked, atomic reservations and non-cloneable borrowed RAII leases.
Reservation, refusal, and release allocate nothing. Aggregate charges can cover
multiple allocations; `checked_allocation_capacity` separately checks the native
or WASM `usize` and `isize` layout bounds of an individual byte buffer. Errors
are closed accounting categories, not consent syntax failures.

The initial test failed with the four missing public symbols. Fresh full runtime
verification passed **93 tests** (84 unit, eight ledger integration, one route
parity) using `--all-targets --locked --offline --target-dir
/private/tmp/auctioneer-privacy-enforcement-runtime-portability -j 1`. Strict all-target Clippy and
scoped formatting passed. Runtime checks passed on `wasm32-unknown-unknown`,
`wasm32-wasip1`, and `wasm32-wasip2`.

Tests cover exact/zero/over-limit reservations, `u64` overflow, early errors,
unwind, cancellation across an await, concurrent reservations, independent
auctions, and layout conversion. Quality review found a test assertion before a
barrier that could hang on failure. The assertion now runs after unconditional
synchronization; an intentionally wrong capacity exited with the expected
failure rather than hanging. The restored full suite passed. Independent spec
and code-quality reviews approved the focused primitive.

This is not an allocator interceptor or a new physical-isolate capacity gate.
Measured phase charges and live call sites remain unintegrated. Boot policy,
existing concurrency capacities, and existing wire errors are unchanged by this
checkpoint. RES-1 and Task 10 remain open.

## Live Resource Integration Candidate

The subsequent working-tree integration prepays six disjoint phase charges
before runtime-config extraction and keeps their RAII receipts through response
serialization. Core normalizes config extraction caps before deriving the row,
adds the serialized baked host/account allowance, and clamps body, impression,
bidder, call and outbound byte limits to the installed envelope. Existing
over-ceiling warning/boot policy and provider traffic capacities are unchanged.
The ledger is logical accounting, not an allocator interceptor or physical-OOM
guarantee. Adapter row goldens exclude the additional compiled-config charge;
the actual boot warning includes it.

The measured allowances now include config envelope/blob/baked overlap (factor
128), projected typed data (128), response decode (256), response assembly (128),
request-dependent privacy work (128), cold GVL plus TCF/DV scratch (10 MiB), and
sequential gzip finalization scratch (512 KiB). These are separate from the
serialized response sizing floor, which remains factor 1. Dense malformed
config, nested container forests, full-size FPD, ordinary/supplemental bidder
parameters, escaped creatives, 23 cold GVL versions and maximum TCF/DV shapes
are measured with a checked 20% margin.

Review found and tests reproduced three missing copy products. Projection
preflight now counts selected ordinary parameters while their source impression
is still live, all supplemental copies, and FPD overlap. Response preflight
counts every participating bidder/impression diagnostic pair before privacy
denials or capping, including expired/no-bid paths. It checks bookkeeping plus
decoded-response charges before `make_bids`, then includes prefix/name-per-bid
targeting amplification before targeting clones. Failures return fixed
`503 overloaded\n`; they are never consent warnings or fallback grants.

Fresh focused verification passed 33 tests across admission, concurrent ledgers,
boot caps, gzip, outbound construction, projection, response, auction and privacy
resource targets. A subsequent actual-auction allocation guard also passed:

| Actual Response Path      | Peak Bytes | With 20% Margin | Held Charge |
| ------------------------- | ---------: | --------------: | ----------: |
| Ordinary bid              |      5,042 |           6,051 |      89,518 |
| 4,000 bids with targeting |  7,843,244 |       9,411,893 |  90,442,924 |
| 20,000 nested maps        | 26,974,896 |      32,369,876 |  61,857,238 |
| 1 KiB prefix, 128 bids    |    898,490 |       1,078,188 |  19,834,220 |
| 16 KiB alias, 32 bids     |  1,176,272 |       1,411,527 |  74,162,988 |

The guard runs `run_auction`, actual Generic response parsing, targeting,
assembly and serialization scratch; it asserts emitted targeting and uses the
same held response receipt checked by the live path. Egress output storage is
separately charged. Live ingress tests prove phase-exhaustion refusal before
config/body reads, cancellation during config and fanout, release after success,
two independently admitted concurrent auctions, and ignored attempts to inflate
the injected budget. A large supplemental projection is rejected before fanout.

This is a candidate checkpoint, not privacy enforcement completion: final independent review,
fresh full feature/cross-target regression and strict Clippy remain outstanding.
The PR is draft and targets `main`; merge bounded fanout first. No feature branch was added
to CI filters. GitHub Actions execution is independently blocked by the account
billing/spend limit and is not evidence of a test failure.

Subsequent resource spec and code-quality reviews approved the integration.
The fresh runtime suite passed 103 tests and strict all-target Clippy; runtime
checks passed on all three WASM targets. Outbound review regressions passed
18 endpoint/classification tests plus seven construction/correlation tests.
All four outbound findings are closed: reject 3xx before parsing, retain
Initial versus Unreachable transport categories, exclude synthetic refusals
from dispatch/timing, and distinguish encoded macro data from unresolved
literal placeholders. The final integration review found no remaining issue,
conditional on the fresh full matrix below. Fixture-only strict-Clippy cleanup
does not change assertions or expected wire values.

## Final Integration Verification

The fresh default workspace run used CI's pinned full upstream fixture corpus
(`PREBID_SERVER_DIR=/private/tmp/auctioneer-privacy-enforcement-ci-fixtures.AyqfuH`) and
`cargo test --workspace --all-targets --no-fail-fast --locked --offline
--target-dir /private/tmp/auctioneer-privacy-enforcement-baseline-target -j 1`. It executed 122
test binaries: 1,574 passed, three failed, four ignored. All 57 hermetic
config-harness cases passed, including emitted-Rust/runtime value parity.

The three failures were confined to two stale fixtures. The baked-default
snapshot now includes the two approved 256 KiB outbound limits. The actual
loopback TCP cancellation fixture now constructs its request through the
bounded sink with a test-local validator accepting only its exact ephemeral
loopback URL; production endpoint security is unchanged. Independent review
approved both corrections. Their fresh rerun is recorded below, not hidden
as an initially clean workspace invocation.

| Final Check                                                       | Result                                                                             |
| ----------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| Corrected snapshot and real-transport cancellation targets        | Passed: 2 snapshot and 3 actual TCP tests; all default-workspace failures resolved |
| Workspace strict Clippy, all targets, `-D warnings`               | Passed                                                                             |
| Conformance-feature strict Clippy, four packages and all targets  | Passed                                                                             |
| Insecure-bidders workspace strict Clippy                          | Passed after Mocktioneer test-import cleanup; no assertion or behavior changes     |
| Insecure-bidders workspace, excluding config                      | Passed: 1,353 tests, 0 failed, 4 ignored across 113 binaries                       |
| Config alternate-feature non-harness selection                    | Passed: 163 tests, 0 failed across 5 binaries                                      |
| Native fanout conformance, core plus three adapters               | Passed: 76 tests, 0 failed across 4 binaries                                       |
| Cloudflare foundation/privacy/adapter check and conformance build | Passed on `wasm32-unknown-unknown`; compile/link only                              |
| Fastly foundation/privacy/adapter check and conformance build     | Passed on `wasm32-wasip1`; compile/link only                                       |
| Spin foundation/privacy/adapter check and conformance build       | Passed on `wasm32-wasip2`; compile/link only                                       |
| Formatting, diff, batch-rebase and roster checks                  | Passed                                                                             |

The strict workspace lint pass only required test style/import cleanup and
backticks around `HttpClient` in the four adapter budget comments. The test
roster still closes at 175 rows. privacy enforcement has no workflow diff against the imported
bounded fanout baseline (`4d830f0`); all PR branch filters remain `main`.

After pushing integration commit `f6a9427`, PR #17 was confirmed to target
`main` and remain draft. The latest hosted test run, `36667665266`, has no
executed job steps: check `109735735949` reports failed account payments or
an insufficient spending limit. This is an external Actions-start refusal,
not evidence that the local test results failed on a hosted runner.

All queued feature and cross-target checks completed with exit code 0.
The corrected default targets resolve the workspace coverage to 1,577 passing
cases and four expected ignored cases, not a claim that the original full
invocation had no failures. Three ignored probes execute through their parent
tests; the remaining ignored case is a soft wall-clock GVL calibration.
The 964 pinned policy fixtures, all 256 scrub combinations, and every design
matrix row now have local acceptance evidence. WASM checks prove compilation
and linking, not hosted execution or provider packaging.

The final documentation audit corrected survivor ordering, distinct auction
and egress permit lifetimes, request-start-anchored write deadlines, and
metadata-only correlation with synthetic-slot reinsertion. Independent
transport/decoded response-cap overlays remain a broader design requirement;
the docs no longer claim those runtime keys are implemented. No requirement
was removed and no additional overlay feature was added to privacy enforcement.

The final documentation re-review found no remaining issue after clarifying
that synthetic finalization slots do not index the compact transport result
vector directly. A fresh post-Clippy cleanup regression selected
`adapters::mocktioneer::tests::make_requests_targets_127_0_0_1_template` with
the same alternate-feature workspace command and `--exact`: one test passed,
all unrelated tests were filtered, and the command exited 0. No production
behavior changed after the full matrix. Final local acceptance closes Tasks
8, 10, and 11; the PR remains separate from merging and provider promotion.

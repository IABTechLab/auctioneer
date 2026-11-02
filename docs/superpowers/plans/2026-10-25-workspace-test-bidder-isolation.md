# Workspace Test Bidder Isolation Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans with disjoint implementation scopes and review checkpoints. Mark steps complete only after verification.

**Goal:** Remove real-bidder coupling from shared workspace tests without replacing real provider evidence with fake-only success.

**Architecture:** One existing test-only fake supplies executable bidder behavior to upper-layer tests through source-module wrappers. Lower-layer tests use synthetic data and existing testkit APIs. Preserve production and source-bound conformance separately; no new crate, feature or serving registry override.

**Tech Stack:** Existing Rust workspace, Cargo integration targets, serde, syn and test-only source modules.

## Tasks

### 1. Guard And Shared Support

**Files:** `crates/auctioneer-bidder/tests/support/fake_bidder/`, downstream `tests/support/`, test-isolation guard, bidder README.

- [x] Record concrete references and distinguish semantic fixtures from immutable captures and serving-data proof.
- [x] Add a failing guard for migrated shared-test scopes, with negative controls that independently detect constructor and literal coupling.
- [x] Reuse the canonical fake without copying its implementation, adding default-runtime exports or changing the Cargo feature graph.

### 2. Exchange Semantics

**Files:** `crates/auctioneer-exchange/tests/`, inline exchange unit tests.

- [x] Replace real factories and arbitrary provider names in generic auction, ordering, targeting, grouping and privacy tests.
- [x] Preserve explicit provider transformation/allocation and debug conformance, separating those cases from generic test logic.
- [x] Run exchange library and integration targets; reconcile moves and assertions.

### 3. Lower-Layer Synthetic Inputs

**Files:** tests and inline unit modules in `auctioneer-config`, `auctioneer-privacy`, `auctioneer-openrtb`, `auctioneer-runtime`.

- [x] Use synthetic names/metadata for parsing, code generation, aliases, activities and route tests without upward bidder dependencies.
- [x] Exercise vendor lookup through explicit synthetic testkit data or a private lookup seam; retain a production lookup conformance check separately.
- [x] Preserve source/capture fixtures and actual baked-default assertions, identifying their conformance scope accurately.
- [x] Run affected default libraries and integration targets; feature verification remains below.

### 4. Core And CLI

**Files:** shared core and CLI test fixtures, inline unit modules, actual catalog/reference tests.

- [x] Use explicit fake registries for generic chooser/catalog and handler internals where existing APIs allow it.
- [x] Replace arbitrary provider examples in config overlays, keys, callbacks and generated parser controls.
- [x] Preserve real App/binary source-bound evidence; repair the explicit unavailable-fanout handler fixture without a deployed fake override.
- [x] Preserve operational qualification and externally pinned delivery contracts, while separating synthetic parser controls.

### 5. Review And Publish

**Files:** this plan, bidder rules, `ISSUES.md`, affected fixture/CI guards only if required.

- [x] Review remaining references, documenting concrete real-data obligations rather than broad test exemptions.
- [x] Audit all four target-adapter test trees. Preserve real App/binary/provider conformance without new injection APIs; required conformance validation remains in the verification task.
- [x] Run fresh focused and broader tests, strict Clippy, rustfmt and raw fixture controls with frozen inputs.
- [x] Obtain independent coverage-preservation and architecture review; repair findings.
- [x] Commit and push the verified round, reporting remaining M7 registry and readiness work separately.

### 6. Existing Model-Coverage Harness Repair

- [x] Reproduce the missing local compiler-patch source, noncanonical temporary path, and intervening sibling exhaustive-model failure.
- [x] In copied sources only, verify exact sibling E0027 diagnostics and apply only rustc's explicit named-field ignore; reject wildcard/unrelated/outside-source edits.
- [x] Bound retries, restore every temporary edit, retain the original exchange matcher, and exercise all three mutations and restored baselines.

## Initial Verification

The new workspace semantic-surface control compiled and failed with actual
production identities and constructors in 16 exchange/core surfaces. This was
a semantic test failure, not a missing module or compile error. The source
guard's independent constructor, nested-literal and synthetic positive controls
remain in the same target. The guard covers named shared semantic owners; it
does not claim to classify every source-bound conformance test automatically.

Read-only audit of all four target-adapter test trees found concrete identities
only in the native `usersync_transport` binary conformance target. It launches
the actual shipped App, verifies a real syncer's URL and cookie family, and must
not be silently changed into a fake-only test. Guest transport fixtures already
use synthetic calls. Their feature/guest contracts remain unchanged.

## Coverage Reconciliation

- Lower OpenRTB/privacy/runtime edits add two test cases and fourteen assertion
  sites. Synthetic alias IDs make permission tests independent of the shipped
  catalog, while a named baked-lookup module retains original fallback,
  collision and tombstone checks. No production resolver helper was changed.
- Nine generic generated-settings checks move to a compiled synthetic consumer;
  all thirty-one original assertion sites and input tables remain. Three
  explicit shipped-settings tests retain real generated ownership/limits.
  Configuration all-targets passed: 710 tests across fourteen targets, including
  all 99 scratch-build cases and the executing synthetic consumer.
- Exchange identity replacements preserve lengths 8/7/8/11 and ordering
  neighborhoods, not just arbitrary new labels. Its locally authored grouped
  golden changes only those names; immutable external captures remain untouched.
  Legacy decoder, debug, real-transform allocation and deployed-roster cases
  retain explicit conformance modules and all original assertions.
- The shared fanout suite reuses the same fake through its existing development
  feature when compiled as an integration-test dependency. The feature graph
  and default production API are unchanged; the normal serving catalog never
  registers the fake.
- Review identified that the fake's own sent-call filter could mask removal of
  the exchange filter. An explicit scoped-request response mode now returns
  cross-call candidates while retaining request-scope/media validation. Its
  independent strict/scoped/foreign control failed before wiring and passes
  afterward. Disabling only the exchange's call-local predicate then admits
  four bids instead of two and fails the original correlation assertion. The
  production predicate was restored before the final suite.
- The disabled-automatic-sync case is a locally authored `auctioneer_design` policy
  row, not an external capture. Its arbitrary provider identities now use the
  synthetic catalog; the other seven divergence rows and all external capture
  outputs remain unchanged. It remains inside the pure semantic source guard.

The lower all-targets run found an existing runtime scratch-workspace failure:
the copied root manifest's local compiler patch pointed into an uncopied
directory. Its fixture now resolves existing local patch paths against the
original workspace. This preserves the original compiler inputs and mutation
assertions rather than dropping the test or patch. The corrected lower
all-targets pass completed 730 tests across 24 targets, including that mutation
test. The broad default core/exchange cohort completed 1,821 tests across 132
targets with two pre-existing ignored cases; it includes the 104 workflow
guards. Explicit feature-only fanout conformance is a separate required pass,
not covered by these default totals.

The explicit four-target native fanout conformance run passed 77 tests. Its
real-handler unavailable-fanout case retains a separately owned serving
identity rather than incorrectly sending the synthetic direct-auction input
through the actual App. The first run exposed that distinction as 400 instead
of the required 500; the fixture repair restores the original route proof.

The ignored scrub-model compile test also ran explicitly: 12 tests passed,
including all three actual mutations and clean restored baselines. Scratch
sources now include root local compiler patches and canonical temporary paths.
Sibling exhaustive patterns are handled through exact machine-applicable
named-field suggestions in copied sources only. The exchange matcher remains
unchanged. Independent review required protecting the entire exchange subtree,
not just `scrub/fields.rs`; an additional non-adapter exchange-file control
enforces that exclusion. Tests reject wildcard replacements, unrelated errors,
ambiguous spans, out-of-tree paths, symlink escapes and overlapping edits, and
verify restoration. The real production models and sibling implementations
remain unmodified.

## Final Verification

Cargo checks used Rust 1.98.1, `CARGO_INCREMENTAL=0`, `--locked --offline`
and two jobs. Counts describe separate runs, not unique tests summed across
feature configurations.

| Scope                                                          | Result                                                                                                          |
| -------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| Bidder library, fake contract/allocation and isolation targets | 792 passed by default; 803 with `insecure-bidders`                                                              |
| Configuration, all targets                                     | 710 passed, including 99 scratch-build cases                                                                    |
| Configuration feature cohort                                   | 441 passed                                                                                                      |
| OpenRTB, privacy and runtime, all targets                      | 730 passed                                                                                                      |
| Core/exchange, all targets                                     | 1,821 passed under each default/development feature run; existing ignored compile guard driven separately below |
| Scrub model guard with `--include-ignored`                     | 12 passed after the final containment and lint repairs                                                          |
| CLI bidder-command unit tests                                  | 251 passed                                                                                                      |
| CLI source, import and wave targets                            | 39 passed                                                                                                       |
| Explicit external-source bidder conformance                    | All three ignored checks passed using read-only pinned Git objects                                              |
| Host fanout conformance                                        | 35 core tests plus 14 each for Cloudflare, Fastly and Spin; 77 passed                                           |
| Source-isolation guard, final rerun                            | All three controls passed                                                                                       |
| Strict workspace Clippy                                        | Default and `insecure-bidders`, all targets, passed with `-D warnings`                                          |
| Strict core conformance Clippy                                 | `--all-targets --features conformance-suite`, passed with `-D warnings`                                         |
| Guest library checks with `conformance-suite`                  | Cloudflare `wasm32-unknown-unknown`, Fastly `wasm32-wasip1` and Spin `wasm32-wasip2` passed                     |
| Formatting and whitespace                                      | `cargo fmt --all --check` and `git diff --check` passed                                                         |
| Raw fixture controls                                           | 172 budgeted fixtures across 52 files; negative-control script passed                                           |

The fourth host conformance target is owned by `auctioneer-core`, not the Axum
package. Run `cargo test -p auctioneer-core --test fanout_conformance --features
conformance-suite` explicitly; selecting a nonexistent Axum target in a
multi-package invocation does not prove those 35 tests ran. The final rerun
selected core explicitly. Guest checks are compilation evidence only, not
executed hosted-handler tests or resource qualification.

Final lint repairs use imports, explicit numeric types and distinct local
bindings; no lint allowances or expectations were added. The private core
fixture module is gated by the existing test/conformance feature and contains
one canonical fake module per core build. Synthetic registry setup stays
test-only. Manifests, lockfile, SDK/compiler pins, vendor sources, external
captures, CI workflows and production registry dispatch remain unchanged.

## Publication

Implementation, harness repairs, issue disposition, design and verification
record were committed as `b0114802` and pushed to `m7-bidder-catalog` for
PR #21. This publication note follows that observed successful push. The
remaining discovered serving-registry cutover, schema/resource acceptance and
R0 readiness blockers are not closed by this test-isolation round.

## Follow-Up: Missed Owners And Serving Discovery

The previous completed checklist records that round, not a claim that every
workspace reference was removed. The follow-up audit found missed inline
handlers, lower-layer baked fixtures and actual-App fixtures. The user also
approved current-roster production wiring on 2026-10-25.

- [x] Add red controls for generated published-binding dispatch, distinct public
      and implementation names, shared implementations, unused candidates and
      feature-off/on behavior in `auctioneer-bidder/build_discovery.rs` tests.
- [x] Generate that dispatch with typed Rust tokens, validating all declarations
      before filtering; call it from `auctioneer-core/src/bidder_registry.rs`. Keep the
      existing registry representation, borrowed metadata and current public roster.
- [x] Migrate missed cookie-sync/setuid unit tests through a private explicit
      catalog seam, preserving lazy setup and error precedence.
- [x] Remove concrete-provider assumptions from config/privacy baked-data tests
      using independently scanned declarations or property-selected metadata; retain
      real GVL lookup and allocation proof.
- [x] Replace missed generic exchange conformance inputs with the capable fake;
      preserve provider-specific protocol and allocation obligations at their owner.
- [x] Make actual-App usersync tests select serving metadata by required roles,
      without a production fake, registry override or new feature.
- [x] Review remaining references and strengthen isolation controls; run fresh
      affected tests, feature variants, strict lint and guest compilation.
- [x] Correct `ISSUES.md` and record verified scope, identifying remaining
      source-bound obligations. Commit/push is the final publication operation and
      must be reported only after Git confirms it.

## Follow-Up Verification: October 26

Current published definitions generate serving dispatch; core contains neither
a bidder-name switch nor a generic fallback. The existing registry/cache and
metadata references remain. No bidder manifest, parameter schema, protocol
capture, workload ceiling, SDK pin, compiler patch or CI workflow changed.
The only dependency addition is the already workspace-managed `quote` package
for build/test code generation; no crate or Cargo feature was introduced.

The initial new dispatch probe failed before implementation. Independent
feature-mismatch probes subsequently demonstrated that an extra config feature
was accepted before the compile-time equivalence check. The final controls
reject an appended row (with the existing prefix unchanged), an independently
changed public name, and an independently changed implementation key. Positive
executables cover shared factories, distinct public names, original metadata
pointers, unused candidates and feature off/on/off for normal, empty and
fully gated catalogs. The real Cargo fixture also rejects config-only feature
activation and recovers in the same target directory.

Independent review caught the feature mismatch and embedded-name guard gaps.
Query, percent-encoding, mixed-case and TOML negative controls failed before
their parser/matcher repairs. Source ownership now includes the full exchange
implementation/conformance trees and core usersync tree, plus the audited
handler and integration owners. It does not claim to prove absence of every
possible dynamically assembled identifier. Actual protocol/reference inputs
remain separate from synthetic shared behavior.

All eight real-decoder/transform conformance tests remain data-driven, using
the single bidder-owned factory-input adapter. Actual App tests select serving
roles without registering a fake in production. The authored mid-flight and
direct scrub fixtures use symbolic/synthetic identities; frozen external
captures remain unchanged. New fixture utility `dead_code` allowances are
limited to reusable test-only source modules; no production lint suppression
or new expectation was added.

Fresh checks used Rust 1.98.1, `CARGO_INCREMENTAL=0`, `--locked --offline`
and two Cargo jobs. Counts are per invocation, not unique tests summed across
features.

| Scope                                                      | Result                                                                                          |
| ---------------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| Discovery, isolation and real Cargo binding gate           | 101 passed; 19 child builds in the binding fixture                                              |
| Config/privacy initial all-targets run                     | 1,022 passed, including all 99 scratch-build cases                                              |
| Final config library/codegen/smoke/bindings/storage cohort | 275 passed under each default and `insecure-bidders` run                                        |
| Final privacy all-targets run                              | 410 passed                                                                                      |
| Final core/exchange all-targets runs                       | 1,838 passed under each default and `insecure-bidders` run; two existing ignored cases          |
| Explicit core host conformance                             | 35 passed with `conformance-suite`                                                              |
| Cloudflare/Fastly/Spin host conformance                    | 14 each, 42 total, passed with `conformance-suite`                                              |
| Actual Axum usersync transport                             | 12 passed; secure-production binary regression passed separately via `check-secure-usersync.sh` |
| CLI artifact-integrity regression                          | Passed all six isolated metadata add/modify/delete cases across capture and component handoff   |
| Strict workspace Clippy                                    | All targets, default and `insecure-bidders`, passed with `-D warnings`                          |
| Guest libraries with `conformance-suite`                   | Cloudflare unknown-unknown, Fastly wasip1 and Spin wasip2 compiled                              |
| Formatting, whitespace and raw fixtures                    | Passed; 172 raw fixtures across 52 files, including negative controls                           |

The privacy-parity ignore is driven by the config scratch harness. The existing
scrub-model compile-mutation ignore was not rerun in this follow-up; the earlier
round's explicit 12-test result above remains historical, not a fresh claim.
Guest compilation is not hosted execution or positive resource qualification.
The debug-profile GVL cold-parse benchmark still reports above its stated
10-ms target, and the existing third-party future-incompatibility warning
remains; this cleanup does not close either issue or R0/catalog readiness.

## Authored GDPR Fixture Follow-Up

The Rust-only guard missed concrete identities in five locally authored GDPR
JSON/TOML fixtures. Keep working in the M7 worktree; do not update `main` or
rename immutable external captures. Preserve the real handler/GVL integration.

- [x] Extend `auctioneer-bidder/tests/test_isolation.rs` to check the authored
      `gdpr_applies` fixture family, and observe the current coupling fail.
- [x] Replace names/parameters in the five authored fixtures with symbolic
      full-consent, no-vendor and basic-alias roles. In
      `auctioneer-core/tests/response_gdpr_applies.rs`, select the serving bases by
      metadata properties and reuse bidder-owned factory parameters. Bind all
      roles as host aliases so requests and expected output keep `fake_` names.
- [x] Preserve literal goldens without rewriting expected identities, errors,
      prices, timings or ordering. Keep the pinned vendor 32 and GVL revision,
      complete expected responses, GPP cases and outbound matrix intact. The
      intermediate expectation projector and its test were removed with that
      unnecessary implementation; the original seven behavioral tests remain.
- [x] Run isolation, handler/privacy fixture tests, strict affected lint and
      fixture/format gates. Update the issue disposition, commit and push this
      follow-up; frozen usersync captures and reference assertions remain distinct.

The subsequent user naming decision applies across shared tests: synthetic
bidder/alias identities start with `fake_`, not merely contain it. Audit core,
exchange, lower layers, CLI and provider test owners in disjoint slices. Keep
real catalog/source evidence and deliberate reserved/invalid protocol values
separate, retain ordering/length obligations, and do not widen resource bounds.
Add a workspace literal `BidderName::new` naming guard with negative controls.
The fixture guard also inspects targeting-key and privacy-warning suffixes,
with independent controls for each. Canonical golden seat ordering follows
the new aliases; the driver does not sort actual or expected responses.

## Workspace Naming And Explicit Handler Access

Ordinary test bidders and aliases now start with `fake_` across core,
exchange, lower layers, bidder infrastructure, CLI controls and native
transport tests. Preserve byte/order/case obligations rather than weakening
limits to accommodate longer names. The unchanged 256-KiB maximum-cardinality
proof retains generated shortest legal punctuation aliases with AG's explicit
2026-10-26 approval; no real bidder name is used for those adversarial keys.
Frozen external captures, actual implementation tags and proprietary wire keys
remain source evidence, not ordinary synthetic identities.

The whole-core guard found APS-specific HTTP tests in the shared handler and
an arbitrary shipping-bidder dependency in the deterministic handler/direct
reuse test. The latter is now `auction_reuse.rs`, binding `fake_bidder` to
published passthrough metadata and using the same resolved host/registry on
both paths. Both byte-equality assertions remain.

APS's two protocol-specific HTTP tests now live in its own
`handler_conformance.rs`. At AG's request, a proposed cross-crate `include!`
was removed before publication, along with its source-guard exception. The
existing `conformance-suite` feature exposes `auction_for_conformance`,
which calls the private handler with explicit test-owned dependencies and
normal refusal rendering. A regular Cargo integration target compiles the
owner-local file. A dedicated CI command and exact workflow gate execute it;
default/all-targets without that feature deliberately does not. There is no
new Cargo feature, dependency, serving registry override or default-build API.
This seam tests preparation/finalization/privacy dispatch, not funded ingress
admission or deployment readiness.

The source guard now discovers complete core and exchange semantic trees
and rejects typed literal bidder names lacking the prefix across workspace
source/tests. Authored GDPR checks inspect bidder maps, seats, timing/error
groups, targeting suffixes and privacy-warning identities. Independent
negative controls distinguish identity slots from arbitrary text. The native
serving-fixture reader is unavailable on WASM; guest conformance selects
declared metadata without pulling a filesystem helper into guest code.

### Verified Scope: October 26

These are separate invocations, not unique tests summed across feature graphs.
Commands used Rust 1.98.1, `CARGO_INCREMENTAL=0`, `--locked --offline`
and two Cargo jobs.

| Check                                                  | Result                                                                                                                                             |
| ------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| Affected bidder library and 12 integration targets     | 1,015 passed; three source-object tests ignored in this invocation                                                                                 |
| Explicit APS/Criteo/TTD ignored source-object tests    | All three passed against the authorized local repository's immutable Git objects                                                                   |
| Configuration all targets                              | 612 passed, including all 99 scratch-build cases                                                                                                   |
| Privacy/OpenRTB/runtime all targets                    | 730 passed                                                                                                                                         |
| Core/exchange all targets with insecure-bidders        | 1,837 passed; two existing ignored cases                                                                                                           |
| Earlier default core/exchange all-targets invocation   | 1,839 passed before the final APS test relocation; not a default execution claim for the feature-gated target                                      |
| Explicit APS HTTP + core fanout conformance            | 2 + 35 passed through the explicit API                                                                                                             |
| Final isolation controls                               | Seven passed without an owner-include exception                                                                                                    |
| Workflow contract tests                                | 104 passed, including the exact new conformance command gate                                                                                       |
| CLI bidder units excluding clean-source resource tests | 113 passed                                                                                                                                         |
| CLI immutable-source priority wave                     | Nine passed with AUCTIONEER__BIDDER__SOURCE_DIR supplied; no external checkout modification                                                              |
| CLI production-receipt fixture controls                | Three passed                                                                                                                                       |
| Strict workspace lint                                  | Default passed before the final target relocation; final insecure-bidders workspace and conformance-suite core all-targets passed with -D warnings |
| Final conformance-suite guest library checks           | Cloudflare unknown-unknown, Fastly wasip1 and Spin wasip2 passed                                                                                   |
| Format, whitespace and raw JSON controls               | Passed; 172 raw fixtures across 52 files and all negative fixture controls                                                                         |

The config scratch harness drives the ignored baked privacy case. The existing
scrub-model compile-mutation case was not rerun in this naming-only round; its
earlier explicit execution remains historical. The three source-object tests
were explicitly run, not silently credited from the ignored library rows.
The initial shared scratch-copy byte census failed after the fixture name grew;
its literal census was corrected, both direct controls passed, then the entire
affected bidder cohort above passed including its real child builds.

Independent read-only review found no coverage loss in the explicit API,
owner-local HTTP tests, serving-path reuse or CI gate. Root main is untouched.
No serving bidder manifest, parameter schema, external capture, workload
limit, charge coefficient, SDK revision or compiler patch changed. Guest
compilation is not hosted execution or positive resource qualification.
R0/catalog/schema readiness and the existing dependency future-compatibility
warning remain outside this repair.

Publication is confirmed by Git after this record, not pre-attested here.
Clean-source CLI resource/identity tests must run after committing the source
tree; their outcome belongs to the publication report. Unrelated concurrent
documentation/evidence formatting is excluded from this commit.

## Complete Shared-Test Ownership Follow-Up

This follow-up's cross-crate owner-local Cargo target approach was rejected
on 2026-10-26. The strict component-boundary work below supersedes that
approach; historical execution results do not approve that architecture.

The earlier checks did not cover all shared response-allocation and
generated-settings tests. This follow-up removes those remaining arbitrary
shipping-protocol dependencies instead of treating a synthetic public alias
as sufficient isolation. The separate cross-crate source-import/API refactor
is now explicitly Open in ISSUES.md; it is not this task's completion claim.

- [x] Extend isolation controls to the missed shared tests and observe the
      real-factory/typed-owner coupling fail. Do not exempt entire semantic files
      merely because one test uses shipped settings or a captured field.
- [x] Use the capable fake for generic exchange response/identity/budget
      behavior. Keep true decoder/allocation obligations beside their bidder
      implementation with their original independent assertions and bounds.
- [x] Remove real generated-field imports from shared bidder/config/core
      tests. Exercise generic binding/ownership with synthetic generated
      declarations through the existing compiled-fixture machinery, retaining
      actual owner-field conformance beside the owner. Do not register fake
      settings or bidders in the serving catalog or add a test-only serving API.
- [x] Give real HTTP/qualification fixtures fake requested identities and
      property-selected serving metadata, preserving actual dispatch, input sizes,
      timeout behavior and protocol proof rather than adding a registry override.
- [x] Retain immutable source-capture names/bytes and required proprietary
      wire identifiers. Audit the remaining literal references and identify
      actual source obligations explicitly, not as arbitrary fixture exemptions.
- [x] Run affected default/feature tests, source/conformance, allocation,
      strict lint, fixture and guest gates. Update ISSUES.md with actual scope and
      verification. Publication is confirmed separately through Git.

## Strict Component Boundary

Provider-specific knowledge belongs to the bidder component. Other crates
must not name providers in code, test selectors, fixtures or configuration;
GVL JSON remains the explicit data exception. Discovery and generic readers
may consume bidder-owned records without compiling provider-specific logic.

- [x] Remove all cross-crate Cargo test target paths. Keep shared normalized
      projection and handler behavior on the capable fake; preserve protocol,
      decoding and provider transformation evidence with each real owner.
- [x] Remove real-name case selectors and provider-specific source assertions
      from core/CLI drivers. Keep immutable captures byte-identical and move
      catalog, priority, usersync and name-bearing protocol capture data under
      bidder conformance assets. Move single-owner assertions beside that owner.
- [x] Replace authored non-GVL fixture identities with fake roles. Preserve
      independent goldens, fixture populations, literal byte denominators and
      every source-attribution/digest witness rather than deleting controls.
- [x] Expand guards beyond manually selected Rust files: cover crate manifests
      and structured data, reject out-of-crate test paths, and exercise independent
      negative controls. Generated OUT_DIR output is not a hand-coded registry.
- [x] Verify affected compiled APIs, fake controls, owner conformance, capture
      integrity, features, guest builds and strict lint. Record fresh scope,
      update ISSUES.md, and leave unrelated changes outside the publication.

### Strict Boundary Verification

The final source checks below supersede the earlier cross-crate HTTP target.
No Cargo test target compiles another crate's provider file. The owning bidder
keeps its decoder and resource controls; shared handler/assembly behavior uses
the canonical fake through normal compiled APIs. This is not a claim that fake
decoding proves an implementation's protocol.

The existing family-specific UID sentinel rule now lives in an automatically
discovered owner-local policy. Add/edit/remove and duplicate-family compile
controls preserve the generic fallback without enabling an unimplemented
factory. Root examples are synthetic; the browser helper and provider
preparation select published metadata by independently required properties.
The latter retains the operator-driven, deferred hosted-execution contract,
using its private prepared request rather than the documentation template.

Nineteen relocated captured assets match their HEAD blobs exactly, including
all six OpenRTB captures and the complete usersync data/license/pixel set.
Source revisions, schemas, protocol captures, workload limits, resource charges
and compiler/SDK pins did not change. One already-locked native-only test
dependency parses workflow YAML structurally; it is not a serving dependency.
The scratch source copier now preserves the serving PNG, with an observed
failing-before/passing-after binary-asset control.

Separate final invocations on Rust 1.98.1, locked/offline and two Cargo jobs:

| Check                                                                                    | Result                                                                                   |
| ---------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| Core/exchange all targets, conformance-suite                                             | 1,871 passed; two existing ignored cases                                                 |
| Affected core/exchange library, cookie, handler and response tests with insecure-bidders | 641 passed                                                                               |
| Generated-settings compiled contract                                                     | 16 parent tests passed, including the exact 14-test child pass set                       |
| Isolation source/data/manifest controls                                                  | Nine passed; grouped-import and embedded-data review controls failed before repair       |
| UID policy discovery/dispatch lifecycle                                                  | Two passed, including standalone duplicate-family compile refusal                        |
| Bidder library and real constructor/decoder allocation                                   | 826 passed; five source tests ignored here and explicitly run separately                 |
| Frozen capture integrity/provenance                                                      | 18 passed; 19 relocation blobs separately compared byte-for-byte                         |
| CLI source/import/wave and lightweight units                                             | 37 immutable-source integration tests and 159 units passed                               |
| Explicit main-reference owner source tests                                               | Four passed using immutable local Git objects; no source execution                       |
| Guest conformance library checks                                                         | Cloudflare, Fastly and Spin passed                                                       |
| Strict workspace all-target lint                                                         | Passed with -D warnings                                                                  |
| Browser setup and cleanup                                                                | Two metadata-bundle tests and two actual-child shutdown tests passed; Node syntax passed |
| Format, whitespace and raw-JSON fixture gates                                            | Passed, including owner-local over-budget/slack controls                                 |

OpenRTB/privacy all-target tests and the exact exchange disclosure target also
passed after the six-capture move (110 / 410 / 17). CLI provider/native/fixture
cohorts passed (85 / 165 / 26); these are separate invocations, not unique tests
summed across feature graphs. The supplemental immutable-source test was
explicitly run as well, not credited from an ignored library row.

Independent read-only review found two new guard bypasses, both now closed
with observed red/green controls, and no regression in the traced settings,
response, UID or resource obligations. Full Chromium execution was not rerun:
Playwright is unavailable locally. Guest compilation is not hosted execution
or positive resource qualification. Existing GVL build diagnostics and the
tracked dependency future-incompatibility warning remain unchanged.

Unrelated concurrent documentation/evidence formatting is left uncommitted.
Only moved-asset links and this refactor's semantic documentation are staged.
Publication and clean-source identity checks must be confirmed after committing;
they are not pre-attested by this record.

## Shared Helper Follow-Up

The wider component scan exposed two remaining shared-helper dependencies:
a concrete constructor roster in media classification tests, and factory-harness
meta-tests borrowing one real implementation's fixture. This follow-up does not
replace caller evidence with fake-only success.

- [x] Observe a failing isolation control on both missed helpers before moving
      their assertions.
- [x] Test shared classification and expected-body transformations directly
      with synthetic inputs; retain all actual caller media/partition assertions
      and the five factory-fixture negative controls inline with their owners.
- [x] Remove unused concrete implementation exports. Resolve actual owner
      factories before allocation measurement without changing measured inputs,
      byte/node charges or literal bounds.
- [x] Scan complete shared source files outside manifest-owned directories,
      including unannotated helpers. Cover direct, grouped and renamed imports,
      owner-qualified helper/factory calls and embedded fixtures without a
      handwritten bidder list.
- [x] Update the compiler-facing conformance target for the generic public API
      and prove feature membership through compiled discovery, not deleted symbols.
- [x] Complete default/feature, allocation, scratch-generated settings, strict
      lint, formatting and guest verification. Publish the verified follow-up;
      confirm the commit and remote through Git rather than this checklist.

Independent read-only review confirmed that the caller/meta-test assertions and
allocation bounds were retained. It also exposed import-alias and separate-file
guard gaps; complete-source scanning and parsed import controls replace the
earlier test-context inference rather than adding exceptions.

### Follow-Up Verification

Separate locked/offline invocations on Rust 1.98.1 passed:

| Check                                                          | Result                                                                                                             |
| -------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| Default bidder library, owner allocation and isolation         | 786 + 50 + 11 passed; five existing source tests ignored in this scoped invocation                                 |
| Feature bidder library, generated settings and isolation       | 797 + 16 + 11 passed; five existing source tests ignored; the scratch settings target ran its exact 14 child tests |
| Generic public-API compiler/runtime conformance                | Four passed in each parent feature state; each invocation builds and executes both dependency feature selections   |
| Workspace all-target strict lint                               | Passed with -D warnings                                                                                            |
| Cloudflare, Fastly and Spin conformance guest library checks   | All three passed                                                                                                   |
| Workspace formatting, whitespace and raw-fixture gate controls | Passed                                                                                                             |

The five source tests were not rerun as part of this helper-only follow-up;
their prior explicit invocation is recorded in the preceding verification.
Logs use /private/tmp/m7-shared-owner-*. No source capture, serving behavior,
published definition, workload limit, charge coefficient or dependency pin
changed. This scoped result is not complete M7 or hosted-readiness evidence.

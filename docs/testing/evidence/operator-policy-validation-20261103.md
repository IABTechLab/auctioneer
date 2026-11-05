# Operator Policy Validation Checkpoint

Date: 2026-11-03. Base: `972d9bb` on PR #140. This checkpoint advances the
approved configuration plan without changing the serving reader, startup
installation, dependency pins, workload limits or resource equation.

## Implemented Boundary

`OperatorPolicy::validate` owns shared structural validation. Host and account
typed children use `validator` derives and `#[validate(nested)]`. Root-only
checks handle alias keys and clear-aware scalar values without treating a
whole map as a derived field-error parameter. Returned root errors preserve
fixed codes and precise field paths, without messages or rejected-value params.
Named-account failures collapse to a fixed map category rather than exposing
account names.

`resolve_policy_host` composes explicit product defaults and policy, then clamps
each resource field against its own caller-supplied capacity. It reuses the
resource and GDPR mergers rather than reading process-global boot state.
The old serving resolver still uses its original boot path and representation.

`validate_operator_policy` uses a borrowed `ConfigValidationContext` containing
product host/account defaults, the validated linked catalog, normalized capacity
and caller time. It checks requested values before clamping, effective deadline
ordering, existing cookie/sync rules, account defaults and every named account,
currency expiry, zero funded caps, immutable-disabled activation and the same
bidder-owned dispatcher. Each supplied settings object is structurally checked
once; each relevant identity receives one semantic callback. No factory,
secret/store/network operation or global installation is performed.

Success is not an admitted snapshot. Callers must already bound and fund input
and temporary work. Trusted product defaults/catalog and independently validated
startup capacity remain context prerequisites. The function retains one merged
host and account-default view; named account candidates are composed and dropped
one at a time, not retained as an expanded account/catalog product. Namespace,
error, clone and snapshot lifetimes still require Task 4's resource review before
any serving adoption.

## Red Controls

- Six new host cases compiled and all failed against the unimplemented merger;
  all six passed after implementation.
- The first structural selection executed 18 tests: 16 failed on missing
  validation/redaction; two existing-behavior controls passed.
- Seven new semantic refusal/dispatch cases failed against the scaffold. The
  separate two-catalog context case also failed before implementation.
- Review identified schema-hook path drift. A literal error-tree control failed
  on `host.__all__`; root orchestration restored the status, alias and truncation
  field paths. The final model suite includes all three literal expected trees.

Spec and separate code-quality reviews found no remaining material issue in
this staged scope. Those were read-only source reviews, not runtime or resource
acceptance.

## Executed Checks

Commands use locked/offline dependencies, the existing shared target and two
build jobs. Heavy Auctioneer Cargo commands run serially; compiler fixtures keep
their existing disposable source/workspace ownership.

| Owner / command selection | Result |
| --- | --- |
| Model-only config library and `root_shape` | 177 library and 12 integration cases passed |
| Native config library with `build-support` | 392 passed |
| Shared model library executed under `wasm32-wasip1` with Wasmtime | 177 passed |
| Core library `cargo check --target wasm32-unknown-unknown` | Passed; compile-only, not provider execution |
| Full core library | 626 passed |
| Actual existing `config_funding`, `resolve_runtime_config`, `runtime_config_failclosed` | 1 / 2 / 19 passed |
| Complete `generated_settings` compiler owner | 24 outer and exactly 27 child cases passed, no filters/ignores; outer execution 381.56 seconds |
| Workspace `test_isolation` owner | 16 passed |
| Native config/core `cargo clippy --lib --tests -- -D warnings` | Passed without added allowances |
| Explicit `generated_settings` compiler-owner Clippy target | Passed with `-D warnings` |
| Raw JSON fixture gate | 205 fixtures across 57 files, unchanged allowance |

The new generated-settings child drives the full policy validator with actual
synthetic generated types. It pins fieldwise alias fallback, request-local
identity treatment, exact per-identity observations and refusal of an invalid
disabled/shadowed parent, without invoking a bidder factory. The existing
compiler owner reconciles the complete unique 27-case child set; no second
heavy execution owner was added.

The existing deleted-vendor build notices and `proc-macro-error2` future
incompatibility notice remain. Formatting/lint repairs changed only equivalent
literal spellings, imports, local names and deterministic error traversal; no
assertion or numeric bound was weakened.

## Remaining Acceptance

This does not finish PR #140 or authorize its merge. Strictness at reused legacy
wire leaves, bounded serving-startup input and environment materialization, snapshot ownership,
runtime/CLI adoption, operator-neutral release generation and unchanged-binary
two-config acceptance remain mandatory under the parent plan.

The locked SDK still has `fs::read_to_string` in its authoring loader and collects
ambient environment values in its default overlay policy. The maintainer
subsequently approved using that existing typed CLI flow and tracking bounded
local authoring reads/accepted-input reuse separately; see the follow-up below.
Task 4's serving-startup input ownership and numeric review remain mandatory.
This work adds neither a parallel Auctioneer loader nor a
last-known-good configuration cache. Hosted CI, provider startup/execution and
new memory-fit/readiness claims are not local results of this checkpoint.

## Typed Startup Source Follow-Up

At base `58bca6101759d044986838549bb8cb1894a8976b`, the new staged
`auctioneer-config::startup` model adds source selection only. An omitted config
section requests store mode with unresolved deployment values; a present
section requires `source`. Host and explicit product-defaults modes reject
store options, including null-valued ones. Store requests preserve the pinned
SDK's logical-ID grammar, nonblank/control-free key rules and inclusive
168-hour extraction deadline ceiling. Adapter-specific binding/key validation
and all resource/health declarations remain unimplemented admission work.

The new authoring root and startup/source sections require objects, not Serde's
positional-array representation. A private Serde map adapter delegates typed
field decoding without another JSON tree, cloning or a public parser API.
Optional/newtype generic sections retain their own decoding while still
requiring present nonnull objects. Policy null handling and diagnostic
redaction share their unchanged helper implementation with startup validation.

Observed red controls before the corresponding repairs:

- Initial startup scaffold: five failures and three positive controls.
- Additional selector/deadline cases: two failures and eight positive controls.
- Replacing the empty struct variants with unit variants: one failure and
  nine passes; Serde silently accepted forbidden host-mode fields. One narrow
  `clippy::empty_enum_variants_with_brackets` expectation documents why the
  strict struct variants must remain; no workspace lint policy changes.
- Positional startup array: the new regression failed before object enforcement.
- Generic optional-section and newtype controls exposed incorrect map visitor
  forwarding; the repairs preserve `visit_some` and `visit_newtype_struct`
  without weakening any required-section assertion.
- Under actual bounded SDK extraction, removing only the application-version
  equality check compiled and failed the unsupported-version row (exit 101).
  The original guard was restored, with no production mutation remaining.

The existing `resolve_runtime_config` target now also checks valid/missing/
unsupported/null application versions under SDK version-1 envelopes with
recomputed correct digests, one bounded store read and exact SDK refusal
details. A separate five-row control compares timeout acceptance with the
pinned SDK's actual `ConfigExtractionLimits::validate`; it does not infer
runtime funding from source validation.

### SDK Audit And Approved CLI Scope

The locked SDK is `775366fdac923135e8430b1e3dffebedc648ef7c`. Trusted Server's
actual CLI uses the same typed command APIs at SDK `87456c7` (`v0.0.11`);
its authoring loader is byte-identical across those revisions. The SDK already
provides single-config typed validate/diff/push, secret metadata and envelope
handling. Its CLI performs a raw load followed by a typed reread; bounded
local file/manifest/environment ingestion and once-only accepted-input capture
are additional hardening, not missing basic configuration capability.

On 2026-11-03 the maintainer approved the existing SDK CLI flow and separated
that hardening in [#142](https://github.com/IABTechLab/auctioneer/issues/142)
and design section 9.1. Serving extraction,
privacy and startup/storage approval were explicitly not waived.

The native audit found independent logging, runner and preflight environment
scans plus direct health inputs before App construction. Existing snapshot
file/resident/startup checks do not retroactively bound those earlier copies.
The installed App extraction tuple remains backend 131,072 B, blob 65,536 B,
depth 64, 8,192 JSON values, secret 1,048,576 B, total payload bytes 16,777,216 B,
timeout 2,000 ms. Preboot Apps instead capture SDK backend/blob defaults of
16 MiB/8 MiB; the guard currently compares only backend/blob/value caps.
Total payload bytes are not a complete allocation bound. All seven fields,
early materialization and retained generations remain part of Task 4 review.

No startup activation, store cache, SDK extension, workload change or resource
equation change is part of this follow-up. Current CLI/serving readers still
use the old DTO. The next cutover must establish funded immutable ownership
and the reviewed startup domain before switching them.

### Follow-Up Verification

Commands ran serially with locked/offline dependencies, two build jobs and the
existing shared target. Counts below identify owners, not an additive claim
that overlapping feature lanes are distinct cases.

| Owner / command selection | Result |
| --- | --- |
| `auctioneer-config --features build-support --lib --test root_shape` | 407 library / 15 root-contract cases passed |
| Default serde-only config `--no-default-features --lib --test root_shape` | 6 library / 13 root-contract cases passed |
| Model-only config selection | 192 library / 15 root-contract cases passed |
| Core `--lib --test resolve_runtime_config` | 626 library / 4 SDK-integration cases passed |
| Config `--features build-support --all-targets` strict Clippy | Passed |
| Core `--test resolve_runtime_config` strict Clippy | Passed |
| Workspace source-import/test-ownership target | 16 passed |
| Config model `cargo check --target wasm32-unknown-unknown --lib` | Passed; compilation only |
| Raw JSON fixture gate | 205 fixtures / 57 files; unchanged allowances |

Existing deleted-vendor and `proc-macro-error2` future-incompatibility notices
remain. The installed Wasmtime is 45.0.0 while this worktree declares 48.0.2;
no new guest execution is claimed. Prior recorded execution with 45.0.0 is
supplementary evidence, not acceptance under the currently declared runner.
No hosted CI, provider test, whole-workspace acceptance or new memory
qualification is claimed. Independent spec and code-quality reviews passed
for the staged source/object-boundary slice, not for the unfinished cutover.

## CI Migration Repair

The published checkpoint `83659d0` exposed three test-side migration misses:

- [Native workspace and secure resource-profile checks](https://github.com/IABTechLab/auctioneer/actions/runs/41789968201)
  failed to compile because the restart/push test passed the dependency-neutral
  model to the SDK. The existing local `SdkAuctioneerAppConfig` wrapper owns
  its SDK traits; the test must use that wrapper at the typed push boundary,
  retaining its ordinary model and actual restart/snapshot assertions.
- The same run's isolated compiler owner passed 99 cases and failed the
  explicit-false/zero/empty defaults case. Its AST assertion traversed
  `HostConfig.user_sync` as a bare struct after the field became optional.
  Explicitly require the generated `Some` before checking its cooperative
  flag; retain every literal zero/false/empty assertion and the real child
  build. The helper does not acquire implicit optional-value traversal.
- [Discovered-source formatting](https://github.com/IABTechLab/auctioneer/actions/runs/41789968584)
  stopped before workspace lint on one Rubicon conformance import. That file
  is discovered by generated wiring, so the workflow's explicit source check
  is required in addition to `cargo fmt`.

Local red controls reproduced the exact SDK trait-bound error, generated
struct-literal error and rustfmt import-order diff before their repairs.
The repaired generated-defaults case passes (1 executed, 99 filtered); the
complete native resource-profile target passes all 14 cases in each of the
default, linked-core-conformance and dependency-only config-override lanes.
The last lane sets `AUCTIONEER__TEST__EXPECT_CONFIG_OVERRIDE=enabled`, as CI
does; lane counts overlap and are not 42 distinct case identities. Workspace
all-target Clippy passes with `-D warnings`; Cargo formatting and the explicit
all-discovered-bidder source formatting check pass. Independent review found
no issue in those three test-only changes. No production type, feature gate,
resource limit, assertion or configuration reader was changed.

These local checks do not rerun the full 100-case compiler harness, complete
workspace test matrix, guest execution or hosted provider acceptance. The
published failing run is retained as failure evidence, not reclassified as
passing by the local repair.

## Active Reader Migration Census

Task 1's source census uses production base `83659d0`; the repair above changes
only tests. Paths below are relative to `crates/` unless explicitly rooted.
Grouped paths identify concrete owners, not test counts or cutover acceptance.
The original field inventory's pinned older paths remain historical facts;
this is the implementation-era package-role map.

| Owner | Still-active boundary and migration obligation |
| --- | --- |
| Authoring/build root | `auctioneer-config/src/lib.rs::AuctioneerConfigSource`; `auctioneer-config-data/build.rs::{read_config_source,emit_host_default,emit_account_default}` still consume root `auctioneer.toml` and `baked.{host,account}`. The final build reader must consume only product data. |
| Policy extraction | `auctioneer-config/src/source.rs::AuctioneerAppConfig`; core `config/{mod,sdk,runtime}.rs` validates and extracts `SdkAuctioneerAppConfig`. The staged `OperatorPolicy` is not yet that reader. |
| Resolution | Core `config/resolve.rs`, `budgets.rs` and `bidder_settings.rs` still compose compiled defaults with old `auction`, `targeting`, `status.banner`, flat privacy/sync/alias and account/bidder/feature/currency overlays. New source scopes must replace, not silently alias, those keys. |
| Serving | Core `handlers/{auction,cookie_sync,setuid,getuids,status,info_bidders}.rs`, `usersync/catalog.rs` and `host_config_handle()` retain old host/account state. `handlers/probe_config.rs` owns separate configuration-work admission; do not create another policy reader. |
| Bootstrap | Config-data `data/runtime-bootstrap.toml`, `build.rs::emit_runtime_bootstrap`, config `codegen/mod.rs::RuntimeBootstrapRaw` and `types.rs::RuntimeBootstrap`; core `configure.rs` captures its timeout and `config/runtime.rs` uses its missing-config fallback. Source selection replaces that fallback only at the approved cutover. |
| CLI authoring | `auctioneer-cli/src/config_source.rs::AuctioneerPushSource` wraps the old root and emits only runtime; `application.rs` sends it to typed validate/diff/push. Preserve shared validators and secret metadata; SDK GC is unchanged. |
| Accounting and identities | Core `host_primitives/gates.rs::resolve_budget_inputs`, CLI `bidders/resources/mod.rs::{baked_identity,row,source_row}`, Axum `src/resource_profile.rs::profile`, and runtime `src/{budget,aux_work}.rs` charge/hash the old default pair. Change their ownership/identity inputs coherently, not just accessor spelling. |
| Qualification inputs | CLI `qualification/harness/native/snapshots.rs` generates and pushes old documents; `providers/preparation.rs` parses root `baked` for its digest. `qualification/{profile,evaluate,report}.rs` and harness command/provider/native input/runner/system owners consume those identities. This is active tooling, not an immutable capture to skip. |
| Adapter startup | Axum `src/main.rs` and guest `src/lib.rs` callers use shared `bootstrap_adapter_with_envelope`; each adapter owns `src/envelope.rs::resource_profile`. Shared core bootstrap/gate/profile changes require compatible guest callers without claiming their separate startup ports are delivered. |
| Active manifests/examples | Root `edgezero.toml`, `wrangler.toml`, `spin.toml`, Axum `examples/usersync_browser_config.rs` and root `fastly.toml`. Fastly carries an actual old-format local envelope: migrate its payload and integrity digest together, not just selectors; envelope version is not application schema version. |
| Harness overrides | Config-data build/harness owners read/set/remove `AUCTIONEER__DEFAULTS__PATH_OVERRIDE` and `AUCTIONEER__RUNTIME_BOOTSTRAP__PATH_OVERRIDE`; core `tests/privacy_config_parity.rs` independently consumes the defaults override. Preserve the test-harness feature-export boundary. |
| Staged replacements | Config `src/{lib,policy,startup,override_value}.rs`; config-data `data/product-defaults.toml`, `emit_product_defaults` and product accessors; core `config/{policy,policy_host,policy_account,validation,budgets}.rs`. Their existence is not serving adoption or funded snapshot admission. |

### Fixture And Acceptance Owners

- Authoring/codegen: config `tests/root_shape.rs`; config-data
  `tests/{baked_defaults_snapshot,usersync_config,usersync_callback_validation,smoke,codegen}.rs`
  and `generated_syntax/mod.rs`. CLI `src/config_source.rs`, core
  `tests/config_reference_examples.rs` and callback validation include the live
  root TOML; those are active inputs, not frozen reference bytes.
- Compiler/privacy parity: config-data `tests/build_failure_harness.rs`,
  bidder `tests/build_workspace/mod.rs` (copies root TOML), existing
  `generated_settings` and `tests/support/generated_settings_contract.rs`.
  Preserve their actual mandatory child dispatch and parent-driven privacy
  oracle; do not add another full compiler suite.
- Store/refusal/semantic controls: core `tests/{resolve_runtime_config,runtime_config_failclosed,config_error_precedence,config_store_failed_open,config_degradation_warning,config_deadline,config_node_cap,config_parse_factor,bidder_settings_conformance,host_alias_overlay_bounds,privacy_config,privacy_config_parity,privacy_config_sharing,response_gdpr_applies}.rs`
  and live `fixtures/gdpr_applies.toml`. New-version control documents must pass
  every earlier layer before testing digest/privacy/alias/secret refusal.
- Shared serving fixtures: core `src/test_support/config_store.rs::runtime_registry`
  intentionally envelopes supplied input unchanged; `tests/support/mod.rs::ctx_with_config`
  parses the old type. Auction, sync, probe, header/body and response-slot
  integration fixtures use those owners plus inline config/handler/catalog
  units. Do not hide a legacy converter in the shared store fixture.
- Resource/process owners: core `config_funding`, `config_peak_charge`,
  `auction_config_retention`, `boot_strict_parity`, `boot_resource_charges`,
  `catalog_resource_profile`, `host_limits_clamped_to_envelope`,
  `boot_warning_lands_in_the_envelopes_sink`, node/impression/admission and real
  bounded-handler targets. Preserve their separate allocators/global domains
  and independent expected charges. Exchange construction/deadline/response/
  projection/allocation owners and privacy `privacy_resource_limits` consume
  the shared default types too.
- Adapter contracts: all four `tests/boot_envelope.rs`; Axum `privacy_wire`
  and native lifecycle targets; Cloudflare/Fastly/Spin manifest/health tests
  and separate cookie-key provisioning checks. Their prerequisites and
  conformance/test-support/harness features must remain explicit.
- CI ownership: `.github/workflows/test.yml` and
  `scripts/raw-json-fixtures.allow` name `baked_defaults_snapshot`; update both
  when renaming it. Existing colocated `verify-config-build-failures.sh` and
  `verify-schema-compiler-conformance.sh` retain their single mandatory owners.

This census completes source-owner enumeration only. Per-case fixture
replacement/earlier-layer validity, field-to-validator/resource disposition,
numeric startup/storage approval and actual reader/default cutover are still
uncompleted plan steps. Do not infer a smaller migration from the focused CI
repair or a larger executed-test count from this file list.

External privacy/sync/reference captures, bidder protocol captures and
provenance witnesses retain their original bytes. Historical qualification
bundles remain historical; active generators may change with new identity
bindings without rewriting earlier results.

## Current Runtime Cutover Checkpoint (2026-11-04)

This append-only checkpoint supersedes the earlier staged-reader, source-mode
and startup/storage-approval descriptions as current status; their historical
bytes and results above are preserved. Source inspection uses the dirty PR #140
worktree at HEAD `58385503a9b77f69686bef060b2b101b403bb481`. The saved runs are
not asserted to share one frozen source snapshot: some precede later adapter
literal/source changes. This documentation pass ran no Cargo or publication.

### Implemented Source Boundary

- The operator file and runtime data are direct `OperatorPolicy`, requiring
  root `schema_version = 1`; baked/startup/runtime wrappers and old flat keys
  are rejected. Core's SDK wrapper preserves layout and delegates validation;
  CLI uses existing typed validate/diff/push, without a new loader or wire model.
- Shared effective validation uses supplied product defaults/catalog; pure
  typed-policy helpers also accept caller-owned context. Command validation
  does not invent an operation clock or installed capacity. Request freshness,
  installed caps and ordering retain their existing owning boundaries.
- Serving uses the shared SDK validator, requires policy, and checks all seven
  captured SDK tuple fields before extraction. Neutral compiled defaults and
  their independent storage/identity owners replace operator build inputs.
  Startup/source models and their orphan raw/effective structs are retired.
- Existing request admission, early release of unrelated accounts, retained
  effective views and cancellation/final-drop accounting remain required.
  No host normalization constructor, global snapshot pool, startup reservation
  or SDK startup hook is introduced. Actual host ingestion/adoption and its
  allocation/capacity/lifetime proof remain #135, not a PR #140 readiness claim.

### Inspected Verification Logs

All filenames below are under `/private/tmp/auctioneer-runtime-cutover-`.
Numbers are terminal test results in the saved logs, not new executions or a
union of unique case identities. Earlier focused results overlap the full core
run; parent-driven and catalog-variant cases must not be added as unique tests.
Top-level command flags/environment are not reconstructed from target names;
the coordinator's next final update will record the fresh full commands.

| Log suffix | Observed result and scope |
| --- | --- |
| `config-final.log` | Config library 398 passed; `root_shape` 5; config-data library 4; `codegen` 69; `product_defaults_snapshot` 2; `usersync_config` 5. All six targets: 0 failed/ignored. |
| `build-harness.log` | Complete selected `build_failure_harness` target: 101 passed, 0 failed/ignored/filtered; 153.12 s. Includes release product output independent of missing/malformed operator file/override controls and the parent-driven privacy parity case. |
| `settings-final.log` | Selected `generated_settings` outer case: 1 passed, 0 failed/ignored, 23 filtered; 125.81 s. Its main child reconciles exactly 31 passed, 0 failed/ignored/filtered. Three further executions of the same catalog-scratch case pass at 1, 256 and 512 definitions (31 filtered each). The five child commands include workspace update; this is not five test suites or the full 24-case outer owner. |
| `core-focused.log` | Earlier core library 633, `config_parse_factor` 19, `resolve_runtime_config` 4 and `usersync_routes` 22 passed; 0 failed/ignored. These are overlapping subsets of core-final, not additional unique cases. |
| `core-final.log` | Fresh full core run: 94 binaries/result rows, 1,136 passed, 0 failed, 1 existing ignored, 0 filtered. Coordinator reports exit 0; direct log reconciliation confirms those counts. Includes library 633, actual SDK application-version/refusal cases, funding/node/peak/factor/boot owners, auction lifecycle/retention, sync lifecycle/integrity/allocation and response/account/privacy owners. |
| `fastly-guest-test.log` | `config_validation_tests::product_defaults_validate_against_the_target_published_catalog`: 1 passed, 0 failed/ignored, 33 filtered. Selected guest execution only, not full guest/provider or hosted acceptance. |
| `browser-tests.log` | Node shutdown regression: 2 tests passed, 0 failed/cancelled/skipped/todo; 20,818.316292 ms. Proves pending-launch subprocess/config cleanup and permission-failure handling with the existing stub, not real Chromium browser acceptance. |
| `axum.log` | Earlier attempt: 76 passed, 5 failed, 1 ignored. Library: 20 passed/4 failed; `boot_envelope`: 2 passed/1 failed. Passing targets include main 4, cancellation 3, health 4, privacy-wire 1, resource-profile 14, sync-transport 12 (1 separate secure-production ignore), wave-transport 14 and browser-example units 2. The zero-case baseline-probe target adds no proof. Not a green Axum all-targets result. |
| `cli-all.log` | Earlier CLI binary: 755 passed, 14 failed, 3 existing parent-driven helper ignores; 13.90 s. Its config-contract family has 15 passing parent cases, including direct schema/wrapper rejection, effective sync validation, root-relative metadata, env/no-env controls and typed validate without writes. The family is a subset of 755, not a green full CLI result. |

Core-final's existing ignored
`privacy_config_parity::tests::emitted_baked_config_matches_runtime_fixture`
is owned by config-data's compiler parent; that parent passed in the 101-case
run. The ignore is still reported as an ignore, not silently converted into
another core pass. The three CLI helper ignores are likewise driven by their
passing parent cases; secure-production Axum execution is a separate lane.

The settings catalog probes observe SDK validation peak/retained allocations
within existing configuration charges using synthetic discovered catalogs.
They are not host Settings ingestion/adoption, physical memory-fit evidence or
proof that every bidder error allocation path was separately remeasured.
Existing over-ceiling boot warnings remain in core logs; green tests do not
turn those warnings into deployment qualification. Deleted-vendor and future
`proc-macro-error2` notices, and the fixture's dead-code warning, are not strict
workspace Clippy acceptance.

The native lifecycle case in the 14-case resource-profile run uses the existing
test owner, real typed local push and unchanged executable hashes across isolated
A/B stores. It checks different banners/node caps, snapshot stability before
restart, replacement visibility after restart and installed node-cap enforcement.
It does not by itself prove every independent account/feature/replacement case
listed in the plan or acceptance of actual adapter port #131.

### Failed Runs And Pending Final Checks

The saved Axum library/boot failures remain failed evidence. Effective-limit
and expected-literal repairs are source changes, not a passing rerun; all four
adapter finals remain pending at this checkpoint. The coordinator verified
CLI's strict source-identity failure against exactly eight adapter-envelope files
changed after its compilation. That stale executable is not classified as a
demonstrated production bug, nor is its log reclassified as green. Fresh CLI
build/full tests against the frozen tree remain pending.

- Strict workspace Clippy found test/style issues; repairs are in progress
  without allowances. No green strict-Clippy result is claimed. Final formatting,
  discovered-source formatting, fixture/isolation gates and their exact fresh
  command receipts still need the coordinator's update.
- Full CLI default/maintenance/feature lanes, actual CLI/core boot-report parity,
  all adapter finals and separate secure-production user-sync acceptance are
  not replaced by the earlier passing subsets or Node cleanup regression.
- The complete outer generated-settings owner, remaining config-data targets
  and runtime-package library checks are not silently marked rerun by the
  selected parent, 31-child contract, 101-case build owner or full core run.
  Exact remaining owners are `auctioneer-bidder --test generated_settings`
  (all outer cases) and `--test test_isolation`, the config-data data targets
  listed in Task 8, and `auctioneer-runtime --lib`. Fresh CLI ownership includes
  its binary/default/maintenance lanes and `--test resource_boot_parity`;
  adapter finals belong to all four adapter packages, with secure user-sync
  separately owned by `scripts/check-secure-usersync.sh`.
- Complete per-case migration/earlier-layer dispatch reconciliation and the
  plan's whole enumerated CLI/SDK/typed-policy matrix, including disposable
  mutation controls, require explicit receipts. Source/selected-case success
  does not certify those compound plan items.
- The coordinator's final command/environment/source-identity record and final
  acceptance review remain open. Exact full commands, exit states and feature
  scopes will be appended, not inferred or substituted with historical results.

### Child Tasks And Review Scope

#131-#134 retain actual adapter/provider integration and their own unchanged-
artifact/ingestion/hosted acceptance. #135 owns actual host Settings allocation
ingestion/adoption, SDK compatibility and lifetime/capacity proof; typed-policy
semantic parity does not close it or integrated artifact #76. #136 retains
remaining deployment/embedding documentation tied to implemented surfaces.
#142 separately owns bounded local authoring reads/once-only accepted-input
reuse; using existing SDK typed commands does not implement that hardening.
No issue status, dependency, workload, floor, ceiling or resource equation is
changed by this evidence append.

The read-only documentation review found no active conflicting startup/storage
approval gate or supported alternate wire format in the revised contract.
This pass also inspects source owners and reconciles saved logs; it is not a
fresh runtime execution, an independent final production-code signoff, a green
full workspace/feature matrix or a deployment/readiness qualification.

## Final Local Receipt Update (2026-11-04)

This append supersedes only the named pending receipts above. Earlier failures,
counts and captures remain unchanged. The coordinator reports exit 0 for the
completed CLI, boot-parity, model, adapter, callback and Clippy lanes below;
inspection independently reconciles their terminal summaries. This documentation
pass runs no Cargo and leaves source frozen. These are local, unpublished
receipts, not a source-commit-ready, complete-matrix, CI or deployment claim.

### Final Command And Result Receipts

Log suffixes retain the `/private/tmp/auctioneer-runtime-cutover-` prefix used
above. The saved Cargo output does not record full top-level argv/environment;
exact invocation receipts remain pending the coordinator's transcript, not
reconstructed from target names. When supplied, commands must use environment
placeholders such as `${WORKTREE}` and `${CARGO_TARGET_DIR}` instead of private
checkout/target paths, preserving actual SDK-owned variable names.

| Log suffix / invocation scope | Result and remaining qualification |
| --- | --- |
| `cli-frozen-final.log` / ordinary CLI targets | Exit 0; 15 test binaries, 926 passed, 0 failed, 3 existing parent-driven helper ignores, 2 filtered. The filtered cases are the existing once-owned producer skips, not new passes. The maintenance-build target's 2 passes are included, not an additional suite total. |
| `boot-cli-final.log` / `resource_boot_parity` | Exit 0; 8 passed, 0 failed/ignored/filtered; 218.79 s. Actual fresh-process boot and real CLI accounting controls ran, not merely the ordinary CLI invocation. |
| `model-final.log` / config library and `root_shape` | Exit 0; 398 + 5 = 403 passed, 0 failed/ignored/filtered across 2 targets. |
| `adapters-verified.log` / four adapter libraries and `boot_envelope` | Exit 0; 87 passed, 0 failed/ignored/filtered across 8 native targets: Axum 24 + 3, Cloudflare 20 + 1, Fastly 15 + 1, Spin 22 + 1. Selected native controls, not complete adapter all-targets, guest or hosted-provider acceptance. |
| `callback-final.log` / `usersync_callback_validation` | Exit 0; 6 passed, 0 failed/ignored/filtered; 0.04 s, after repairing the independent neutral fixture. This is a separate rerun, not a rewrite of the failed invocation below. |
| `data-runtime-final.log` / selected config-data targets and runtime library | Original saved invocation remains failed: config-data 196 passed, 6 failed; runtime library 266 passed, 0 failed. The six callback cases subsequently pass in `callback-final.log`; config-data's latest per-target passing receipts total 202 across those two invocations, not one green unified rerun. The 101-case compiler harness is separate. |
| `clippy-pass.log` / strict default workspace all-targets lane | Coordinator reports exit 0; terminal `Finished` in 12.14 s. No allowances added to obtain this result. Exact invocation flags/environment await the command transcript. |
| `clippy-insecure.log` / strict insecure-bidders lane | Coordinator reports exit 0; terminal `Finished` in 52.85 s. No allowances added to obtain this result. Exact invocation flags/environment await the command transcript. |
| `core-conformance-final.log` / fresh full core including otherwise-gated conformance targets | Running at this receipt cutoff; no final count, exit status or green result claimed. The earlier 94-binary/1,136-pass core receipt above is not substituted for this run. |
| Exact commands, final source identity and remaining feature/format/fixture/isolation receipts | Pending coordinator reconciliation; not inferred from completed subsets or Clippy. Existing acceptance owners remain in place. |

Boot parity records 40 distinct fresh boot PIDs, each with one installation:
32 rows cover all four adapters across all eight explicit/default cap masks;
8 further rows cover the existing warn-and-install cases, retaining strict CLI
exit 1 for over-ceiling inputs. The passing owner checks all seven fields of
the actual captured SDK tuple: `max_backend_bytes`, `max_blob_bytes`,
`max_json_depth`, `max_json_nodes`, `max_secret_bytes`, `max_total_bytes` and
`timeout_ms`. Four traced scratch-only mutations (`raw_zero`, `zero_baked`,
`raw_zero_cloudflare`, `zero_baked_spin`) refuse with exit 2 and no corrupt
accounting output. These resource controls do not establish physical fit or
actual provider/host adoption.

### Acceptance Scope And Remaining Checks

The active spec/plan's latest directed scope alignment replaces the earlier
requirement to replay the whole field matrix through three paths: exhaustive
nullable/clear/semantic oracles belong once to shared model/resolver tests with
nonempty supplied defaults. Actual CLI/SDK owners prove direct payload/object
guards and shared-validator delegation, including bidder-settings miswiring;
actual lifecycle owners prove policy A/B and no fallback. The canonical same
underlying model removes subtree-projection drift, not boundary verification.
No new validator, synthetic CLI-default rebuild or duplicate field Cartesian
matrix is required; every existing test, negative control and limit remains.

The earlier 101-case build harness, selected settings parent/31-child contract
and catalog-allocation probes retain their recorded scope; they do not certify
all outer settings cases or all bidder allocation paths. The running full-core
lane, exact commands/source identity, remaining command receipts and final
acceptance review still require reconciliation. No unchecked plan item is
marked complete by this append. #131-#136 and #142 retain the separate owners
above, including #135's actual host ingestion/adoption and capacity/lifetime
proof. SDK extraction bounds and per-request admission/retention remain
mandatory. Nothing here authorizes publication or claims CI/readiness approval.

### Full Core Feature Run: Failed Receipt (2026-11-04)

`core-conformance-final.log` has completed and supersedes its earlier running
status only. Reconciliation of all terminal result rows with `awk` gives
97 binaries, 1,185 passed, 2 failed, 1 existing ignored and 0 filtered.
The log ends with `error: 2 targets failed`; the full run is not green.

| Failed owner | Preserved result / observed failure |
| --- | --- |
| `auctioneer-core --test fanout_conformance` | 34 passed, 1 failed. The no-outbound-capability case reaches unfunded configuration extraction instead of the expected outbound-HTTP refusal; the coordinator identifies an unfunded raw-context fixture. |
| `auctioneer-core --test native_bidder_onboarding` | 7 passed, 1 failed. The frozen-flow case receives malformed-host-configuration status 500 instead of 200; policy validation remains under investigation. |

Bohr owns fixture repairs and exact-owner reruns; no repair or passing rerun is
claimed here. Preserve this failed invocation even after later targeted results.
The boot/CLI receipt remains 8 passed with all 40 fresh PIDs and the actual
seven-field captured tuple. Next coordinator receipts are `isolation-final`
and the CLI maintenance feature check; neither is marked complete by the
earlier ordinary CLI maintenance-build subset. Exact commands/environment and
final acceptance reconciliation remain pending. This append changes no source,
plan checkbox, captured bytes or publication status.

### Accepted PR #140 Completion Receipts (2026-11-04)

This append records the completed accepted shared-runtime scope, superseding
only earlier pending/failed current-status conclusions, never their historical
bytes or invocation results. The coordinator reports exit 0 for the completed
core, Axum, isolation, Clippy, secure-production and combined data/runtime jobs; terminal summaries
below were independently reconciled. No command was executed by this docs pass.
Three authorized present-tense Rust comment edits followed the first runs.
Post-comment formatting and a freshly compiled CLI source-boundary check have
now passed, as recorded below. Publication still requires explicit approval;
no hosted-CI or deployment-readiness assertion follows from these local checks.

The table records confirmed command/feature selections, not an invented exact
shell transcript: saved output omits full top-level argv/environment. External
path arguments must remain placeholders such as `${WORKTREE}` and
`${CARGO_TARGET_DIR}` when the coordinator supplies that transcript. Toolchain
pins remain in `.tool-versions` (Rust 1.98.1); the SDK revision remains
`775366fdac923135e8430b1e3dffebedc648ef7c`. Pins identify the repository inputs,
not a substitute for an unrecorded invocation-version command.

| Command / confirmed feature selection | Log suffix | Current receipt |
| --- | --- | --- |
| Core full test owner, including `conformance-suite` gated targets | `core-complete.log` | Exit 0; 97 binaries, 1,187 passed, 0 failed, 1 existing ignored, 0 filtered. Includes both formerly failing owners: fanout 35 passed and native onboarding 8 passed. The earlier 1,185-pass/2-fail invocation remains failed evidence. |
| Axum full all-targets, `insecure-bidders` | `axum-complete.log` | Exit 0; 11 result rows, 81 passed, 0 failed, 1 existing ignored, 0 filtered. Includes actual `resource_profile` 14, `privacy_wire` 1 and `wave_transport` 14. The zero-case baseline-probe row adds no executed case. |
| `scripts/check-secure-usersync.sh`, separate secure-production binary | `secure-usersync-final.log` | Exit 0 reported; actual previously ignored production test: 1 passed, 0 failed/ignored, 12 filtered; 1.06 s. The Axum all-targets receipt above still reports its original ignore; this is a separate execution, not an extra Axum pass. |
| Bidder `test_isolation` owner | `isolation-complete.log` | Exit 0; 16 passed, 0 failed/ignored/filtered; 3.92 s. |
| Strict workspace all-target Clippy, core `conformance-suite` | `clippy-conformance.log` | Exit 0; terminal `Finished` in 1m 06s. Test builds are included; no allowances added to obtain green. |
| Strict explicit CLI `resource_boot_parity` Clippy owner | `clippy-boot-parity.log` | Exit 0; terminal `Finished` in 17.68 s. Distinct from the executed parity tests below. |
| Strict default / insecure-bidders lanes | `clippy-pass.log`, `clippy-insecure.log` | Previously recorded exit-0 receipts remain: 12.14 s / 52.85 s; not new test executions. |
| CLI maintenance library, binary and help owners, `--no-default-features` | `cli-maintenance-final.log` | Exit 0 reported; 37 + 168 + 4 = 209 passed across 3 targets, 0 failed/ignored/filtered. |
| Ordinary CLI owners | `cli-frozen-final.log` | Existing exit-0 receipt: 15 binaries, 926 passed, 0 failed, 3 existing parent-driven ignores, 2 existing once-owned producer filters. Not added to maintenance as unique case identities. |
| Actual CLI/core boot parity | `boot-cli-final.log` | Existing exit-0 receipt: 8 passed, 0 failed/ignored/filtered; 218.79 s. All 40 fresh PIDs, seven actual captured SDK fields and four traced mutation refusals remain the proof described above. |
| Combined config-data/runtime targets | `data-runtime-verified.log` | Exit 0 confirmed; 14 binaries, 468 passed, 0 failed/ignored/filtered (config-data 202 + runtime 266). This is a separate invocation from the earlier failed callback run. |
| Model / four native adapter library-and-boot owners / once-owned compiler and settings contracts | Earlier named logs above | Model 403; adapter 87 across 8 targets; build harness 101; selected settings parent 1 with 23 filtered, its 31 children and three catalog probes retain their exact recorded scope. Not full outer-settings, hosted or whole-workspace test qualification. |
| Full existing `generated_settings` outer owner | `settings-complete.log` | Exit 0; 24 passed, 0 failed/ignored/filtered; 140.12 s. Includes the 31-case child contract and all three catalog-allocation probes. This supersedes the earlier selected-parent-only scope, not its historical result. |
| Fresh CLI source-boundary verification after the final prose-only source edits | `cli-boundary-final.log` | Exit 0; library 37, binary 770 and help 4 passed: 811 per-invocation passes, 0 failures, 3 existing parent-driven helper ignores. These overlap the ordinary CLI cases and are not additional unique coverage. |
| Post-comment format, discovered sources and raw JSON controls | Coordinator command transcript below | Cargo formatting and discovered-source rustfmt passed. Raw fixture gate: 203 fixtures in 56 files; its regression controls passed. Shell syntax and final whitespace checks passed. |

The main plan's completed items refer to the latest explicitly accepted
ownership: exhaustive shared nullable/clear/semantic oracles with supplied
nonempty defaults, independent actual CLI/SDK direct-object guards/delegation
and bidder-miswiring controls, actual lifecycle A/B/no-fallback controls, and
unchanged extraction/funding/retention limits. This does not claim a second
per-field/per-path matrix, every historical preparatory listing/mutation
experiment or every bidder allocation error path.
Existing tests, negative controls, privacy, captures and limits are retained.

The feature-enabled core fixture failures and earlier Axum/CLI/data failures
remain recorded as failed runs. Their later passing owners do not rewrite them.
The local accepted-scope checklist is not the full CI matrix, guest/provider
readiness or physical fit. #131-#136 retain actual provider/host/deployment work,
including #135 ingestion/adoption capacity/lifetime proof; #142 retains local
authoring-read/reuse hardening. These are separate owners, not unchecked #140
implementation gates. Publication still requires explicit approval.

### Final Command Transcript

The coordinator executed the following top-level selections serially with the
existing shared Cargo target. Paths are symbolic to avoid recording a private
checkout location; `REFERENCE_SOURCE` contains both pinned Git objects used by
the source and supplemental import checks. The commands use the unchanged
lockfile and SDK pin above. Child commands and their reconciliation remain in
their owning compiler/parity logs.

```sh
cd "$WORKTREE"
export CARGO_TARGET_DIR="$SHARED_TARGET"
export AUCTIONEER__BIDDER__SOURCE_DIR="$REFERENCE_SOURCE"
export AUCTIONEER__BIDDER__SUPPLEMENTAL_SOURCE_DIR="$REFERENCE_SOURCE"

cargo test --locked --offline -j2 -p auctioneer-config --features build-support --lib --test root_shape
cargo test --locked --offline -j2 -p auctioneer-core --all-targets --features conformance-suite --no-fail-fast
cargo test --locked --offline -j2 -p auctioneer-cli --all-targets --no-fail-fast -- --skip tests::selected_schema_source_producer_records_exact_real_dispatch_without_qualification --skip tests::controlled_native_acceptance_requires_all_producers_but_never_completes_the_full_catalog
cargo test --locked --offline -j2 -p auctioneer-cli --test resource_boot_parity -- --nocapture
cargo test --locked --offline -j2 -p auctioneer-cli --no-default-features --lib --bin auctioneer-cli --test help --no-fail-fast
cargo test --locked --offline -j2 -p auctioneer-cli --lib --bin auctioneer-cli --test help --no-fail-fast
cargo test --locked --offline -j2 -p auctioneer-adapter-axum -p auctioneer-adapter-cloudflare -p auctioneer-adapter-fastly -p auctioneer-adapter-spin --lib --test boot_envelope --no-fail-fast
cargo test --locked --offline -j2 -p auctioneer-adapter-axum --all-targets --features insecure-bidders --no-fail-fast
cargo test --locked --offline -j2 -p auctioneer-config-data -p auctioneer-runtime --lib --test codegen --test disclosure_codegen --test gvl_archive --test smoke --test url_safety --test activity --test product_defaults_snapshot --test simple_fold --test usersync_config --test usersync_callback_validation --test bidder_bindings --test bidder_storage --no-fail-fast
cargo test --locked --offline -j2 -p auctioneer-config-data --test build_failure_harness
cargo test --locked --offline -j2 -p auctioneer-bidder --test generated_settings -- --nocapture
cargo test --locked --offline -j2 -p auctioneer-bidder --test test_isolation

cargo clippy --locked --offline -j2 --workspace --all-targets --keep-going -- -D warnings
cargo clippy --locked --offline -j2 --workspace --all-targets --features insecure-bidders --keep-going -- -D warnings
cargo clippy --locked --offline -j2 --workspace --all-targets --features conformance-suite --keep-going -- -D warnings
cargo clippy --locked --offline -j2 -p auctioneer-cli --test resource_boot_parity -- -D warnings
CARGO_BUILD_JOBS=2 CARGO_NET_OFFLINE=true ./scripts/check-secure-usersync.sh

cargo fmt --all -- --check
find crates/auctioneer-bidder/src/bidders -type f -name '*.rs' -exec rustfmt --edition 2024 --check {} +
./scripts/check-raw-json-fixtures.sh
./scripts/check-raw-json-fixtures-fixtures.sh
bash -n .github/workflows/scripts/test/verify-config-build-failures.sh
git diff --check
```

Observed compiler tools: `rustc 1.98.1 (48a229cea 2026-09-01)` and
`cargo 1.98.1 (797e8a9bc 2026-08-05)`. Source-bound CLI checks were freshly
compiled after the last source-comment edits; subsequent receipt edits affect
only documentation, which is excluded from that source identity. These are
working-tree receipts on PR #140's base `58385503a9b77f69686bef060b2b101b403bb481`,
not a fabricated final commit or a hosted run.

No global lint policy was relaxed. Three narrow lint expectations document
Serde's generated same-name helpers, the deliberately value-free remainder of
a non-exhaustive SDK error match, and an explicit test-fixture capture panic.
The temporary shared-fixture allowances were removed in favor of direct calls
to the existing test-support owner. Earlier shorthand about "no allowances"
does not mean that these scoped expectations do not exist.

The native-onboarding repair preserves all 14 site identities, 12 applicable
app identities and 300 privacy dispatch cases. Missing required deployment
settings now have an exact configuration-level refusal before any outbound
call or secret read; a valid policy whose secret lookup fails still preserves
its healthy sibling. A valid-policy precondition now prevents those tests from
passing through an unintended earlier failure. No frozen capture was changed.

The two excluded CLI producer cases retain their existing mandatory CI owner;
they were not rerun by the ordinary CLI command and are not credited as local
passes here. Live deployments, new physical-fit proofs and actual host adoption
remain outside this accepted shared cutover. Nothing authorizes merging PR #140.

### Lint Exception Scope Clarification

Earlier "no allowances added" wording means no global lint relaxation, not zero
lint exceptions. Three precisely scoped `#[expect]` exceptions cover Serde
remote-Self helper names in their module, the SDK nonexhaustive error classifier,
and a test-only capture panic. Shared-helper `allow` attributes were removed;
the literal-allow policy is unchanged. This clarifies the existing Clippy
receipts without rewriting their history or claiming any new verification run.

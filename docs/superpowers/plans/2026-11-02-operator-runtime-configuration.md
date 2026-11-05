# Operator Runtime Configuration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish PR #140's actual runtime cutover to one bounded, validated operator policy through existing EdgeZero typed validate/diff/push, with same-binary/two-config acceptance and the same typed host model.

**Architecture:** Keep shared policy/effective models in `auctioneer-config`, neutral compiled data in `auctioneer-config-data`, and generated settings/endpoints in `auctioneer-bidder`. Core reuses shared validators and resolution at push and bounded typed consumption. The file is the direct `OperatorPolicy` payload; any SDK wrapper is a zero-layout trait boundary only. Keep host/adapter facts, bindings, installed limits and the existing additive budget under their current owners.

**Tech Stack:** Rust workspace, Serde, validator, existing TOML/JSON support, quote/syn for retained code generation, EdgeZero, existing resource and test-support APIs. No new workspace crate, config framework, third-party package or cryptographic scheme is planned. The bidder crate adds a workspace-inherited dependency on the already-used `validator` package for its local views.

## Contract And Scope

The [design](../specs/2026-11-02-operator-runtime-configuration-design.md) and
[field inventory](../specs/2026-11-02-operator-runtime-configuration-inventory.md)
were merged in [PR #139](https://github.com/IABTechLab/auctioneer/pull/139),
commit `3b1b6d929bc99b800459fdafb3080655b8c77c29`. Their original draft-status
and review-scope statements describe the design review, not runtime acceptance.
This plan does not grant broader SDK, resource or deployment approval.

**Accepted scope, 2026-11-04:** The maintainer explicitly directed PR #140 to
finish the single runtime configuration, not a startup package. No separate
Auctioneer startup config/file/loader, source-mode config, retained-snapshot
publication subsystem, global host snapshot pool, new numeric startup
reservation or SDK startup hook. This replaces the former Task 4 stop/go gate,
not merely its deadline. Existing safety/accounting and end-to-end acceptance
remain required. The scope amendment alone completes no task; final checkmarks
below reconcile the actual accepted-scope source and verification receipts.

Section 5.1 of the design is a local bidder-validation amendment requested
after that merge, not content approved by PR #139. Task 3A follows its separately
approved contract; it is part of #127 and the same implementation branch.

Tracking: [epic #124](https://github.com/IABTechLab/auctioneer/issues/124).
PR #140 coordinates #126, #127, #128, #130 and #129 for the shared runtime
cutover. It includes shared typed-policy semantic validation/resolution over
explicit caller-owned input/context, not host ingestion or adoption. Actual
host allocation/capacity/lifetime integration remains #135. Native acceptance
uses the existing adapter surface, not a new deployment loader or a claim that
the actual adapter port #131 is complete.
Use `Refs`, not `Closes`, for partially delivered issues. Do not append code
to the merged design PR or create a third design PR for this plan.

Actual Axum #131, Cloudflare #132, Fastly #133, hosted Spin #134, host integration
#135 and remaining deployment documentation #136 retain separate acceptance.
They may reuse the shared contract after the first cutover. Compiling their
updated callers does not count as implementing their provider/host ports.

### Fixed Boundaries

- No backward-compatible old authoring parser or silent default-source fallback.
- No second EdgeZero App/router/store/HTTP hop for embedding.
- No change to privacy policy, bidder publication, workload defaults, ceilings,
  schema reservations, resource floors or the additive budget equation.
- No new cross-request store cache or last-known-good store behavior.
- No unbounded serving read, post-allocation-only bound, or invented SDK hook.
- One direct `OperatorPolicy` document with root `schema_version = 1`, `host`,
  `account_defaults`, `accounts`, `bidders`, `features` and `floor_currency`.
  Reject `baked`/`startup`/`runtime` wrappers and backward wire aliases. Local
  CLI read bounding/accepted-input reuse is separately tracked hardening under
  the 2026-11-03 amendment to design section 9.1. Serving bounds remain required.
- No rewriting historical evidence, reference captures or attribution.
- No raw config values, credentials, consent or store locations in diagnostics.
- No handwritten source imports, new per-feature duplicate compiler owners,
  real bidders as generic test subjects, or workflow-source snapshot tests.

### Cutover And Review Gates

Tasks 1-3A prepare and test foundations without changing which configuration
serving consumes. Do not remove the old default/source path in those commits.
The temporary coexistence is internal development staging, not a supported
dual-format release interface. Keep the branch unmerged until cutover succeeds.

Task 4 maps actual runtime bounds and allocation ownership into existing funded
owners; it does not request a new startup/parser/publication package. Tasks 5-9
cut over the active readers, defaults and CLI while preserving those limits.
Host/adapter deployment facts and store bindings are not new operator knobs;
runtime policy may request supported existing caps within installed capacity.

Tasks 5-9 form one serving cutover group. Their intermediate development
commits must compile; none is separately shippable while it leaves a reader
on invalid defaults, old policy, wider captured caps or unfunded storage.
Task 10 is the acceptance gate for that group. No issue is closed by a plan,
compile-only pass or a filtered command that dispatched zero tests.

## File And Test Ownership

### Current Cutover Status (2026-11-04)

The [final local receipts](../../testing/evidence/operator-policy-validation-20261103.md#accepted-pr-140-completion-receipts-2026-11-04)
reconcile the accepted shared-runtime scope with source and executed owners.
Checked items do not certify the full CI/provider matrix or an unrun historical
permutation. Earlier failures and staging records remain historical. The final
comment-only edits passed post-edit formatting and a freshly compiled CLI
boundary run. Provider and host-integration work remain separately tracked.

| Task | Evidenced status and remaining boundary |
| --- | --- |
| 1 | Reader/field ownership, direct fixtures, preserved captures and affected test owners reviewed; final shared/CLI/adapter dispatch is recorded. |
| 2 | Strict root/override/context implemented; model 403 passed, including root-shape 5 and discovery's actual missing/wrong-hook compiler controls. Historical preparatory listing/red-green chronology is not reconstructed. |
| 3 | Neutral defaults, optional sync and pure resolution implemented; shared semantic/clear oracles passed with nonempty supplied defaults. Config-data's callback repair has its separate passing receipt. |
| 3A | Discovery/callbacks, miswiring and credential-canary controls passed. Full settings owner: 24 outer cases passed, including the 31-child contract and three catalog-allocation probes; 140.12 s. Isolation: 16 passed; conformance Clippy passed. |
| 4 | Existing command/request ordering, all seven captured SDK fields and existing funding/retention owners preserved. No startup/storage or host adoption gate. |
| 5 | Bounded SDK reader/guards and configuration funding complete; runtime library 266 passed and all named core funding/node/peak/factor/boot owners passed. |
| 6 | Shared typed semantics and actual SDK lifecycle/retention/refusal owners passed in core-complete: 97 binaries, 1,187 passed, 0 failed, 1 existing ignored. Actual host ingestion/adoption remains #135. |
| 7 | Direct authoring, existing SDK typed commands and scalar overlays complete. Ordinary CLI: 926 passed, 3 existing ignores, 2 existing once-owned filters; maintenance: 209 passed. Boot/CLI parity: 8 passed, 40 fresh PIDs and four traced mutation refusals. |
| 8 | Product-only generation, root/bootstrap retirement and snapshot-owner rename complete. Build harness: 101 passed; combined data/runtime: exit 0, 468 passed, 0 failed/ignored across 14 targets (202 data + 266 runtime). Model/isolation owners passed. |
| 9 | Four adapter library/boot owners: 87 passed. Axum full insecure all-targets: 81 passed, 1 existing ignored across 11 rows, including resource-profile 14, privacy-wire 1 and wave 14. The separate secure-production owner passed 1 case; its ordinary-run ignore remains reported. Actual ports/hosted qualification remain separate. |
| 10 | Local shared-runtime implementation, accepted-scope review and final verification complete. Full outer settings, post-comment formatting and fresh CLI boundary checks passed. Publication still needs approval. No full CI/readiness claim. |

The earlier Axum/CLI and two-target core failure logs remain failed evidence;
later passing invocations do not rewrite them. The three final prose comments
also changed CLI source identity; the final boundary run verified those bytes
after those comment edits. Actual #131-#136 delivery and #142 local authoring-read/
reuse hardening retain their explicit child owners, not unchecked #140 gates.

### Ownership Map

Paths below are relative to the repository root and incorporate the approved
ownership amendment and revised runtime scope. Historical staged startup/root
types are retirement work, not new serving APIs or acceptance claims.

| Owner | Existing files / planned changes |
| --- | --- |
| Product data and generation | `auctioneer-config-data/data/product-defaults.toml`, `auctioneer-config-data/{build.rs,src/lib.rs}` and `auctioneer-config/src/codegen/mod.rs`. Retain quote/syn, discovery, disclosure and public-reference generation. |
| Effective sync state | `auctioneer-config/src/{types.rs,usersync.rs}` and their existing renderers represent an unconfigured service explicitly. |
| Direct authoring payload | `auctioneer-config/src/{lib.rs,policy.rs}` and `tests/root_shape.rs`; use `OperatorPolicy` directly and retire staged startup/runtime roots at cutover. |
| Operator policy DTO | `auctioneer-config/src/{source.rs,policy.rs,override_value.rs}` owns declarations and pure structural validation. Core aliases compose bidder-owned settings; no duplicate model. Private units remain inline. |
| Resolution and validation | `auctioneer-core/src/config/{resolve.rs,policy_host.rs,policy_account.rs,budgets.rs,validation.rs}` owns non-installing context-based composition and shared semantic admission. No snapshot publication subsystem. |
| Bidder-owned policy validation | `auctioneer-bidder/src/config_validation.rs`, `src/bidders/mod.rs`, `src/registry/catalog.rs` and `auctioneer-config/src/bidder_discovery.rs`. Settings-owning executable bidders use local `config.rs`, ordinary module wiring and inline validator units; no central bidder-name list. |
| Runtime extraction / host facts | `auctioneer-core/src/config/{runtime.rs,mod.rs,sdk.rs}` and `configure.rs` consume the single policy with a zero-layout SDK trait boundary. Preserve `host_primitives/{bootstrap.rs,gates.rs,resource_profile.rs}` ownership/order; retire staged config `startup.rs`, without a new core startup file. |
| Accounting | Modify `auctioneer-runtime/src/{budget.rs,aux_work.rs}` and core's existing config-cost/funding owners. Preserve one derivation and independent product-default storage. |
| Request consumers | Update `auctioneer-core/src/handlers/{auction.rs,cookie_sync.rs,setuid.rs,getuids.rs,sync_http.rs,usersync_keys.rs,probe_config.rs,status.rs,info_bidders.rs}`. Do not create a parallel route family. |
| CLI | Modify `auctioneer-cli/src/application.rs` and existing resource/evidence input owners; retire `config_source.rs`'s root projection in favor of the core SDK trait boundary. Preserve config validate/diff/push/gc and maintenance-only features. |
| Adapter compatibility | Existing adapter main/envelope/resource-profile callers only as required by shared API changes. No new `startup.rs` or deployment parser; actual ports remain #131-#134. |
| Public configuration integration | Extend core `tests/resolve_runtime_config.rs` for real SDK store and caller-owned typed-policy validation/resolution parity, plus `runtime_config_failclosed.rs` for refusals. Use existing setup and independent case expectations; actual host ingestion/adoption is #135. |
| Global/allocator isolation | Keep `config_funding`, `config_peak_charge`, `boot_strict_parity`, `auction_config_retention` and sync allocation targets isolated. Use their existing processes/allocators, not a combined binary. |
| Native artifact acceptance | Use Axum's existing `tests/resource_profile.rs` lifecycle/two-config owner, preserving child-process boot isolation. Keep boot/health controls in `tests/boot_envelope.rs` and `tests/health_boot.rs`; do not add an `operator_configuration` target. |
| Build rejection and neutrality | Extend config-data's existing `tests/build_failure_harness.rs` and its existing mandatory compiler owner. Rename `baked_defaults_snapshot.rs` to `product_defaults_snapshot.rs` at cutover, updating Cargo/CI callers. Do not add a second full child-build suite. |
| Shared semantics and CLI/SDK boundaries | Exhaustive field oracles belong once to the shared model/resolvers over supplied defaults. Actual CLI/SDK owners prove direct payload/object guards and shared-validator delegation, including bidder-settings miswiring; existing lifecycle owners prove A/B and no fallback. No new model, validator, host constructor or storage proof. |
| Synthetic settings and discovery proof | Extend bidder `tests/generated_settings.rs`, `tests/support/generated_settings_contract.rs`, `tests/support/generated_exchange_settings.rs` and existing `tests/build_workspace/` setup. Use its actual synthetic generated settings and normal copied crate APIs; no production fake settings variant or source import. Keep the existing once-owned compiler job. |

All `auctioneer-*` rows above are under `crates/`. Before editing, find
crate-local `AGENTS.md` files. Follow [test ownership](../../testing/development.md):
inline units, owner-local integrations, fake bidders, one mandatory owner for
hermetic heavy compiler experiments, and actual dispatch checks.

## Task 1: Record The Real Baseline And Migration Set

**Issues:** #126/#127/#128/#129/#130/#131 preparation.
**Files:** Existing files in the ownership table; implementation verification
record to add under `docs/testing/evidence/` using the implementation date.

- [x] Record base commit, `.tool-versions`, lockfile and pinned SDK revision.
  Use the SDK checkout resolved from Cargo metadata, not another local checkout.
  Read existing tests and list targets before changing schemas. Run one heavy
  Cargo job at a time; retain caches and incremental output.
- [x] Record the actual pre-edit test names, dispatched counts and results.
  This block records the original owners; the current owners are listed in the
  ownership table and checkpoint evidence, not inferred from these old commands:

```sh
cargo test --locked -p auctioneer-config-schema --test root_shape
cargo test --locked -p auctioneer-config --test baked_defaults_snapshot --test usersync_config
cargo test --locked -p auctioneer-core --lib config::
cargo test --locked -p auctioneer-core --test resolve_runtime_config --test runtime_config_failclosed --test config_funding
cargo test --locked -p auctioneer-cli --bin auctioneer-cli config_source::
```

Expected: actual tests execute; record failures and unavailable prerequisites
before editing, not a guessed baseline count. Do not call a skipped target a pass.

- [x] Enumerate all active readers/callers of `AuctioneerConfigSource`,
  `AuctioneerAppConfig`, `host_default`, `account_default`, `RUNTIME_BOOTSTRAP`,
  root operator-file includes, old overlay keys, default/boot override controls
  and resource report fields. Include examples, scripts, manifests, bidder
  compiler scratch workspaces, schema tests and all four adapter callers.
  Use `rg` across the workspace; the ownership table is not an exhaustive list.
  Historical 2026-11-03 census note:
  The [active-reader census](../../testing/evidence/operator-policy-validation-20261103.md#active-reader-migration-census)
  records the remaining old readers, staged replacements, live synthetic
  fixtures and immutable-capture boundary at `83659d0`. This is an inventory,
  not the per-case fixture migration, startup approval or reader cutover below.
  Its startup-approval reference is superseded by the accepted scope above;
  the census and evidence remain intact.
- [x] Separate live fixtures from immutable captures/evidence. Retain the affected
  owner/feature map and verify its guarantees through the final executed owners.
  A syntactic fixture migration must not weaken privacy/refusal assertions.
  Update valid fixture builders explicitly to the direct versioned policy; do
  not hide a legacy-to-new converter in the shared store fixture. For each
  hostile case, prove its control document passes every earlier layer so a
  missing schema version cannot replace the intended digest, privacy, alias,
  secret or effective-validation failure. Retain original external captures
  and translate only at their documented consumer boundary when required.
- [x] Turn the inventory's C/S/P/V/D census into an implementation checklist with
  concrete DTO path, resolver/reader, validator and resource owner. Derived and
  immutable fields have explicit rejection controls, not new public setters.
  Apply the revised design: former `S` proposals remain host-owned, not startup
  DTO fields; required schema version lives at the direct payload root.
- [x] Record build prerequisites as blockers if absent. Do not refresh archives,
  change SDK revisions or run provider mutations incidentally to this plan.

Checkpoint commit: `Record operator configuration implementation baseline`.

## Task 2: Prepare Strict Policy And Clear-Value Types

**Issues:** #127, prerequisites for #129/#135.
**Files:** Shared config `src/{policy.rs,override_value.rs,lib.rs}` and
`tests/root_shape.rs`; core `src/config/{validation.rs,mod.rs}`.

- [x] Write inline red tests for a required application `schema_version = 1`,
  strict `host`, `account_defaults`, `accounts`, `bidders`, `features` and
  `floor_currency` scopes. Reject missing/wrong application versions, unknown
  nested fields, old flat policy roots and mixed old/new roots. The final
  direct authoring root rejects `baked`, `startup` and `runtime` wrappers,
  retired flat overlay keys, positional roots and backward wire aliases.
- [x] Introduce the new types without changing the deployed extractor's type
  yet. Historical staging added `OperatorPolicy` and dependency-neutral generic
  `OperatorConfigSource<Startup, Runtime>` alongside the old build-only root;
  do not change the old generic root's arity while build.rs still uses it.
  Those wrapper-root tests record the old staging contract, now superseded.
  Task 7 switches CLI to direct `OperatorPolicy`; Task 8 retires both wrapper
  roots and startup DTOs. No released parser accepts both layouts.
  Reuse existing leaf types/validators, with `validator` for supported
  scalar/nested constraints and explicit checks for actual cross-field rules.
  Do not duplicate generated bidder settings or put native generators in core.
- [x] Implement a presence-aware field override with three semantic states:
  inherit, typed value, clear. Its deserializer must reject direct null; an
  `Option<T>` that silently maps null to absence is not sufficient. A clear
  object accepts exactly `clear = true`, no extra keys. Apply it only to the
  design's nullable-field whitelist, including clear of the complete sync
  service. Required/scalar/map fields do not acquire a generic patch mechanism.
- [x] Drive JSON and TOML controls for omission, value, clear, false clear,
  extra key, wrong type and null. For generated bidder settings, independently
  preserve only owner-declared accepted nulls; do not apply the override union.
- [x] Add `ConfigValidationContext` with explicit borrowed product defaults,
  catalog eligibility and host-owned normalized limits. Semantic
  validation must not call global boot accessors. Two different valid contexts
  in one process must produce their respective decisions without installation.
- [x] Verify final model/root-shape and core config owners execute nonzero cases.
  Model 403 and full conformance core 1,187 passed. Preparatory listing/red-green
  chronology remains historical, not an invented final receipt or cutover gate.

Use the proposed module owners for the new focused commands:

```sh
cargo test --locked -p auctioneer-config --no-default-features --features config-model --lib policy::tests::
cargo test --locked -p auctioneer-config --no-default-features --features config-model --lib override_value::tests::
cargo test --locked -p auctioneer-core --lib config::validation::tests::
```

Required controls include the existing `override_value::tests::toml_clear_marker_is_strict`,
the per-field null/clear tests and
`validation_uses_its_context_without_boot_installation`. The still-required
`application_version_is_required_under_valid_envelope` belongs in the existing
store integration target, not a pretend Serde-only unit.

Historical staging added source intent in `auctioneer-config/src/startup.rs`
and a startup/runtime root without changing serving readers. The 2026-11-04
direction supersedes that design: retire those types/callers, retain relevant
strictness/redaction controls at the direct policy or host-owned boundary.
Inherited leaf strictness and serving admission remain actual cutover work;
this note completes no checkbox.

Checkpoint commit: `Add strict operator policy and reset semantics`.

## Task 3: Prepare Neutral Defaults And Pure Resolution

**Issues:** #126/#127; no serving/default accessor cutover yet.
**Files:** Product-default artifact; schema `types.rs`, `usersync.rs`, codegen;
core `config/{resolve.rs,budgets.rs,validation.rs}`.

- [x] Write failing controls for explicit unconfigured user-sync. Product
  resource defaults must equal the reviewed existing numeric defaults, not
  `ResourceLimits::default()` zeros. Product operator aliases, account rosters,
  publisher lists, origins, domains and host vendor identity contain no
  customer settings; no example external URL means a configured service.
- [x] Prepare a product-only artifact and generation path separate from the
  operator file. Keep existing serving accessors until the cutover group.
  Retain useful quote/syn generation rather than writing Rust strings.
- [x] Prepare the effective-model migration so sync service state is optional;
  neutral preferences may still default. Update all affected typed builders,
  generated syntax tests and direct sync consumers together when activating it.
  Configured sync still requires an explicit valid external origin and current
  cookie/redirect/origin checks. Do not replace a configured URL with `""`.
- [x] Test host = product + host overlay; account = product + account_defaults
  + selected account. Removing a named account must not select it through the
  synthesized `unknown` fallback identity. Keep host-provided identity distinct.
- [x] Test each optional clear field with a nonempty/nondefault inherited
  before-state and literal expected effective result. Vary activity `default`
  and `rules` independently: parent false + child `rules = []` remains false.
  Empty arrays replace, omitted fields inherit, account treatment clears
  restore host fallback rather than permission.
- [x] Add account-default response caps to `budgets.rs`. Vary transport and
  decoded caps separately across bidder, alias base, named account,
  account_defaults, host, product/historical fallback and installed min.
  Validate invalid values before successful capacity-clamp/warning behavior.
- [x] Reuse the actual bidder/settings/currency/alias/privacy validators.
  Prove immutable disable, unlinked/guest-ineligible factories, bad settings,
  sample/timeout bounds, expired currency and namespace conflicts still refuse.
  Use synthetic catalog metadata rather than a production bidder as a fake.
- [x] Run schema/config owning units and core config units. Until the active
  cutover, new-resolution tests exercise explicit new inputs without claiming
  the old serving path already has the new semantics.

Checkpoint commit: `Prepare neutral defaults and explicit policy resolution`.

## Task 3A: Register Bidder-Owned Configuration Validators

**Issue:** #127; prerequisite for typed runtime/CLI adoption in Tasks 5-7.
**Files:** Bidder/config/discovery owners and synthetic tests in the ownership
table; bidder `Cargo.toml`, `src/bidders/README.md` and each settings owner's
ordinary `mod.rs`/new `config.rs`. No request schema engine changes.

- [x] Follow the approved design section 5.1 bidder-validation contract.
  Keep one bidder-owned `validator::Validate` mechanism and the generated
  settings ownership/current per-field fallback. The discovered function is
  only a thin adapter to it, not a second validator or selectable validation
  phase. Do not implement an external trait on another crate's generated type, add a
  constraint DSL to TOML, or require config values that validly come from an
  impression. The new local borrowed view owns validation, not another DTO.
- [x] Add red inline units for the single borrowed context and fixed callback
  errors. Extend the capable fake with synthetic validation behavior. Use the
  existing `generated_settings` disposable workspace when actual nonempty
  generated fake settings are needed; those variants must not enter release
  generation. Existing metadata-selected whole-catalog controls stay separate.
- [x] Extend discovery's declaration/registration with the callback reference,
  deriving its requirement from `settings` on executable manifests. Use
  quote/syn and existing feature/target predicates. A settings-free factory
  gets no empty module; a settings owner missing its owner-level
  `validate_config` fails compilation. Generated wiring names that narrow
  function in `mod.rs`, never a private child path or test-only export.
  Extend validated registry plan/definition lookup with the callback using
  existing indexes, without invoking factories or boot.
- [x] Add bidder-local borrowed views using `validator` derive/custom rules
  and existing pure protocol helpers. Inherit the workspace dependency. The
  registered adapter must call that `Validate` implementation rather than
  maintain its own conditions. Keep existing intentional zero/empty/optional
  behavior and do not invent ranges
  or deployment-required fields. An owner with no additional semantic rule
  records that explicitly rather than acquiring fabricated constraints.
- [x] Add one shared orchestration owner in core `config/validation.rs`.
  After structural/binding admission, visit the settings-owning identity union
  once: known enabled definitions/configured aliases and all explicit settings
  entries. Settings-free identities have no callback and reject an incorrectly
  attached settings object; do not generate a second validator or empty module.
  Invoke the same bidder validator with requested/base settings, definition
  and resolved enablement; no public supplied/effective mode or second pass.
  Use implementation-tag dispatch for request-dependent identity entries;
  preserve their existing per-request binding/effective checks. Do not create
  a raw bidder-name switch, second lookup registry or whole-account expansion.
- [x] Prove supplied-value and effective-requiredness rules are each observable
  through that single context, with an exact one-call count per identity:
  disabled-invalid supplied values refuse; enabled missing intrinsic settings
  refuse; valid request-provided fallback remains allowed; an overridden bad
  parent is not hidden. Vary base/alias fields and enablement independently.
  On rejection, assert exact static reasons and no partial snapshot, factory
  invocation, secret read, network access or global installation.
- [x] Test callback removal and miswiring with synthetic owners in the existing
  generated-settings/build-discovery controls. A generated function's presence
  alone is insufficient: swap two owners' callbacks and require a behavioral
  failure. Update child dispatch reconciliation to its enumerated real cases,
  not an approximate count. Do not duplicate the child builds in every lane.
- [x] Add a credential-reference canary to the validator failure path and
  verify the callback result/log/status contains no raw or partial input.
  Bound/fund temporary `ValidationErrors` allocations and any added indexes
  in Task 4 before serving activation; callback purity is not zero-allocation.
- [x] Execute the once-owned generated-settings contract parent and its 31
  reconciled children, model discovery/compiler controls and core validation
  owners. Preserve old guarantees/features and catalog-allocation probes.
  Isolation 16 and conformance test-build Clippy passed. Do not claim the full
  24-case outer generated-settings suite was rerun by the selected parent.

Checkpoint commit: `Register bidder-owned configuration validation`.

The 2026-11-03 staged checkpoint adds the pure bidder subvalidator and its
synthetic behavioral controls; see the
[foundation evidence](../../testing/evidence/operator-configuration-foundations-20261103.md#shared-bidder-validation-checkpoint).
This checkbox does not complete the full `ConfigValidationContext`, product
resolution, runtime allocation admission or serving adoption. The existing CLI/store/host
readers remain active until the later cutover group.

## Task 4: Map Runtime Bounds To Existing Owners

**Issues:** #128/#130; actual runtime cutover preparation, not numeric startup approval.
**Files:** Core `config/{validation.rs,runtime.rs}`, `configure.rs`, existing
resource/profile/gate and config-cost owners; runtime `budget.rs`/`aux_work.rs`.

- [x] Inventory the exact pinned SDK and Axum operations for startup reads,
  environment/health materialization, parser structure, store selectors,
  logging and App construction. The 2026-11-03 source audit also traced CLI
  raw/typed rereads and Trusted Server's actual typed command wiring; see the
  validation evidence follow-up. It established existing single-config support,
  not bounded local rereads or serving qualification. The old startup-package
  conclusion is superseded; local read/reuse hardening remains #142.
- [x] Preserve existing SDK command ordering and flags for validate/diff/push,
  including target selection, confirmation and remote reread behavior. Shared
  validation means semantic parity, not identical CLI/request execution order.
  Requests retain ingress/admission, pre-read funding/captured-cap checks,
  bounded extraction and semantic validation before effective use, with existing
  input/privacy/secret-read ordering and error precedence. Pure validation must
  not install state or move those side effects earlier.
- [x] Map the complete captured SDK tuple: `max_backend_bytes`, `max_blob_bytes`,
  `max_json_depth`, `max_json_nodes`, `max_secret_bytes`, `max_total_bytes` and
  `timeout`. Keep their existing host enforcement/accounting owners and reject
  policy attempts to set them. Preserve independent byte/value units and
  internal normalization sentinels without exposing public unlimited.
- [x] Map policy parsing, merged views, namespace indexes, bidder callbacks
  (including allocating `ValidationErrors` failure paths) and retained effective
  data to existing funded config/work owners. Admission precedes construction;
  charges follow actual ownership through final drop/cancellation. Keep
  independent product-default storage and all conservative existing terms.
- [x] Keep the reusable typed-policy semantic validator/resolvers on explicit
  caller-owned `OperatorPolicy` and context. They do not ingest/adopt host
  allocations or establish capacity/lifetime readiness. Actual host ingestion,
  copying/borrowing/adoption and its allocation proof belong to #135; no new
  normalization constructor, host pool or storage package gates PR #140.
- [x] Preserve real SDK extraction and overlapping per-request policy/effective
  views under existing admission/retention owners through final drop/cancellation.
  Do not assume two generations or retain the full policy tree through fanout.
  Unknown provider ingestion facts remain child-task acceptance work.

Checkpoint: runtime ownership mapped without changing budgets, limits or workload.

## Task 5: Cut The Runtime Reader Over To OperatorPolicy

**Issues:** #127/#128/#130. Starts the coordinated serving cutover group.
**Files:** Core `config/{runtime.rs,mod.rs,validation.rs}`, `configure.rs`,
existing config-cost owners and boot/config allocation/parity tests.

- [x] Add red controls for actual bounded SDK extraction of the direct policy,
  shared semantic rejection and missing/wrong app version inside a valid SDK
  envelope with a correct recomputed digest. Reject old wrappers and flat keys;
  a checksum is not schema compatibility or writer authentication.
- [x] Switch the active SDK typed boundary to `OperatorPolicy` and invoke the
  same explicit-context validators used at push before any effective use.
  Any SDK wrapper delegates traits without changing wire layout or model.
  Preserve current request admission and host boot order, not a startup DTO.
- [x] Check Apps against every field of the accepted SDK tuple before reads.
  Independently exceed each captured field and assert zero store/secret reads;
  retain preboot-App refusal, fixed `/status` diagnostics and distinct immutable
  registry failures. Policy cannot alter bindings, installed limits or gates.
- [x] Preserve product-default clone/storage allowance independently of blob
  caps and name it correctly in active reports. Use the existing shared checked
  additive derivation and actual multiplicities; no hidden baseline substitution,
  `max(startup, steady)`, new numeric startup reserve or lowered conservative term.
- [x] Exercise independent literal/layout/differential allocation expectations:
  asymmetric auction/aux/probe populations, envelope/blob/value limits,
  product-default size, semantic error peaks and retained data through drop.
  Preserve factors, floors, configured/platform ceilings and boot warnings.
- [x] Run runtime library and core `config_funding`, `config_peak_charge`,
  `config_node_cap`, `config_parse_factor`, `config_envelope_value_factor`,
  `boot_resource_charges`, `boot_strict_parity`, `boot_order_gates` and
  `bootstrap_order` owners in their existing isolated processes.

Checkpoint: serving consumes bounded, semantically validated direct runtime policy.

## Task 6: Reuse Typed Policy Validation And Finish Request Consumption

**Issues:** #127/#128/#130; typed-policy semantics only, not #135 host adoption.
**Files:** Core config/resolve/validation/runtime, handlers, config-store
test-support and existing public integration/lifecycle/retention tests.

- [x] Reuse the same `OperatorPolicy` semantic validator and resolution helpers
  with explicit caller-owned input/context, without another model, store/App/HTTP
  hop or new constructor. Test representable semantic failures and independent
  resolution directly, without Serde or installed globals. This proves policy
  semantics, not host allocation ingestion/adoption or serving readiness; #135
  owns that implementation and its actual capacity/lifetime proof.
- [x] Capture one generation per request and migrate auction/sync/info/status
  readers together. Preserve permanent/transient/unknown categories,
  503/Retry-After behavior and liveness-only degradation. No privacy grant on
  failed/missing policy, fallback source, last-known-good cache or mid-auction read.
- [x] Preserve `auction_config_retention`'s early release of unrelated accounts
  before fanout. Derive required effective views coherently, release unused
  root storage and fund the allocations actually retained, not the whole tree
  by default. Do not add a global retained-snapshot publication subsystem.
- [x] Test store A -> B removal of accounts/aliases/overrides, overlapping
  requests on distinct generations, existing admission refusal, cancellation
  and final release in different orders. Invalid typed policies fail shared
  validation; actual host update/adoption behavior remains #135. Failed required
  store loads still refuse rather than reuse previous state.
- [x] Refuse unconfigured sync before signing-key/callback work: zero secret
  reads/callbacks and no opt-out cookie rewrite; auctions/info remain independent.
  Configured sync retains key rotation, callback, CORS and privacy checks.
- [x] Run `runtime_config_failclosed`, `resolve_runtime_config`,
  `auction_config_lifecycle`, `auction_config_retention`,
  `usersync_config_lifecycle`, `usersync_integrity`, `usersync_routes`,
  `usersync_handler_alloc`, `privacy_config_parity` and `response_account_resolution`.

Checkpoint: shared typed-policy semantics and coherent SDK request consumption;
host storage/ingestion/adoption remains explicitly unqualified under #135.

## Task 7: Move Authoring And CLI To The Shared Contract

**Issues:** #129, mandatory #127 acceptance.
**Files:** CLI application and retired config_source projection; shared policy;
core `config/sdk.rs`, test-support and
existing CLI/core contract targets. The checked-in `auctioneer.toml` changes
only in Task 8, atomically with removal of its build reader.

- [x] Make the operator file the direct staged `OperatorPolicy`: root
  `schema_version = 1`, `host`, `account_defaults`, `accounts`, `bidders`,
  `features` and `floor_currency`. Deserialize/serialize this exact payload;
  any SDK wrapper is a zero-layout trait boundary only, not another root or
  runtime projection. Reject `baked`/`startup`/`runtime` wrappers and backward wire aliases.
  Physical bindings, installed limits and secret values are not policy fields.
  Preserve actual SDK envelope integrity behavior; do not claim signing.
  At this preparation checkpoint, use explicit new-format temporary authoring
  fixtures for CLI tests; do not migrate or parse the still build-owned root
  file with the new type. Move shipped-file payload-identity controls to Task 8's
  checkpoint. The branch remains unshippable until that checkpoint passes.
- [x] Wire existing typed SDK validate/diff/push through the shared application
  contract and explicit validation context, not ambient installed boot globals.
  Use the approved existing SDK flow; its raw/typed local rereads remain a
  separately tracked hardening item, not a second Auctioneer loader or a cache.
  Changed-file/environment-after-acceptance controls belong to that follow-up.
  These commands validate the same policy as bounded consumption, not startup.
  Preserve secret metadata paths without adding a `runtime.` prefix.
  Audit all secret metadata when policy fields move beneath `host`.
- [x] Update exact env overlay paths/callers to `AUCTIONEER__...__...`.
  Keep the SDK's existing scalar-leaf-only behavior and `--no-env` scope;
  host/SDK store selector env behavior is not silently disabled or repeated
  per request. Direct leaf paths have no `RUNTIME` wrapper component (for example,
  `AUCTIONEER__HOST__RESOURCE_LIMITS__DEFAULT_TMAX_MS`). Use child processes for
  environment controls, not unsafe global mutations. Reject old names without aliases.
- [x] Complete the shared-semantics and actual-boundary gate below before allowing publication.
  Preserve config gc/deploy and archive-independent maintenance CLI behavior;
  no migration/export subcommand or new feature split is needed.
- [x] Run and reconcile the migrated CLI application/core `config::sdk::` owners,
  CLI `--test help`, core store/typed-policy parity, plus CLI default-feature and
  `--no-default-features` checks; assert nonzero dispatch, not a retired filter.
  Test the actual
  typed SDK push/validate entry points using the existing local test backend;
  do not publish to a production store as verification.

### Required Shared Semantics And Actual Boundary Gate

**Latest directed scope alignment, 2026-11-04:** Own exhaustive nullable/clear
and semantic field oracles once in the shared model/resolvers with explicit
caller-owned input/context and nonempty supplied defaults. The canonical direct
policy and zero-layout SDK trait wrapper use the same underlying model, removing
subtree-projection drift, not the need to verify actual boundary delegation.
Neutral compiled host defaults leave some clears unobservable there; do not
rebuild synthetic CLI artifacts or repeat a field-by-path Cartesian matrix to
manufacture inherited host values. Do not add another validator.

Actual CLI/SDK owners verify the direct payload, object guards and shared-validator
delegation; actual lifecycle owners verify A/B and no fallback. Keep independent
literal expectations, every existing test/negative control and all limits. Fixture
setup may be shared, but not the expected-output resolver with production code.
Assert dispatch counts. This explicitly revises acceptance ownership, not pending
command results, safety/storage requirements or the separate #135 integration.

| Family | Required exercise |
| --- | --- |
| Every nullable/clear field (shared owner) | Exhaustive omission, concrete value, strict clear and malformed-union oracles in shared model/resolver tests; supply nonempty inherited defaults and assert literal effective results. No per-field CLI/SDK replay requirement. |
| Typed-policy invalid values (shared owner) | Pass representable semantic errors directly to the existing shared validator/resolvers with caller-owned input/context; cover scalar/cross-field/catalog/cap/refusal behavior once, without Serde or a new constructor. |
| Actual CLI / SDK boundaries | Independently verify the direct schema-version-1 payload/object guards, serialization/overlay behavior and shared-validator delegation through existing typed validate/diff/push and bounded store consumption. Retain malformed-wire controls for null, false/extra-key clear, clear on a nonnullable field, unknown scope/field, wrong type and app version with fixed refusal categories. |
| Bidder-owned validation | Keep the existing synthetic generated-settings controls for supplied/effective failures, disabled entries, alias/base precedence and valid request fallbacks with their current owners. Actual CLI and SDK store extraction must detect missing/miswired settings-validation callbacks; keep discovery/behavior controls without factories, secrets or boot globals. No duplicate bidder-field Cartesian matrix. |
| Unrepresentable wire shapes | Retain independent CLI/SDK decoding refusals; record typed-model exclusion rather than claiming a typed validator processed an unrepresentable shape. Exhaustive field-shape oracles remain shared-model tests. |
| SDK storage and retention | Real extraction bounds, per-request admission, early release, cancellation and final-drop accounting remain mandatory. Actual host ingestion/adoption and capacity/lifetime proofs remain #135, not a fabricated storage pass here. |
| Independent resolution (shared owner) | Vary supplied defaults, named account, activity default/rules, both response caps and per-field clears separately in shared resolver tests; assert before-state presence and no surviving parent after a clear. |
| Actual lifecycle | Existing lifecycle owners exercise policy A/B replacement, coherent per-request generations and no fallback after missing/corrupt/invalid required policy. Keep same-binary/two-config and retention controls; do not substitute two pure-helper calls or claim #135 host storage proof. |

The accepted #127/#129 gate retains shared clear/malformed controls, actual
CLI/SDK invalid-policy refusals, swapped-callback behavioral controls and
independent explicit-context decisions without boot installation. The actual
boot/CLI owner also passed four traced scratch-only resource mutations. An
additional disposable clear/bypass/global mutation campaign is not reported
executed or required as another field-by-path matrix. No existing negative
control is removed, and no unrun command is marked complete.

Checkpoint commit: `Cut config authoring and CLI over to operator runtime policy`.

## Task 8: Remove Operator Inputs From Release Builds

**Issues:** #126; coordinated with Tasks 5-9, not an independent early cutover.
**Files:** Config build/lib/data; schema codegen; old bootstrap data; config
snapshots/build-failure tests; Cargo and existing CI callers; active docs/examples.

- [x] Switch serving product-default accessors to the reviewed neutral artifact
  after optional sync, resolution and bounded policy readers/funding are ready.
  Delete release-build access to root `auctioneer.toml` and the compiled
  `data/runtime-bootstrap.toml`; remove obsolete operator-default/bootstrap
  harness overrides and migrate their remaining rejection guarantees.
  In this same compiling checkpoint, migrate the checked-in authoring file to
  direct `OperatorPolicy` and retarget shipped-file CLI/schema/example tests.
  Retire both old and staged startup/runtime root types, source-mode DTOs and
  their exports/callers after the final build reader moves. Preserve their
  applicable strictness/refusal guarantees at the new boundary. Never commit
  a new root document that the still-active old build reader cannot parse.
- [x] Keep product artifact validation, bidder aliases/metadata/settings,
  discovery, disclosure, GVL/GCL and test-harness feature-export checks.
  Retarget operator alias/source failures to runtime validation; do not delete
  unrelated build rejection cases. No operator-supplied file path override
  remains as a way to customize a release.
- [x] Rename the defaults snapshot target and its Cargo/CI/script references
  together. Freeze neutral defaults independently, preserving existing resource
  numbers/privacy defaults and documenting only intentional sync/domain/list
  changes. No snapshot regeneration from the function under test as the oracle.
- [x] In the existing hermetic owner, prove identical release product output
  with the root operator file missing and changed to malformed bytes; obsolete
  overrides are ignored without the explicit harness feature. Retain independent
  product/code/catalog rejection and positive controls in the 101-case owner.
  SDK payload/trait guards remain in their actual model/CLI owners. Additional
  provider packaging permutations remain child qualification, not claimed runs.
- [x] Preserve fresh refusal workspaces and one mandatory compiler owner.
  Update exact case/dispatch lists in its script if intentionally changed;
  do not reintroduce YAML snapshot guards or triple-run the same child plans.
- [x] Run config-data `product_defaults_snapshot`, `codegen`, `usersync_config`,
  `activity`, `bidder_bindings` and the explicit `build_failure_harness` owner;
  run shared config `root_shape` and bidder `test_isolation` after wiring edits.

Checkpoint commit: `Make release generation independent of operator files`.

## Task 9: Verify Existing Adapter Compatibility And Two-Config Behavior

**Issues:** Shared #128/#130 acceptance; actual #131-#135 ports remain separate.
**Files:** Existing adapter callers only where shared API changes require them;
Axum's existing `tests/{resource_profile.rs,boot_envelope.rs,health_boot.rs}`.

- [x] Adjust shared-API callers without moving host/adapter facts, store
  bindings, health/logging or installed limits into policy. Preserve existing
  boot order, identical/conflicting installation, Axum-fatal health resolution,
  guest health-only Closed degradation and fatal health install conflicts.
  Do not add startup files/parsers or claim provider-port completion.
- [x] Preserve native `--resource-profile` and CLI parity on the same normalized
  host inputs, independent product defaults and complete SDK tuple. Keep
  strict-vs-boot policy, existing over-ceiling warnings and native schema admission.
- [x] Build once without operator input, then run the unchanged executable with
  isolated direct policies A and B through the existing SDK store/binding surface.
  Assert independent banner/node-cap results and equal executable hashes; never
  rebuild for either policy. Account/feature/removal, unconfigured sync and
  missing/corrupt/wrong-schema refusals retain their shared/core lifecycle owners,
  not a duplicate native field matrix. Preserve bounds and shutdown cleanup.
  There is no defaults source mode or actual #135 host storage claim.
- [x] Run the full Axum insecure all-targets owner and verify `privacy_wire`
  dispatch separately from secure-production acceptance. Keep health/boot
  isolation and the native four-adapter library/boot controls. Record selected
  Fastly guest execution as guest-only; remaining provider/target qualification
  retains its child owner. Core lifecycle/strict parity and the separate
  secure-production user-sync script passed; the ordinary ignore stays reported.

Checkpoint: shared runtime cutover has same-binary/two-config native evidence.

## Task 10: Verify And Review PR #140 Runtime Cutover

**Issues:** Acceptance evidence for completed child scopes, not epic closure.
**Files:** Operator/cutover docs, development commands, dated new evidence,
existing CI ownership only where new/renamed targets
require it. Local CLI input hardening remains the separate follow-on
[#142](https://github.com/IABTechLab/auctioneer/issues/142).

- [x] Publish a complete old-field -> new-field mapping from the inventory.
  Show the direct payload, required version, clear semantics, unconfigured sync,
  secret references, no-fallback refusals and scalar-overlay limits. Mark physical
  facts, bindings and installed limits host-owned, not startup/runtime config.
  Retire wrapper examples and legacy wire names without aliases.
- [x] Reconcile the accepted shared-runtime owners: model/data/runtime, full
  conformance core, ordinary/maintenance CLI, explicit boot parity, four native
  adapter library/boot owners, full insecure Axum, secure-production user-sync,
  once-owned compiler/settings contracts and isolation. Record their feature
  scopes and actual dispatch; this is not the entire CI/provider test matrix.
- [x] Execute the actual boot/CLI parity owner, not just two calls to one pure
  helper. Extend `auctioneer-cli --test resource_boot_parity` and core
  `boot_strict_parity` to the complete captured tuple/product-default identity;
  vary one independent input at a time and retain fresh process installation.
- [x] Run strict default/insecure/conformance workspace all-target Clippy and
  explicit boot-parity Clippy; isolation passed 16 cases. Retain existing fixture
  guards and pinned evidence. Guest execution remains guest-only, not hosted
  acceptance. Final post-comment formatting is recorded separately below.
- [x] Record confirmed command/feature scopes, repository toolchain/SDK references,
  counts, durations and pass/fail/not-run outcomes. Full shell argv/environment
  absent from saved output is explicitly unrecorded, not reconstructed. Preserve
  actual policy/default/retention charges, limits, workload and reservations.
- [x] Review once-owned shared semantic oracles, actual CLI/SDK boundary delegation,
  lifecycle A/B/no-fallback behavior, privacy/refusals, all issue field dispositions,
  exact normalized limits, simultaneous allocation owners, release neutrality
  and one-artifact/two-config native evidence. Compare migration
  test names/counts; a disappearing assertion needs an identified retained owner.
- [x] Coordinator: after the three present-tense comment edits, run final
  `cargo fmt --all -- --check` and the fresh source-frozen CLI boundary check.
  Append their exact receipts; earlier CLI fingerprints do not certify these
  final source bytes. This is final verification, not remaining runtime code.
- [x] Review remaining work against the existing PR #140 and child-task scopes;
  distinguish shared cutover from unresolved actual provider/host integration.
  No new spec/PR, GitHub/status update, commit, push or public operation is
  authorized by this documentation revision.

Expected outcome: reviewable single-runtime cutover with concrete evidence.
Not an assertion that the epic, all providers, Trusted Server integration or
guest resource qualification are complete.

## Follow-On Acceptance Owners

These are explicit dependencies and acceptance requirements, not implementation
work silently included in PR #140 acceptance. Their scoped plans use the same
policy/validators and actual host/provider APIs, not a shared startup package.

| Issue | Required next work / blocking fact |
| --- | --- |
| #131 Axum | Actual native operator/deployment integration under existing SDK host surfaces and bounded ingestion; shared caller compatibility/two-config proof alone does not close the port. |
| #132 Cloudflare | Bounded binding/env/health materialization; captured App tuple; actual Worker artifact run with two configs. Provider bound/SDK gaps block adoption, not a local native green. |
| #133 Fastly | Adapter-owned stores/secret translation; key-to-store alias constraint; bounded preboot materialization; unchanged component with two configs. Keep local manifest secret sources credential-free. |
| #134 Spin | Hosted Akamai Functions surface, bounded variables/health/secret ownership, complete tuple; actual component acceptance. Generic Spin tests alone do not prove hosted readiness. |
| #135 embedding | Implement actual host ingestion/adoption, allocation/capacity/lifetime proof, SDK compatibility with #34 and host-owned App/services using the shared typed policy/validators. Prove controlled standalone/embedded results; PR #140's typed semantics do not establish serving/storage readiness. No global host pool or new Auctioneer publisher. Blocks artifact #76, not bridge review. |
| #130/#9 resources | Remaining target facts, physical fit and deployment evidence remain explicit qualification blockers. Native source neutrality does not close them. |
| #136 documentation | Finish provider/embedding release instructions only against implemented surfaces and evidence. No required wait for all four adapters before accepting one selected host path. |

## Historical Plan Verification

These dated reviews record earlier plan revisions, not fresh checks or current
gates. The 2026-11-04 accepted scope supersedes their startup-package, wrapper
and source-mode assumptions. Existing checkmarks below/above are retained
checkpoint history, not completion asserted by this revision. Documentation
checks are not Cargo, allocation, serving or deployment verification.

The initial plan's documentation review on 2026-11-02 found and corrected the shared-file cutover
ordering: Task 8 now changes the build reader and checked-in authoring root
together. The independent rereview found no remaining material plan blockers.
Local checks resolved all three document links, found all 32 named existing
test targets, and found no trailing whitespace or legacy delivery labels.
No Cargo tests, allocation measurements or serving/deployment checks were run
for that documentation-only preparation. At that review, Task 4 had a separate
numeric startup approval gate; it is now superseded, not outstanding. The
subsequent bidder-validation amendment/Task 3A had its own review below.

The bidder-validation amendment and Task 3A subsequently passed a separate
read-only review, including the inventory row and the single-contract/no-settings
disposition. The three documents have 49 resolving local links and pass
whitespace checks. At that review no production code or runtime tests had
changed; the source/document review is not runtime qualification. The maintainer subsequently approved the
written amendment and implementation on 2026-11-02. The former numeric startup
reservation gate is removed by the 2026-11-04 direction.

## Recorded Execution Checkpoints

The following preserves the 2026-11-02/03 local implementation/evidence record.
Staged startup/root work is superseded and must be retired, not promoted to an
active requirement. No tests or new completion checkmarks accompany this scope
revision; the remaining tasks above define actual PR #140 acceptance.

The maintainer approved implementation on 2026-11-02. The
[pre-edit baseline](../../testing/evidence/operator-configuration-baseline-20261102.md)
records the actual selected test owners and results, including the corrected
CLI binary owner. This is not acceptance of all tasks above.
The [foundation checkpoint](../../testing/evidence/operator-configuration-foundations-20261103.md)
records subsequent test execution, the pinned SDK audit and registry growth.

| Slice | Current status |
| --- | --- |
| Pre-edit selected baselines | Executed; 176 parent test cases passed, with the existing child-only CLI probes driven by their parent cases. |
| Strict operator authoring root (superseded staging) | Implemented alongside the unchanged old reader; 12 root integration cases and 3 schema library cases pass. Spec and code-quality review approved; strict schema test lint passed. Retire its startup/runtime layout in favor of the direct policy. |
| Explicit field override | Implemented test-first; 29 inline cases pass, including duplicate/escaped marker keys, strict clear shape, direct null refusal and rejection of programmatic null-producing roots. Temporary parser allocations are not resource-qualified. |
| Versioned policy model | Staged DTO implemented; 16 inline cases pass, including JSON/TOML opt-out leaf inheritance and null/omission/value controls for all optional fields declared locally. Reused legacy nested DTO strictness remains part of cutover. Combined core configuration selection: 195 passed; full core library: 670 passed. Spec/quality reviews and strict test lint passed. Full semantic admission is not implemented. |
| Bidder-owned validation | Discovery, local callbacks and the staged pure shared dispatcher are implemented; bidder library 1,188 passed (9 existing source-dependent ignores), registry 21 and ownership 16 passed. Strict bidder lint passed. The once-owned settings contract passed 19 outer / 25 reconciled child cases, including absent-feature settings refusal; discovery passed 38. Exact ABI controls and matched native construction observations completed; staging spec and quality reviews approved. Allocation target: 8 passed, including independent regression ceilings and populated callback slots. Earlier guest test compilation and strict guest library/allocation lint passed; guest checks were not rerun for these review repairs. No serving callback activation or resource admission. |
| SDK and CLI audit | Exact locked source and Trusted Server's real typed command wiring inspected. The existing typed SDK commands already supply single-config support. Local authoring read bounds and accepted-input reuse remain separately tracked hardening. Serving bounds still apply; the numeric startup review is superseded. |
| Typed startup source intent (superseded staging) | Staged source-selection DTO, validation/redaction helpers and wrapper-root contracts were implemented without changing CLI/serving readers. They supply no resource/health admission and are retirement work under the revised scope. |
| Neutral product data / optional sync | Prepared without switching serving accessors. Existing exact defaults snapshots pass, and unconfigured services refuse before signing-key/registry work. Source/default separation remains unfinished until the reader cutover. |
| Pure account and scoped-cap composition | Implemented without admission/boot installation: 16 account cases and 14 total budget cases remain in the passing core library. Explicit-context host composition and staged policy validation are now implemented; their reservation and serving adoption remain separate. |
| Shared configuration ownership | Implemented under the approved amendment: shared models in config, compiled assets in config-data, settings/endpoints in bidder, and composition/SDK integration in core. All 698 baseline core case identities are preserved; native models, actual compiler consumers and WASI model execution pass. See the [ownership record](../../testing/evidence/configuration-model-ownership-20261103.md) for exact local checks and CI-only acceptance. |
| Context-based policy foundation | Host composition uses supplied defaults/capacity; the staged validator uses supplied defaults, catalog, capacity and time. Typed children use nested validation; maps and explicit clears retain their specialized rules. Success is not snapshot/input/storage admission. See the [validation checkpoint](../../testing/evidence/operator-policy-validation-20261103.md) for actual execution and remaining cutover gates. |
| Serving/default/CLI cutover | Not started at this checkpoint; actual runtime reader, default and CLI migration remains. No startup reservation gate remains. |

The maintainer subsequently requested one physical owner for shared configuration
models, with bidder-specific settings remaining bidder-owned. The
[ownership amendment](../specs/2026-11-03-configuration-model-ownership-design.md)
and [migration plan](2026-11-03-configuration-model-ownership.md) specify the
cycle-free package/declaration move before extending the runtime cutover.
The maintainer explicitly approved the config/config-data package-role rename
on 2026-11-03. This supersedes the earlier file-owner mapping; it does not waive
this plan's resource and end-to-end acceptance gates.

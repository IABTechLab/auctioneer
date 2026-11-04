# Operator Runtime Configuration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Release one operator-neutral native executable that accepts different bounded, validated operator configurations without rebuilding, while establishing the shared contract for the remaining adapters and embedding host.

**Architecture:** Keep shared source/policy/effective models in `auctioneer-config`, compiled product data in `auctioneer-config-data`, and generated settings/endpoints in `auctioneer-bidder`. Core composes those models and owns semantic resolution, startup coordination, SDK integration and snapshot admission. Use the existing EdgeZero store/envelope, host services and additive resource model. Prepare foundations without changing serving, then cut defaults, policy loading, bootstrap, CLI and the native adapter over together.

**Tech Stack:** Rust workspace, Serde, validator, existing TOML/JSON support, quote/syn for retained code generation, EdgeZero, existing resource and test-support APIs. No new workspace crate, config framework, third-party package or cryptographic scheme is planned. The bidder crate adds a workspace-inherited dependency on the already-used `validator` package for its local views.

## Contract And Scope

The [design](../specs/2026-11-02-operator-runtime-configuration-design.md) and
[field inventory](../specs/2026-11-02-operator-runtime-configuration-inventory.md)
were merged in [PR #139](https://github.com/IABTechLab/auctioneer/pull/139),
commit `3b1b6d929bc99b800459fdafb3080655b8c77c29`. Their original draft-status
and review-scope statements describe the design review, not runtime acceptance.
This plan does not grant broader SDK, resource or deployment approval.

Section 5.1 of the design is a local bidder-validation amendment requested
after that merge, not content approved by PR #139. Task 3A implements it after
written-contract review; it is part of #127 and the same implementation branch.

Tracking: [epic #124](https://github.com/IABTechLab/auctioneer/issues/124).
The first implementation PR coordinates #126, #127, #128, #130 and #129 with
the Axum port #131. It also implements the shared typed constructor needed
for validation parity; that alone does not close host integration #135.
Use `Refs`, not `Closes`, for partially delivered issues. Do not append code
to the merged design PR or create a third design PR for this plan.

Cloudflare #132, Fastly #133, hosted Spin #134, actual host integration #135,
and the remaining deployment documentation #136 retain separate acceptance.
They may reuse the shared contract after the first cutover. Compiling their
updated callers does not count as implementing their operator startup ports.

### Fixed Boundaries

- No backward-compatible old authoring parser or silent default-source fallback.
- No second EdgeZero App/router/store/HTTP hop for embedding.
- No change to privacy policy, bidder publication, workload defaults, ceilings,
  schema reservations, resource floors or the additive budget equation.
- No new cross-request store cache or last-known-good store behavior.
- No unbounded startup read, post-allocation-only limit, or invented SDK hook.
- One operator configuration document and one SDK load/validate pipeline;
  startup/runtime are internal scopes, not separate files or loaders. Follow
  design section 9.1 for the bounded, reload-free handoff.
- No rewriting historical evidence, reference captures or attribution.
- No raw config values, credentials, consent or store locations in diagnostics.
- No handwritten source imports, new per-feature duplicate compiler owners,
  real bidders as generic test subjects, or workflow-source snapshot tests.

### Cutover And Review Gates

Tasks 1-3A prepare and test foundations without changing which configuration
serving consumes. Do not remove the old default/source path in those commits.
The temporary coexistence is internal development staging, not a supported
dual-format release interface. Keep the branch unmerged until cutover succeeds.

Task 4 is a required review checkpoint for the exact bounded startup-input,
parser/storage and snapshot-retention package. The design did not approve
numeric startup reservations. Capture proposed values and independent proofs
in #128/#130, obtain maintainer approval, then implement Tasks 5-9. A rejected
or missing package blocks raw-input adoption; it is not permission to tune
workload or fabricate a reserve. Pure DTO/resolver tests can proceed meanwhile.

Tasks 5-9 form one serving cutover group. Their intermediate development
commits must compile; none is separately shippable while it leaves a reader
on invalid defaults, old policy, wider captured caps or unfunded storage.
Task 10 is the acceptance gate for that group. No issue is closed by a plan,
compile-only pass or a filtered command that dispatched zero tests.

## File And Test Ownership

Paths below are relative to the repository root and incorporate the approved
ownership amendment. Unimplemented startup/snapshot paths remain proposed;
their presence here is not a claim of serving acceptance.

| Owner | Existing files / planned changes |
| --- | --- |
| Product data and generation | `auctioneer-config-data/data/product-defaults.toml`, `auctioneer-config-data/{build.rs,src/lib.rs}` and `auctioneer-config/src/codegen/mod.rs`. Retain quote/syn, discovery, disclosure and public-reference generation. |
| Effective sync state | `auctioneer-config/src/{types.rs,usersync.rs}` and their existing renderers represent an unconfigured service explicitly. |
| Strict authoring root | `auctioneer-config/src/lib.rs` and `tests/root_shape.rs`; the final generic root carries typed startup/runtime, not opaque operator subtrees. |
| Operator policy DTO | `auctioneer-config/src/{source.rs,policy.rs,override_value.rs}` owns declarations and pure structural validation. Core aliases compose bidder-owned settings; no duplicate model. Private units remain inline. |
| Resolution and validation | `auctioneer-core/src/config/{resolve.rs,policy_host.rs,policy_account.rs,budgets.rs,validation.rs}` owns non-installing context-based composition and semantic checks. `snapshot.rs` remains proposed for admitted ownership. |
| Bidder-owned policy validation | `auctioneer-bidder/src/config_validation.rs`, `src/bidders/mod.rs`, `src/registry/catalog.rs` and `auctioneer-config/src/bidder_discovery.rs`. Settings-owning executable bidders use local `config.rs`, ordinary module wiring and inline validator units; no central bidder-name list. |
| Startup | Shared input declarations belong in `auctioneer-config`; startup coordination belongs in proposed `auctioneer-core/src/config/startup.rs` with `host_primitives/{bootstrap.rs,gates.rs,resource_profile.rs}`, `configure.rs` and `config/runtime.rs`. Keep provider reads out of core. |
| Accounting | Modify `auctioneer-runtime/src/{budget.rs,aux_work.rs}` and core's existing config-cost/funding owners. Preserve one derivation and independent product-default storage. |
| Request consumers | Update `auctioneer-core/src/handlers/{auction.rs,cookie_sync.rs,setuid.rs,getuids.rs,sync_http.rs,usersync_keys.rs,probe_config.rs,status.rs,info_bidders.rs}`. Do not create a parallel route family. |
| CLI | Modify `auctioneer-cli/src/{config_source.rs,application.rs}` and existing resource/evidence input owners; preserve config validate/diff/push/gc and maintenance-only features. |
| Native startup | Modify `auctioneer-adapter-axum/src/{main.rs,envelope.rs,resource_profile.rs}` and add `startup.rs` for bounded deployment translation. Adjust `src/lib.rs` exports only when an integration needs a public contract. |
| Public configuration integration | Extend core `tests/resolve_runtime_config.rs` for SDK store/host parity and `runtime_config_failclosed.rs` for refusals. Add ordinary submodules beneath `tests/resolve_runtime_config/` only as needed. Shared setup uses existing `test_support::config_store`; expected values stay with the cases. |
| Global/allocator isolation | Keep `config_funding`, `config_peak_charge`, `boot_strict_parity`, `auction_config_retention` and sync allocation targets isolated. Use their existing processes/allocators, not a combined binary. |
| Native artifact acceptance | Extend Axum `tests/boot_envelope.rs` and `tests/health_boot.rs`. Add one `tests/operator_configuration.rs` only for executable lifecycle/two-config acceptance, which cannot share an installed boot domain. |
| Build rejection and neutrality | Extend config-data's existing `tests/build_failure_harness.rs` and its existing mandatory compiler owner. Rename `baked_defaults_snapshot.rs` to `product_defaults_snapshot.rs` at cutover, updating Cargo/CI callers. Do not add a second full child-build suite. |
| Three-path CLI parity | Extend inline `config_source.rs` tests and core's opt-in test-support setup with fixed test cases, not a second validator. CLI tests call the actual store extraction and host snapshot constructor for equivalent inputs. |
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

- [ ] Enumerate all active readers/callers of `AuctioneerConfigSource`,
  `AuctioneerAppConfig`, `host_default`, `account_default`, `RUNTIME_BOOTSTRAP`,
  root operator-file includes, old overlay keys, default/boot override controls
  and resource report fields. Include examples, scripts, manifests, bidder
  compiler scratch workspaces, schema tests and all four adapter callers.
  Use `rg` across the workspace; the ownership table is not an exhaustive list.
- [ ] Separate live fixtures from immutable captures/evidence. For each affected
  test, record its retained guarantee, required features and replacement owner.
  A syntactic fixture migration must not weaken privacy/refusal assertions.
  Update valid fixture builders explicitly to the new versioned scopes; do
  not hide a legacy-to-new converter in the shared store fixture. For each
  hostile case, prove its control document passes every earlier layer so a
  missing schema version cannot replace the intended digest, privacy, alias,
  secret or effective-validation failure. Retain original external captures
  and translate only at their documented consumer boundary when required.
- [ ] Turn the inventory's C/S/P/V/D rows into an implementation checklist with
  concrete DTO path, resolver/reader, validator and resource owner. Derived and
  immutable fields have explicit rejection controls, not new public setters.
- [ ] Record build prerequisites as blockers if absent. Do not refresh archives,
  change SDK revisions or run provider mutations incidentally to this plan.

Checkpoint commit: `Record operator configuration implementation baseline`.

## Task 2: Prepare Strict Policy And Clear-Value Types

**Issues:** #127, prerequisites for #129/#135.
**Files:** New `config/{policy.rs,override_value.rs,validation.rs}`; core
`config/mod.rs`; schema `lib.rs` and `tests/root_shape.rs`.

- [ ] Write inline red tests for a required application `schema_version = 1`,
  strict `host`, `account_defaults`, `accounts`, `bidders`, `features` and
  `floor_currency` scopes. Reject missing/wrong application versions, unknown
  nested fields, old flat policy roots and mixed old/new roots. The final
  authoring root rejects `baked` and strictly types both startup and runtime.
- [x] Introduce the new types without changing the deployed extractor's type
  yet. Stage the new `OperatorPolicy` and dependency-neutral generic
  `OperatorConfigSource<Startup, Runtime>` alongside the old build-only root;
  do not change the old generic root's arity while build.rs still uses it.
  New strict-root tests target the new type. Task 7 switches CLI callers and
  Task 8 deletes the retired root; no released parser accepts both layouts.
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
  catalog eligibility and normalized resource/startup inputs. Semantic
  validation must not call global boot accessors. Two different valid contexts
  in one process must produce their respective decisions without installation.
- [ ] Run the new owner filters with `-- --list` first, then execute each with
  a verified nonzero count. Run all `root_shape` cases and core `--lib config::`
  after implementation. Record red reason, green count and no serving switch.

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

Checkpoint commit: `Add strict operator policy and reset semantics`.

## Task 3: Prepare Neutral Defaults And Pure Resolution

**Issues:** #126/#127; no serving/default accessor cutover yet.
**Files:** Product-default artifact; schema `types.rs`, `usersync.rs`, codegen;
core `config/{resolve.rs,budgets.rs,validation.rs}`.

- [ ] Write failing controls for explicit unconfigured user-sync. Product
  resource defaults must equal the reviewed existing numeric defaults, not
  `ResourceLimits::default()` zeros. Product operator aliases, account rosters,
  publisher lists, origins, domains and host vendor identity contain no
  customer settings; no example external URL means a configured service.
- [ ] Prepare a product-only artifact and generation path separate from the
  operator file. Keep existing serving accessors until the cutover group.
  Retain useful quote/syn generation rather than writing Rust strings.
- [ ] Prepare the effective-model migration so sync service state is optional;
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
- [ ] Reuse the actual bidder/settings/currency/alias/privacy validators.
  Prove immutable disable, unlinked/guest-ineligible factories, bad settings,
  sample/timeout bounds, expired currency and namespace conflicts still refuse.
  Use synthetic catalog metadata rather than a production bidder as a fake.
- [ ] Run schema/config owning units and core config units. Until the active
  cutover, new-resolution tests exercise explicit new inputs without claiming
  the old serving path already has the new semantics.

Checkpoint commit: `Prepare neutral defaults and explicit policy resolution`.

## Task 3A: Register Bidder-Owned Configuration Validators

**Issue:** #127; prerequisite for snapshot/CLI adoption in Tasks 6-7.
**Files:** Bidder/config/discovery owners and synthetic tests in the ownership
table; bidder `Cargo.toml`, `src/bidders/README.md` and each settings owner's
ordinary `mod.rs`/new `config.rs`. No request schema engine changes.

- [ ] Read the design's section 5.1 and review this amendment before coding.
  Keep one bidder-owned `validator::Validate` mechanism and the generated
  settings ownership/current per-field fallback. The discovered function is
  only a thin adapter to it, not a second validator or selectable validation
  phase. Do not implement an external trait on another crate's generated type, add a
  constraint DSL to TOML, or require config values that validly come from an
  impression. The new local borrowed view owns validation, not another DTO.
- [ ] Add red inline units for the single borrowed context and fixed callback
  errors. Extend the capable fake with synthetic validation behavior. Use the
  existing `generated_settings` disposable workspace when actual nonempty
  generated fake settings are needed; those variants must not enter release
  generation. Existing metadata-selected whole-catalog controls stay separate.
- [ ] Extend discovery's declaration/registration with the callback reference,
  deriving its requirement from `settings` on executable manifests. Use
  quote/syn and existing feature/target predicates. A settings-free factory
  gets no empty module; a settings owner missing its owner-level
  `validate_config` fails compilation. Generated wiring names that narrow
  function in `mod.rs`, never a private child path or test-only export.
  Extend validated registry plan/definition lookup with the callback using
  existing indexes, without invoking factories or boot.
- [ ] Add bidder-local borrowed views using `validator` derive/custom rules
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
- [ ] Prove supplied-value and effective-requiredness rules are each observable
  through that single context, with an exact one-call count per identity:
  disabled-invalid supplied values refuse; enabled missing intrinsic settings
  refuse; valid request-provided fallback remains allowed; an overridden bad
  parent is not hidden. Vary base/alias fields and enablement independently.
  On rejection, assert exact static reasons and no partial snapshot, factory
  invocation, secret read, network access or global installation.
- [ ] Test callback removal and miswiring with synthetic owners in the existing
  generated-settings/build-discovery controls. A generated function's presence
  alone is insufficient: swap two owners' callbacks and require a behavioral
  failure. Update child dispatch reconciliation to its enumerated real cases,
  not an approximate count. Do not duplicate the child builds in every lane.
- [ ] Add a credential-reference canary to the validator failure path and
  verify the callback result/log/status contains no raw or partial input.
  Bound/fund temporary `ValidationErrors` allocations and any added indexes
  in Task 4 before serving activation; callback purity is not zero-allocation.
- [ ] Run focused bidder `--lib config_validation::`, core
  `--lib config::validation::`, config `--lib --features build-support
  bidder_discovery::` and the explicit bidder `--test generated_settings` owner.
  List and execute new cases, preserving each old guarantee and its features.
  Run `--test test_isolation` after module wiring and test builds under Clippy.

Checkpoint commit: `Register bidder-owned configuration validation`.

The 2026-11-03 staged checkpoint adds the pure bidder subvalidator and its
synthetic behavioral controls; see the
[foundation evidence](../../testing/evidence/operator-configuration-foundations-20261103.md#shared-bidder-validation-checkpoint).
This checkbox does not complete the full `ConfigValidationContext`, product
resolution, snapshot funding or serving adoption. The existing CLI/store/host
readers remain active until the later cutover group.

## Task 4: Freeze Startup And Snapshot Ownership Before Adoption

**Issues:** #128/#130, reviewed prerequisite for raw/native adoption.
**Files:** `config/startup.rs`, resource profile/bootstrap/gates owners; review
record attached to #128/#130 before measurement, with repository evidence link.

- [ ] Inventory the exact pinned SDK and Axum operations for startup reads,
  environment/health materialization, parser structure, store selectors,
  logging and App construction. Do not assume `EnvConfig`, whole-file TOML
  loading or a typed host Settings object establishes a bounded read.
  Trace CLI raw/typed reloads as well: a preliminary bounded read does not
  bound a later SDK whole-file reread or a second environment materialization.
  The SDK pipeline must reuse the same accepted input within each command,
  without reopening the file or resampling ambient environment. A missing
  reusable SDK hook stays an explicit prerequisite, not an assumed
  implementation or permission for an Auctioneer-specific parallel loader.
- [ ] Freeze each proposed input cap, unit, parser/depth/value limit, deadline,
  retained storage allowance, owner and under/at/over test. Include startup
  file, scalar overlays, binding names/maps, health/token inputs and temporary
  copies. Identify an operation that enforces the bound before allocation, or
  a published provider bound plus independently funded materialization.
- [ ] Map the complete SDK extraction tuple: backend/blob bytes, JSON depth,
  JSON values, secret bytes, total bytes and timeout. Expose a startup knob
  only when both enforcement and accounting are identified. Otherwise it stays
  a fixed reviewed product/SDK value and an attempted override is rejected.
  Preserve internal normalization sentinels without exposing public unlimited.
- [ ] Freeze the startup resource-intent whitelist from the inventory, keeping
  measured factors, actual provider facts, baseline/layout costs and derived
  totals out of caller control. For each admitted intent name its actual
  enforcement owner and floor; do not advertise charge-only declarations as
  working knobs. Keep default workload unchanged.
- [ ] Specify snapshot ownership: store-request/candidate temporary allocations,
  owned normalized storage, current host snapshot and all retained generations.
  Acquire admission before construction/copy. A reservation follows the owning
  allocation through final drop; a request receipt cannot be reused as though
  each historical snapshot were free. Specify bounded candidate/update refusal
  and cancellation releases without assuming only two versions can survive.
  Include Task 3A's callback/registry additions and the bounded failure-path
  allocations made by `validator`; static diagnostics do not erase that peak.
- [ ] Require a typed-host control with short logical data and oversized
  String/Vec/Map capacities. Count bounded logical content before copying,
  normalize into funded owned storage, and keep the caller's source allocation
  explicitly host-owned on success and refusal. No serialize-to-Value detour
  or arbitrary-capacity adoption may bypass this boundary.
- [ ] Obtain maintainer review of exact numbers, derivation and lifecycle.
  This is an explicit stop/go gate, not a preapproved package. Record unknown
  SDK/provider facts as blocked raw paths with owners. Continue only pure
  foundation work while blocked; do not mark startup/provider work complete.

Checkpoint commit after approval: `Freeze startup and policy storage ownership`.

## Task 5: Implement Shared Funding And Conflict-Safe Startup

**Issues:** #128/#130. Starts the cutover group; Task 4 approval required.
**Files:** Core startup/snapshot/bootstrap/gates/configure; runtime budget/aux
owners; existing boot/config allocation and parity tests.

- [ ] Add red tests for fatal invalid source/binding/limit input, zero public
  caps, below-floor factors, unknown facts, two independent ceilings, identical
  concurrent install and conflicting valid install. Assert no policy read or
  new domain/gate publication on fatal domain validation; logging/closed health
  may already exist as allowed by the design.
- [ ] Implement typed `AuctioneerStartupConfig` and a fully validated pending
  domain before publication. Select exactly store, host or explicit
  product_defaults; store is the standalone default. Missing/corrupt required
  sources never select defaults. Changing source/trust/caps requires a new
  domain, not new counters in a live App. Do not add a multi-domain engine.
- [ ] Normalize once and derive the budget through runtime's existing checked
  additive owner. Install the accepted immutable domain consistently with
  gates/host primitives, then construct Apps. Handle identical/conflicting
  initialization explicitly instead of ignoring a failed OnceLock set. Publish
  the authoritative validated startup and its gates through one installation
  owner; independently published cells with a check-then-set race are not an
  atomic domain. Keep legacy public gate access backed by that same owner.
- [ ] Preserve independent product-default clone allowance while naming it
  for product defaults in active APIs/reports. Include the newly approved
  startup/snapshot terms with their actual multiplicities. Do not hide them
  in the old baseline, fold them into catalog cost or introduce `max(startup,
  steady)`. If tighter bounds are not independently established, retain the
  prior conservative charge.
- [ ] Extend `configure.rs` and the runtime pre-read guard to the complete
  accepted SDK extraction tuple. Independently exceed each captured field;
  assert zero store/secret reads on refusal. Preserve bounded `/status`
  diagnostics and immutable-registry-failure treatment.
- [ ] Measure new owners only after the approved reservations are frozen.
  Use independent literal/layout/differential expectations, not the same
  charge constants on both sides. Exercise asymmetric auction/aux/probe counts,
  backend/blob/value caps, default size, startup size and retained generations.
- [ ] Run `auctioneer-runtime --lib`, core `config_funding`, `config_peak_charge`,
  `config_node_cap`, `config_parse_factor`, `config_envelope_value_factor`,
  `boot_resource_charges`, `boot_strict_parity`, `boot_order_gates` and
  `bootstrap_order`. Preserve separate test processes and measured floors.

Checkpoint commit: `Fund and validate the operator configuration domain`.

## Task 6: Admit Immutable Policy Snapshots And Preserve Refusals

**Issues:** #127/#128/#130 and shared #135 constructor only.
**Files:** Core `config/{snapshot.rs,validation.rs,runtime.rs,resolve.rs}`,
handlers, config store test-support, existing lifecycle/retention tests.

- [ ] Test actual bounded SDK store extraction of the new DTO, then shared
  semantic validation and admitted immutable construction. Include a correct
  SDK envelope/version/digest containing missing/wrong app schema_version;
  it must refuse as malformed application policy, not pass on its checksum.
- [ ] Make `ValidatedPolicySnapshot` construction private. CLI validation uses
  the same non-installing semantic validator without acquiring serving globals;
  store and typed-host paths additionally require funded normalization and
  ownership. Do not pretend a dry-run allocation is free; retain its own
  explicit bounded caller context.
- [ ] Load once per admitted request, then derive every route decision from
  that snapshot. Preserve exact permanent/transient/unknown error categories,
  503/Retry-After behavior and liveness-only degradation. No missing-policy
  privacy grant, default-source fallback or hidden store retry cache.
  Preserve `auction_config_retention`'s early release of unrelated accounts:
  coherent decisions do not require the whole policy tree to survive fanout.
  Derive the required owned effective view from one generation, release unused
  root storage when no longer needed, and keep each remaining allocation funded.
- [ ] Test replacement A -> B omitting A's account/alias/overrides and prove
  they disappear. Hold several requests on distinct generations, replace
  repeatedly, refuse over-admission, cancel and finish in a different order;
  prove coherent policy and release of the final owning reservation. An invalid
  host candidate leaves the host's admitted current snapshot unchanged; a store
  load failure is still a refused request, not that host publication behavior.
- [ ] Update unconfigured sync handling before signing-key/callback work.
  Assert unavailable sync, zero secret reads, zero callbacks and no opt-out
  cookie rewrite; independently prove auctions/info still work. Configured
  sync retains real key rotation, callback, CORS and privacy behavior.
- [ ] Migrate handler access to admitted snapshots; do not leave direct
  extractor calls or separately re-resolve one route's policy. Keep source,
  immutable catalog and requested-cap refusal distinct.
- [ ] Run core `runtime_config_failclosed`, `resolve_runtime_config`,
  `auction_config_lifecycle`, `auction_config_retention`,
  `usersync_config_lifecycle`, `usersync_integrity`, `usersync_routes`,
  `usersync_handler_alloc`, `privacy_config_parity` and `response_account_resolution`.
  Include all host-typed capacity and admission controls from Task 4.

Checkpoint commit: `Resolve requests from admitted operator policy snapshots`.

## Task 7: Move Authoring And CLI To The Shared Contract

**Issues:** #129, mandatory #127 acceptance.
**Files:** CLI config_source/application; final schema root; core test-support;
existing CLI/core contract targets. The checked-in `auctioneer.toml` changes
only in Task 8, atomically with removal of its build reader.

- [ ] Make the final authoring root strictly typed `startup` + `runtime`.
  Serialize only runtime, including required app schema_version. Startup,
  physical bindings and secret values must not appear in the pushed payload.
  Preserve actual SDK envelope integrity behavior; do not claim signing.
  At this preparation checkpoint, use explicit new-format temporary authoring
  fixtures for CLI tests; do not migrate or parse the still build-owned root
  file with the new type. Move shipped-file projection controls to Task 8's
  checkpoint. The branch remains unshippable until that checkpoint passes.
- [ ] Wire existing validate/diff/push through the same explicit validation
  context and single bounded loading pipeline, not ambient installed boot
  globals. Exercise changed-file and changed-environment controls after input
  preparation: validation and emission must still use the identical accepted
  input. No second typed file load or cross-command cache. These commands
  validate startup as well as the projected policy. Preserve deployed secret
  metadata paths without adding a `runtime.` prefix to projected payload paths.
  Audit all secret metadata when policy fields move beneath `host`.
- [ ] Update exact env overlay paths/callers to `AUCTIONEER__...__...`.
  Keep the SDK's existing scalar-leaf-only behavior and `--no-env` scope;
  startup/store selector env behavior is not silently disabled or repeated
  per request. Use child processes for environment controls, not unsafe global
  mutations. Reject old names without adding aliases.
- [ ] Implement the hard three-path gate below before allowing publication.
  Preserve config gc/deploy and archive-independent maintenance CLI behavior;
  no migration/export subcommand or new feature split is needed.
- [ ] Run CLI `--bin auctioneer-cli config_source::`, `--test help`, core store/host parity,
  plus CLI default-feature and `--no-default-features` checks. Test the actual
  typed SDK push/validate entry points using the existing local test backend;
  do not publish to a production store as verification.

### Required CLI / Store / Host Parity Gate

Use one enumerated input matrix with independent expected results. Core owns
effective/constructor behavior, CLI owns authoring/projection/overlay behavior.
Sharing fixture data or setup is allowed; sharing the expected-output resolver
with production code is not. Every filter and case family asserts its count.

| Family | Required exercise |
| --- | --- |
| Every clear whitelist member | Nonempty inherited value, omission, concrete value and strict clear; identical literal effective outputs through CLI, actual SDK extraction and host construction. |
| Malformed wire values | Null, false/extra-key clear, clear on a nonnullable field, unknown scope/field, wrong type and app version. CLI and SDK deserializers must refuse with fixed categories. |
| Typed-host invalid values | Construct representable semantic errors directly and reach the real constructor; test scalar/cross-field/catalog/cap/refusal equivalence. Do not route them only through Serde. |
| Bidder-owned validation | In the existing synthetic generated-settings workspace, drive actual CLI, SDK store extraction and host construction with independent supplied/effective failures, disabled entries, alias/base precedence and valid request fallbacks. Missing/wrong callbacks must fail discovery or behavior without factories, secrets or boot globals. |
| Shapes unrepresentable by host DTO | Record compile-time exclusion or strict shared-DTO decoding control, rather than claim the constructor processed a null or unknown field it cannot represent. |
| Host storage | Oversized capacities, candidate limits and old-generation retention reach funded normalization; serialized length is not the expected allocation bound. |
| Independent resolution | Different defaults, named account, activity default/rules, both response caps and per-field clears varied separately; assert before-state presence and no surviving parent after a clear. |

The gate blocks #127/#129 implementation acceptance. Required targeted negative
controls disable a representative clear, bypass one path's validator and read
an installed global instead of the explicit context; each must fail an owning
test in a disposable workspace. Do not mutate the active review tree.

Checkpoint commit: `Cut config authoring and CLI over to operator runtime policy`.

## Task 8: Remove Operator Inputs From Release Builds

**Issues:** #126; coordinated with Tasks 5-9, not an independent early cutover.
**Files:** Config build/lib/data; schema codegen; old bootstrap data; config
snapshots/build-failure tests; Cargo and existing CI callers; active docs/examples.

- [ ] Switch serving product-default accessors to the reviewed neutral artifact
  after optional sync, resolution, startup and funding readers are ready.
  Delete release-build access to root `auctioneer.toml` and the compiled
  `data/runtime-bootstrap.toml`; remove obsolete operator-default/bootstrap
  harness overrides and migrate their remaining rejection guarantees.
  In this same compiling checkpoint, migrate the checked-in authoring file to
  startup/runtime and retarget shipped-file CLI/schema/example tests. Only then
  remove the retired root type after its final build caller moves. Never commit
  a new root document that the still-active old build reader cannot parse.
- [ ] Keep product artifact validation, bidder aliases/metadata/settings,
  discovery, disclosure, GVL/GCL and test-harness feature-export checks.
  Retarget operator alias/source failures to runtime validation; do not delete
  unrelated build rejection cases. No operator-supplied file path override
  remains as a way to customize a release.
- [ ] Rename the defaults snapshot target and its Cargo/CI/script references
  together. Freeze neutral defaults independently, preserving existing resource
  numbers/privacy defaults and documenting only intentional sync/domain/list
  changes. No snapshot regeneration from the function under test as the oracle.
- [ ] In the existing hermetic build owner, test root operator file absent,
  added, malformed and changed to a synthetic canary; build/generation succeeds
  identically and the canary is not emitted. Check SDK macros/packaging as well
  as Auctioneer build.rs. Keep controlled code/catalog changes as positive
  controls that the inspection really observes generated input changes.
- [ ] Preserve fresh refusal workspaces and one mandatory compiler owner.
  Update exact case/dispatch lists in its script if intentionally changed;
  do not reintroduce YAML snapshot guards or triple-run the same child plans.
- [ ] Run config `product_defaults_snapshot`, `codegen`, `usersync_config`,
  `activity`, `bidder_bindings`, schema `root_shape` and the explicit
  `build_failure_harness` owner. Run bidder `test_isolation` after wiring edits.

Checkpoint commit: `Make release generation independent of operator files`.

## Task 9: Port Native Startup And Finish The Atomic Cutover

**Issues:** #131 and shared #128/#130 completion on the selected native path.
**Files:** Axum startup/main/envelope/resource_profile and existing core setup;
other adapters' compile-time caller adjustments only; native process tests.

- [ ] Translate supported native file/env/binding inputs in adapter-owned
  `startup.rs` using Task 4's approved bounded operations. Keep only fixed
  sanitized error codes/paths. Validate source, approved secret references,
  actual platform facts, complete extraction tuple and resource requests
  before installing gates or building the App.
- [ ] Preserve Axum fatal health errors; test a credential placed in the flag
  slot, not only in the token slot. Closed/no-token defaults and conflicting
  valid installation behavior stay explicit. Do not copy SDK/provider external
  env names into a new Auctioneer naming scheme.
- [ ] Install/configure in the required order. Native `--resource-profile`
  diagnostics and CLI derivation consume the same validated normalized inputs
  without starting a serving domain. Preserve distinct strict/boot policies,
  over-ceiling warning semantics and stricter native schema admission.
- [ ] Update shared-API callers in all three guests so the workspace/guest
  builds remain valid. They use fixed product startup settings until their
  individual bounded input ports land, not a hidden unbounded parser. Keep
  their existing health-only Closed degradation and fatal install conflict.
  Explicitly document that these compatibility edits do not close #132-#134.
- [ ] In the Axum process target, start the already-built executable with
  isolated config A, then B, using distinct bind directories/services. Change
  banner/account/feature decisions independently and assert actual handler
  outputs. Hash the executable before/after; do not rebuild for either run.
  Also test no sync service, explicit defaults mode, missing required store,
  malformed policy, invalid startup, boundary input and shutdown cleanup.
- [ ] Run `cargo test --locked -p auctioneer-adapter-axum --all-targets` and
  verify `privacy_wire` actually executes. At this baseline it has no required
  feature; the adapter's dev-dependency enables loopback support for tests.
  Keep this feature-unified run distinct from secure-default package acceptance.
  Run health/boot process cases separately where globals require it. Re-run
  core lifecycle and strict parity.

Checkpoint commit: `Start the native adapter from validated operator settings`.

## Task 10: Verify And Review The Shared Native Implementation PR

**Issues:** Acceptance evidence for completed child scopes, not epic closure.
**Files:** Operator/cutover docs, development commands, dated new evidence,
`ISSUES.md`; existing CI ownership only where new/renamed targets require it.

- [ ] Publish a complete old-field -> new-field mapping from the inventory.
  Show the startup/runtime split, required version, clear semantics,
  unconfigured sync, secret references, source-selection failures, overlay
  limits and restart-only fields. Do not ship an obsolete live example.
- [ ] Run every affected test owner, including secure-default package tests
  separately from feature-unified workspace runs, and every explicit once-owned
  compiler target touched by Task 1's migration set. Enumerate new target
  `required-features`; assert actual dispatch. Do not select the entire expensive
  workspace on every edit or replace full acceptance with the focused loop.
- [ ] Execute the actual boot/CLI parity owner, not just two calls to one pure
  helper. Extend `auctioneer-cli --test resource_boot_parity` and core
  `boot_strict_parity` to the accepted startup tuple/product-default identity;
  vary one independent input at a time and retain fresh process installation.
- [ ] Run strict Clippy including test builds for changed packages, `cargo fmt
  --all -- --check`, source-import isolation and the existing fixture gates.
  Guest check/clippy/available execution remain compile/guest controls, not
  hosted acceptance. Keep pinned evidence unchanged and add new results for
  changed resource identities.
- [ ] Record commands, versions, counts, durations and pass/fail/not-run/blocked
  outcomes. Check new startup/default/snapshot storage deltas against frozen
  values without adjusting workload or headroom to make tests pass.
- [ ] Review three-path equivalence, privacy/refusal behavior, all issue field
  dispositions, exact normalized limits, simultaneous snapshot owners, release
  neutrality and one-artifact/two-config native evidence. Compare migration
  test names/counts; a disappearing assertion needs an identified retained owner.
- [ ] Update `ISSUES.md` only for fixes demonstrated by this implementation;
  distinguish partially delivered work and unresolved guest/host prerequisites.
  Request code review for a separate implementation PR after verification.
  Obtain permission before commit/push/public PR operations; no automatic merge.

Expected outcome: reviewable native implementation with concrete evidence.
Not an assertion that the epic, all providers, Trusted Server integration or
guest resource qualification are complete.

## Follow-On Acceptance Owners

These are explicit dependencies and acceptance requirements, not implementation
work silently included in a native completion claim. Their scoped plans must
use the approved shared startup/storage package and actual provider/host APIs.

| Issue | Required next work / blocking fact |
| --- | --- |
| #132 Cloudflare | Bounded binding/env/health materialization; captured App tuple; actual Worker artifact run with two configs. Provider bound/SDK gaps block adoption, not a local native green. |
| #133 Fastly | Adapter-owned stores/secret translation; key-to-store alias constraint; bounded preboot materialization; unchanged component with two configs. Keep local manifest secret sources credential-free. |
| #134 Spin | Hosted Akamai Functions surface, bounded variables/health/secret ownership, complete tuple; actual component acceptance. Generic Spin tests alone do not prove hosted readiness. |
| #135 embedding | Real host Settings ingestion/ownership, SDK type/service compatibility with #34, funded snapshot publication/retirement and host-owned App/services; standalone/embedded controlled result parity. Blocks integrated artifact #76, not independent bridge review. |
| #130/#9 resources | Remaining target facts, physical fit and deployment evidence remain explicit qualification blockers. Native source neutrality does not close them. |
| #136 documentation | Finish provider/embedding release instructions only against implemented surfaces and evidence. No required wait for all four adapters before accepting one selected host path. |

## Plan Verification

All tasks are initially unchecked. Proposed tests/types are not reported as
implemented. Before starting Task 1, complete an independent plan review against
the merged spec/inventory, check local document links and command/target names,
and resolve material contradictions. Documentation checks are not Cargo,
allocation, native-serving or deployment verification.

The initial plan's documentation review on 2026-11-02 found and corrected the shared-file cutover
ordering: Task 8 now changes the build reader and checked-in authoring root
together. The independent rereview found no remaining material plan blockers.
Local checks resolved all three document links, found all 32 named existing
test targets, and found no trailing whitespace or legacy delivery labels.
No Cargo tests, allocation measurements or serving/deployment checks were run
for this documentation-only preparation. Task 4's separate numeric approval
gate remains required; plan review does not satisfy it. That initial approval
does not cover the subsequently added bidder-validation amendment or Task 3A;
both require their own written-contract review before implementation.

The bidder-validation amendment and Task 3A subsequently passed a separate
read-only review, including the inventory row and the single-contract/no-settings
disposition. The three documents have 49 resolving local links and pass
whitespace checks. At that review no production code or runtime tests had
changed; the source/document review is not runtime qualification. The maintainer subsequently approved the
written amendment and implementation on 2026-11-02. Task 4's numeric startup
reservation approval remains separate and outstanding.

## Execution Progress

The maintainer approved implementation on 2026-11-02. The
[pre-edit baseline](../../testing/evidence/operator-configuration-baseline-20261102.md)
records the actual selected test owners and results, including the corrected
CLI binary owner. This is not acceptance of all tasks above.
The [foundation checkpoint](../../testing/evidence/operator-configuration-foundations-20261103.md)
records subsequent test execution, the pinned SDK audit and registry growth.

| Slice | Current status |
| --- | --- |
| Pre-edit selected baselines | Executed; 176 parent test cases passed, with the existing child-only CLI probes driven by their parent cases. |
| Strict operator authoring root | Implemented alongside the unchanged old reader; 12 root integration cases and 3 schema library cases pass. Spec and code-quality review approved; strict schema test lint passed. |
| Explicit field override | Implemented test-first; 29 inline cases pass, including duplicate/escaped marker keys, strict clear shape, direct null refusal and rejection of programmatic null-producing roots. Temporary parser allocations are not resource-qualified. |
| Versioned policy model | Staged DTO implemented; 16 inline cases pass, including JSON/TOML opt-out leaf inheritance and null/omission/value controls for all optional fields declared locally. Reused legacy nested DTO strictness remains part of cutover. Combined core configuration selection: 195 passed; full core library: 670 passed. Spec/quality reviews and strict test lint passed. Full semantic admission is not implemented. |
| Bidder-owned validation | Discovery, local callbacks and the staged pure shared dispatcher are implemented; bidder library 1,188 passed (9 existing source-dependent ignores), registry 21 and ownership 16 passed. Strict bidder lint passed. The once-owned settings contract passed 19 outer / 25 reconciled child cases, including absent-feature settings refusal; discovery passed 38. Exact ABI controls and matched native construction observations completed; staging spec and quality reviews approved. Allocation target: 8 passed, including independent regression ceilings and populated callback slots. Earlier guest test compilation and strict guest library/allocation lint passed; guest checks were not rerun for these review repairs. No serving callback activation or resource admission. |
| SDK bounded-input audit | Exact locked source confirmed through Cargo metadata. Store extraction has bounded reads; the TOML authoring loader and typed CLI rereads do not expose equivalent bounded-input hooks. This remains a Task 4 prerequisite, not an excuse for unbounded bootstrap. |
| Neutral product data / optional sync | Prepared without switching serving accessors. Existing exact defaults snapshots pass, and unconfigured services refuse before signing-key/registry work. Source/default separation remains unfinished until the reader cutover. |
| Pure account and scoped-cap composition | Implemented without admission/boot installation: 16 account cases and 14 total budget cases remain in the passing core library. Explicit-context host composition and staged policy validation are now implemented; their reservation and serving adoption remain separate. |
| Shared configuration ownership | Implemented under the approved amendment: shared models in config, compiled assets in config-data, settings/endpoints in bidder, and composition/SDK integration in core. All 698 baseline core case identities are preserved; native models, actual compiler consumers and WASI model execution pass. See the [ownership record](../../testing/evidence/configuration-model-ownership-20261103.md) for exact local checks and CI-only acceptance. |
| Context-based policy foundation | Host composition uses supplied defaults/capacity; the staged validator uses supplied defaults, catalog, capacity and time. Typed children use nested validation; maps and explicit clears retain their specialized rules. Success is not snapshot/input/storage admission. See the [validation checkpoint](../../testing/evidence/operator-policy-validation-20261103.md) for actual execution and remaining cutover gates. |
| Serving/default/bootstrap/CLI cutover | Not started; the exact startup reservation gate is still outstanding. |

The maintainer subsequently requested one physical owner for shared configuration
models, with bidder-specific settings remaining bidder-owned. The
[ownership amendment](../specs/2026-11-03-configuration-model-ownership-design.md)
and [migration plan](2026-11-03-configuration-model-ownership.md) specify the
cycle-free package/declaration move before extending the runtime cutover.
The maintainer explicitly approved the config/config-data package-role rename
on 2026-11-03. This supersedes the earlier file-owner mapping; it does not waive
this plan's resource and end-to-end acceptance gates.

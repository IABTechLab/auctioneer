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
wire leaves, application-version refusal under an actual valid SDK envelope,
bounded startup/raw input and environment materialization, snapshot ownership,
runtime/CLI adoption, operator-neutral release generation and unchanged-binary
two-config acceptance remain mandatory under the parent plan.

The locked SDK still has `fs::read_to_string` in its authoring loader and collects
ambient environment values in its default overlay policy. Task 4 must establish
the bounded single-document load/reuse contract and reviewed storage limits
before adoption. This work adds neither a parallel Auctioneer loader nor a
last-known-good configuration cache. Hosted CI, provider startup/execution and
new memory-fit/readiness claims are not local results of this checkpoint.

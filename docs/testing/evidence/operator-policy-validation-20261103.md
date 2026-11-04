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

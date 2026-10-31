# Catalog Arithmetic Comparison Controls

Date: 2026-10-21
Branch: `m7-bidder-catalog`, based on `e96279ab`
Scope: Task 4 numeric declaration comparison, not catalog qualification.

Follow-up: [patched-lock continuity](catalog-baseline-continuity-20261021.md)
adds a separate source capture with a checked, explicitly reported transition.
The sections below record the initial sole-baseline increment. The original
capture and its lock-mismatch refusal remain unchanged; the new record does
not reset catalog costs or qualify a target.

## Command And Ownership

The existing resource command has an explicit optional mode:

```sh
cargo run --offline --locked -p auctioneer-cli -- bidders resources \
  --check --all-adapters \
  --catalog-baseline crates/auctioneer-core/tests/fixtures/resource_envelopes/catalog-baseline.toml
```

It requires all four adapter rows and refuses audit, capture, review and
single-adapter combinations. Its bounded output labels the scope
`matched_catalog_arithmetic_only` and catalog qualification `not_evaluated`.
The normal resource report remains nested as the candidate, including its
unchanged readiness flags and external blockers. `--require-fit` remains
independent: a relative arithmetic pass cannot qualify a target.

The loader admits only the raw-byte-pinned original baseline through the
existing frozen-record reader. The comparison matches SDK, complete lockfile,
features, compiler/toolchain, reporter/target ABI and layout evidence. Code
commit, tree digest, file count and dirty state may differ. Workload, platform
and dispatch facts, construction costs, constants, metadata charge, GVL cache
and complete baked-default allowance must match. Frozen probe-config minimum,
probe-response population and GVL-cache runtime facts must also equal current
code even when a dominant term hides their numeric effect. Only C/I/V/K and the declared
auxiliary peak may grow; reductions cannot hide a component breach.

Both rows must independently reproduce their effective inputs, complete shared
runtime derivation, named isolate terms and auction receipts. C/I enter once;
V/K enter per admitted auction. Auxiliary peak growth uses the existing handoff
factor and `aux + max(aux, probe_config_min)` population. Independent component
ceilings are C 16 MiB, I 8 MiB and V 512 KiB. The settings-only 1-MiB limit is
not a limit on all K. Auxiliary peak growth is at most 8 MiB; aggregate deltas
are at most 96/128/64/64 MiB for Axum/Cloudflare/Fastly/Spin. Native arithmetic
must also retain policy headroom below the smaller known/configured ceiling,
or the configured ceiling alone when platform facts remain unpublished.

This checks declared scalars, not their physical upper bounds or provenance.
It does not establish qualified owner records, auxiliary subowner bounds,
guest layout, post-build residency, live precharging or full Task 4 acceptance.
Production catalog descriptors remain `UNCHANGED`; workload limits, dependency
pins and the additive budget equation are unchanged by this increment.

## Frozen Identity And Rebase Prerequisite

The original 131,237-byte capture remains unchanged:

| Item                        | Identity                                                           |
| --------------------------- | ------------------------------------------------------------------ |
| Baseline commit             | `e71e40aba611215bb9305f82e2449e13fb0f41ed`                         |
| Baseline raw SHA-256        | `6da360d57f08267018035cedce6157ada2e955ab08913517a5d6020dee0b9e39` |
| Baseline Cargo.lock SHA-256 | `f20a05604c912c0d9946e7c86020f1e12549abbf6007bfe8d56883dda1a7a1c1` |
| Current Cargo.lock SHA-256  | `aca2f5fc543402752f6e8c39aa79fa0c9269600c8ad08c2a617d81ec27ac6dbc` |

Consequently the real command above refuses with exit 2 and the fixed
`bidder_resource_input` diagnostic today. The lock transition contains the
approved local `jsonschema-macros-core` patch and two dependency edges:
bidder's test `tar` dependency and config's `auctioneer-runtime` dependency. Package
versions did not change. That observation does not authorize ignoring the lock
identity, recertify layouts or qualify the compiler.

An explicit future rebase must retain this capture, enumerate the lock/source
transition, compare raw/effective profiles and every term/receipt, and preserve
catalog-attributable owner changes since the checkpoint. In particular, count
foundations already changed auxiliary chooser subcharges even where a dominant
peak hides their change. A fresh zero-delta capture must not erase those costs.
The new capture needs matched feature/toolchain/ABI evidence and review before
its pin is admitted. No rebase or replacement pin is applied here.

## Control Boundary

The original-baseline loader checks a valid-TOML comment mutation, so digest
refusal does not depend on parse failure. Pure comparison controls independently
vary lifetimes, auction/auxiliary/probe populations, handoff factors, aggregate
boundaries, input identities, raw/effective caps, unrelated refunds, forged
terms/receipts, overflow and unequal native ceilings/headroom.

Real command tests exercise selection errors, immutable-input refusal and the
current build-identity mismatch. A separate synthetic matched-baseline unit
control exercises successful assembly of all four rows, literal aggregate
allowances, zero deltas, `not_evaluated` and strict-fit refusal. Its synthetic
baseline is explicitly not a historical capture or positive qualification.
The real command asserts the current lock mismatch without a conditional
positive path; independent unit controls reject each feature/ABI mismatch. Complete
owner qualification and the actual matched positive capture remain open.

The first compiling comparison stub failed 16 of 31 new pure controls; 15
refusal-only controls passed. The real command controls failed before the flag
existed. The new runtime-facts projection control separately failed before its
guard was added. Review then corrected a normalization test whose earlier input
was rejected by workload matching. Its revised input varies only the raw
auxiliary peak and leaves effective/derived values intact. Disabling only the
raw/effective equality now fails that test. Independently bypassing only the
baseline digest guard fails the valid-comment control. Both temporary mutations
are restored; no historical byte, workload limit or production equation changes.

## Verification

Rust: `1.98.1`. Counts below are per invocation and overlap; they are not a
summed workspace total.

| Scope                                                                                    | Result                                                            |
| ---------------------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| Secure CLI unit binary and resource-command target                                       | 654 + 15 passed, zero failures; two existing ignored child probes |
| Final secure resource-unit selection after normalization-test refinement                 | 117 passed, zero failed/ignored                                   |
| Resource-unit selection with linked insecure-bidder/conformance features                 | 117 passed, zero failed/ignored                                   |
| Real resource-command target with those linked features                                  | 15 passed, zero failed/ignored                                    |
| Workspace all-target Clippy with `insecure-bidders,conformance-suite`, `-D warnings`     | Passed                                                            |
| Formatting, diff, raw-JSON gate and its negative fixtures, roster and batch-rebase gates | Passed; 168 raw fixtures across 50 files, roster 139              |

The complete secure run uses `cargo test --offline --locked -j2 -p auctioneer-cli
--bin auctioneer-cli --test bidders_resources`; its existing archive/handoff tests
completed in the normal unit run, not by exclusion. The two ignored config
probe entries are subprocess fixtures driven by their passing parent tests.
The final secure resource selection uses `--bin auctioneer-cli bidders::resources::`
after restoring the temporary mutations. Feature runs select that same unit
prefix and the complete `--test bidders_resources` in separate invocations with
`--features auctioneer-core/insecure-bidders,auctioneer-core/conformance-suite`.
Lint uses `cargo clippy --offline --locked -j2 --workspace --all-targets
--features insecure-bidders,conformance-suite -- -D warnings`; its log is
`/private/tmp/m7-catalog-comparison-clippy-final.log`. Initial type/style lint
findings were repaired without suppressions. Existing GVL deletion notices and
the tracked `proc-macro-error2` future-compiler warning remain. Formatting uses
`cargo fmt --all --check`; script checks use the existing
`check-raw-json-fixtures{,-fixtures}.sh`, `check-roster.sh` and
`check-batch-rebase.sh` under `scripts/`.

Logs are `/private/tmp/m7-catalog-comparison-cli-final-secure.log`,
`/private/tmp/m7-catalog-comparison-resources-final-secure.log`,
`/private/tmp/m7-catalog-comparison-resources-final-features.log` and
`/private/tmp/m7-catalog-comparison-cli-final-features.log`. Compiling-stub RED
logs are `/private/tmp/m7-catalog-matched-arithmetic-red.log` and
`/private/tmp/m7-catalog-baseline-cli-red.log`. Isolated repair/mutation logs are
`/private/tmp/m7-catalog-comparison-runtime-facts-red.log`,
`/private/tmp/m7-catalog-comparison-normalization-mutation.log` and
`/private/tmp/m7-catalog-comparison-pin-mutation.log`.

Spec and code-quality reviews approved only this bounded preparation after
the runtime-facts and normalization-control corrections. They were read-only;
the parent executed verification. No full workspace test run, hosted CI result,
deployment, positive matched capture, completed Task 4 or M7/R0 readiness is
claimed.

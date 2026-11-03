# Resource Policy Foundation Verification

Date: 2026-10-17
Baseline: `cfd69dae`, Rust 1.98.1, EdgeZero PR #275 revision
`f6c708ccb5be19ba9fa92575db3c08c0f17e0d72`.

This records the first part of resource qualification Task 1, not a completed resource audit,
remediation package, WASM fit result or deployment qualification.

## Implemented Boundary

- Runtime-owned `IsolateTerms` retains all eleven population-scaled summands,
  including zero catalog terms. Checked summation and named rows destructure
  every field explicitly. Existing charges and numeric goldens are unchanged.
- `minimum_headroom_bytes` accepts a nonzero ceiling, uses overflow-safe
  ceiling division by sixteen, and applies native's 256-MiB minimum.
- Qualification profile validation selects the target class explicitly and
  enforces the floor at the smaller configured/platform ceiling. A larger
  declaration remains binding in accounting, measurement and report output.
- Actual `qualify validate` and `qualify evaluate` controls reject below-floor
  declarations without publishing a report. Accepted synthetic WASM declarations
  that exceed the accounted envelope remain nonqualified.
- Boot's warn-and-install behavior and historical schema-2 output are unchanged.
  The Axum export explicitly ignores the new internal `Derived.isolate_terms`
  field rather than silently changing its wire contract.

## Recorded Migration Runs

The baseline commands were run before changing production validation:

```sh
cargo test --locked -p auctioneer-cli --bin auctioneer-cli --no-fail-fast qualification::
cargo test --locked -p auctioneer-cli --test qualification_cli --no-fail-fast
```

| Stage                               | Qualification unit tests | Qualification CLI tests |
| ----------------------------------- | ------------------------ | ----------------------- |
| Original baseline                   | 398 passed, 0 failed     | 14 passed, 0 failed     |
| New floor, original shared fixtures | 355 passed, 44 failed    | 6 passed, 9 failed      |

The red counts include the new policy tests, so each target contains one more
test than the original baseline. Both new refusal tests also failed behaviorally
under the old production check, which accepted the below-floor declarations.

The complete red runs finished before either shared fixture was edited.
A first sandboxed unit run additionally failed localhost/process operations
with `PermissionDenied`: 306 passed, 93 failed. That is not the policy-failure
count. The recorded 355/44 run repeated the same source and old fixtures with
localhost/child-process access, without weakening a test or adding an ignore.

The shared synthetic profile and CLI fixtures now declare a 5,000,000,000-byte
configured/platform ceiling, 8-GiB hardware, and 312,500,000-byte headroom.
Accepted native declarations at a 4,000,000,000-byte configured ceiling separately
exercise the 268,435,456-byte minimum. Nondivisible WASM and native ceilings,
the maximum `u64` ceiling, and an intentional trillion-byte native control have
independent literal expectations.

All four ceiling-override tests were reconciled. Measured installed-cap peaks
3,731,564,543/3,731,564,544/3,731,564,545 plus 268,435,456 bytes of headroom
produce cap-minus-one/cap/cap-plus-one at the 4,000,000,000-byte installed cap.
The elevated observation is last in its process schedule so lifetime-high-water
validation cannot hide the intended result. The otherwise fitting accounted
bound is asserted in all three cases.

The broad green sweep exposed one additional test-only consumer:
`frozen_probe::tests::current_profile_bytes` derives current input from an older
trillion-byte fixture. Only its in-memory current copy is migrated and its debug
references rebound. Captured evidence and original fixture bytes are unchanged;
production frozen-probe declarations still call strict profile validation.

## Verification Scope

Completed focused checks include runtime's 170 unit tests, 22 profile tests,
32 evaluator tests, 15 actual CLI tests, 10 frozen-probe tests, and 21 native
envelope tests. The runtime also compiles for `wasm32-unknown-unknown`.
These overlapping focused counts must not be added to the complete sweep.

Final non-overlapping regression results:

| Command                                                                                | Result     |
| -------------------------------------------------------------------------------------- | ---------- |
| `cargo test --locked -p auctioneer-runtime --lib`                                          | 170 passed |
| `cargo test --locked -p auctioneer-cli --bin auctioneer-cli --no-fail-fast qualification::`    | 399 passed |
| `cargo test --locked -p auctioneer-cli --test qualification_cli --no-fail-fast`            | 15 passed  |
| `cargo test --locked -p auctioneer-adapter-axum --bin auctioneer-adapter-axum envelope::tests` | 21 passed  |
| `cargo test --locked -p auctioneer-adapter-cloudflare --lib envelope::tests`               | 13 passed  |
| `cargo test --locked -p auctioneer-adapter-fastly --lib envelope::tests`                   | 12 passed  |
| `cargo test --locked -p auctioneer-adapter-spin --lib envelope::tests`                     | 12 passed  |
| `cargo test --locked -p auctioneer-adapter-axum --test resource_profile`                   | 35 passed  |

These selected targets contain 677 passing tests and zero failures/ignores;
this is not a full-workspace test result. The complete unit sweep ran with
localhost/child-process access. The first post-migration attempt retained one
frozen-probe fixture failure; the final complete run passed after that helper's
in-memory migration. No retry or weakened assertion was added to a test.

Also passed: `cargo fmt --all -- --check`,
`cargo clippy --locked -p auctioneer-runtime -p auctioneer-cli --all-targets -- -D warnings`,
`cargo check --locked -p auctioneer-runtime --target wasm32-unknown-unknown`, and
`bash scripts/check-raw-json-fixtures.sh` (169 fixtures in 50 files), and
`bash scripts/check-roster-fixtures.sh` (baseline plus seven refusal controls).
Existing GVL deleted-vendor warnings and the dependency future-incompatibility
notice remain; no lint suppression was introduced.

The three WASM adapter envelope suites above run host-side declarations, not
guest handler execution. The runtime cross-check proves compilation, not WASM
memory fit, a hosted observation or an SDK fact. Independent spec-compliance
and code-quality reviews approved this bounded scope, not all of Task 1.

## Remaining Work

Task 1 remains open: implement the resource command and complete row mappings,
the isolated added-field compile control, matched four-target source reports,
the SDK mapping, typed audit/change records and review-scope checks. The later
boot/strict process-parity proof is also pending.

There is no AG-approved allocation/admission/workload remediation package.
All four current SDK descriptors still leave host-ingress accounting unknown;
native additionally lacks published ceiling/population facts, Cloudflare lacks
an inbound bound, and Spin deployment provenance needs alignment. None becomes
known because this foundation passes tests. No platform ceiling, workload,
memory coefficient or physical-memory acceptance criterion was changed.

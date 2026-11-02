# Privacy Wire And Config Review Repairs

Date: 2026-10-12
Worktree: `.claude/worktrees/m4-privacy`
Starting revision: `3fdd10a`

## Scope And Sequence

This follow-up verifies the seven new peer findings. It supersedes the prior
node-cap clamp and four-owner raw-slot assumptions; it does not change the
approved privacy permission tables or the shipped resource limits.

1. Make the handler-to-socket privacy target run under default Axum all-targets
   tests, with local-bidder support confined to a dev-dependency.
2. Vary CCPA, UFPD, precise geo and transaction-ID decisions independently,
   checking both permitted preservation and denied removal at the socket.
3. Apply each nonzero declared config node cap to the real extractor, resolve
   zero to the SDK default, and fund the same effective cap at boot.
4. Derive raw-slot restrictions from structured Rust model declarations,
   separately from wire inventory membership. Test actual inventory expansion.
5. Protect compensating config-test commands and retain standalone secure
   feature coverage despite Cargo's test-time feature unification.
6. Rerun the isolated rejected-build harness with a clean, uniquely named log.
7. Verify, independently review, commit and push only this round's changes.

## Findings

| Finding                                         | Disposition                                                                                                                                                                                                                                                                                 |
| ----------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Default `--all-targets` skips `privacy_wire`    | Removed `required-features`. A dev-dependency enables the loopback bidder only during tests. A manifest regression guards both target registration and the production/test dependency boundary.                                                                                             |
| Correlated wire decisions mask activity defects | Socket test sweeps all 16 combinations. ID, FPD, opaque, geo and TID paths have separate expectations and before-state presence assertions.                                                                                                                                                 |
| Declared config nodes ignored                   | Nonzero declarations now become `max_json_nodes`; zero selects the SDK default. Boot accounting and aux floors use that same effective count. Fresh-process tests exercise smaller, larger and zero/default caps through extraction.                                                        |
| Four-name raw guard misses inventory expansion  | Compiler scans Rust model slots using `syn`, derives wrapped `RawValue` storage, and rejects non-opaque dispositions on those admitted slots. Tests admit the six previously unscoped owners before probing traversal; rejection must name unsupported raw storage, not unknown membership. |
| Featureful config step unguarded                | Exact command gate plus deletion, weakening, disabled-step, inert-shell and suppressed-failure mutations protect the compensating tests.                                                                                                                                                    |
| `pub_restriction_for` has no production caller  | No correctness fix needed. Retained documented decoded-record lookup; authorization uses the effective restriction resolver, not first-match selection.                                                                                                                                     |
| Rejected-build count from a contaminated log    | Fresh verification uses one writer per uniquely named log and records completed result summaries, not just announced test counts. Prior reports are historical, not fresh execution evidence.                                                                                               |

## Evidence And Boundaries

The default-target manifest regression failed first on the old
`required-features` registration. After repair, default Axum all-targets ran
seven targets, including `privacy_wire`: 77 passed, zero failed or ignored.

Forcing `deny_user_fpd` independently to true and false made the socket test
fail on wrong removal and leakage of `user.keywords`, respectively. Forcing
`deny_tid` to true and false failed on wrong removal and leakage of `source.tid`.
All four temporary mutations were restored before final verification. A
sandbox-denied socket launch is not counted as successful mutation evidence.

Normal build dependency resolution does not enable `insecure-bidders`.
Cargo unifies dev-dependency features during workspace tests, so CI retains
standalone secure-feature test commands as well as the featureful command.
The hermetic rejected-build harness separately controls each child's features.

Independent review found that a renamed `RawValue` import could bypass the
first source scanner. The repair resolves renamed/grouped imports and
nongeneric local aliases, including transitive wrappers; cycles and unsupported
local alias syntax reject. Tests include typed and opaque controls as well as
negative raw-slot cases. Rust storage discovery handles the model's direct
and wrapped declarations and wire-name mappings, but is not a general Rust
type checker. Cross-file aliases, inline-module models, cfg alternatives and
macro-generated model definitions remain unsupported representations requiring
explicit disclosure-adapter review and coverage. Wire-catalogue membership
never implies traversal support.

Final verification results are recorded below. Hosted CI,
vendor packaging and physical-isolate qualification are not rerun by this
local repair round. The concurrent `ISSUES.md` edit is outside this commit.

## Verification

Each suite has a separate log under `/tmp/auctioneer-m4-review-20261012.F1QlfO`.
Counts below are completed result summaries, not `running N tests` headers.

| Check                                              | Result                                                                                                                                             |
| -------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| Full default-feature workspace all-targets         | 2,434 passed, 0 failed, 7 ignored across 175 targets; command exited 0. Ignored qualification checks are not counted as executions.                |
| Default Axum all-targets                           | 77 passed, 0 failed, 0 ignored across 7 targets, including `privacy_wire`.                                                                         |
| Final default-feature socket target                | 14 passed, including the 16-case independent wire matrix after the final assertion edits.                                                          |
| Standalone secure bidder/exchange/core all-targets | 1,234 passed, 0 failed, 2 ignored across 108 targets.                                                                                              |
| Standalone secure config non-harness targets       | 213 passed, 0 failed, 0 ignored across 6 targets; the final generator rerun below covers subsequent alias additions.                               |
| Featureful config non-harness targets              | 167 passed, 0 failed, 0 ignored across 5 targets.                                                                                                  |
| Final disclosure generator                         | 50 passed, including inventory expansion, alias handling and projection rejection.                                                                 |
| Final workflow and real-extraction guards          | 46 passed across 2 targets. The extraction guard launches fresh processes for smaller, larger and zero/default caps.                               |
| Final node-budget unit guards                      | 2 passed, driving 24 fresh-process boundary cases.                                                                                                 |
| Adapter conformance feature targets                | 77 passed, 0 failed, 0 ignored across 4 targets.                                                                                                   |
| Rejected-build harness                             | 61 passed, 0 failed, 0 ignored in the clean full-workspace log; completed summary at 818.85 seconds.                                               |
| Normal Axum production build                       | Passed; normal dependency feature resolution contains no `insecure-bidders`.                                                                       |
| Native workspace Clippy                            | Passed with `insecure-bidders conformance-suite` and `-D warnings`.                                                                                |
| WASM Clippy                                        | Cloudflare `wasm32-unknown-unknown`, Fastly `wasm32-wasip1` and Spin `wasm32-wasip2` all passed with `conformance-suite` and `-D warnings`.        |
| Repository gates                                   | Raw JSON: 92 fixtures across 36 files; roster: 139 members; both negative-fixture scripts, catalogue check and batch-document rebase check passed. |

The full workspace run began before the last scanner and lint-only refinements;
the final affected-target runs and strict native Clippy above verify those edits.

## Completeness Follow-Up

Starting revision: `c7e4f71`. The preceding results describe the previous repair
round, not verification of this follow-up.

1. Restore secure-default coverage of config's `activity`,
   `baked_defaults_snapshot` and `simple_fold` targets. Guard the complete
   non-harness integration-target inventory, not only a selected command list.
2. Discover Rust model slots throughout the source tree and reject an
   unclassified slot rather than granting traversal. Protect source relocation,
   recursion and the ten shipped raw extension slots with independent tests.
3. Preserve the deliberate over-ceiling boot policy, but exercise an ordinary
   isolate ceiling where a changed node cap changes the warning outcome. Pin
   the charged node unit cost independently in the boot integration test.
4. Clarify that today's four adapter rows still use 8,192 nodes and expose no
   operator override; host declarations are a forward-looking embedding API.
   Rename command-gate tests to describe exact-command checks, not general
   shell analysis.

### Follow-Up Evidence

The target-inventory test failed first on exactly the three omitted config
targets. The repaired command runs the library plus all eight non-harness
integration targets, and exact-command mutations cover removal of each target.

The model-tree regressions failed on accepted missing storage and root/nested
source relocation before the fix. A JSON-backed parent also failed to protect
a missing child model until the unneeded model-less-schema inference was
removed. Final validation requires discovered storage for every policy field;
the sole catch-all capability is a known owner's explicit flattened JSON map.
The exact ten raw extension slots and typed extension controls guard discovery
of both the `types` and `ext` sources. A missing source root also rejects.

The accounting fixture uses one aux permit and 512 KiB config byte caps so its
ordinary node counts fit within 4 GiB. At that same ceiling, a 1,048,576-node
declaration must warn and still install gates. This changes only test inputs,
not shipped adapter rows or the deliberate warn-and-boot policy. The two unit
guards drive ten fresh-process cases each.

Changing the production node charge from 623 to 1 made the boot assertion fail
at its first case: installed config term 134,881,281 versus independently
expected 134,881,903. The constant was restored with no remaining runtime diff.
The earlier attempt that hit a partially edited compiler is excluded from
mutation evidence, as is the incomplete first model-tree log.

Fresh follow-up logs are under `/tmp/auctioneer-m4-review-followup-20261012.1myKpO`;
the compiler red/green logs use `/tmp/auctioneer-m4-model-tree-20261012-*`.
Secure config passed 228 tests across nine targets. Core library, workflow and
real-extraction targets passed 334 tests across three targets. These targeted
runs, not the preceding full-workspace totals, apply to this follow-up.

After the final local-binding lint fixes, the compiler suite passed 56 tests
again and the default-feature real-socket target passed 14 tests, including
its 16 independent disclosure cases. Strict native workspace Clippy with
`insecure-bidders conformance-suite`, formatting and whitespace checks passed.
Both repository gates and their negative-fixture suites also passed. The full
workspace harness, WASM lint and vendor packaging were not rerun in this
follow-up; their preceding records remain historical. Hosted CI is not verified.

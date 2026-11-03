# Resource Review Checkpoint Controls

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

Date: 2026-10-18. Continues the
[source/lifetime audit](resource-envelope-audit-20261018.md) on the same bidder catalog branch.

This round implements audit/package validation and mechanical Git scope
controls. It changes no production workload, allocation ownership, coefficients
or boot policy. The canonical source-bound audit, concrete remediation package,
AG approval, allocation fixes, boot/strict parity and platform proofs remain
outstanding. Synthetic records used by tests are not proposals or approvals.

## Two Validation Boundaries

`bidders resources --check --audit <path> --plan <path>` validates bounded typed
TOML records against the current clean compiled source and the four actual
profile digests. It reuses runtime's derivation and checked named summands. A
coherent but invented tuple of caps, baked bytes or layout costs is not accepted
as the current source profile.

`bidders resources --check --review-commit <sha> --staged` checks committed
review-to-HEAD changes plus the index. The alternative `--head <sha>` checks the
complete committed review-to-head diff. Both records are read from Git's frozen
review tree, never mutable working copies. Baseline-to-review changes are
limited to the two records and human audit. The baseline lockfile digest, SDK
revision and declared Rust version are independently checked against Git B.

Frozen validation checks the stored sums and ordered projections without
recomputing the old baseline through new accounting coefficients. It cannot
claim current resource fit or human approval. Initial source validation and
AG's approval of immutable D remain separate prerequisites.

## Scope And Input Controls

- All four targets, all runtime-owned terms including zeros, six receipts,
  concrete owners, pending-fact actions and dominant-component decisions are
  required. Full numeric closure does not manufacture missing platform facts.
- Ordered changes and packages apply to the preceding state; savings cannot
  be counted twice. Workload proposals need explicit caps and product scope.
- Records are limited to 1 MiB each. Unknown fields, duplicate identities and
  missing required fields refuse. Packages have at most 128 exact file paths.
- Git checks include additions, deletions and both sides of renames. Staged
  reverts cannot erase committed scope violations. Frozen records cannot change
  even if their paths appear in a package. Unstaged user edits are excluded.
- Raw Git diff modes reject symlinks, gitlinks and conflicted index entries.
  `--ignore-submodules=none` prevents repository settings from hiding changes.
- Git output is bounded per pipe and operation, with command/operation deadlines
  and a bounded cleanup wait. Errors emit fixed diagnostic codes, not record
  contents or subprocess diagnostics.

## Verification

| Executed check                                            | Result                                                                            |
| --------------------------------------------------------- | --------------------------------------------------------------------------------- |
| Complete CLI unit run from clean source commit `3534d501` | 593 passed, 0 failed, 2 existing child-probe entries ignored by the parent runner |
| CLI resource/help/qualification command targets           | 8 / 4 / 15 passed                                                                 |
| Maintenance-only help and isolated build targets          | 4 / 2 passed, including corrupt-archive repair                                    |
| Runtime unit and non-installing core profile targets      | 170 / 2 passed                                                                    |
| Native schema-2 resource profile target                   | 35 passed                                                                         |
| Default and maintenance-only CLI all-target Clippy        | Passed with `-D warnings`, no suppressions added                                  |
| Formatting, whitespace, raw-JSON and documentation gates  | Passed; 169 budgeted raw-JSON fixtures across 50 files                            |
| Actual accounting / required-fit commands                 | Exit 0 / 1 with byte-identical JSON; no qualified target                          |

The complete unit run includes 56 resource controls; do not add their counts
again. The final run took 234.17 seconds and exercised the successful
current-source audit command path at full commit
`3534d501aa45c3d54c6240efd31408bf011d6d63`, not only its dirty-source refusal.
The unrelated unstaged roadmap edit is non-Rust documentation and does not
change the source identity. Source edits and compilation were serialized
during provider tests, which bind the running executable digest.

The first sandboxed full run had 545 passes, 48 failures and two ignored child
entries. Its failures came from denied loopback listeners/process probes.
The unrestricted run passed without changing those tests' assertions or
timeouts. The two existing ignored child entries are driven by parent tests;
this round adds no ignored tests.

Red controls independently reproduced the hidden-gitlink bug and both final
review findings (unknown fields on tagged unit states and staged-deletion
false-clean provenance). Record mutations also produced exactly the expected
three failures for byte limits, the 5% boundary and ordered savings, then passed
after restoration. The subprocess-budget fixture initially failed because the
macOS Git launcher emitted a sandbox warning on stderr; the reader correctly
charged it. The fixture now emits controlled bytes, without weakening the
actual output budget.

Spec-compliance and code-quality reviews approved only this bounded tooling
slice after repairs. No physical allocation, guest execution, hosted-handler
or full-workspace acceptance is claimed.

No canonical review commit or active package window is created by this round.
There is no approved review SHA to bind into CI yet. Ordinary resource checking
still lacks the planned matched-baseline/component-allowance gates, and required
fit remains nonpassing for the previously recorded local and external reasons.

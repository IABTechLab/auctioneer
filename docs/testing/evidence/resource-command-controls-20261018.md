# Resource Command Controls

> Editorial normalization (2026-11-02): delivery labels and displayed local names are descriptive. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

Date: 2026-10-18
Starting code baseline: `036bb439`; SDK `bc300ace`.

This round strengthens source/accounting verification. It changes no workload
limit, coefficient, numeric equation, allowance, deployment ceiling or artifact
qualification. Historical captures and canonical audit data are unchanged.

## Owner And Consumer Checks

- The strictly non-installing profile target now covers dominant auction demand
  with nonzero initialization, independent auxiliary/probe populations,
  independent calls/stream width, exact ceiling boundaries and seven overflow
  controls. Every case uses the existing runtime owner; gates remain absent.
- Ninety-two pure term-map corruptions cover missing/wrong values for all eleven
  named terms on all four adapters, plus zero-valued unknown extras. Both zero
  catalog terms are mandatory. Twenty whole-report controls additionally test
  rehashed zero-catalog edits and independent row/validation fit-flag edits.
- The existing source/digest checks already reject fit promotion. No redundant
  fit-policy comparison or new calculator was added. The term-map comparison
  was extracted unchanged into one private helper used by the real validator.
- The runtime comment now accurately describes full scoped request trees
  retained through `make_bids`; no unsupported population saving is credited.

The first test implementation repeated complete repository hashing for every
term mutation: nine selected CLI tests took 175.31 seconds. Separating pure
term checks from the retained whole-source checks yields 22 passing selected
CLI tests in 26.10 seconds with broader coverage.

## CI Execution Contract

`test.yml` now explicitly captures a clean four-adapter source audit in runner
temporary storage, validates that audit, and emits a strict source report. Only
exit statuses zero and one are valid for the strict command. Its JSON must have
the expected schema, accounting completeness, scope, exact adapter order and
boolean fit fields. Zero is accepted exactly when every row fits; otherwise
the only valid status is one. An input/build failure is not a successful
refusal, and a future legitimate fit does not require preserving today's failure.

Semantic guards pin the commands, dependency/order requirements and active,
failure-enforcing execution. The initial version rejected 52 workflow mutations.
A separate local execution of that strict shell step checked 59 synthetic
cases: 16 truthful outcomes accepted, 43 invalid status/report combinations
refused, including malformed/missing data and nonboolean fit values.

Independent quality review then found two defects before the code checkpoint:
the two external CI reference checkouts appeared as nonfile source inputs, and
streaming `jq -e` could accept an invalid report followed by a valid one. Both
were reproduced by failing tests. Exact root `.gitignore` entries now isolate
only those two external trees; a real nested-Git test derives their paths from
the workflow and still detects tracked/untracked sources and refuses unknown
nested repositories. The strict step now slurps and requires exactly one
top-level report. Three committed tests execute the actual workflow shell
with Bash and jq: 108 cases, 16 accepted and 92 refused. These include every
fit mask, statuses 0/1/2/101, empty/malformed/multiple reports and ten independent
field corruptions. Semantic controls also protect the single-report rule.

## Verification

| Check                                                   | Result                                                                                                    |
| ------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| Required source-command guard before workflow edits     | Expected failing test                                                                                     |
| Pure profile target                                     | 8 passed                                                                                                  |
| Complete semantic CI target before/after review repairs | 98 / 101 passed                                                                                           |
| Selected resource CLI unit targets                      | 22 passed                                                                                                 |
| Broader CLI resource prefix after review repairs        | 77 passed                                                                                                 |
| Runtime budget unit tests                               | 75 passed; 102 unrelated tests filtered                                                                   |
| Targeted Clippy, formatting and diff checks             | Passed                                                                                                    |
| Actual three commands against a clean code commit       | Passed at `5817bde3`; capture/validation exited 0, strict CLI exited 1 and its CI reconciliation exited 0 |

The parent independently reran the final two complete core targets (8 + 101
passing), the broader CLI `bidders::resources` prefix (77 passing, 530 unrelated
tests filtered, 28.66 seconds), and runtime's `budget` prefix (80 passing, 97
unrelated tests filtered). The final CI rerun executed all 108 shell cases.
Before the review repairs, the same core and CLI selections passed 8 + 98 and
76 respectively. CLI/core targeted Clippy passed with `-D warnings`; formatting, diff checks and the
raw-JSON gate passed (168 fixtures across 50 files). The prefixes include sibling
modules, so their totals differ from the worker's narrower selections above;
they are not additional unique tests to sum together.

The review repair run also passed 12 CLI integration tests and its all-targets
Clippy checks. Independent spec review and code-quality review approved the
slice; quality approval followed re-review of both reproduced defects.

## Clean-Source Execution

The three commands in `test.yml` were executed sequentially under Bash after
code commit `5817bde3ee9b7ab342a0625733cf13c00be2038e`. The only dirty path was
the unrelated `docs/roadmap.md`, excluded from source identity and untouched by
this round. Each row recorded `dirty = false`,
`source_tree_matches_commit = true`, that exact code commit, SDK `bc300ace`
and application-only features. All eleven terms, including both zero catalog
terms, remained present. Rust was 1.98.1 on `aarch64-apple-darwin`.

Capture and audit validation exited zero. The strict CLI exited one with all
four `required_fit` values false; the actual CI shell accepted that consistent
refusal. Independent literal checks reproduced unchanged accounted totals:
2,533,631,672 (Axum), 138,048,496,272 (Cloudflare), and 23,589,646,360
(Fastly/Spin). The WASM rows explicitly retain `host_native_substitute` layouts.
These are not new guest measurements or passing readiness gates.

Outputs are in `/private/tmp/auctioneer-resource-commands.FHREYg/`:

| File                  | SHA-256                                                            |
| --------------------- | ------------------------------------------------------------------ |
| `resource-audit.toml` | `0a5a3730cbba2243dc4ce2b39787e3d1809536f4d074db5dda1299cc0d4e1606` |
| `resource-audit.md`   | `e8f74cb47553df2c6f4b8ad4a2346b5c33cbfa57bd131be7adabb0fd86b110cb` |
| `resource-fit.json`   | `012f1b0c5a09ac66ba564f6e3230b90e55e476e73e211bf78eff31c0f7eea648` |

No canonical audit, baseline or historical evidence was overwritten. This
documentation-only follow-up records execution at the code checkpoint; it is
not a recapture against a later documentation commit.

Logs: `/private/tmp/resource-qualification-resource-controls-*.log` and
`/private/tmp/resource-qualification-parent-resource-*.log`; review repairs additionally use
`/private/tmp/resource-qualification-quality-*.log`. These scoped targets are not a
new full-workspace or guest-runtime execution. Existing GVL/future-compatibility
warnings remain. Current absolute-fit failures and unavailable provider/stack
facts still prevent resource qualification closure; this round is not the unfinished matched-catalog
allowance gate, physical allocation proof or hosted qualification.

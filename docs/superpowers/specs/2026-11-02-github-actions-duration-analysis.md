# GitHub Actions Duration Analysis

Status: verified findings and approved repair scope, 2026-11-02.
Scope: `.github/workflows/` on `import/auctioneer`, including runs
41383030459 through 41459435846. Implementation is tracked in the
[repair plan](../plans/2026-11-02-test-and-runtime-performance.md).

## Summary

The supplied review correctly identifies a long test-workflow critical path,
duplicate cache contents and missing job-level timeouts. Its stronger claims
about cache history, billing, runner size and a stuck completed workflow do not
survive verification. Those distinctions change the recommended repair.

The sampled completed or failed `Run Tests` executions take 18-34 minutes;
sibling workflows finish in 2-10 minutes. These are not complete passing
baselines. Initial verification found three native jobs running in 41459435846;
later both workspace lanes passed at about 36 minutes, while standalone CLI
remained active. Those intermediate timings are not a final workflow duration.
The run subsequently completed: all Rust steps passed; only the CLI lane's
browser-shutdown step failed, before browser startup. Its missing Axum/config
example build prerequisite is repaired explicitly, with both shutdown cases
passing locally. This is not yet a complete passing hosted baseline.

Compilation, child Cargo builds, repeated source preparation and hashing all
contribute. They must be distinguished from ordinary unit execution and serving
latency. At `bc27744` the five workflow matrices expand to 24 jobs, with
overlapping but different feature and target graphs, not 23 identical cold
builds. The repair adds one mandatory heavy-producer owner.

## Observed Durations

| Workflow | Sampled Duration | Result |
| --- | --- | --- |
| Run Tests | 18m33s-34m30s | Failed or incomplete; no passing baseline |
| CI: Fastly | 5m45s-9m42s | Success |
| CI: Cloudflare | 3m48s-6m18s | Success |
| Run Format | 2m28s-2m56s | Success |
| CI: Spin | 2m12s-3m07s | Success |

Run 41393299498 predates the native/compiler lane split. Its 34m24s native job
is not today's `secure` lane. Selected job durations:

| Job | Duration |
| --- | --- |
| Pre-split native acceptance | 34m24s |
| Pre-split compiler conformance | 27m33s |
| Configuration build failures | 16m19s |
| Selected-schema source probe | 13m29s |
| Private-layout source probe | 6m05s |
| Application artifact jobs | 5m36s-5m42s |

Within that native job, default workspace acceptance took 879 seconds and
secure bidder/exchange/core acceptance 399 seconds. The workspace invocation
failed; subsequent job steps were skipped. Its passing duration cannot be
derived from that partial run. Parallelization changes scheduling, not the
underlying repeated work, and a complete matched baseline is still required.

Reported suite execution itself includes child builds. Examples from that run:

| Target | Reported Duration | Work Included |
| --- | --- | --- |
| Schema build controls | 372.00s | Synthetic compiler builds and controls |
| Schema source conformance | 316.81s | Source/compiler contracts |
| CLI binary unit suite | 289.89s | Source copies, hashing and fixture preparation |
| Sealed-body API controls | 147.12s | Child builds and exact compile refusals |
| Feature-export source controls | 92.08s | Explicit child feature graphs |

These are suite durations, not per-case attribution or projected savings.
The 1,177 bidder unit cases took 4.01s; 607 core unit cases took 0.80s. Slow
acceptance does not imply that normal auction execution takes minutes.

## Cache Findings And Corrections

The original retained-cache snapshot listed 13 entries totaling roughly 14 GB,
all newly created by run 41459435846. The separate usage and entry-list APIs
returned different counts during verification, so these non-atomic snapshots
are not an eviction trace.

| Original Entry | Approximate Compressed Size (MiB) |
| --- | --- |
| Native secure | 2476 |
| Compiler schema | 1673 |
| Compiler discovery/settings/exports, each | 1259-1261 |
| Private-layout source | 1209 |
| Selected-schema source | 1108 |
| Native application artifact | 1032 |
| Guest application artifacts, each | 762-869 |
| General format cache | 703 |
| Configuration build failures | 323 |

This duplication is a credible storage-pressure concern. However:

- Equal `created_at` and `last_accessed_at` describes the retained entries only.
  It does not establish that older or evicted caches were never restored.
- The compiler log for run 41393299498 records `Cache hit`, a
  1,758,813,746-byte cache, and `Cache restored successfully`, between
  05:19:16Z and 05:20:03Z on 2026-11-02. Restore took about 47 seconds.
- The current workspace-default log also restores an implicit toolchain-action
  cache: 1,170,592,783 bytes, from 14:10:57Z to 14:11:24Z. The setup action
  enables caching by default, so auditing explicit cache blocks alone misses
  an additional cache policy. The repair sets `cache: false` on every setup.
- GitHub's default storage limit is 10 GB, but administrators can configure a
  higher limit. The actual repository limit and historical eviction sequence
  were not retrieved; an over-default snapshot does not prove permanent
  thrashing, zero cache benefit or an impossible cache policy.
- A warm registry/Git cache avoids downloads. It contains no compiled Rust
  dependencies and must not be credited with compilation reuse.
- Stable lockfile keys make reuse plausible, not proven. Scope, cache version,
  write success, quota, access history and real restore logs all matter.

The approved repair separates download identity from compiled-product identity:

1. All five workflows share a registry/Git download-cache namespace, keyed by
   OS, `.tool-versions` and `Cargo.lock`.
2. Slow native lanes additionally restore normal `target/debug/` build products.
   One default-workspace owner saves the seed. Cargo still checks each lane's
   source, features and fingerprints; every test runs normally.
3. Nested scratch workspaces, guest artifacts and qualification observations
   are outside that compiled seed. No test-result caching is introduced.
4. Native save-after-failure remains; cancellation and exact hits do not save.
   No remote cache deletion or quota/billing-setting change is authorized.
5. Disable the toolchain action's implicit cache so it cannot duplicate or
   overwrite the explicit policy.

The new footprint, hit rate and net time benefit require hosted measurement.
Check restore logs and duration, subsequent builds, usage and access metadata;
no one metric alone proves an end-to-end improvement.

## Timeouts, Concurrency And Runners

The previous workflows have step limits on some setup commands but no job-level
limits. Explicit owner-specific safety ceilings and limits around long Cargo
steps prevent a runaway from reaching GitHub's default six-hour job ceiling.
With no complete passing population, initial values are safety policy, not
claims about measured p95 or performance targets.

Run 41459435846 does not demonstrate 19 completed jobs with a stuck workflow:
the API reported active native jobs. Later the two workspace jobs passed while
CLI remained active; the run eventually completed with the browser prerequisite
failure described above. Long-running work motivates diagnosis and finite limits,
but does not alone establish an orchestration hang.

Existing workflow-specific `cancel-in-progress` groups correctly cancel
superseded runs without cancelling sibling workflows. Cancellation is not a
replacement for execution deadlines.

The repository API reports `public`. Standard public `ubuntu-latest` runners
are documented as 4 vCPU / 16 GB, not the 2-vCPU private tier. Standard hosted
usage is free for this public repository; paid larger runners and cache
storage are separate decisions. Aggregate job time is not the duration of the
slowest job. Paid runner upgrades are outside this repair.

Skipped matrix steps cost little. Splitting the shared lane definition into
more jobs would primarily improve readability, not remove compiler work. Keep
the current graphs while repairing the expensive owners and duplicate setup.

## Test And Runtime Repairs

- Give hermetic sealed-body and feature-export child compiler cases one
  mandatory owner. Keep parent-feature predicates in normal feature lanes.
- Replace whole-checkout unit mutation fixtures with small synthetic source
  trees and independent expected identities, retaining live compiled binding.
- Prepare immutable qualification seeds once per matrix, restoring and freshly
  validating every mutation. Never cache identity results across source edits.
- Keep exhaustive verifier field mutations at their unit boundary and actual
  CLI integration coverage for dispatch, provenance and representative refusals.
- The 42 synthetic schema-helper unit cases are repeated in four integration
  roots. Do not introduce special cfgs, private-source imports, runtime features
  or public APIs solely to reduce this count. Broader ownership consolidation
  needs a clean module boundary; these cases are not the dominant build cost.
- Configuration revalidation, discarded impression clones and per-impression
  bid scans are runtime profiling candidates. Only behavior-preserving request
  copying with independent output/allocation controls is in this repair.
  Cross-request config caching, a new regex engine and new resource allowances
  are excluded.

## Verification Boundary

No complete passing `Run Tests` baseline exists in the queried history. Repair
verified duplication and missing bounds now; claim no speedup until matched,
complete runs establish it. Preserve all feature lanes and unique assertions,
real compiler refusals, independent oracles, immutable captures and fresh
allocator/socket/measurement processes.

Current local runtime samples are exploratory: the optimized 946,133-byte GVL
parse had medians of 5.3-8.6ms across three processes, with unstable tails on a
heavily loaded host. Warm lookups use a parsed-list cache. Those measurements
are neither publisher qualification nor an end-to-end auction benchmark.

## Primary References

- [Dependency caching and configurable limits](https://docs.github.com/en/actions/reference/workflows-and-actions/dependency-caching).
- [Hosted runner specifications](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).
- [Actions billing](https://docs.github.com/en/actions/concepts/billing-and-usage).
- [Toolchain action cache inputs](https://github.com/actions-rust-lang/setup-rust-toolchain/blob/v2/action.yml).
- [Workflow timeout syntax](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#jobsjob_idtimeout-minutes).
- [Run 41393299498](https://github.com/IABTechLab/auctioneer/actions/runs/41393299498)
  and [run 41459435846](https://github.com/IABTechLab/auctioneer/actions/runs/41459435846).

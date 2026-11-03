# Bidder Evidence Run Scope Implementation Plan

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans. Execute with red/green controls and separate specification and quality reviews.

**Goal:** Implement the first executable run-scoped evidence checkpoint in bidder catalog
Task 16 without granting migration or foundation acceptance.

**Architecture:** The existing CLI owns a fixed reference-integrity producer.
Fresh exclusive run and producer directories bind its actual execution to
caller-supplied identity and current inputs. The consuming checker independently
requires this producer and reports unavailable foundation requirements; a
reference receipt can never certify migration. No new crate, engine change,
dependency, serving behavior, schema change or resource-limit change.

**Tech Stack:** Existing Rust CLI, Clap, Serde, SHA-256, bounded native I/O,
pinned Git source inspector and source snapshots.

## Scope And Authority

This executes a subset of the approved
[bidder catalog Task 16](2026-10-16-bidder-catalog.md#task-16-evidence-derived-migration-checking),
not a replacement completion contract. The reference-integrity producer executes
one operation, `inspect_reference_inventory`: fresh pinned source inspection and
complete inventory comparison using existing code. Its receipt reports one
operation and its fixed outcome. Source-document counts are not positive or
negative behavioral test counts; neither gets invented here.

The run scope is explicitly preparation-only. Schema/resource qualification,
registry lifecycle, proprietary decoder, reviewed current-shipped-subset branch
requirements, reviewed priority/APS requirements and composed onboarding remain
unavailable. These blockers come from code-owned requirements, not a supplied
manifest or the receipts present. Even a complete preparation run has zero
migrated identities and exits nonzero when checked as acceptance. The local
`run` command uses the same begin/produce/check sequence and cannot return
overall acceptance success. Ordinary inventory-only checks retain their honest
unfinished semantics. Pending wave designs are rejected for execution before
creating any output; existing wave-design checks remain available.

## Task 1: Exact CLI Controls First

**Files:** `crates/auctioneer-cli/tests/bidders_evidence.rs`.

- [x] Add real CLI controls for begin, produce, run and consuming check using
  temporary output outside the source tree and the existing pinned source env.
- [x] Drive independent run and input mutations: same inputs/old run receipt
  cannot replace a skipped current producer; wrong run, attempt/commit binding,
  failed/missing/unknown producers, changed inventory/code, duplicate run and
  producer publication. Assert exact status/error and zero migrated.
- [x] Assert a successful reference operation is complete only in its stated
  preparation scope, with exact operation identity/count. Deleting or forging
  that result cannot grant preparation completion or migration.
- [x] Refuse malformed/duplicate/unknown JSON fields, invalid IDs, unsafe final
  file/directory links, nonregular/oversized inputs and unexpected artifact
  membership. Assert unsupported waves create no output.
- [x] Run the new target and record the actual missing-command red before code.

## Task 2: Fixed Run And Producer Ownership

**Files:** `crates/auctioneer-cli/src/bidders/{mod.rs,evidence/}`, shared native I/O
only if a reusable byte-parsing boundary is needed.

- [x] Add `bidders evidence begin`, `produce` and `run`. Source and inventory
  flags use existing inspection conventions; no arbitrary executable or command
  appears in a manifest. The only supported producer is `reference_integrity`.
- [x] Require a bounded caller-supplied run ID, explicitly binding expected
  commit/attempt when supplied as identity components. Never infer expected ID
  from an artifact. Bind manifest version/scope, executable/build identity,
  source revision, inventory bytes, lockfile, target/features and current
  workspace snapshot. Revalidate current inputs before work and publication.
- [x] Exclusively create a fresh run directory and manifest. Exclusively claim
  the fixed producer directory before running work. A crash, failure or missing
  receipt stays incomplete; no overwrite, old-success lookup or retry reuse.
- [x] Read/hash the same bounded input bytes. Reject duplicate keys and unknown
  typed fields before interpretation. Fixed root-relative artifact names cannot
  escape their run; refuse linked/nonregular final objects and unsafe parents.
  Do not claim adversarial concurrent filesystem mutation is authenticated.
- [x] Reuse fresh `check::run` logic or its extracted typed equivalent, checking
  report status rather than treating `Ok(report)` as success. Write a completed
  receipt only after the operation succeeds and current inputs still match.
  On failure publish only a fixed failed outcome if safe; otherwise absence
  remains failure. Do not include source payload or parser text in diagnostics.

## Task 3: Independent Consumer, No Acceptance Default

**Files:** CLI `bidders/{check.rs,mod.rs,evidence/}` and its integration target.

- [x] Extend `bidders check` with paired `--evidence-root` and `--run-id` flags.
  Preserve existing inventory/wave-design invocations when absent.
- [x] Independently derive the fixed preparation producer/operation set and
  require exact manifest/receipt/artifact membership. Check run ID, current
  input/build binding, fixed operation identity/count, outcome and digest.
  Missing, failed, foreign, unknown or stale input remains explicitly refused.
- [x] Always enumerate unavailable acceptance requirements and keep migrated
  zero; `--require-complete` cannot accept this shortened producer set. Report
  preparation completion separately from acceptance. Actual CLI `run` ends at
  this same refusal rather than returning success after its lone producer.
- [x] Add mutation-sensitive literal controls, including manifest-set shortening
  and copying a prior valid receipt into a current skipped producer directory.
  No test-only registration or manufactured complete-foundation report.

## Task 4: Verification And Checkpoint

**Files:** This plan, master Task 16 status and `ISSUES.md`.

- [x] Run the complete new target plus existing source/import/wave commands and
  CLI unit tests touched by the extraction. Serialize Cargo with jobs 2.
- [x] Run maintenance/no-application build checks if shared wiring changes.
  Run strict CLI Clippy, source-import isolation, formatting and raw-fixture
  gates. Record exact passing dispatch counts, not predicted counts.
- [x] Obtain source/spec review, fix issues, then independent quality review.
  Re-run affected controls after fixes. Do not mark full Task 16 complete.
- [x] Record unimplemented acceptance and producer requirements explicitly,
  commit the verified checkpoint and push `bidder-catalog`.

## Verification Record

The initial missing-command red failed with CLI exit two and `unrecognized
subcommand 'evidence'`. The first implementation's complete integration target
passed ten tests. Subsequent counterfactuals add consistently repinned input and
requirement mutations, forged completion over source drift, and producer-path
refusals. A disposable Git source fixture separately tests tracked code/lock
changes and untracked additions; it does not mutate the real CLI workspace.

Specification review approved this preparation-only scope. Quality review found
that a bare `manifest.json` parent becomes the empty path, which directory reads
reject. Both direct CLI reproduction and the new automated control failed with
exit two and `bidder_evidence_membership`. All four file helpers now use the
validated absolute path; the final 13-test target passes, and quality re-review
reports no remaining actionable findings in scope. Shared status labels use no
real bidder names. Claim-before-work is verified by source order, not a runtime
inspection-count control. No full Task 16 or migration acceptance is claimed.

Executed with Rust 1.98.1, the existing pinned SDK, offline/locked Cargo and two
build jobs:

| Run | Actual Result | Scope |
| --- | --- | --- |
| CLI `--lib --bin auctioneer-cli --test bidders_evidence --test bidders_source --test bidders_import --test bidders_wave --test help`, four test threads | 796 passed, zero failed, two existing ignored | Before the final relative-path repair: library 36, binary 706, evidence 12, source 21, import 8, wave 9, help 4 |
| Exact relative-path regression before repair | Zero passed, one failed, 12 filtered | Expected producer success versus actual exit two; not a compile failure |
| Complete evidence target after repair, four test threads | 13 passed, zero failed | Includes relative manifest, all consistently repinned binding rows and forged completion over source drift |
| CLI `clippy --all-targets -- -D warnings` after repair | Exit zero | Existing GVL deletion and upstream future-incompatibility notices remain separate |
| CLI `check --no-default-features --all-targets` | Exit zero | Maintenance remains independent of embedded application configuration |
| CLI `test --no-default-features --lib` | 36 passed, zero failed | Shared I/O/process/source controls without application features |
| CLI `clippy --no-default-features --all-targets -- -D warnings` | Exit zero | Maintenance-only lint lane |
| Bidder `test_isolation` target | 16 passed, zero failed | Ordinary imports, test ownership and synthetic shared bidder inputs |
| `cargo fmt --all -- --check`, `git diff --check` | Exit zero | No formatting or whitespace errors |
| Both raw-JSON fixture gates | Exit zero; 188 fixtures across 54 files | Repository counts plus gate negative controls |

The two ignored binary probes are explicitly executed by the passing
`env_overlay_reaches_a_live_runtime_key` and
`absent_runtime_key_is_not_env_overridable` parents; child execution is not added
again to the totals. This is not a fresh full-workspace or guest execution run.
Historical profiles/evidence, schemas, root dependencies/lockfile, compiler
patches, workload limits and serving behavior have no diff.

Local execution logs: `/tmp/auctioneer-evidence-regression.log`,
`/tmp/auctioneer-evidence-relative-red.log`, `/tmp/auctioneer-evidence-final.log`,
`/tmp/auctioneer-evidence-final-clippy.log`,
`/tmp/auctioneer-evidence-maintenance-check.log` and
`/tmp/auctioneer-evidence-isolation.log`, plus
`/tmp/auctioneer-evidence-maintenance-tests.log` and
`/tmp/auctioneer-evidence-maintenance-clippy.log`. These are test logs, not captured migration
or deployment qualification artifacts.

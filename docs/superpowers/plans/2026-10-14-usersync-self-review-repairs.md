# User-Sync Self-Review Repairs

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Repair the three confirmed findings at `99c7ee1` without changing cookie integrity, privacy policy, or production timestamp precision.

**Architecture:** Keep optional Spin secrets provisionable with empty, unusable defaults. Validate declarations through the pinned vendor CLI independently of component execution. Make the routed cookie boundary fixture deterministic and isolate the duplicate-UID failure from bidder binding.

**Tech Stack:** Rust integration tests, TOML/YAML parsing, Spin 4.1.0, existing Auctioneer fake clock and CI gates.

## Spin Secret Declarations

- [x] Update `crates/auctioneer-adapter-spin/tests/manifest_usersync_cookie_keys.rs` and `manifest_health_variables.rs` to require empty secret defaults; run the targets and observe rejection of the old manifest.
- [x] Add a vendor-CLI test to the cookie manifest target. Replace only the component source with a deliberately absent local file: valid declarations must reach component loading with secrets absent or supplied. Removing either secret default must fail variable validation instead. Isolate provider environment and filesystem state; this does not qualify guest execution.
- [x] Set both defaults to `""` in `crates/auctioneer-adapter-spin/spin.toml`. Wire the CLI-dependent ignored test into `.github/workflows/ci-spin.yml`, and require that command in `crates/auctioneer-core/tests/ci_workflows.rs` with disabling/removal controls. Run manifest tests, the explicit ignored test, and CI guards.

## Deterministic Cookie Boundary

- [x] Add a fixed-clock assertion to `crates/auctioneer-core/tests/usersync_routes.rs`; confirm it fails against the live-clock fixture. Replace that fixture's wall clock with `FakeClock` at a fixed nanosecond timestamp. Keep real request-start/deadline handling and all acceptance/refusal assertions. Run the target and repeat the exact boundary case.

## Duplicate UID Guard

- [x] Add the required `bidder={{.SyncerKey}}` binding to the duplicate-UID fixture in `crates/auctioneer-core/tests/usersync_template.rs`, retaining the exact `InvalidConfig` outcome. In a temporary copy, remove only the config UID-count check: the repaired regression must fail while the unmodified code passes.

## Verification And Publication

- [x] Update `docs/deployment/usersync-cookie-keys.md` and `ISSUES.md` with the actual defaults and repairs. Record fresh evidence and qualification limits in `docs/superpowers/plans/2026-09-30-usersync-verification.md`.
- [x] Run core/reference `--all-targets`, Spin native tests, focused lint, formatting, and fixture gates. Confirm no captured reference artifact or production Rust source changed.
- [x] Commit the whole repair round, including documents, and push `user-sync`.

## Provenance Negative Controls: Round Five

The peer review targeted `99c7ee1`; this follow-up starts at `03bb984`. It adds
tests for existing verifier requirements, without changing expected outputs
or upstream attribution and without adding capture generation.

- [x] Add exact-error controls for wrong source revisions, each privacy row count, undeclared typed fields, and every GPP syntax/identity/tuple/header check, keeping all other inputs valid.
- [x] Extract private manifest parsing and attribution validation from the existing entrypoints only to inject independent malformed inputs. Exercise zero/fewer/extra captures, wrong manifest versions, and each attribution mismatch; retain compiled-in pins and the existing production call path.
- [x] Compile temporary single-guard mutations and prove the new tests fail for the targeted reason. Run the complete reference utility targets and actual-tree CLI; rerun the existing cookie-preflight allocation and flattened-map codegen guards.
- [x] Update `ISSUES.md` and the verification record with current-head dispositions and fresh evidence, then commit and push the follow-up.

# Capability Naming And Evidence Migration Implementation Plan

> **For agentic workers:** Use the executing-plans workflow, with the approved design as the boundary. Initial execution was local-only; commits and publication require separate user approval. Preserve existing GitHub roadmap tracking.

**Goal:** Replace numbered delivery labels with capability names across project-owned files while preserving behavior, test coverage and genuine evidence.

**Architecture:** Direct coordinated renames and annotation edits. Frozen payloads remain byte-identical. Only Rubicon's two local whole-manifest bindings are recalculated after a commentary-only TOML diff.

**Tech Stack:** Existing Rust/Cargo targets, shell guards, structured JSON/TOML comparisons and one-time local migration tooling; no new production dependency or reusable validation subsystem.

**Design:** [Approved contract](../specs/2026-11-02-capability-naming-and-evidence-migration-design.md).

**Tracking:** [Task #137](https://github.com/IABTechLab/auctioneer/issues/137). The user separately authorized a scoped issue and pull request on 2026-11-02. The two earlier GitHub roadmap-planning drafts remain local and are excluded from this PR.

## 1. Preserve The Baseline

- [x] Record tracked and intended-to-be-added files and their digests outside the source tree; preserve both existing untracked planning drafts.
- [x] Record a collision-free map for all 35 dated path moves, including the synchronization plan's generic delivery label, plus renamed test symbols and same-length synthetic IDs.
- [x] Preserve byte snapshots for external captures, qualification bundles/profiles, other protected capture trees, the three editable divergence documents and Rubicon's manifest/factory fixture.
- [x] Inspect `required-features`; record the renamed fanout cases and their callers before editing. Run the roster and its negative fixtures as a baseline.

## 2. Migrate Names And References

**Owners:** `docs/`, `README.md`, `ISSUES.md`, `AGENTS.md`, `Cargo.toml`, application TOML comments, `.prettierignore`, workflow comments, `scripts/check-roster{,-fixtures}.sh`, `scripts/check-batch-rebase.sh`, and matching crate source/tests.

- [x] Rename the dated plans/specs/evidence/progress paths through the reviewed map; retain dates and historical decisions.
- [x] Update project-owned references with longest-specific labels first, including document-scoped task identifiers, heading fragments, script parser labels/variables and callers. Keep distinct tasks distinct.
- [x] Rename the transient-error no-retry conformance helper and its three wrapper tests for behavior, preserving bodies/features/call counts. Update same-length synthetic request/account identifiers and secret canaries consistently.
- [x] Review remaining matches semantically. Keep hardware names and shell options. Replace stale current privacy/user-sync comments with verified present behavior; preserve dated history and readiness limitations.
- [x] Normalize historical branch and temporary-path annotations explicitly, without pretending replacement branches or commands were executed. Add notices before evidence digest tables. Preserve raw certification runner lines.

## 3. Migrate Approved Annotations And Bindings

**Owners:** privacy `tests/data/sync/{divergences,extraction-divergences}.json`, bidder `tests/conformance/fixtures/usersync/divergences.json`, privacy `tests/sync_reference.rs`, Rubicon `bidder.toml`, `tests/conformance.rs`, `fixtures/factory.json`.

- [x] Update only provenance citation paths/label wording in the three divergence documents and the exact approved-design path set in their consumer.
- [x] Replace only Rubicon's delivery-label citation in the designated `port_differences`; structurally compare TOML against the baseline before computing its new SHA-256.
- [x] Update exactly `MANIFEST_SHA256` and the factory fixture's `manifest_sha256`; leave every factory case, source digest and retained capture unchanged.
- [x] Independently compare all immutable baseline hashes, divergence cases outside permitted fields, Rubicon source data/factory cases and raw certification lines. Reject unexpected changes rather than widening the migration.

## 4. Verify And Review

Run serialized, with the locked dependency graph and bounded Cargo parallelism. Report unavailable prerequisites separately; do not refresh archives, recapture evidence or weaken assertions.

- [x] Run `bash scripts/check-roster.sh`, `bash scripts/check-roster-fixtures.sh`, and `bash scripts/check-batch-rebase.sh`; verify the same roster and negative controls.
- [x] Run `cargo run --locked --offline -j2 -p auctioneer-cli --no-default-features -- reference check --root .`.
- [x] Run `cargo test --locked --offline -j2 -p auctioneer-cli --test qualification_evidence` (default `application` feature required).
- [x] Run `cargo test --locked --offline -j2 -p auctioneer-privacy --test sync_reference` and `cargo test --locked --offline -j2 -p auctioneer-exchange --test disclosure_policy_diff`.
- [x] Run `cargo test --locked --offline -j2 -p auctioneer-core --test usersync_reference --test usersync_choose`; run the inline handler divergence consumer by its actual discovered name.
- [x] Run `cargo test --locked --offline -j2 -p auctioneer-bidder --lib rubicon` and `cargo test --locked --offline -j2 -p auctioneer-bidder --test registry_source_conformance`.
- [x] Run `cargo test --locked --offline -j2 -p auctioneer-core --features conformance-suite --test fanout_conformance`; compare the renamed no-retry cases and counts with the baseline/source inventory.
- [x] Run `cargo fmt --all -- --check`, `git diff --check`, and existing relevant source/raw-fixture checks. Check moved-file links and heading anchors; scan filenames, plain/compact labels and hidden project files for stragglers.
- [x] Review the final diff, record exact executed checks and blockers, and leave the local change uncommitted for the user. Full hosted acceptance and publication require separate authorization.

## Local Verification Record

Executed on 2026-11-02. This records naming-migration checks, not new deployment
qualification. All Cargo tests below used `--locked --offline -j2`.

| Package and selection | Result |
| --- | --- |
| CLI, `--test qualification_evidence` with default application feature | 5 passed |
| Privacy, `--test sync_reference` | 1 passed |
| Exchange, `--test config_overlay --test disclosure_policy_diff` | 31 + 17 passed |
| Exchange, `--lib response::tests::base_response_does_not_emit_debug -- --exact` | 1 passed |
| Core, `--features conformance-suite --test fanout_conformance --test usersync_reference --test usersync_choose` | 35 + 18 + 11 passed, including all three renamed no-retry controls |
| Core, `--lib handlers::tests::safe_diagnostics_are_explicit_divergences_not_reference_parity -- --exact` | 1 passed |
| Core, `--test privacy_config_parity --test privacy_outbound --test privacy_wire_signal_400 --test usersync_config_lifecycle` | 4 + 6 + 8 + 5 passed; one baked-override control left with its existing harness owner |
| Bidder, `--lib rubicon` | 119 passed; one source-object check selected separately below |
| Bidder, `--lib bidders::rubicon::tests::conformance::source_objects_match_pinned_rubicon_tree_and_all_artifacts -- --ignored --exact` | 1 passed with the pinned source Git objects supplied locally |
| Bidder, `--test registry_source_conformance` | 31 passed |
| OpenRTB, `--test prepare_integration` | 9 passed, including the renamed consent-warning control |

Total: 303 passing test executions, zero failures. The baked-override case in
`privacy_config_parity` remains owned by the existing config build-failure
harness and was not executed in this local round. No ignore or feature gate
was added, removed or weakened.

- The Rust `reference check` command verified all ten frozen captures.
- The 139-member roster and all eight roster controls passed before and after
  migration; the batch-API rebase guard also passed.
- The raw JSON gate checked 200 budgeted controls across 57 files. Workspace
  formatting and tracked/moved-file whitespace checks passed.
- Independent byte comparisons preserved 782 protected files, all measurement
  payloads/profiles and both raw certification records. Divergence documents
  changed only approved provenance annotations. Rubicon's whole-manifest
  digest changed from `4c6c0181cfd81c0ed16d3ba439ba1b6e414d7b098853bf40539231f6d779a61e`
  to `b0a7e55c0e5bee88c2c3dc3410ca7da078352d7ce5eaac6220848c454609eb1e`;
  exactly two local bindings changed, with unchanged factory cases and source
  fingerprints. The three application/workspace TOML files are structurally
  unchanged; Rubicon's separate commentary exception was checked explicitly.
- A one-time check using the existing Markdown parser scanned 189 documents
  and 1,194 local references without introducing a missing file/heading.
  Its local file/heading model still cannot resolve 191 pre-existing archival
  or imported references; the baseline had 203. Twelve obsolete sibling-
  checkout links became explicit archival citations. This is not a claim
  that every historical source citation resolves in the current checkout.
- Independent review found no behavior, coverage or evidence regressions in
  the changed Rust/config/script and provenance contracts. GitHub scheduling
  fields and assignments were not edited.

Full workspace/CI, guest/provider execution, hosted readiness, the baked-override
harness and historical qualification recapture were not run. Existing GVL
deletion notices and the dependency future-incompatibility warning remain;
warning policy and dependency pins were not changed. No commit or publication
had been performed at the end of that verification round. The later publication
approval is recorded above. The baseline and one-time migration inventory remain outside
the source tree; no new migration tool or evidence format is shipped.

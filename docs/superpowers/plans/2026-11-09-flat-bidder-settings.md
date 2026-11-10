# Bidder implementation and settings subconfig plan

> **For agentic workers:** Use superpowers:subagent-driven-development or superpowers:executing-plans. Follow the maintainer's final exact table structure.

**Goal:** Place required `implementation` beside bidder controls and retain a
separate, strictly typed `.settings` subconfig for every owner.

**Architecture:** Keep the original adjacent-tag enum and owner types, renaming
its content key to `settings`. Flatten that envelope at the shared bidder-entry
boundary with a strict private deserialization adapter that propagates errors.

**Tech stack:** Existing Rust, Serde, quote/syn and test owners; no new dependencies.

**Design:** `docs/superpowers/specs/2026-11-09-flat-bidder-settings-design.md`.

## Tasks

- [x] Add a focused generic model test using the final sibling selector/settings
  layout and observe the old entry reject the unknown `implementation` row field.
- [x] Preserve concrete bidder APIs and adjacent tagging; rename `values` to
  `settings`. Add a private strict adapter claiming only the two envelope keys.
  Return absence only when both are omitted; propagate every supplied error.
- [x] Exercise real bidder-entry JSON/TOML round trips, both key orders, duplicate
  selector/settings/common controls, partial/null/malformed pairs and unknowns.
- [x] Migrate owner/compiler fixtures, synthetic shared contracts, README, root
  reference, controlled-native guide and CLI/core examples to the exact layout.
  Preserve historical captures via strict test-boundary conversion only.
- [x] Run focused generator/owner checks, both bidder/exchange feature lanes,
  synthetic compiler/SDK allocation variants, public contracts, config
  peak/lifecycle checks, native onboarding and both Axum envelope lanes.
- [x] Run strict native and Fastly guest lint, guest publication/storage controls,
  Fastly release build, actual CLI/Viceroy adoption and both CLI resource lanes.
- [x] Update only the task-created local config, preserving its real endpoint;
  never commit it. Run format, raw JSON, source isolation and diff checks.
- [x] Complete independent review of the implementation and test migrations.
- Commit and update requested PR #160, then inspect its new CI. Publication
  and CI results are recorded on the PR. No deployment, merge or public review submission.

## Verification discipline

Keep heavy Cargo work serialized and caches intact. Freeze source edits during
CLI identity measurements. Record exact executed commands and counts. Do not
raise caps, reduce workloads or rewrite evidence. Serde buffering requires
measured qualification, not assumptions based on unchanged Rust object layout.

## Authorization

The final maintainer example requires `bidders.NAME.implementation` and nested
`bidders.NAME.settings` containing owner fields. Implementation retains its
original schema-selection/binding role. Each bidder owns and validates its
subconfig separately. Continue the already requested PR within this scope.

# Device Context And Review Closure

**Goal:** Close the confirmed device context, JSON fidelity, diagnostics, and CI
review gaps without reopening opaque identifier channels or weakening budgets.

**Design:** Keep strict typed context. Distinguish absent, explicit null, and
typed JSON values for newly classified fields; retain the JSON number form for
`pxratio`. Screen dimensions use signed 64-bit integers. Type only known
interstitial constraints and scrub unknown fields at every extension boundary.
No request-log evidence is available to justify string or float-to-int coercion.

## Execution

- [x] Reproduce integer-number, explicit-null, empty-container, and integer-width
      failures in `crates/auctioneer-openrtb/src/types/device.rs`.
- [x] Add presence-aware typed fields, classify device interstitial constraints,
      and prove actual bidder-body preservation while nested IDs stay denied.
- [x] Add the ID-allowed/geo-denied integration row using actual permissions;
      do not use a bypass that grants both permissions.
- [x] Extend structured measurement/ceiling/trigger mutation protection to
      Cloudflare and Fastly without changing size ceilings or vendor commands.
- [x] Emit bounded, input-free warnings for invalid custom price granularity
      through global and media response paths, retaining safe fallback semantics.
- [x] Strengthen redaction tests for partial secret leakage and useful key/error
      diagnosis without retaining rejected input.
- [x] Resolve negative-coordinate rounding against the explicit spec and the
      reference; retain current semantics unless a policy change is approved.
- [x] Repair the round-trip contract comment and add corpus regression shapes.
- [x] Run affected suites, full native tests, strict native/WASM lint checks,
      allocation guards, and a real Spin build with its unchanged 50,000,000 ceiling.
- [x] Commit and push verified repairs; propagate to M4, M5, and M6 with clean
      worktrees, preserving each milestone's implementation and documents.

## Verification Commands

`cargo test --locked -p auctioneer-openrtb --all-targets`

`cargo test --locked -p auctioneer-exchange --all-targets`

`cargo test --locked -p auctioneer-core --test ci_workflows --test response_gdpr_applies`

`cargo test --locked --workspace --all-targets --features conformance-suite`

`cargo clippy --locked --workspace --all-targets --features conformance-suite -- -D warnings`

Provider Clippy uses each adapter's target and `--all-targets` with
`--features conformance-suite -- -D warnings`: Cloudflare
`wasm32-unknown-unknown`, Fastly `wasm32-wasip1`, Spin `wasm32-wasip2`.

From `crates/auctioneer-adapter-spin`, run `spin build` and measure
`target/wasm32-wasip2/wasm-release/auctioneer_adapter_spin.wasm` at the workspace
root. Record measured bytes, exact revisions, failures, and verification limits.

## Evidence

- OpenRTB defines coordinate ranges and accuracy, not a privacy-rounding
  algorithm. Keep Auctioneer's documented `floor(value * 100 + 0.5) / 100` policy;
  negative coordinates and ties are explicit regression cases. The reference's
  truncation behavior is not an OpenRTB requirement.
- The OpenRTB model, preparation, allocation and JSON-corpus suites passed:
  102 tests. Presence-aware browser fields initially consumed the allocation
  margin at 512 empty entries. Lazy backing fields fix that without changing
  either the 30x measured bound or the 36x funded bound; 64, 512 and 513 browser
  rows now pass, including the vector-growth boundary.
- A real `spin build` produced a 25,328,736-byte component, below the unchanged
  50,000,000-byte ceiling. This is an artifact measurement, not a manifest-text
  assertion.
- Full Linux native verification passed: 1,828 tests in 132 target groups,
  zero failures, five ignored declarations. A subsequent full-workspace run
  excluding the independently verified config package passed 1,609 tests in
  125 groups, including the final scrub, permission, warning, JSON fidelity,
  allocation, and CI-guard changes. All 55 isolated config build-failure cases
  executed in the complete run. The slow macOS full run was interrupted and
  replaced by Linux verification, not recorded as passing.
- Strict native Clippy passed with `conformance-suite`. Each provider's strict
  WASM Clippy passed on its actual target. The additional native lint run with
  `insecure-bidders,conformance-suite` also passed.
- The real Fastly health-store/logging test passed in Viceroy using freshly
  rebuilt WASM library tests. It exercised malformed and credential-bearing
  flags, partial-secret guards, useful static diagnostics, and closed access.
- The CI guard passed 32 tests. Its skipped-upstream dependency mutation was
  first observed failing before the guard repair, then passed after repair.
  Raw-JSON and roster gates and their mutation fixtures passed; formatting,
  diff whitespace, and batch-API rebase checks passed. No size ceiling or
  allocation factor was increased. Remote CI results are not local evidence.
- M4 propagation preserved its composed privacy policies, response-reservation
  preflight, and genuine JSON encoding-error regression. The Linux workspace
  follow-up excluding the unchanged config package passed 1,924 tests in 153
  target groups; strict native Clippy, formatting, raw-JSON, roster, and
  batch-API rebase checks passed. The concurrent scrub-classification design
  was retained as documentation, not implemented as part of this repair.
- M5 propagation preserved usersync and the renamed master specification. Its
  Linux workspace run excluding config passed 2,235 tests in 172 target groups,
  zero failures and six ignored declarations; strict native Clippy, formatting,
  raw-JSON, roster, and batch-API rebase checks passed. Stale generated schemas
  in shared main and nested compile-probe caches initially broke verification;
  only disposable Auctioneer build artifacts were invalidated before the successful
  rerun. No source or test expectation was changed to accommodate those failures.
- M5's additional config unit and integration run passed 199 tests in ten
  target groups, including callback validation and usersync configuration.
  The isolated build-failure harness was not repeated downstream; all 55 of
  its cases had passed in M3b, and this repair does not change config sources.
- M6's merged Rust sources, manifests, lockfile, and workflows exactly match
  verified M5. Its own OpenRTB, exchange, and core all-target conformance run
  passed 1,298 tests in 110 groups, zero failures and one ignored declaration.
  Formatting, raw-JSON, roster, and batch-API rebase checks also passed. Its
  separate hardening spec, plan, roadmap, and architecture changes were kept.
- Propagation parents are M3b `ca19384` (repair `545ad21` plus concurrent
  design-only updates), M4 `7078810`, and M5 `5c2dfb4`. The final M6 merge
  carries these repairs and this verification record without implementing
  the separately proposed scrub-classification redesign.
- Remote M3b checks at `ca19384` failed before any runner was assigned: the
  inspected jobs have empty runner names and no executed steps. Local evidence
  does not make these remote checks green or establish a merge approval.
- A concurrent documentation-only snapshot at M3b `1b91ca4` was subsequently
  pushed and carried through M4 `b38126c` and M5 `84cc451` into M6. Source,
  manifests, lockfile, and workflows still match their verified repair trees;
  this follow-up changes only the separate classification design and records.

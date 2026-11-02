# Browser Auction Output Implementation Plan

> **For agentic workers:** Use subagent-driven-development and test-driven-development. Keep bidder-owned writes disjoint; serialize Cargo in the existing target directory.

**Goal:** Carry bounded, correlated optional browser configurations from a real
bidder through the exchange to the final wire response.

**Architecture:** Implement the approved [output contract](../specs/2026-10-25-browser-auction-output-design.md)
inside existing bidder/exchange crates. Move parsed JSON; the exchange owns
identity, correlation and output funding. Criteo supplies the first real producer.

**Tech Stack:** Existing Rust, Serde, capped sinks, sealed node-budgeted responses
and held resource receipts. No new crate, dependency, feature or runtime engine.

## 1. Review And Freeze

- [x] Review the written contract against source and current resource ownership.
- [x] Park known unfinished OpenX drafts without discarding them; PubMatic stays
      unchanged. Freeze all source inputs during red/green and publication checks.

## 2. Shared Types And Exchange

**Files:** `crates/auctioneer-bidder/src/types.rs`, `src/lib.rs`,
`crates/auctioneer-exchange/src/response.rs`, `src/run/mod.rs`,
`src/run/resource.rs`; existing constructors migrate mechanically.

- [x] Add minimal compiling type/channel scaffolding plus failing inline tests:
      empty omission, config-only output, moving a large value, multiple aliases,
      exact sent/request correlation and currency-independent output.
- [x] Observe semantic failures, then implement the move-only assembly path and
      exchange-owned canonical attribution. Preserve ordinary empty behavior.
- [x] Add independent output-name charge tests with unequal lengths/counts;
      observe their failures and implement preflight before wrapper allocations.
- [x] Run `cargo test --offline --locked -j2 -p auctioneer-bidder -p auctioneer-exchange --lib`.

Shared red runs compiled and failed on disclosure through `Debug`, omitted
configuration output, missing sent-call validation and missing escaped-identity
charges. A separate factory-driver red accepted an intentionally wrong payload
before its exact checks were implemented. The shared green run completed with
727 bidder tests and 275 exchange tests passing; two pre-existing source tests
remain explicitly driven in the publication lane. Producer and integration
proofs below are still pending.

## 3. Criteo Producer

**Files:** `crates/auctioneer-bidder/src/bidders/criteo/` only.

- [x] Prepare inline units and verbatim adjacent captures, provenance and local
      `fixtures/factory.json`; keep the refusing draft unregistered until the red slot.
- [x] Activate discovery atomically, observe nonzero semantic failures against
      a compiling stub, then implement bounded request and response transforms.

The registered refusing stub compiled, with 38 semantic failures, four passing
controls and one pre-existing-style checkout-dependent source control held for
explicit execution. The retained source artifacts were not executed.

- [x] Extract optional IGI values by moving parsed fields; preserve ordinary
      good siblings and fixed diagnostics. Do not execute external source programs.
- [x] Verify the independent captured cases and local factory case, including
      the first config with no server-side bid. Update final manifest witnesses.
- [x] Run `cargo test --offline --locked -j2 -p auctioneer-bidder --lib 'bidders::criteo::'`.

## 4. Integrated Proof And Publication

**Files:** Existing owning exchange integration/allocation targets, bidder rules,
the priority execution record and `ISSUES.md`.

- [x] Drive a real Criteo factory through exchange fanout and assert the exact
      alias/base/config wire output, mixed unsent records and config-only behavior.
- [x] Measure actual decode-to-wire allocations with independently varied
      count, bytes, nodes, call multiplicity and identity lengths under unchanged
      coefficients. Derive the response reservation from an unchanged declared
      runtime profile, exclude debug funding and preserve the existing twenty-percent
      safety margin. Include fixed bundle bookkeeping and config-only alias
      amplification; a receipt sized from the new output formula is insufficient.
      Assert pre-wrapper receipt and final-cap refusals without silent truncation.
- [x] Independently review source/spec behavior, then engineering quality.
- [x] Freeze the publication tree; run affected secure/feature suites, source/
      catalog, allocation, strict lint, formatting and three guest-library checks.
- [x] Record actual results and remaining readiness boundaries.

Publication procedure: commit and push the coherent round to the existing M7
PR after these checks. Restore parked unfinished work only after the verified
publication, not into its claim.

The first integrated run passed 17 response-allocation tests and eight owning
core serialization tests. The configuration-only grid has 144 cases with
independent aliases, count, payload size and nodes, plus 24 single-alias cases
varying call multiplicity. Fixed owners retain their separate bookkeeping
check. The pre-wrapper control measures cumulative allocations without counting
success-only serialization. A temporary delayed-check regression failed with
only 33 bytes saved against its independent 2,240-byte identity-only minimum;
the original production source was restored byte-for-byte afterward.

Independent spec and engineering reviews found no remaining code blocker after
the controls were strengthened. The initial native-capture failure was a driver
assumption about omitted headers, not a response or request transform; the
capture stays unchanged and outbound headers must still be empty. The importer
also correctly rejected empty directories left by parking OpenX; removing only
those empty directories restored all seven import tests. Immutable source and
wave suites passed 23 and nine tests respectively.

## Publication Verification

- Secure-default bidder/exchange/core all-targets: 3,332 passed across 152
  completed binaries, zero failures, five expected ignored controls. The three
  checkout-dependent bidder controls were subsequently executed: two main-source
  controls and the separate supplemental-source control all passed. The other
  two unchanged ignores belong to the documented host-driven disclosure/config
  build harnesses, not this output channel.
- Feature-enabled bidder library: 781 passed; registry conformance: 24 passed.
  Secure-default registry conformance also passed all 24 checks. Discovery and
  feature-binding probes exercised rebuilt executables rather than link-only
  checks; their source snapshots included the new factory and captures.
  The final focused Criteo run passed all 42 local cases; its separate primary-
  source control passed in the explicit source lane.
- Strict native Clippy passed all targets for bidder/exchange/core under secure
  defaults, and all bidder targets with `insecure-bidders`, with `-D warnings`.
  No lint suppression was added. The final style adjustment changes only a
  unit-test iterator; its owning library/allocation rerun passed 276 and 17
  tests respectively.
- Cloudflare, Fastly and Spin guest libraries type-checked on their respective
  existing targets. These are compilation checks, not hosted delivery or memory
  fit evidence.
- Formatting and direct discovered-source checks passed. The raw-JSON gate
  remains 172 fixtures across 52 files, with all rejection controls passing.
  Existing GVL deletion notices and the tracked `proc-macro-error2` future-
  incompatibility warning remain unchanged.

No dependency, crate, feature, deployment ceiling, workload limit, coefficient
or source revision changed. Newly retained captures match immutable source
bytes; existing captures remain unchanged. The only CI change replaces the
single-family source command with the common `source_objects_` selector, which
executes both primary-source controls without growing a bidder roster. Its
required-command guards failed before the workflow update and passed afterward.
`ISSUES.md`, bidder rules and the priority execution queue record this completed
channel without claiming public activation, full source parity, complete M7,
absolute target readiness or live provider authorization. The parked OpenX draft
remains preserved separately and is not part of this verified publication.

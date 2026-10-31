# Bidder Execution Evidence

The evidence commands implement **reference integrity, selected source/compiler
conformance and controlled-native priority acceptance**. The native scope uses
the [approved tested-reservation boundary](../superpowers/specs/2026-10-30-m7-controlled-native-completion-design.md),
not formal resource qualification, guest readiness or full-catalog migration.
They are available in the default application-enabled CLI build. Reference inventory and repair commands
remain available without application features.

## Run Ownership

Supply a fresh run ID independently of the files being checked. In CI, include
the current workflow run, attempt and commit in that ID. Locally, generate a new
ID before beginning. Use a fresh output directory outside the source tree;
writing new source inputs during a run invalidates its build binding.

```sh
export AUCTIONEER__BIDDER__SOURCE_DIR=/path/to/pinned-reference
export AUCTIONEER__BIDDER__RUN_ID=workflow-123.attempt-2.commit-COMMIT
export AUCTIONEER__BIDDER__EVIDENCE_ROOT=/private/tmp/auctioneer-evidence-new-run

cargo run --locked -p auctioneer-cli -- bidders evidence run \
  --output "$AUCTIONEER__BIDDER__EVIDENCE_ROOT" \
  --run-id "$AUCTIONEER__BIDDER__RUN_ID"
```

The pinned source must contain the immutable reference objects required by the
inventory tool. `--source` may supply its path instead of the environment
variable. The default inventory path is relative to the current directory;
run from the workspace root or pass `--inventory` explicitly.

For separate producer jobs using the same bound workspace and built CLI, begin
once, then execute all three fixed producers:

```sh
cargo run --locked -p auctioneer-cli -- bidders evidence begin \
  --output "$AUCTIONEER__BIDDER__EVIDENCE_ROOT" \
  --run-id "$AUCTIONEER__BIDDER__RUN_ID"
cargo run --locked -p auctioneer-cli -- bidders evidence produce \
  --producer reference_integrity \
  --run-manifest "$AUCTIONEER__BIDDER__EVIDENCE_ROOT/manifest.json" \
  --output "$AUCTIONEER__BIDDER__EVIDENCE_ROOT/reference_integrity" \
  --run-id "$AUCTIONEER__BIDDER__RUN_ID"
cargo run --locked -p auctioneer-cli -- bidders evidence produce \
  --producer selected_schema_source_conformance \
  --run-manifest "$AUCTIONEER__BIDDER__EVIDENCE_ROOT/manifest.json" \
  --output "$AUCTIONEER__BIDDER__EVIDENCE_ROOT/selected_schema_source_conformance" \
  --run-id "$AUCTIONEER__BIDDER__RUN_ID"
cargo run --locked -p auctioneer-cli -- bidders evidence produce \
  --producer controlled_native_acceptance \
  --run-manifest "$AUCTIONEER__BIDDER__EVIDENCE_ROOT/manifest.json" \
  --output "$AUCTIONEER__BIDDER__EVIDENCE_ROOT/controlled_native_acceptance" \
  --run-id "$AUCTIONEER__BIDDER__RUN_ID"
cargo run --locked -p auctioneer-cli -- bidders check \
  --source "$AUCTIONEER__BIDDER__SOURCE_DIR" \
  --evidence-root "$AUCTIONEER__BIDDER__EVIDENCE_ROOT" \
  --run-id "$AUCTIONEER__BIDDER__RUN_ID"
```

Beginning an existing directory or producing into an already claimed slot is
an error. An interrupted producer remains incomplete; use a new run rather
than overwriting a receipt. No ledger field can select an arbitrary executable.
The checker takes its expected run ID from its caller, not from a manifest.

## Current Producers

`reference_integrity` executes `inspect_reference_inventory`: fresh inspection
of the pinned source and complete comparison with the supplied inventory. Its
receipt records one operation, not a count of behavioral tests. It grants no
protocol, schema, constructor, privacy, target-build or resource qualification.

The run manifest binds the operation contract and current source/build inputs;
the producer checks those inputs before work and before publication. The
consumer independently checks requirements, receipt identity and current
reference integrity. Old successes, failed or absent producers, changed inputs,
unexpected members and malformed records cannot complete preparation.

`selected_schema_source_conformance` builds the owning Rust test target with
locked, offline dependencies and runs exactly two named source/compiler
membership controls. Receipts retain each successful nonzero Rust-test dispatch
and the executable digest. The consumer holds an existing-only shared cache lease
through artifact, current-source and final input-binding validation. Cooperating
producers cannot rebuild or discard that cache during consumption. It verifies
the current source prerequisite, operation set and artifact independently of
the inventory producer's receipt.
Two Rust cases do not mean two parameter fixtures, complete schema semantics,
linked-application inspection or resource qualification. Those producers remain
explicitly unavailable, and this receipt cannot authorize promotion.

`controlled_native_acceptance` executes four closed plans: generated-App
onboarding in secure and development feature lanes, cold native schema
allocation/stress acceptance, and the shipped Axum accounting/headroom check.
The onboarding case itself covers all 14 frozen flows, runtime activation,
compiled-ancestor vetoes, privacy/parameter/service refusal and applicable signed
sync callbacks. Each plan requires its exact named, nonzero passing dispatch;
an empty, failed, ignored, substituted or stale result cannot complete it.
Receipts bind each actual executable and retain the literal test transcripts.
They state `tested_only`, `formal_qualified = false` and `wasm_ready = false`.

Schema version 3 adds this native producer to the bound operation contract.
Earlier records are not reinterpreted as satisfying it. The child uses
a separate, exclusively leased Cargo cache beneath the selected target directory;
a fresh target directory is supported. Its sampled logical ceiling is 4 GiB,
100,000 entries and 64 directory levels, not a physical disk quota. Builds have
a 600-second deadline and 16-MiB output cap; each test has a 120-second deadline
and a 1-MiB native or 8-MiB source-conformance cap, within a 1,200-second run.
Failed owned work is stopped and reaped
before its leased child cache is discarded. Successful artifacts remain available
for consumption-time verification. A concurrent producer refuses rather than
sharing mutable build state.

If child cleanup cannot be proven, the producer quarantines the fixed cache's
lease before releasing ownership. Both readers and subsequent producers refuse
that cache, preserving its files. There is no automatic reset: an operator must
first establish that no owned children remain, then remove only the quarantined
cache and its adjacent lease file before starting a fresh run. A failed cleanup
never supplies completion credit.

These are local/CI execution records under the existing trusted-job model, not
signed attestations against a malicious producer or concurrent hostile
filesystem replacement. Treat the artifact directory as a trusted job output;
external cleaners must not remove it during validation.

## Exit Status And Remaining Work

`begin` and a successful individual `produce` exit zero for their own narrow
operations. `run` and evidence-consuming `check` exit zero only when all three
current producers validate, reporting `native_acceptance_complete`, 14 selected
flows, and the explicit controlled-native scope. Missing or invalid native
evidence yields `native_acceptance_incomplete` and no migrated-flow credit.
An invocation or producer failure may instead exit two with a fixed diagnostic.
`full_catalog_complete`, `formal_qualified` and `wasm_ready` remain false.
`--require-complete` cannot promote this scoped acceptance to full-catalog closure.

Complete worst-case schema/resource certification and absolute WASM readiness
remain M7-R0 work. Actual hosted/publisher execution remains M7.6. Remaining
catalog flows belong to M7.7/M20; the full-catalog unavailable list in the report
does not reinterpret native acceptance as completing those producers. Protocol
source suites and the full CI matrix remain separate required checks.

An ordinary inventory check without evidence keeps its existing behavior: it
can exit zero for an honest unfinished inventory, never certify migration.

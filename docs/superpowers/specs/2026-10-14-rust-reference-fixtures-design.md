# Rust Reference Fixture Tooling

Status: boundary and slim-checker option approved by AG on 2026-10-14.

## Boundary

All repository-owned executable reference tooling is Rust. Remove the Go
programs, embedded Go harness fragments, and their module manifests. Auctioneer's
build, tests, and fixture verification must not invoke a Go toolchain.

Keep the already captured reference outputs, source fingerprints, original
license and pixel, and capture-toolchain metadata unchanged. Those records
describe how the evidence was obtained; they are not executable dependencies.
Never regenerate reference expectations by executing Auctioneer's implementation.

Future captures from an external reference happen outside this repository.
Removing the harness deliberately retires in-repository reference recapture;
Rust verification proves artifact integrity, not that upstream was re-executed.
The prior harness was retired before the fresh-history repository import and
is not included here; see `docs/repository-import.md`.

## Tool

**Operator-command supersession:** CLI consolidation replaces the standalone
`auctioneer-reference-fixtures` package and its `validate` command with
`auctioneer reference check`, without a compatibility alias. Historical capture and
execution records retain their original tooling descriptions.

The native checker lives in `crates/auctioneer-cli/src/maintenance/reference` and
verifies externally captured test evidence. Maintenance-only CLI mode has no
dependency on Auctioneer's application/config crates or embedded archive. Use
workspace-managed dependencies and the existing strict lint configuration.

From the worktree root:

```sh
cargo run --locked -p auctioneer-cli --no-default-features -- reference check --root .
cargo run --locked -p auctioneer-cli --no-default-features -- reference check --root . --source /path/to/extracted-pinned-reference
cargo run --locked -p auctioneer-cli --no-default-features -- reference check --root /path/to/staged-worktree
```

`reference check --root <tree>` verifies compiled-in artifact paths, sizes,
digests, revision metadata and source-call provenance. The tree can be the
checkout or a staged copy containing the same relative capture paths. Optional
`--source <archive>` verifies the existing upstream source fingerprints.
There is one checker command; the retired `check-capture` command has no alias.

The capture manifest pins the original usersync and privacy reference data,
GPP header fixtures, source lock, license and pixel. Include all captured files
that the retired tools produced, not only a sample. Design divergences remain
separate reviewed Auctioneer expectations, not captured upstream results.

Fail on changed/missing captured artifacts, malformed provenance envelopes,
duplicate case IDs, wrong capture metadata, or missing source-call provenance.
Retain source-lock coverage, attribution, bounded reads and unsafe-path/symlink
refusal. Do not derive a
new lock from current files during validation or automatically bless changed
results. Reject unsafe relative paths. Report concise file-level errors, not
fixture payloads or credentials.

## Ownership

The utility owns artifact integrity and provenance only. Delete its generic
`Shape`/`typed` operation-schema engine and Rust-syntax parsing of GPP fixtures,
and remove its direct `syn` dependency. Replace `schema.rs` with a small
`provenance.rs` module for existing revision/toolchain metadata, source-lock
fingerprints, unique IDs and source-call references. Do not inspect or compute
operation-specific input/expected values.

The internal pin manifest becomes version 2 with path, SHA-256 and byte count
only. Remove schema/case-count dispatch metadata while preserving every
artifact path, digest, byte count and all ten captured files unchanged.
No staged manifest can replace compiled-in pins.

Existing core fixture consumers own cookie/URL/HTTP case shapes and expected
results. Privacy consumers own policy row counts and content; the GPP module's
compiler-checked tuples and parser tests own header validity. Add missing count,
uniqueness and fold-order checks to those owners instead of moving the deleted
schema engine wholesale. Keep mutation controls for the retained provenance
guards. Unknown operation-field validation is no longer a standalone CLI
promise: those fields remain the responsibility of behavioral consumers and
reviewed pin changes. No server/privacy enforcement behavior changes.

## Template Evidence

The existing twelve captured template cases exercise actual bidder metadata,
callback structure and round trips. Their values are form-safe; GPP and GPPSID
are empty. Preserve them rather than relabel new Rust output as reference data.

Add separately identified Rust-authored controls that vary all five privacy
macros independently with nonempty delimiter-bearing values, at the callback
and direct bidder splice points. Require exact encoded URLs and decoded value
preservation without extra query parameters. Keep historical reference behavior
and adversarial encoding coverage explicitly distinct in the README.

## Verification

Colocate unit tests with implementation. Exercise the real CLI against frozen
captures and staged changed/missing artifacts. Prove that every pinned capture
is read, traversal fails, source mismatches fail, and no command modifies data.
Run the existing Rust consumers after removing the Go tools. Add a regression
guard against reintroducing repository-owned Go programs or harness fragments.
Capture metadata and upstream source paths containing `.go` remain legitimate.

# Development And Test Ownership

## Choose The Smallest Honest Loop

Run from the active worktree. Install `.tool-versions`; source/SDK-dependent
controls also need the inputs described in their owning qualification docs.
See [Fresh Checkout](#fresh-checkout) before using offline commands.
Normal editing uses Cargo's existing incremental/cache behavior. Do not set
`CARGO_INCREMENTAL=0` or clean `target/` as a routine precondition. Run one
heavy Cargo job at a time; `-j2` is useful on memory-constrained machines.

### Fresh Checkout

`--offline` reuses dependencies that have already been fetched; it cannot
bootstrap an empty Cargo cache. Install `.tool-versions`, then acquire the
locked sources online before running offline verification:

```sh
rustup target add wasm32-unknown-unknown wasm32-wasip1 wasm32-wasip2
cargo fetch --locked
cargo fetch --locked --target wasm32-unknown-unknown --target wasm32-wasip1 --target wasm32-wasip2
```

The current EdgeZero dependency is the immutable PR #275 integration revision
`9fe6c56c648f0a601b1c6c051ce71c09d3733ea9`, not a moving branch selector.
Acquisition requires that upstream Git object to remain available. A missing
object is a source-availability failure, not a failed runtime test; do not
silently select another revision. Resolve the actual fetched checkout for
SDK-dependent tests rather than guessing Cargo's cache directory:

```sh
set -euo pipefail
pin=9fe6c56c648f0a601b1c6c051ce71c09d3733ea9
metadata=$(mktemp)
cargo metadata --locked --offline --format-version 1 > "$metadata"
manifest=$(jq -er --arg source "git+https://github.com/IABTechLab/edgezero.git?rev=$pin#$pin" '
  [.packages[] | select(.name == "edgezero-cli")]
  | if length == 1 and .[0].source == $source
    then .[0].manifest_path
    else error("expected exactly one pinned edgezero-cli dependency") end
' "$metadata")
rm "$metadata"
export EDGEZERO_SDK_ROOT
EDGEZERO_SDK_ROOT=$(/usr/bin/git --no-replace-objects -C "$(dirname -- "$manifest")" rev-parse --show-toplevel)
test "$(/usr/bin/git --no-replace-objects -C "$EDGEZERO_SDK_ROOT" rev-parse HEAD)" = "$pin"
```

Archive tests also require the pinned yq/jq tools described in
[provider-handler preparation](provider-handlers.md). Git provenance tests
require `AUCTIONEER__BIDDER__SOURCE_DIR` containing commit
`d7f8515b86258688304b0d9b6668c6a0e258bc9e` and
`AUCTIONEER__BIDDER__SUPPLEMENTAL_SOURCE_DIR` containing commit
`f1e9d0d2164e76a8bcb5f10021ebb56e00dbfe9a`. The tests read immutable objects,
not the checkout's mutable working files. Missing inputs fail instead of skip.

Historical evidence retains the revision and original bytes it exercised.
Its older SDK pin is not a current-source reproduction instruction; use the
[current SDK update record](evidence/edgezero-pr275-update-20261030.md) and
the commands above for the current tree. Fetching dependencies alone does not
requalify historical results for another executable.

### Edit

Check the package you changed, then its owning unit module:

```sh
cargo check --locked -p auctioneer-bidder --lib
cargo test --locked -p auctioneer-bidder --lib endpoint_builder::tests::
cargo test --locked -p auctioneer-core --lib config::
```

Cargo filters can succeed with zero tests. Check the dispatched names/count;
use `-- --list` when selecting a new filter. A fast filtered pass is not a
package checkpoint or milestone acceptance. Core/CLI unit binaries also have
process/socket controls; `--lib` alone does not mean every case is cheap.

For supported maintenance commands, the CLI's existing archive-independent
mode avoids linking the application graph:

```sh
cargo run --locked -p auctioneer-cli --no-default-features -- --help
```

Do not use that mode to qualify an application/resource command: it must
exercise the actual enabled application and its identity.

### Package Checkpoint

Run affected libraries and public contracts, then lint the selected targets.
For an endpoint-construction change, for example:

```sh
cargo test --locked -p auctioneer-bidder --lib
cargo test --locked -p auctioneer-bidder --test endpoint_expansion_bound --test fake_bidder_contract
cargo test --locked -p auctioneer-bidder --test test_isolation
cargo clippy --locked -p auctioneer-bidder --lib --tests -- -D warnings
cargo fmt --all -- --check
```

For shared handler setup/configuration, run the affected handler integration
targets as well as core units. Preserve individual binaries where boot gates
or other process state would interfere. Grant loopback/process access only
for tests that require it; permission failures are not correctness results.

Read `required-features` before selecting conformance: Cargo can silently
skip a target without them. Core's full conformance checkpoint includes:

```sh
cargo test --locked -p auctioneer-core --features conformance-suite --test fanout_conformance --test bidder_handler_conformance --test native_bidder_onboarding
```

Controlled-native schema allocation acceptance uses the isolated
`auctioneer-bidder --test native_schema_resources` target. Its cold pattern state and
counting allocator cannot share a process with warmed matcher unit controls.
The onboarding target similarly owns installed boot gates and the generated-App
registry; it cannot share the unfunded/custom-registry handler process. Both
preserve the ordinary inline-unit and owner-local protocol fixture convention.

Historical handler/receipt controls explicitly use `default_active_publications`
or `default_active_site_media`, preserving their existing unfunded/baseline
workload. `published()` still returns the full compiled public catalog. A cohort
control requires baseline and native-admission sets to cover it exactly once;
the funded onboarding/resources targets own the newly opt-in native paths.
Baseline tests must not be described as all-publication acceptance.

Catalog publication has a portable cross-module target. Run both feature
lanes natively and with the Wasmtime version in `.tool-versions` on `PATH`:

```sh
cargo test --locked -p auctioneer-bidder --test catalog_publication
cargo test --locked -p auctioneer-bidder --test catalog_publication --features insecure-bidders
export CARGO_TARGET_WASM32_WASIP1_RUNNER="wasmtime run --dir $PWD"
cargo test --locked -p auctioneer-bidder --test catalog_publication --target wasm32-wasip1 -- --test-threads=1
cargo test --locked -p auctioneer-bidder --test catalog_publication --target wasm32-wasip1 --features insecure-bidders -- --test-threads=1
```

Each command must execute both tests, with zero ignored or filtered cases.
The guest needs the workspace preopen to read source manifests independently.
This checks actual generated metadata, bindings and serving membership; it is
not a WASM memory-fit proof or hosted-provider qualification. Native-only
publication does not remove compiled candidate factories or declarations.

The colocated CI script also executes config's isolated `bidder_storage` test
in both WASI feature lanes. Its one cold test compares the generated census
with actual requested allocations, an independent typed heap walk and warm
access. Keep it in its own process. Baked-alias target selection is exercised
by `build_failure_harness` with real native and `wasm32-wasip1` child builds;
that target must be installed before selecting the case locally.

### Acceptance

The workflows in `.github/workflows/` remain the authoritative full matrix:
secure-default package controls, workspace/features, conformance, all adapter
guest builds/execution and resource/evidence gates. In particular, workspace
feature unification is not a substitute for secure-default package tests.
Neither an edit loop nor a native-only pass qualifies guest resource fit,
hosted deployments or the complete milestone. No CI step is removed by this
local workflow.

Workflow configuration is reviewed directly, not duplicated in Rust snapshot
tests. Substantive shell steps live beside their owning workflow under
`.github/workflows/scripts/<workflow-stem>/`; run the appropriate script or tool
to validate behavior. Inspect the actual GitHub job results separately from
local tests, including whether a runner executed any steps.

## Where Tests Belong

| Test | Owner And Layout |
| --- | --- |
| Private/pure behavior | Inline `#[cfg(test)] mod tests` in its implementation module |
| Public cross-module contract | Owning crate's `tests/<domain>/main.rs` with ordinary submodules; a small existing single-file domain target is also valid |
| Real bidder protocol | Bidder-owned `request.rs`/`response.rs` inline units; bidder-local `tests/` conformance and `fixtures/` data |
| Shared bidder infrastructure | Capable fake and synthetic metadata, never a real bidder name used as a generic stand-in |
| Published registry/catalog path | Real discovery/metadata integration; do not replace the subject of the test with a fake |
| Allocation/cold boot/process-global state | Separate binary when required by the measurement or incompatible setup; document why near the root |

Before adding a test target, name the production owner and check whether an
existing compatible domain target already owns that contract. Move a pure
unit to its source owner instead of adding another binary. Group new related
integration cases using ordinary modules, not `#[path]`, handwritten
`include!` or cross-crate private-source compilation.

Share setup, not conclusions. Fixed runtime-store setup belongs to core's
opt-in `test_support::config_store`; the existing `tests/bounded_store/`
shim supplies the same fallback to custom delayed/error/counting stores.
Callers own config contents, caps, boot installation and expected results.
Helper units compile once in their source owner. Immutable captures and
independent comparison oracles are not duplicated setup to deduplicate.

When moving tests, compare before/after names and dispatch counts, retaining
assertions, feature gates and isolation. Do not merge binaries merely to hit
a target-count goal. New helper APIs stay behind existing test-support
boundaries; do not expose private production internals for convenience.

## Build Invalidation

Bidder discovery watches directories to detect additions/deletions, including
nested fixture/model guards. The CLI's application build also binds the full
source/Git identity. These can rebuild more than an owner-local edit would
suggest. Do not drop those watches or identity inputs as a speed fix. Any
future narrowing needs a dedicated add/remove/change proof and an actual
same-command timing comparison. Keep ordinary edits out of the application
CLI graph until that graph is the thing being tested.

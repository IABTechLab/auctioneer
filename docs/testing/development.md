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
rustup component add rust-src
rustup target add wasm32-unknown-unknown wasm32-wasip1 wasm32-wasip2
cargo fetch --locked
cargo fetch --locked --target wasm32-unknown-unknown --target wasm32-wasip1 --target wasm32-wasip2
```

The field-metadata compiler tests require `rust-src` with the pinned toolchain:
their diagnostics include standard-library source excerpts. CI installs the same
component; do not regenerate snapshots to compensate for missing Rust sources.

The current EdgeZero dependency is the immutable PR #275 integration revision
`e9ffaa8f8057144155ffe164280994445fa0f6d8`, not a moving branch selector.
Acquisition requires that upstream Git object to remain available. A missing
object is a source-availability failure, not a failed runtime test; do not
silently select another revision. Resolve the actual fetched checkout for
SDK-dependent tests rather than guessing Cargo's cache directory:

```sh
set -euo pipefail
pin=e9ffaa8f8057144155ffe164280994445fa0f6d8
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
require one `AUCTIONEER__BIDDER__SOURCE_DIR` checkout containing both commits
`d7f8515b86258688304b0d9b6668c6a0e258bc9e` and
`f1e9d0d2164e76a8bcb5f10021ebb56e00dbfe9a`. Fetch either missing commit into
that same repository; a second working tree is unnecessary. Each bidder keeps
its own recorded revision, trees, blobs and digests. The tests read immutable
objects, not the checkout's mutable working files. Missing inputs fail instead
of skip. For a shallow checkout at the catalog revision, use:

```sh
git --no-replace-objects -C "$AUCTIONEER__BIDDER__SOURCE_DIR" fetch --no-tags --depth=1 origin f1e9d0d2164e76a8bcb5f10021ebb56e00dbfe9a
```

Historical evidence retains the revision and original bytes it exercised.
Its older SDK pin is not a current-source reproduction instruction; use the
[current SDK update record](evidence/edgezero-pr275-update-20261102.md) and
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
package checkpoint or capability acceptance. Core/CLI unit binaries also have
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

For unit-test edits, include `--tests` or use `--all-targets` when running
Clippy. `cargo clippy --lib` alone checks the ordinary library, not its
`cfg(test)` unit-test build; adding named integration targets does not fix that.

For shared handler setup/configuration, run the affected handler integration
targets as well as core units. Preserve individual binaries where boot gates
or other process state would interfere. Grant loopback/process access only
for tests that require it; permission failures are not correctness results.

Run Fastly's vendor tooling from the workspace root, where `fastly.toml` selects
the adapter binary explicitly:

```sh
fastly compute build --package-name auctioneer
fastly compute serve
```

The package is `pkg/auctioneer.tar.gz`. Fastly CLI 16.0.0 fails to read inherited
package metadata when invoked in the adapter directory; root invocation retains
Cargo workspace inheritance without bypassing the vendor packager.

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

Ten hermetic child-build targets have explicit owners rather than running
again in every broad `--all-targets` scope. Their `[[test]] test = false` Cargo
setting changes default selection, not availability. Run the relevant owner
explicitly when changing the compiler, discovery or config rejection behavior:

```sh
cargo test --locked -p auctioneer-bidder --test schema_build_controls -- --test-threads=1
cargo test --locked -p auctioneer-bidder --test schema_source_conformance -- --test-threads=1
cargo test --locked -p auctioneer-bidder --test factory_binding_build
cargo test --locked -p auctioneer-bidder --test registry_build -- --test-threads=1
cargo test --locked -p auctioneer-bidder --test generated_settings -- --test-threads=1
cargo test --locked -p auctioneer-bidder --test insecure_bidders_snapshot -- --test-threads=1
cargo test --locked -p auctioneer-bidder --test response_body_is_sealed -- --test-threads=1
cargo test --locked -p auctioneer-bidder --test feature_exports_source_conformance -- --test-threads=1
cargo test --locked -p auctioneer-config-data --test build_failure_harness
cargo test --locked -p auctioneer-cli --test resource_boot_parity
```

These remain mandatory on every PR. The source suite's independent adapter,
private-layout and selected-probe experiments have parallel CI owners; a local
full target still runs all of them. Source `schema_selection` feature-view tests
also run explicitly in secure-package and both workspace scopes. The existing
four normal/staged metadata/core checks retain their distinct feature graphs.
An ordinary broad Cargo pass alone is therefore not full acceptance. The local
full-source command is deliberately serial because its compiler experiments
share a build lease; CI parallelizes those experiments on separate runners.
Broad Clippy selection also omits these targets. Their mandatory owner jobs run
strict explicit `cargo clippy --test NAME -- -D warnings` checks.

The settings and synthetic-export suites select their own child features and
clear inherited environment. Running them under all three parent scopes would
repeat four fresh temporary build trees without testing a different contract.
Keep their separate fixtures, fresh processes and every original assertion;
this ownership does not authorize artifact reuse. Both workspace scopes use
`--no-fail-fast` to report later target failures while retaining a failed exit.

CI schedules the original four native graphs (secure, default workspace,
insecure workspace and standalone CLI), one heavy evidence-producer owner,
and four compiler lanes (schema, settings, exports and discovery). Only the
two closed producer cases are skipped by libtest in broad runs; Cargo roots and
features remain intact. The 17 lighter evidence cases still exercise those
graphs, with exhaustive field matrices owned by inline verifiers. The dedicated owner
enumerates the target, runs both full case names with `--exact`, and refuses
missing, ignored, zero or mismatched dispatch. Each producer still executes its
fresh child plans, including secure and development onboarding.

The existing CLI lane also owns `resource_boot_parity` explicitly. Its eight
tests build their own child feature graph, compare actual compiler feature
receipts and exercise ten profiles through forty fresh adapter boot processes.
Repeating that same recipe in outer workspace feature lanes adds no distinct
feature experiment; genuine parent-feature checks stay in the normal suites.

For a complete local producer check, use the same owner script with the pinned
source checkout available and `RUNNER_TEMP` set to a disposable directory:

```sh
bash .github/workflows/scripts/test/verify-evidence-producers.sh
```

The ordinary feature predicate stays in `registry_source_conformance`; moving
child compiler suites does not move that predicate out of the parent graphs.
Large schema diagnostic reports are opt-in with
`AUCTIONEER__TEST__SCHEMA_DIAGNOSTICS=true`; assertions and artifact inspection
do not depend on that flag.

### CLI Socket Isolation

The joint [#168](https://github.com/IABTechLab/auctioneer/issues/168) /
[#174](https://github.com/IABTechLab/auctioneer/issues/174) owner covers six
inline CLI cases under `qualification::harness::`: protocol EOF/closed port
and malformed-frame cleanup in `fixture::protocol::tests`, delay/stop join,
both socket-capacity populations and header-timeout EOF/recovery in
`fixture::server::tests`, and exact-port release/rebind in
`native::system::tests`. Their names, feature gates, complete bodies and
assertions stay with those owners; the [cohort script](../../.github/workflows/scripts/test/verify-cli-socket-isolation.sh)
pins their full selectors. This is not isolation of every socket test or a
full macOS CLI qualification.

Each parent delegates before constructing resources to the private test-only
`socket_test_support` helper. It invokes `std::env::current_exe()` directly
with the closed full selector, `--exact`, `--test-threads=1`, `--nocapture`
and `--color=never`; no shell, Cargo child build or guessed executable is used.
The child environment is cleared and receives only
`AUCTIONEER__TEST__FIXTURE_SOCKET_CASE`. The helper immediately retains the
`std::process::Child` before fallible setup, and validates the child's exact
single-case success report. Selected child bodies spawn no native descendants;
their existing Tokio tasks remain. Only children are single-threaded: the
parent suite is not serialized and does not set `RUST_TEST_THREADS`.

The fixed test-only bounds are a **20-second execution watchdog** anchored
before spawn, a separate **2-second kill/reap fence**, and **16 KiB per stream**
in private regular-file captures. The deadline is observed, not preemption of
synchronous OS calls, and does not extend any original inner wait. Post-reap
reads are limited to cap plus one byte to detect overflow; diagnostics retain
only bounded child stderr, never child stdout or nested libtest summaries.
Lengths are monitored, not an OS disk quota: a writer may overshoot between
observations. Finite overflow controls use cap-plus-one payloads.

All wrappers directly await the helper. Returning success/refusal requires
observed direct-child reap followed by checked capture removal; cleanup-fence
failure reports that cleanup was not established. Parent panic, cancellation,
runtime teardown or process termination is outside that guarantee: drop only
requests best-effort termination, not verified reap or capture removal. Do not
spawn, externally time out or abandon the helper future, or treat it as a
general process-tree supervisor.

The existing Linux CLI lane runs the finite checker controls before its full
CLI suite, without adding a runner or repeating the Cargo test suites:

```sh
bash .github/workflows/scripts/test/check-cli-socket-isolation-fixtures.sh
```

The existing full CLI step uses `verify-cli-socket-isolation.sh full-cli` to
retain the same all-targets command and its two existing producer skips. Cargo
must succeed; then the captured output must contain one exact success for
each of the six cases and 19 independently required helper controls. Missing,
renamed, ignored, failed or duplicate owned cases are refused. Other targets
and unrelated ignored tests remain governed by the full Cargo invocation.
This checks the existing run, not a second compilation or test suite.

The cohort script remains available for separately authorized focused
verification in either graph:

```sh
bash .github/workflows/scripts/test/verify-cli-socket-isolation.sh default
bash .github/workflows/scripts/test/verify-cli-socket-isolation.sh workspace-insecure
```

The checker first enumerates each locked CLI binary graph, refuses duplicate
or missing names, and selects all six cases plus every discovered helper test,
including 19 independently required helper names. It runs one parallel parent
process per graph with multiple exact selectors, requires one matching summary
and one success per selected name, and counts every other listed test as
filtered. Finite fixtures exercise the same selection/report functions without
Cargo and do not print synthetic libtest reports. The optional cohort command
retains its roster, output and exit statuses under `RUNNER_TEMP`. Existing full
Linux suites execute all six wrappers and the helper controls with their
unchanged feature selections. Rust comes from `.tool-versions`; only Cargo
downloads are cached.

On 2026-11-15 the maintainer declined macOS GitHub runners because of cost.
There is no added macOS job or proposed required-check setting. Existing Linux
checks remain mandatory. The current delivery uses GitHub verification only;
no local script or Cargo execution was authorized. macOS execution is deferred
and unverified, not established by Linux results or historical diagnostics.
That limitation remains explicit for #174 and PR #162's acceptance decision.

All workflows share one registry/Git download cache. Compiled outputs are not
restored or saved across jobs: the whole-workspace archive exhausted runners
during restoration, before any tests executed. Cargo still reuses compilation
products within each job. Toolchain-action implicit caching is disabled so it
cannot restore a second compiled cache. Save-after-failure retains dependency
downloads, not observations or passing test results. Nested scratch workspaces
and guest artifacts keep their existing fresh-build ownership.
Every job and long acceptance step has an explicit safety limit. New cache
footprint and whole-CI timing still require complete hosted measurement.

The workflows in `.github/workflows/` remain the authoritative full matrix:
secure-default package controls, workspace/features, conformance, all adapter
guest builds/execution and resource/evidence gates. In particular, workspace
feature unification is not a substitute for secure-default package tests.
Neither an edit loop nor a native-only pass qualifies guest resource fit,
hosted deployments or the complete capability. No CI step is removed by this
local workflow.

Workflow configuration is reviewed directly, not duplicated in Rust snapshot
tests. Substantive shell steps live beside their owning workflow under
`.github/workflows/scripts/<workflow-stem>/`; run the appropriate script or tool
to validate behavior. Inspect the actual GitHub job results separately from
local tests, including whether a runner executed any steps.

## Where Tests Belong

| Test                                      | Owner And Layout                                                                                                           |
| ----------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| Private/pure behavior                     | Inline `#[cfg(test)] mod tests` in its implementation module                                                               |
| Public cross-module contract              | Owning crate's `tests/<domain>/main.rs` with ordinary submodules; a small existing single-file domain target is also valid |
| Real bidder protocol                      | Bidder-owned `request.rs`/`response.rs` inline units; bidder-local `tests/` conformance and `fixtures/` data               |
| Shared bidder infrastructure              | Capable fake and synthetic metadata, never a real bidder name used as a generic stand-in                                   |
| Published registry/catalog path           | Real discovery/metadata integration; do not replace the subject of the test with a fake                                    |
| Allocation/cold boot/process-global state | Separate binary when required by the measurement or incompatible setup; document why near the root                         |

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

When removing a redundant case, name the retained test that owns its guarantee.
Keep exhaustive field matrices beside their verifier and retain representative
real CLI/socket/compiler checks for wiring and error mapping. Independent
oracles are distinct coverage, even when they share inputs or similar code.

## Build Invalidation

Bidder discovery watches directories to detect additions/deletions, including
nested fixture/model guards. The CLI's application build also binds the full
source/Git identity. These can rebuild more than an owner-local edit would
suggest. Do not drop those watches or identity inputs as a speed fix. Any
future narrowing needs a dedicated add/remove/change proof and an actual
same-command timing comparison. Keep ordinary edits out of the application
CLI graph until that graph is the thing being tested.

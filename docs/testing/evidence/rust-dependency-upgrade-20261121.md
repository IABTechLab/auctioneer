# Rust Dependency Upgrade, 2026-11-21

This records the user's request to refresh the Rust packages and repin EdgeZero
PR #275. It follows [#165](https://github.com/IABTechLab/auctioneer/issues/165)
and the [approved dependency-upgrade contract](../../superpowers/specs/2026-11-11-rust-dependency-upgrade-design.md).
The source baseline is `e9ad25a0f9979fb22ac4ff5d1aca03dce11030de`.
Earlier upgrade records, source archives and independent captures remain historical.

## Selection And Source Ownership

The registry selection was frozen from the 2026-11-21 `cargo upgrade --dry-run
--incompatible allow --pinned allow --recursive false` and `cargo update
--dry-run --verbose` audits. The baseline contains 468 resolved package
identities. Broad compatible requirements are refreshed in `Cargo.lock`;
project-owned exact requirements need an explicit selection. No prerelease,
toolchain, provider tool, workload limit or resource ceiling is selected here.

All six EdgeZero workspace requirements and eight resolved SDK packages advance
from `e9ffaa8f8057144155ffe164280994445fa0f6d8` to official
[PR #275](https://github.com/IABTechLab/edgezero/pull/275) head
`ea2549fb13728debf5afc34da2e70593061aa05f`, as returned by GitHub at selection.
The 20-commit delta repairs retained response failures, revocation cleanup,
Fastly response/progress ownership and CLI target selection. Its workspace
dependency requirements are unchanged. The active codec/source guard, SDK
locator, provider/qualification identities and current reproduction instructions
use the same immutable revision. Updating those identities does not recertify
earlier resource or hosted measurements.

## Final Resolved Inventory

[`rust-dependency-inventory-20261121.tsv`](rust-dependency-inventory-20261121.tsv)
accounts for all 468 resolved package identities, including their source,
baseline versions, latest non-yanked stable registry release observed at
selection, current disposition, independent change classification and actual
resolved immediate owners' requirements.
Local source identities are workspace-relative. The graph retains the same
package names and count: 40 crate versions advance, including 39 reaching the
observed latest stable release and Redb advancing from 4.5.0 to 4.6.0 within
the SDK's required major while latest 5.0.0 remains constrained. Eight Git SDK
instances are repinned, 306 external instances were already current and 16 are
workspace members. The 99 behind-latest instances include the upgraded Redb;
the change and currency classifications are separate. No
baseline package name disappears. This combined target/build/development graph
is not the deployed dependency closure or a memory-fit proof.

The exact root changes are `jsonschema` / patched `jsonschema-macros-core`
0.62.0, Regex 1.13.2 / patched engine 0.4.19 / syntax 0.8.12, YAML Rust2
0.13.2 and Trybuild 1.0.125, alongside the six SDK requirements. Broad
requirements retain their existing declared ranges and pick up compatible
lockfile updates. The excluded vendors' original manifests/lockfiles remain
published-source provenance; they are not independently rewritten application
resolution graphs.

Final `Cargo.lock` SHA-256:
`ae849e777f8216f983112f631168bf585c4604076381d265459249d62f88df38`.

## Retained Constraints

The selected SDK still owns exact shared requirements. Their future coordinated
upgrade belongs to [EdgeZero #467](https://github.com/IABTechLab/edgezero/issues/467).
This is not an all-latest graph. The SDK is not patched locally to force its
provider or decoder dependencies forward, and the single-copy codec guard is
preserved.

| Family | Selected | Latest stable observed | Reason retained |
| --- | --- | --- | --- |
| Fastly provider family | 0.13.1 | 0.13.2 | Exact SDK/provider companion tuple. |
| Worker | 0.8.5 | 0.9.0 | SDK provider types and exact requirement. |
| Bytes | 1.12.1 | 1.12.2 | Exact shared SDK requirement and asserted ingress tuple. |
| HTTP | 1.4.2 | 1.5.0 | Exact SDK requirement and shared boundary types. |
| Serde JSON | 1.0.150 | 1.0.154 | Exact SDK requirement and parser audit tuple. |
| URL | 2.5.8 | 2.5.9 | Exact SDK requirement. |
| SDK Reqwest | 0.13.4 | 0.13.5 | Exact native adapter requirement. |
| Maintenance Reqwest | 0.12.28 | 0.13.5 | Preserve both clients' existing TLS policies; see below. |
| Brotli | 8.0.4 | 9.1.3 | SDK decoder tuple and single-copy codec guard. |
| Brotli decompressor | 5.0.1 | 6.3.2 | Exact SDK decoder requirement. |
| Async compression | 0.4.43 | 0.4.52 | Exact SDK decoder requirement. |
| Compression codecs/core | 0.4.38 / 0.4.32 | 0.4.48 / 0.4.34 | Exact SDK decoder requirements. |
| Flate2 | 1.1.9 | 1.1.11 | Exact SDK decoder requirement. |
| CRC32fast | 1.5.0 | 1.5.5 | Exact SDK requirement. |
| Miniz oxide / SIMD Adler32 | 0.8.9 / 0.3.9 | 0.9.1 / 0.3.10 | Exact SDK decoder requirements. |

Moving maintenance to Reqwest 0.13.4 with `native-tls` would unify features with
the SDK's Rustls client. Reqwest's
[`TlsBackend::default`](https://github.com/seanmonstar/reqwest/blob/v0.13.4/src/tls.rs#L621)
selects native TLS when
that feature is present, while the SDK's
[`try_transport`](https://github.com/IABTechLab/edgezero/blob/ea2549fb13728debf5afc34da2e70593061aa05f/crates/edgezero-adapter-axum/src/outbound.rs#L135)
uses an unqualified
`Client::builder`. That would silently change the bidder transport's TLS
backend. Using only the new defaults would instead change maintenance TLS.
Preserving both existing policies requires an explicit SDK backend selection
before unifying the client cohorts; 0.13.5 also conflicts with the SDK's exact
requirement. The maintenance timeout, redirect, gzip and bounded-read behavior
remain unchanged.

Other older transitive versions remain under their resolved owners' compatible
major requirements; they are not forcibly replaced across an upstream API.

## Vendor Rebase

The compiler uses the authentic 0.62.0 archive (298 published members), with
all three type-check, initialization and explicit-cache patches rebased and
replayed independently. The engine uses the authentic 0.4.19 archive (107
members); its resource-layout patch applies unchanged. Current archive/VCS,
portable-diff and patched-member identities are in [vendor provenance](../../../vendor/README.md).
Original earlier archives, versioned patches and snapshots remain available;
published new snapshots are preserved rather than recaptured from patched output.

Compiler 0.62.0 enables a new evaluation method by default. Auctioneer's staged
boolean-only declarations explicitly set `evaluate = false`, including the
source-bound options descriptor. The existing exact generated-declaration
control first failed (five passed, one failed), then the full config command
passed 424 cases after the explicit method choice. The explicit-cache patch
refuses opted-in evaluation and keeps the new reference memo state in its
owned optional context rather than using runtime ambient memo helpers.

Regex syntax's Unicode table files are byte-identical across this upgrade.
The maintained parser/matcher repairs include repeated-parser reset,
leftmost-first prefix lifting and empty/boundary search fixes. The engine's
default pool capacity changes to eight for stock/unset constructors;
Auctioneer's explicit cache builders and selected custom hostname owners
already select one. The isolated hostname allocation controls pass on the
native host; complete physical/guest resource qualification is not certified
by these controls. Existing analytic
bounds, workload and storage ceilings are retained, including conservative
boundary cases that the new meta engine no longer selects.

## Verification

The baseline and first selected SDK graph each passed:

```sh
cargo test --locked -p auctioneer-core --test codec_pins --test upstream_primitives --test edgezero_fanout
```

Both executions dispatched 2 codec/source cases, 5 upstream primitive cases and
2 fanout cases, with zero failures. These initial checks precede the final
vendor selection and are not final-graph qualification.

The following additional commands passed on the selected SDK graph before the
vendor rebase. All use `--locked` and the existing shared incremental target:

| Command | Actual passed cases | Result summaries |
| --- | --- | --- |
| `cargo test --locked -p auctioneer-macros --all-targets` | 26 | 2 |
| `cargo test --locked -p auctioneer-config --features build-support --lib --test root_shape` | 424 | 2 |
| `cargo test --locked -p auctioneer-cli --no-default-features --all-targets` | 259 | 9 |
| `cargo test --locked -p auctioneer-adapter-axum --test cancellation --test wave_transport` | 17 | 2 |
| `cargo test --locked -p auctioneer-core --test ingress_admission --test egress_binding` | 37 | 2 |

All five commands exited 0 with no failures or ignored cases. The maintenance
lane initially exited 101 because `AUCTIONEER__BIDDER__SOURCE_DIR` was absent;
that command did not pass. After supplying the existing read-only reference
checkout containing both documented immutable objects, the complete command
above passed. No test was skipped or changed to repair the prerequisite.
Application-gated CLI tests are outside this maintenance-only command's scope.

### Final-Graph Controls

The updated generated-declaration control was run before changing the emitted
method choice: five passed, one failed, 413 filtered out. After the compatibility
change, `cargo test --locked -p auctioneer-config --features build-support
--lib --test root_shape` exited 0 with 419 library and five root-shape cases.

```sh
cargo test --locked -p auctioneer-bidder --test schema_compiler_controls --test schema_cache_controls --test schema_initialization --test schema_cache_initialization --test schema_cache_bounds -- --quiet
```

Exit 0: **460 passed**, zero failed/ignored/filtered, across five targets:
193 compiler, 17 cache API, two initialization, one cache initialization and
247 bound cases. These cover the new evaluation refusal and owned memo cases.

Additional source-owner checks used `cargo test --locked -p auctioneer-bidder
--test <target> <filter> -- --test-threads=1 --quiet`, each exiting 0:

| Target | Filter | Passed | Filtered out |
| --- | --- | --- | --- |
| `schema_build_controls` | `schema_regex_vendor` | 7 | 323 |
| `schema_build_controls` | `portable` | 3 | 327 |
| `schema_build_controls` | `layout_api_preserves` | 1 | 329 |
| `schema_source_conformance` | `tests::native_layout` | 2 | 690 |
| `schema_source_conformance` | `tests::fake_regex_resource_layout_public_api` | 1 | 691 |
| `schema_source_conformance` | `tests::hostname_fresh_cache_and_guard_observations_fit_the_narrowed_owner_bound` | 1 | 691 |
| `schema_source_conformance` | `tests::schema_pool_heap_source_statics_and_actual_clone_requests_reconcile` | 1 | 691 |
| `schema_source_conformance` | `schema_syntax_construction::tests` | 11 | 681 |

These **27 filtered cases** preserve their independent analytic bounds and
source checks. They are not the full compiler/source producer/conformance suite.

The CI-owned foundation/adapter `cargo check --locked -j2` selections passed
for Cloudflare `wasm32-unknown-unknown`, Fastly `wasm32-wasip1` and Spin
`wasm32-wasip2`. Each selected `auctioneer-runtime`, `auctioneer-openrtb`,
`auctioneer-config-data` and the actual target adapter. All exited 0. These are
compilation results, not executed guest tests or provider packages.

Both final dependency audits exited 0: compatible update reports **zero**
package changes; requirement upgrade reports only the five documented holds
(Bytes, Fastly, Brotli, Reqwest and Worker). The lock SHA above is unchanged.

Formatting (including separately discovered bidder sources), the raw-JSON gate
(203 fixtures across 56 files) and its refusal controls, batch rebase, roster
and roster-refusal controls, all 18 libtest-summary controls, shell syntax and
`git diff --check` passed. The complete commands use their existing workflow
scripts, not replacement tests.

Independent read-only specification review verified the complete TSV against
frozen metadata and replayed both vendor source sets. It found and corrected
an initially undercounted Redb upgrade and incomplete parent requirement joins;
the current artifact separates change/currency and joins resolved kind/target
and published library aliases. The review ran no builds or tests.

Independent read-only code review found no correctness or code-quality defect
in the rebase, memo ownership, phase integration, generator or source bounds.
It ran no builds or tests. The first strict workspace Clippy run exited 101 on
four `default_numeric_fallback` diagnostics in two newly added test loops;
subsequent commands in that serial batch did not run. The fix types the loop
counters without changing their 12 iterations. All 17 cache controls and their
strict target lint passed again after that test-only correction.

The complete strict rerun then exited 0 for both commands:

```sh
cargo clippy --locked -j2 --workspace --all-targets -- -D warnings
cargo clippy --locked -j2 -p auctioneer-bidder --test schema_build_controls --test schema_source_conformance -- -D warnings
```

Final-graph native SDK contracts also exited 0:

```sh
cargo test --locked -j2 -p auctioneer-core --test codec_pins --test upstream_primitives --test edgezero_fanout --test ingress_admission --test egress_binding
cargo test --locked -j2 -p auctioneer-adapter-axum --test cancellation --test wave_transport
```

The first command executed 46 cases across five targets; the second executed
17 across two targets. Both had zero failures, ignored cases and filtered cases.
These runs bind the SDK source/codec tuple, terminal admission/response ownership
and real-socket cancellation/wave behavior to the final graph.

Local storage pressure prevented running the complete workflow matrix
locally. The run's task-owned repair-build sandbox and completed test
executables were removed after passing results to recover space; shared
libraries and incremental caches were retained.
The full workflow feature/compiler/producer/conformance matrix, executed guests,
provider packaging/size gates and complete resource qualification are **not run**
for this candidate. They remain required through the existing workflow owners;
native tests and guest compilation do not prove hosted fit or deployment
readiness. This record describes local verification; hosted CI status belongs
to the carrying PR.

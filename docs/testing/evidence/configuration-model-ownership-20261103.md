# Configuration Model Ownership Verification

Date: 2026-11-03. Scope: the approved ownership amendment inside PR #140,
not completion of the operator-runtime configuration cutover.

## Ownership

- `auctioneer-config` declares shared source, policy, override, currency and
  effective models, with optional native build support and no build script.
- `auctioneer-config-data` generates compiled defaults, metadata, disclosure
  tables and reference archives through its existing build script.
- `auctioneer-bidder` generates settings and endpoint bindings from the complete
  discovered catalogue. Type generation does not promote a factory.
- Core composes the shared generic model with discovered settings, performs
  semantic admission/resolution and adapts the model to SDK traits.

Cargo metadata still contains 16 workspace packages. The model library has no
dependency on the data package, bidder, core or EdgeZero. Maintenance-only CLI
checking passed, and its normal dependency tree contains the model library but
not the data package, core, bidder or provider adapters.

## Executed Checks

All Cargo commands used the locked, offline dependency set, the existing shared
target directory and two build jobs. Heavy Cargo commands were serialized.

| Command Selection | Result |
| --- | --- |
| Pre-move `cargo test -p auctioneer-core --lib` | 698 passed |
| Model-only `cargo test -p auctioneer-config --lib --test root_shape --no-default-features --features config-model` | 158 library and 12 root-shape cases passed |
| `cargo test -p auctioneer-config --lib --features build-support` | 373 passed |
| Post-move `cargo test -p auctioneer-core --lib` | 611 passed |
| `cargo test -p auctioneer-config-data --lib --test codegen --test baked_defaults_snapshot --test usersync_config --test usersync_callback_validation` | 3 / 74 / 2 / 5 / 6 passed respectively |
| Complete CI secure-default data selection, excluding the separately owned refusal harness | 206 passed across the library and 12 integration targets |
| Core `auction_config_retention`, `config_funding`, `dependency_purity`, `resolve_runtime_config`, `runtime_config_failclosed`, `usersync_config`, `usersync_routes` | 2 / 1 / 1 / 2 / 19 / 9 / 22 passed respectively |
| Core `config_parse_factor`, `host_alias_overlay_bounds`, `usersync_handler_alloc` | 18 / 11 / 8 passed; existing independent bounds unchanged |
| `cargo check -p auctioneer-cli --no-default-features` | Passed; no compiled-data build |
| CLI `config_source::tests` under normal application features | 9 passed; 2 existing ignored subprocess probes |
| Bidder library, `catalog_alloc`, `registry_join`, `registry_source_conformance`, `test_isolation` | 1,192 passed / 9 existing ignored; 8 / 21 / 32 / 16 integration cases passed |
| Shared model library under `wasm32-wasip1`, executed with Wasmtime 45.0.0 | 158 passed |
| Cloudflare guest `cargo check --target wasm32-unknown-unknown` | Passed |
| Cloudflare, Fastly and Spin guest `cargo clippy` on their respective guest triples with `-D warnings` | Passed |
| Native model/data/bidder/core `cargo clippy --lib --tests -- -D warnings` | Passed without added allowances |
| Complete `generated_settings` compiler-owner run | 24 outer and 26 core-child cases passed; no filters or ignores |
| Explicit data and bidder compiler-owner Clippy targets with `-D warnings` | Passed; no added allowances |
| Exact synthetic settings/endpoint consumer after helper lint repairs | Passed, including all five child commands; other 23 outer cases filtered explicitly |
| `bash scripts/check-raw-json-fixtures.sh` | 205 fixtures across 57 files, unchanged total allowance |

## Case And Byte Preservation

The before/after executable lists reconcile all 698 full baseline core test
identities: 608 remain in core and 90 execute in the model library. Three new
SDK-wrapper tests account for the post-move core total of 611. The relocation
inventory separately covers all 99 affected model/configuration cases: 90 moved,
nine retained. Leaf names alone were not used to establish this reconciliation.

All 41 files relocated from the old compiled-data directory retain their exact
Git blob hashes. Historical qualification artifacts and external captures have
no content changes. Default snapshots still pass with their existing expected
values. Public configuration status headers retain their original spelling.

## Negative Control

Temporarily removing settings' nested `Validate` delegation executed three
supplied-settings tests and failed all three, without compiler errors. They
cover a standalone entry, the source root and the versioned policy root. The
restored implementation passed the complete model-only suite. Omitted settings
are separately unvisited, and the synthetic settings type does not implement
`Default`. Errors use exact fixed nested codes without rejected input values.

The mutation was fully restored. Because restoring the source preserved its
older timestamp, the first post-restore model invocation reused the mutation
binary. A byte comparison confirmed correct source; refreshing only that source
timestamp forced the rebuild. The recorded green model/root results are from
the rebuilt restored source, not the stale binary.

## Acceptance Boundary

The full hosted CI matrix, provider execution and the entire data build-failure
sweep have not been claimed as local passes. The mandatory CI owners remain in
place; focused execution is not a substitute for those checks before merging.
The complete compiler-owner run preceded lint-only repairs to its diagnostic
reader and structured fixture writer. The exact synthetic consumer was rerun
after those repairs; the full unfiltered owner remains mandatory in CI.

The parent plan's bounded startup/input funding review, complete semantic host
resolution, serving reader/CLI cutover and same-binary/two-config acceptance
remain required. The pre-existing CLI/SDK double structural-validation pass
must be removed at that cutover without weakening validation or changing error
contracts. This ownership change does not establish provider memory fit or
close the configuration epic.

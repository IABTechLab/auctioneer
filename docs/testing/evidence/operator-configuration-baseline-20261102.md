# Operator Configuration Implementation Baseline

Date: 2026-11-02

Source base: `3b1b6d929bc99b800459fdafb3080655b8c77c29`.
Design: [operator configuration](../../superpowers/specs/2026-11-02-operator-runtime-configuration-design.md).
Plan: [implementation](../../superpowers/plans/2026-11-02-operator-runtime-configuration.md).

This record precedes implementation source edits. Local documentation changes
were present; the executable/configuration source was unchanged from the base.
It records selected existing guarantees, not workspace or deployment acceptance.

## Executed Baseline

Cargo 1.98.1 was selected by `.tool-versions`. Commands ran in the isolated
implementation worktree, serially, with `--locked --offline -j2` and the existing
shared Cargo target directory. No targets were cleaned, incremental settings
changed, source pins updated or test-harness bypass enabled.

| Command selection | Executed result |
| --- | --- |
| `cargo test -p auctioneer-config-schema --test root_shape` | 4 passed, 0 failed; 9.47 seconds build/run command |
| `cargo test -p auctioneer-core --lib config::` | 134 passed, 0 failed, 475 filtered; 54.96 seconds build/run command |
| `cargo test -p auctioneer-config --test baked_defaults_snapshot --test usersync_config` | 2 + 5 passed, 0 failed; 23.78 seconds build/run command |
| `cargo test -p auctioneer-core --test resolve_runtime_config --test runtime_config_failclosed --test config_funding` | 2 + 19 + 1 passed, 0 failed; 5.23 seconds build/run command |
| `cargo test -p auctioneer-cli --bin auctioneer-cli config_source::` | 9 passed, 0 failed, 2 child-only probes ignored in the parent selection, 744 filtered; 67 seconds build/run command |

The CLI's two environment-overlay parent cases spawn their named ignored probe
children in controlled processes. Ignoring those children in the ordinary
parent selection is existing isolation, not a disabled failure.

The plan originally selected `auctioneer-cli --lib config_source::`. That
command dispatched zero cases and is **not** a successful authoring baseline.
Inspection found `config_source` belongs to `src/main.rs`, not `src/lib.rs`;
the plan now selects the binary owner shown above. That correction preserved
all tests and assertions. The library-only attempt took 81 seconds building
its selected graph; it supplied no config-source execution evidence.

Existing warnings were vendor-list deletion notices and a future-incompatible
`proc-macro-error2` notice. Neither was introduced, suppressed or repaired by
this baseline. The locked archive was fresh enough for these builds; it was
not refreshed as an incidental configuration change.

## Migration Surface

An `rg` inventory over `crates`, `scripts`, `.github` and `auctioneer.toml`
found 117 files matching at least one of: `AuctioneerConfigSource`,
`AuctioneerAppConfig`, product-default accessors, `RUNTIME_BOOTSTRAP`, the
retired default/bootstrap path overrides, or `baked_config_bytes`.
This is a lexical migration set, not 117 production readers or a failing-test
count. Captures and frozen resource fixtures within it remain immutable.

The concrete owners are:

- Config/schema build source, typed root, defaults snapshots, generated syntax,
  alias validation and build-failure controls.
- Core policy DTO/resolution/budgets, captured extractor settings, gates,
  status/info/auction/sync consumers and their lifecycle/funding/allocator tests.
- CLI config-source projection, application commands, resource reports,
  qualification input bindings and fresh boot/CLI parity controls.
- All four adapter resource rows and boot tests, the native browser-config
  example and native privacy/transport process tests.
- Synthetic generated-settings child workspaces and their exact dispatch
  reconciliation; exchange/response/account tests that construct effective DTOs.

The approved field inventory remains the leaf-disposition checklist. Source
search results are not authority to rename historical evidence, delete a test,
replace immutable outputs or remove a resource summand.

## Not Established

No new strict policy, bidder validator, startup loader or snapshot constructor
is qualified by these pre-edit results. No full workspace, compiler-owner,
guest, live-provider or integrated-host acceptance is claimed. New startup
reservations still require the plan's separate review before adoption.

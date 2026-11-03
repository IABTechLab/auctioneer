# Local Development And Test Ownership Verification

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

Date: 2026-10-30. Base: `6f62fc77`, branch `bidder-catalog`.
This covers the approved first slice, not complete bidder catalog acceptance.

## Case And Target Inventory

| Former Integration Target | Cases Moved Inline | New Owner |
| --- | ---: | --- |
| bidder `endpoint_builder` | 10 | `src/endpoint_builder.rs` |
| core `account_resolution` | 18 | `src/config/resolve.rs` |
| core `usersync_alias_limit` | 6 | `src/config/mod.rs` |
| core `outbound_limits` | 2 | config resolution and validation units |

All 36 cases passed before relocation and occur by name in the passing new
unit runs. Assertions and inputs remain unchanged; independent read-only
review checked the moves. Cargo metadata reports exactly these four removed
targets and no added target: 243 to 239, core 99 to 96, bidder 30 to 29.

## Executed Checks

Cargo used `--locked --offline -j2`, Rust 1.98.1, existing source/SDK inputs
and normal incremental state. No profile/dependency/CI changes were made.

| Scope | Passed | Failed | Existing Ignored |
| --- | ---: | ---: | ---: |
| core library, including three new fixture units | 599 | 0 | 0 |
| 24 handler/setup integration targets below | 124 | 0 | 0 |
| bidder library | 1161 | 0 | 9 |
| `endpoint_expansion_bound` | 12 | 0 | 0 |
| `fake_bidder_contract` | 17 | 0 | 0 |
| `test_isolation` | 16 | 0 | 0 |
| Total | 1929 | 0 | 9 |

The final core command selected its library and these targets:

```sh
cargo test --locked --offline -j2 -p auctioneer-core --lib \
  --test auction_budget_concurrency \
  --test auction_reuse \
  --test boot_resource_charges \
  --test bounded_path_in_the_real_handler \
  --test clock_domain \
  --test config_error_precedence \
  --test handler_resolution \
  --test handler_wave \
  --test health_gvl_observer \
  --test health_gvl_stale_observer \
  --test impression_limit_on_the_auction_path \
  --test mid_fanout_code_12 \
  --test node_budget_on_the_auction_path \
  --test normalized_numeric_source \
  --test original_request_extension \
  --test privacy_outbound \
  --test privacy_wire_signal_400 \
  --test probe_pending_reads \
  --test response_account_resolution \
  --test response_gdpr_applies \
  --test response_mid_flight_timeout \
  --test response_oversize_response \
  --test runtime_config_failclosed \
  --test usersync_routes
```

Bidder verification:

```sh
cargo test --locked --offline -j2 -p auctioneer-bidder --lib \
  --test endpoint_expansion_bound --test fake_bidder_contract --test test_isolation
```

Strict Clippy passed for the same core library/24 targets and bidder
library/three targets, using `cargo clippy` and `-- -D warnings`.
`cargo fmt --all -- --check`, `git diff --check` and the raw-JSON fixture
gate passed; the latter reports 192 fixtures across 55 files.

During repair, compilation caught a transitive `usersync_app` dependency
on the existing bounded-store shim; its root declaration was retained.
Strict lint caught missing fixture API docs and parameter-style `impl Trait`;
these were fixed without new allowances. Final checks supersede those failures.

## Timing And Boundaries

A cached invocation of
`cargo test --locked --offline -j2 -p auctioneer-bidder --lib endpoint_builder::tests::`
ran 21 cases in 0.68 seconds wall time (`/usr/bin/time -p`). This is one
warm observation, not a cold-build benchmark or an old/new speedup claim.
The earlier unchanged runtime library check took 1.24 seconds; it is a
different command, not a comparison baseline for the endpoint loop.

Fixed store setup has one source owner behind existing test-support.
The 21 context-helper and three direct consumers retain their configs,
assertions and process isolation. Custom-store fallback semantics are
unchanged. Public endpoint and real-handler controls remain external.

Full workspace CI, feature/guest matrices, provider execution and resource
qualification were not rerun for this test-only slice. Existing GVL
deleted-vendor warnings and `proc-macro-error2`'s future-incompatibility
notice remain. Discovery watches, source identity, historical captures
and bidder catalog publication/readiness requirements are unchanged.

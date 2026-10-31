# Controlled Native Completion Verification

Date: 2026-10-30. Worktree: `m7-bidder-catalog`.

## Scope

This verifies the [approved native amendment](../../superpowers/specs/2026-10-30-m7-controlled-native-completion-design.md),
not complete worst-case resource certification, WASM production readiness or
live bidder authorization. New priority publications are native-only and
inactive by default. Existing guest publications are unchanged.

The resource reservations were frozen before these runs: 16 MiB resident,
8 MiB initialization and 512 KiB sequential scratch per auction. Workload
limits, deployment ceilings, parameter schemas and the additive equation were
not raised or relaxed to pass. The portable generated-App fixture is not the
shipped Axum memory profile; the latter has a separate accounting test.

## Reproduction

Commands below use Rust 1.98.1, locked offline dependencies, two build jobs,
the pinned immutable source checkout in `AUCTIONEER__BIDDER__SOURCE_DIR` and its
supplement in `AUCTIONEER__BIDDER__SUPPLEMENTAL_SOURCE_DIR`. SDK source inspection
uses `EDGEZERO_SDK_ROOT` at the lockfile pin `0a04ddb0`.

```sh
cargo test --locked --offline -j2 -p auctioneer-bidder --lib
cargo test --locked --offline -j2 -p auctioneer-bidder --lib -- --ignored
cargo test --locked --offline -j2 -p auctioneer-core -p auctioneer-exchange -p auctioneer-runtime --lib
cargo test --locked --offline -j2 -p auctioneer-config-schema --lib
cargo test --locked --offline -j2 -p auctioneer-config --test bidder_bindings --test bidder_storage --test codegen
cargo test --locked --offline -j2 -p auctioneer-bidder --test registry_source_conformance --test response_envelopes --test test_isolation
cargo test --locked --offline -j2 -p auctioneer-bidder --test schema_source_conformance tests::schema_selection_
cargo test --locked --offline -j2 -p auctioneer-bidder --test native_schema_resources -- --nocapture
cargo test --locked --offline -j2 -p auctioneer-core --features conformance-suite --test native_bidder_onboarding
cargo test --locked --offline -j2 -p auctioneer-core --features conformance-suite,insecure-bidders --test native_bidder_onboarding
cargo test --locked --offline -j2 -p auctioneer-adapter-axum --lib
cargo test --locked --offline -j2 -p auctioneer-core --test ci_workflows
cargo test --locked --offline -j2 -p auctioneer-cli --bin auctioneer-cli bidders::
cargo test --locked --offline -j2 \
  --config 'profile.dev.package.sha2.opt-level=3' \
  --config 'profile.test.package.sha2.opt-level=3' \
  -p auctioneer-cli --test bidders_evidence -- --test-threads=1
```

Socket, FIFO and supervised-process controls require an environment allowing
those local operations. Sandbox refusal is not a passing result.

The unoptimized aggregate CLI run was interrupted for runtime cost during
receipt controls, not counted as passing. The replacement command above keeps
every test/assertion and optimizes only its SHA-256 dependency; no repository
profile is added. The evidence producer's fixed child-build environment and
resource thresholds remain unchanged.

## Resource Observations

The cold process observed 47,944 resident bytes, 27,073 temporary initialization
bytes and a 75,017-byte combined initialization peak; cumulative initialization
allocations were 271,714 bytes. These observations are not upper-bound proofs.

The native-schema census covered 14 bindings / 13 owners, 149 captured source
rows (five lexical refusals), 120 authored rows and 564 stress rows. Alternating
inputs and repeated fresh caches produced 2,526 validation attempts, with a
50,846-byte maximum per-call peak and 73,149-byte cumulative allocation maximum.
Seven owners have no captured parameter table; their authored/stress coverage
is explicitly distinct from source-capture evidence. Selected legacy schemas
add 360 attempts (26 captured rows, one lexical refusal, 25 authored rows and
70 stress rows). Not every attempt reaches a regex matcher.

The source-only NFA/OnePass construction models are preparatory R0 controls,
not actual target-layout certification or production acceptance.

## Final Acceptance

Completed scoped runs:

| Scope | Passed | Failed | Notes |
| --- | ---: | ---: | --- |
| Bidder library | 1177 | 0 | Nine pinned-source cases ignored in this ordinary run |
| Explicit pinned-source bidder cases | 9 | 0 | The nine ignores above executed separately |
| Core / exchange / runtime libraries | 607 / 283 / 251 | 0 | Includes schema deadline/work and scrub regressions |
| Config build support library | 259 | 0 | Includes native/guest generation controls |
| Config binding / storage / codegen | 3 / 1 / 74 | 0 | Exact generated metadata fields retained |
| Registry source / response envelopes / source ownership | 28 / 22 / 16 | 0 | Source-fatal versus candidate-local controls |
| Selected source membership | 3 | 0 | Includes omission and ordering controls |
| Secure / development generated-App onboarding | 8 / 8 | 0 | All 14 flows and 300 privacy dispatches in each lane |
| Cold native schema reservations | 1 | 0 | Observations and census above reproduced after test refactor |
| CLI bidder/evidence units | 292 | 0 | Includes inherited-descriptor and quarantine regressions |
| Real CLI evidence integration | 18 | 0 | Three producers, four native plans and receipt refusal controls |
| Axum shipped profile library | 23 | 0 | Includes the actual four-GiB envelope and 256-MiB headroom |
| Workflow guards | 124 | 0 | Exact feature-bearing acceptance commands remain required |

Repeated suites and alternate feature lanes are not summed into a unique-test
headline. The source construction-model subset also passed 32 controls; those
remain R0 preparation, not target-layout certification.

All 18 aggregate CLI cases passed, including actual execution of the three
code-owned producers and all four native plans. The replacement integration
run completed in 946.41 seconds after its 55.10-second build. The optimized
hashing control completed an evidence `begin` in 0.78 seconds; it is a tooling
observation, not an application latency or resource qualification result.

Strict Clippy passed with `-D warnings`, `--all-targets` and
`--features auctioneer-core/conformance-suite` across bidder, exchange, core, CLI,
config-schema, runtime and Axum. Formatting and whitespace checks passed.
The roster gate accounts for 139 members; the raw-JSON gate accounts for
200 fixtures across 57 files. Their negative controls and the batch-rebase
gate also passed during this round.

Fresh `cargo check --locked --offline -j2 --lib` runs passed for Cloudflare
on `wasm32-unknown-unknown`, Fastly on `wasm32-wasip1` and Spin on
`wasm32-wasip2`, using each respective adapter package. These checks do not
authorize guest deployment or certify memory fit.

This closes the approved local controlled-native M7 acceptance, not the
retained requirements below. No historical capture was recertified or rewritten.

## Retained Limitations

- Complete schema residency, initialization, scratch, work and stack proofs and
  absolute WASM fit remain M7-R0 requirements.
- Guest library builds are compilation checks, not guest execution, deployment
  package-size or physical memory qualification.
- Hosted/provider authorization, live failure/recovery and publisher rollout
  remain M7.6. Trusted Server embedding remains M7.5.
- Additional bidder waves remain M7.7/M20; native priority acceptance never
  sets `full_catalog_complete`, `formal_qualified` or `wasm_ready` true.
- The existing dependency future-incompatibility notice remains tracked in
  `ISSUES.md`; the archived-vendor build notices are not new failures.
- These scoped local runs do not claim execution of the complete remote CI
  matrix or credentialed deployments.

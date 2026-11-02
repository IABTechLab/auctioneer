# Catalog Cost Aggregation Controls

Date: 2026-10-20
Branch: `m7-bidder-catalog`, based on `a97fc586`
Scope: the scalar aggregation boundary in Task 4, not completed qualification.

## Ownership

`auctioneer-bidder/src/catalog.rs` is the single checked join for component cost
projections. Metadata, validators, registry and context each require one record,
including a reviewed explicitly empty contribution. The caller supplies expected
source/qualification-record digests independently of those records. All records
must match the expected ABI, feature-set digest, toolchain digest and lock digest.
Unknown expected facts, missing/duplicate owners and stale records refuse.

Every owner supplies a strictly increasing stream of storage identities. A
four-way merge counts shared identities once only if their complete resident,
initialization, validation and context costs agree. Separate storage with equal
byte costs is not shared. The sum uses checked arithmetic for each lifetime;
overflow and ambiguity return an error, never a partial total. It applies no
auction/auxiliary population or baked-default multiplier: those remain the
existing runtime equation's responsibility.

The aggregator borrows inputs and allocates no heap. It cannot construct a
bidder, access metadata or warm a lazy cache. It has no file reader, signature
protocol or qualification evaluator. Build/evidence owners must verify the actual
record-to-projection binding, genuine storage sharing and all-path bounds before
publishing scalar projections. An arbitrary matching digest is not evidence.

No qualified production owner records, serving consumer or new live catalog
allocation are installed. Provider profiles retain `CatalogResourceCosts::UNCHANGED`.
Metadata's preceding requested-storage census is not converted into a physical
resident bound. Generated/schema/cache/registry bounds, aggregate precharging,
auxiliary accounting, post-build artifact proof and live cutover remain open.
Neither M7 nor R0 readiness is established by this increment.

## Controls

The first compiling stub run failed all twelve new aggregation unit tests; sixteen
existing registry tests selected by the same filter passed. Controls independently
vary each owner and lifetime, all common build facts, owner source/record digests,
missing/duplicate owners, ordered/unique identities, sharing conflicts and each
overflow. Empty records remain distinct from missing records. Additional controls
check maximum-valued shared storage without intermediate double charging and
identities differing at every byte position.

The dedicated integration binary delegates to the System allocator with
thread-local synchronous intervals. Positive controls observe allocation,
deallocation, zero-filled allocation and reallocation before testing zero work.
Prebuilt records vary owner order, disjoint/pairwise/all-shared storage and input
size; success and refusal are measured separately from fixture construction.
Literal expectations are independent of production arithmetic. These controls
prove scalar mechanics, not complete registry or schema allocation bounds.

A deliberate 64-byte allocation inserted at the aggregation entry caused the
integration test to fail with one allocation, one deallocation and a 64-byte peak.
The probe is not part of the implementation. The workflow command guard also
failed before the allocator target was added to the real CI invocation.

## Verification

The RED logs are `/private/tmp/m7-catalog-aggregate-red.log`,
`/private/tmp/m7-catalog-alloc-red.log` and
`/private/tmp/m7-catalog-aggregate-ci-red.log`.

Rust is `1.98.1 (48a229cea 2026-09-01)`. Final selected scopes report zero
failures and zero ignored tests; totals below are per invocation, not disjoint
workspace totals.

| Scope                                                                                                            | Result                                               |
| ---------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------- |
| Secure bidder library, allocator, generated join, discovery, feature snapshot and isolated factory-build targets | 198 passed: 159 + 1 + 5 + 24 + 3 + 6                 |
| Same selection with `insecure-bidders`, without repeating the isolated factory-build target                      | 203 passed: 170 + 1 + 5 + 24 + 3                     |
| Core library and complete workflow target                                                                        | 606 passed: 503 + 103                                |
| Workspace all-target Clippy, `insecure-bidders,conformance-suite`, `-D warnings`                                 | Passed                                               |
| Bidder guest-library compilation, three ABIs and two feature modes                                               | Six configurations passed                            |
| Formatting, diff, raw-JSON, roster and batch-rebase checks                                                       | Passed; 168 raw fixtures across 50 files, roster 139 |

Bidder tests use `cargo test --offline --locked -j2 -p auctioneer-bidder --lib
--test catalog_alloc --test registry_join --test discovery_codegen --test
insecure_bidders_snapshot`, with `--test factory_binding_build` in the secure
run and `--features insecure-bidders` in the feature run. The existing Unix
socket fixture needs local host permissions; no deployed service is accessed.
The isolated factory driver completed in 51.99 seconds. Logs are
`/private/tmp/m7-catalog-aggregate-bidder-final-{secure,insecure}.log`.

Core uses `cargo test --offline --locked -j2 -p auctioneer-core --lib --test
ci_workflows`, logged in `/private/tmp/m7-catalog-aggregate-core.log`.
Final lint uses `cargo clippy --offline --locked -j2 --workspace --all-targets
--features insecure-bidders,conformance-suite -- -D warnings`, logged in
`/private/tmp/m7-catalog-aggregate-clippy-workspace-final.log`.

Guest commands use `cargo check --offline --locked -j2 -p auctioneer-bidder --lib
--target wasm32-unknown-unknown --target wasm32-wasip1 --target wasm32-wasip2`,
independently with and without `--features insecure-bidders`. Logs are
`/private/tmp/m7-catalog-aggregate-guest-{secure,insecure}.log`. These are library
compile checks, not linked package, guest execution or physical-memory evidence.

Initial lint/type/format findings were repaired without weakening tests or
adding suppression; the allocator alone retains its necessary narrow unsafe
exception. Existing GVL deletion notices and the tracked `proc-macro-error2`
future-compiler warning remain. Spec and fresh quality reviews approved the
bounded increment through static inspection; parent verification is separate.
No full workspace test run, hosted CI result, deployment, completed Task 4 or
M7/R0 readiness is claimed. Workload limits, dependency pins, original captures
and the additive runtime equation are unchanged.

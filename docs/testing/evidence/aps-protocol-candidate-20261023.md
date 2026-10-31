# APS Protocol Candidate

Scope: the ordinary Rust bidder contract and local controls, not metadata
promotion, catalog completion or provider qualification. The supplemental
[source contract](../../superpowers/specs/2026-10-23-aps-priority-integration-design.md)
retains the immutable source revision, approved SDK identity and null policy.
No public APS metadata binding or serving schema validator is installed.

## Implementation And Ownership

- `auctioneer-bidder/src/bidders/aps/` owns the stateless factory, bounded sink
  construction, borrowed request serializers and sealed response decoding.
  Registration uses its adjacent declaration, not a central production match.
- Inline `aps::tests` owns 22 independent controls and protocol assertions.
  The native `aps/conformance.rs` owns 16 source protocol cases and capture
  integrity/source checks. Supplied bytes must match their pinned witness
  before any protocol dispatch, even for a filtered case. All 21 captured
  artifacts live in adjacent `fixtures/` and remain byte-identical after
  relocation, including the license and source witnesses.
- Existing AppNexus, Generic, PubMatic and Mocktioneer retain all 40 original
  inline unit tests and 94 assertions, unchanged. APS follows the same inline
  style for its borrowed request view and units. No routine `request.rs` or
  `tests.rs` split was introduced. The shared test harness owns fixture
  registration and real-sink driving, not individual bidder copies.
- `auctioneer-bidder/tests/bidder_request_allocation.rs` owns process-level
  allocation instrumentation for both transforms. Shared catalog allocation,
  discovery and join controls remain in their existing integration targets.
- Core's colocated private-handler tests own nine original-extension/region
  combinations and eight independently varied activity/gzip combinations.
  They inspect real finalized calls and successful decoded bids, not only
  constructor existence or a no-bid path. Test fanout is not live networking.

The allocation matrix varies caller payload across 64, 8,192 and 262,144 bytes,
body caps across 0, 32, 1,024 and 32,768 bytes, and test mode independently:
24 cases. Its literal independent bound covers two body capacities plus the
existing metadata/header allowance. This is a control for the fixed source
endpoint, not qualification of arbitrary template/query scratch.

## Remaining Delivery Gates

The source catalog remains 358 identities. The APS factory is an unused
candidate in the validated join, not a new enabled public identity. Complete
schema/constructor/cache/work and target resource bounds, metadata/media
admission, live registry cutover and full declared producer/cancellation
acceptance remain open. In particular, these handler fixtures do not establish
the still-needed shared site/app/media capability gate. Hosted authorization
and provider acceptance remain separate; no credential policy was invented.

No dependency, SDK pin, workload cap, resource ceiling or budget equation
changed for this candidate or layout cleanup.

## Verification

Selected native results, counted separately rather than as distinct tests
across repeated feature runs:

| Command Scope                                                                        | Result                                                   |
| ------------------------------------------------------------------------------------ | -------------------------------------------------------- |
| Secure bidder library, including the explicit immutable-source check                 | 273 passed, no failures or ignored tests                 |
| Feature-enabled bidder library, including the same source check                      | 284 passed, no failures or ignored tests                 |
| `bidder_request_allocation`, secure and feature runs                                 | Seven passed in each run                                 |
| `catalog_alloc`, secure and feature runs                                             | Five passed in each run                                  |
| `registry_join`, secure and feature runs                                             | Six passed in each run                                   |
| Core library, including actual APS raw/gzip/privacy handler controls                 | 513 passed                                               |
| `registry_build` with real incremental build/run lifecycle                           | Nine passed; 16 child invocations                        |
| Workspace/all-target Clippy with `insecure-bidders,conformance-suite`, `-D warnings` | Passed                                                   |
| Core library check for `wasm32-unknown-unknown`, `wasm32-wasip1`, `wasm32-wasip2`    | All three passed; no guest execution or memory-fit claim |

The bidder commands select `--lib --test registry_join --test catalog_alloc
--test bidder_request_allocation`. Both supply `AUCTIONEER_APS_SOURCE` pointing to
the provided immutable source repository and `-- --include-ignored`; the
feature run additionally enables `insecure-bidders`. All Cargo commands use
`CARGO_INCREMENTAL=0`, `--offline --locked -j2`. Existing GVL deletion notices
and dependency future-incompatibility notices remain; no lint suppression was
added to pass the new code.

Two actual single-bypass controls failed behaviorally and were restored:

- Bypassing only the conformance wrapper's integrity call makes its same-length,
  semantically unchanged JSON whitespace control fail. A standalone digest
  helper test alone would not establish this before-dispatch ordering.
- Renaming an existing inline unit module while retaining its tests makes the
  declaration-driven layout conformance fail for that implementation.

The four original bidder files were compared byte-for-byte with the previous
commit, retaining all 40 tests and 94 assertions. Every relocated capture was
also compared with its original Git blob, not merely re-pinned after moving.

The first lifecycle attempt correctly refused a newly colocated capture license
as an unexpected serving-workspace input. A focused copier control reproduced
that refusal before the fix. Serving probes now exclude only
`src/bidders/<key>/fixtures/`; a positive control still copies ordinary runtime
fixtures, implementation code and declarations. The source file allowlist and
budgets were not widened. The repaired full lifecycle passed in 81.19 seconds,
reporting 699,493,583 peak logical scratch bytes, not physical allocation.

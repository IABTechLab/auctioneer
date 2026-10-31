# Catalog Count And Usersync Storage Controls

Date: 2026-10-19
Branch: `m7-bidder-catalog`, based on `e1437d53`
Scope: M7 Task 10 and the request-local storage needed to honor its charges.

## Independent Owners

Runtime owns five separate literal constants. The shipped tuple is asserted
independently as `(catalog, priority entries, requested names, filter names per
list, debug rows) = (512, 256, 256, 256, 256)`. Priority groups remain 32 and
each runtime alias overlay remains 64. This is not a global replacement of
256 with 512. Unequal synthetic field values also guard the budget mapping.

Config uses that catalog owner in its shared library/build-script validator.
The two actual build owners are metadata count and metadata plus baked host
aliases. Core validates the merged baked/runtime host aliases against the
complete compiled count. An explicit borrowed-catalog entrypoint permits
independent boundary fixtures without replacing process-global registries.

The catalog boundary accepts 511 and 512 identities and refuses 513 with the
catalog diagnostic. Caller controls independently accept 255/256 and refuse
257, including duplicate requested names, two filter lists, debug insertion
without requested-name validation, and priorities split across groups.

Catalog-only growth from 256 to 512 adds exactly **65,536 chooser bytes**.
Coefficients, workload declarations and the additive total are unchanged.
The Axum derived auxiliary receipt changes from 58,580,992 to 58,646,528;
its declared 134,217,728-byte slot and **2,533,631,672-byte** isolate total
remain unchanged. The corresponding unused slot margin is 75,571,200.

## Request-Local Ownership Repair

The first full-size allocation control failed: a 512-entry, 128-byte-name
catalog peaked at 303,978 requested-heap bytes during construction and
403,050 through choosing, against the relevant 131,072/132,480-byte charges.
The subsequent cookie-priority control exposed a 239,248-byte projection
peak against 131,456 bytes. These failures were not repaired by increasing
coefficients, limits, or unrelated config/privacy/signing allowances.

The catalog now stores compact sorted entries borrowing names, static metadata
and families. ASCII-folded binary lookup preserves case-insensitive lookup
and last-insertion alias precedence. Candidate ordering and seen-family sets
borrow existing strings; only actual lookup/output values are copied.
`/setuid` resolves priority families through the same catalog index rather
than constructing another complete owned map. Disabled configured families
remain represented, and unconfigured families remain excluded.

The final allocator fixture injects one real existing family plus baked-style
aliases, independently of the changing shipped roster. Fixture setup and
privacy preparation are outside the measured windows. It varies catalog
counts 128/256/512, name lengths 8/64/128, lowercase/mixed-case names, and
fallback/requested/requested-debug/cooperative calls with output limit one.
All **72 cases and 216 stages** must execute. Each stage uses only its own
independently computed literal charge; positive allocator controls prevent
a zero-measurement pass. These are native requested-heap observations, not
physical-memory or guest-allocation qualification.

The final 512-entry, 128-byte mixed-case cooperative row reports:

| Stage                             | Requested-heap peak | Relevant charge |
| --------------------------------- | ------------------: | --------------: |
| Catalog construction              |              49,152 |         131,072 |
| Catalog through choosing          |              50,064 |         132,480 |
| Catalog through family projection |              49,152 |         131,456 |

The initial regression used the shipped registry; the final fixture makes
its one-family registry explicit. Both stress 512 total identities and
128-byte aliases, but they are not byte-identical fixture populations.

The routed fixture separately retains 256 requested/debug rows and a real
257-name refusal. Small, 193-entry and full 512-entry synthetic populations
exercise fixture construction, including 128-byte compiled names, zero alias
headroom and reversed input order. Actual generated-App dispatch still runs
against the real compiled registry. This does not claim a synthetic registry
was installed in that App.

## Mutation Ownership

Each of the four caller constants was independently widened to 512:

- Its runtime 257-entry refusal test failed; the other three passed.
- The separately invoked literal shipped-profile test failed.
- Its real core chooser/filter/debug consumer test failed; the other three
  named consumer controls passed. The priority mutation also failed two
  additional priority-owner controls.

Each of the six chooser accounting summands was independently removed:
catalog, requested names, both filters, priority entries, debug rows and
priority groups. Every mutation failed its corresponding owner test.

Removing the metadata build invocation failed its exact child-build test:
513 metadata identities incorrectly reached a separately proved malformed
defaults sentinel. Thus the later defaults-count guard cannot mask it.
Removing the defaults build invocation separately let 449 compiled names
plus 64 baked aliases build, failing that owner's exact child-build test.
Both restored build controls passed. Removing error propagation from the
runtime validator failed three catalog-boundary tests while its other
eight overlay tests passed.
No mutation is retained in the implementation.

## Executed Tests

All commands used the locked dependency graph, offline mode and two Cargo
build jobs. Counts below are per invocation, not disjoint totals.

| Scope                                                                                    | Result                                           |
| ---------------------------------------------------------------------------------------- | ------------------------------------------------ |
| Runtime library                                                                          | 194 passed                                       |
| Config all targets, secure defaults                                                      | 487 passed, including all 85 build-harness tests |
| Both restored catalog child-build owners, isolated rerun                                 | 2 passed                                         |
| Core library and 14 affected integration targets, secure defaults                        | 679 passed                                       |
| Same core targets with `insecure-bidders`                                                | 679 passed                                       |
| Final secure boundary/allocation/routes fixture rerun                                    | 41 passed                                        |
| Bidder library and insecure-default snapshot target                                      | 132 passed                                       |
| Actual Axum boot-envelope target                                                         | 25 passed                                        |
| Workspace all-targets Clippy with `insecure-bidders,conformance-suite` and `-D warnings` | Passed                                           |
| `cargo fmt --all -- --check` and `git diff --check`                                      | Passed                                           |
| Raw-JSON fixture gate                                                                    | 168 fixtures across 50 files; passed             |

Each invocation reported zero failures and no ignored tests in these scopes.
This is not a complete workspace test run. The feature-enabled run is
separate from config's secure-default invocation, not a replacement for it.
Guest builds/execution were not rerun in this scoped increment.

Reproduce core's affected scope with:

```sh
cargo test --offline --locked -j2 -p auctioneer-core --lib \
  --test usersync_handler_alloc --test usersync_routes --test usersync_choose \
  --test usersync_catalog --test usersync_precedence --test usersync_reference \
  --test usersync_wire_reference --test usersync_handlers --test usersync_flow \
  --test usersync_integrity --test usersync_config_lifecycle \
  --test usersync_alias_limit --test host_alias_overlay_bounds \
  --test catalog_resource_profile
```

Repeat with `--features insecure-bidders`. The allocation target's exact
`tests::catalog_and_chooser_peak_is_funded_with_limit_one_and_small_callers`
test accepts `-- --exact --nocapture` to print all numeric rows. For build
guard controls, filter `build_failure_harness` with
`sync_config_child_build_catalog_`; its existing bounded child driver owns
compilation, timeout failure and fixture/target cleanup. Local invocation
logs use `/private/tmp/m7-counts-*`; they are diagnostic scratch, not
committed qualification artifacts.

## Verification Boundary

The complete compatibility, work/cache/concurrency and target-resource schema
qualification remains open. This increment neither cuts over the strict live
registry join nor promotes another bidder. Ordered shared info/auction/sync
indexes and their comparison-count acceptance remain M7 Task 11, beyond this
request-local allocation repair. R0 absolute fit and provider facts remain
explicit readiness blockers; no historical evidence is recertified here.

The pre-existing GVL deletion notices and `proc-macro-error2` future-compiler
warning remain. No new ignored test, lint suppression, budget coefficient,
deployment ceiling, or toolchain/SDK pin was introduced.

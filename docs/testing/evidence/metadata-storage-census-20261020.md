# Generated Metadata Storage Census

Date: 2026-10-20
Branch: `m7-bidder-catalog`, based on `6303f3e4`
Scope: config-owned preparation for M7 Task 4, not full storage qualification.

## Contract And Limits

The config build analyzes the same rendered Rust constructor expressions that
it emits into `bidders()`. A closed `syn` expression grammar counts decoded
UTF-8 string payloads, nonempty strings, typed string/media/sync vector elements
and nonempty vectors. Unsupported calls, methods, macros, vector element types,
conditional fields or struct defaults refuse generation instead of receiving
zero cost. No arbitrary expression is evaluated by this analysis.

The generated constant getter is available without materializing metadata. Its
source SHA-256 binds the actual constructors, complete binding/declaration source
and declaration count, using versioned, decimal-length-prefixed framing. Active
metadata/bindings follow the compiled feature mode; declarations include inactive
identities. Same-length content changes are tested against the digest itself,
not merely against a changed field elsewhere in generated output.

`MetadataStorageDescriptor` performs checked arithmetic using target-side type
sizes. Requested heap includes the outer bidder buffer, owned UTF-8 payloads and
typed nested buffers. Allocation counts exclude zero-size strings/vectors.
Logical static layout includes only the lazy cell, binding/declaration arrays
and their static slice-reference slots.

These are requested allocation and logical-layout counts, not physical resident
bounds. Allocator overhead, literal pooling/alignment, executable/code residency,
the accessor's own code/data, complete generator/helper/ABI/toolchain/lock
qualification identities, schema/cache storage and registry storage remain open.
The source hash is not a qualification record. No aggregate `CatalogResourceCosts`
is derived or installed here. Existing serving constructors, workloads, limits,
accounting coefficients and SDK/toolchain pins remain unchanged.

## Independent Controls

Runtime arithmetic tests first failed against zero-returning stubs: zero passed,
three failed. Six syntax-census controls then failed against an empty-census stub.
The real generated integration test failed against an empty generated descriptor
before wiring actual constructors. No compile error or zero-match run counts as
RED; an invalid two-filter Cargo invocation was corrected before the helper run.

The final helper controls cover unequal nested populations, decoded escapes,
empty allocation behavior, exact unsupported-expression errors, mixed vector
types, source-digest sensitivity and each checked counter's overflow without
partial publication. Runtime controls independently vary each size/population,
the active versus complete row counts, empty storage and each overflow path.

The integration binary contains exactly one test so no prior test warms the
metadata cell. Its thread-local System delegate has a separate positive control
with known allocations, deallocation, reallocation and retained/peak totals.
The resize control separately pins its conservative old-plus-new peak. Static access is
measured before first metadata access; the cold constructor's actual requested
bytes, allocation count, retained bytes and peak all match the census, with no
transient buffer or reallocations. Warm access requests nothing and returns the
same slice. An independent typed model walk, with no rest patterns, checks every
owned field and capacity after the interval; it imports no generator expectations.

Native measurements from the current three/four-definition catalog:

| Feature mode       | Active definitions | Requested heap bytes | Allocations | Logical static-layout bytes |
| ------------------ | -----------------: | -------------------: | ----------: | --------------------------: |
| Secure defaults    |                  3 |                1,895 |          23 |                         352 |
| `insecure-bidders` |                  4 |                2,403 |          28 |                         384 |

Both retain four complete metadata declarations. Descriptor/static-table access
and warm metadata access allocate zero bytes in both modes. Measurements are
requested native allocations, not guest or provider memory observations.

## Verification Ownership

CI's secure-default config step and its feature-enabled compensation step both
name the new allocator target. The secure-default directory/set-equality guard
also requires it, and exact command guards retain failure propagation. The
workflow control first failed before these commands were added.

Commands use Rust 1.98.1, locked/offline dependency resolution and two build jobs.
Native logs are `/private/tmp/m7-metadata-config-full-secure.log` and
`/private/tmp/m7-metadata-config-final-insecure.log`. The allocator prints its census
and source digest outside the measured interval so the retained log is auditable.
RED logs use the `layout-red`, `constructor-red`, `materialization-red` and
`ci-red` suffixes under `/private/tmp/m7-metadata-*.log`.

Final results below are per invocation, not disjoint totals. All selected test
scopes report zero failures and zero ignored tests.

| Scope                                                                                   | Result                                                                              |
| --------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| Complete secure-default config non-harness CI scope                                     | 444 passed across the library and twelve integration targets                        |
| Config library, codegen, binding, allocator and smoke targets with `insecure-bidders`   | 208 passed: 49 + 123 + 20 + 1 + 15                                                  |
| Core library and complete workflow target                                               | 606 passed: 503 + 103                                                               |
| Isolated factory-build controls plus generated registry join                            | 11 passed: 6 + 5; five intended refusals, five recoveries, four executable controls |
| Config harness metadata boundary target                                                 | One passed, 91 filtered; 511/512 accept and 513 refuses                             |
| Config guest-library compilation in three ABIs and two feature modes                    | Six passed                                                                          |
| Strict workspace all-target Clippy, `insecure-bidders,conformance-suite`, `-D warnings` | Passed                                                                              |
| Formatting, diff, raw-JSON, roster and batch-rebase checks                              | Passed; raw-JSON count remains 168 across 50 files                                  |

The secure command is CI's complete `auctioneer-config` non-harness command plus
`--offline -j2`. Feature-mode checks use `--lib --test codegen --test
bidder_bindings --test bidder_storage --test smoke --features insecure-bidders`.
Core uses `--lib --test ci_workflows`; bidder uses `--test factory_binding_build
--test registry_join`. All use `cargo test --offline --locked -j2 -p <package>`.
Their logs are `m7-metadata-core.log` and `m7-metadata-factory-build.log` under
`/private/tmp/`. The factory driver completed in 54.28 seconds.

The selected config build-harness command additionally exports
`CARGO_NET_OFFLINE=true` to its children and filters exactly
`sync_config_child_build_catalog_metadata_511_512_513`; it completed in 33.85
seconds. `/private/tmp/m7-metadata-catalog-boundaries-final.log` retains the
result. This is not a claim that the other 91 harness tests ran.

Guest commands use `cargo check --offline --locked -j2 -p auctioneer-config --lib
--target <ABI>`, independently with and without `--features insecure-bidders`,
for `wasm32-unknown-unknown`, `wasm32-wasip1` and `wasm32-wasip2`. Their logs use
`m7-metadata-<ABI>-{secure,insecure}.log`. These compile the target-side
arithmetic; they do not execute guest allocation tests or qualify physical fit.
Final lint is `cargo clippy --workspace --all-targets --offline --locked -j2
--features insecure-bidders,conformance-suite -- -D warnings`, recorded in
`/private/tmp/m7-metadata-clippy-workspace-final.log`. Initial lint findings
were repaired without exemptions; only the necessary test allocator retains
its narrowly scoped unsafe exception. Existing GVL deletion notices and the
tracked `proc-macro-error2` future-compiler warning remain.

Spec and quality reviewers found no remaining issues in this census-only slice;
both reviews were static, with parent verification kept separate. This record
does not claim a full workspace test run or a hosted CI/deployment result and
does not complete Task 4, schema qualification, live registry cutover, physical
WASM fit, hosted-provider qualification or M7.

# Explicit Metadata Binding Controls

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

Date: 2026-10-19
Branch: `bidder-catalog`, based on `52dd63f3`
Scope: the data-only binding-generation part of bidder catalog Task 6, not its full cutover.

## Build Contract

Each baked metadata file now explicitly declares an `implementation` key.
Public names remain separate data, never an inferred implementation name.
The pure compiler accepts several public identities sharing one implementation;
its controls distinguish `yahooAds` from `yahoo_ads` and retain exact spelling.
No central bidder-name match, constructor execution, bidder dependency or
runtime plugin loader was introduced in config generation.

The compiler uses `validator::Validate` with custom scalar checks and the
existing Rust identifier parser. It rejects invalid names/keys, exact and
ASCII-case-folded duplicate public identities, undeclared or developer-only
feature requirements, and Cargo feature names that collide after environment
normalization. Known features come from the config manifest, not a second
feature roster. All declarations are validated before any feature filtering.

The generated `bidder_implementation_bindings()` accessor exposes only active
bindings. `bidder_metadata_declarations()` retains all declarations and their
feature predicates. Both return deterministic, stable static slices. The live
metadata list uses the same declared predicate. The existing inactive loopback
metadata must explicitly require `insecure-bidders`; the directory alone cannot
authorize unconditional metadata or an HTTP endpoint in a default build.

All four current metadata inputs carry explicit keys. Their public names,
endpoints, activation defaults, capabilities and sync policy are unchanged.
The previously ignored baked `appnexus.platform_id` field was removed rather
than given a compatibility exception. The external source capture is unchanged.
Its existing HTTPS override is not newly reviewed or grandfathered: the later
routing cutover still requires the endpoint evidence specified by Task 6.

## Validation And Diagnostics

Root metadata rejects unknown fields. The existing renderer also owns timeout,
GVL-ID and endpoint-macro checks, so loading deliberately invokes that complete
owner before filtering, discarding its transient host-build string. Active
metadata is rendered normally afterward. This small build-time repetition
avoids a separate, incomplete list of pre-filter rules; it does not introduce
guest work or a runtime validator.

TOML parsing first produces a structured table, then the typed metadata model.
Syntax failures report only the file, available byte span and a fixed class.
Missing implementation has a fixed field hint; other shape failures have a
fixed file-level class. Neither source lines nor parser/Serde error text are
formatted. This intentionally trades detailed value-bearing shape diagnostics
for bounded, value-free output. Canonical macro, URL, host-allowlist and enum
diagnostics also retain their field/reason and literal help text without
echoing templates, hosts, rejected macro text or rejected enum strings.
Host/account-default diagnostic policy is outside this metadata boundary;
this is not a global secret-logging guarantee.

Actual child regressions exercise unknown-key, wrong-type and syntax errors
with a credential marker, asserting marker absence before the diagnostic class.
Inactive rows separately challenge timeout zero, out-of-range vendor ID and an
unsupported endpoint macro. Further three-row macro and eight-row semantic
matrices place markers in rejected endpoint/sync templates, authority and
allowlist inputs, compression and media strings. Whole-marker and independent
prefix/suffix assertions reject partial echoing. Field/reason checks replace
the old expectations that deliberately required rejected strings in logs;
their build-refusal assertions remain. The existing flat `skipWhen` negative now requires
the fixed metadata-field class; its typed positive counterpart remains.

## Test Ownership

Pure compiler unit tests live beside `build_bindings.rs`. The new
`bidder_bindings` integration target imports the compiler and executes the
actual generated APIs, independently requiring the current all/active rows,
metadata membership, order and stable slice identity in both feature modes.
The config child harness retains its bounded driver, watchdog, leased target
reuse and cleanup. Existing rejection fixtures gained valid explicit bindings
so they still reach their original URL, privacy, enum and alias guards.

The workflow runs this target explicitly in secure-default and feature-enabled
config lanes. Exact command guards and directory-set equality protect its
inclusion; other secure-default targets and prerequisites remain unchanged.

Observed RED controls include the old generator accepting a missing binding,
the CI guard refusing the omitted target, the inactive renderer bypass, and
the credential marker reaching source-line, macro and URL diagnostics. A
separate validation-branch ablation made an invalid inactive implementation
build, failing its actual child test. The branch was restored and all six
metadata controls passed again. These are nonzero
test failures, not compilation failures or filters matching zero tests.

## Final Verification

Locked/offline commands used two Cargo build jobs on Rust 1.98.1. Counts are
per invocation, not disjoint totals. Every listed test scope reported zero
failures and zero ignored tests.

| Scope                                                                                              | Result                                           |
| -------------------------------------------------------------------------------------------------- | ------------------------------------------------ |
| Config all targets, secure defaults                                                                | 518 passed, including all 92 child-harness cases |
| Config library/codegen/archive/smoke/URL/binding targets, `insecure-bidders test-harness-override` | 307 passed                                       |
| Bidder library/discovery compiler/feature snapshot, secure defaults                                | 149 passed                                       |
| Same bidder targets with `insecure-bidders`                                                        | 160 passed                                       |
| Core library, workflow guards and 14 affected catalog/usersync targets, secure defaults            | 782 passed                                       |
| Same core targets with `insecure-bidders`                                                          | 782 passed                                       |
| Workspace all-targets Clippy with `insecure-bidders,conformance-suite` and `-D warnings`           | Passed                                           |
| Bidder/config library checks for all three guest ABIs, both feature states                         | Six passed                                       |
| Formatting, diff and raw-JSON fixture gates                                                        | Passed; 168 budgeted fixtures across 50 files    |

Run `cargo test --offline --locked -j2 -p auctioneer-config --all-targets` for
the full secure config scope. The feature run mirrors the explicit config
feature-lane command, including the new `--test bidder_bindings` target.
Bidder runs select `--lib --test discovery_codegen --test insecure_bidders_snapshot`.
Core repeats the [count-control scope](catalog-count-controls-20261019.md)
plus `--test ci_workflows`. Guest checks use `cargo check --offline --locked
-j2 -p auctioneer-bidder --lib --target wasm32-unknown-unknown --target wasm32-wasip1
--target wasm32-wasip2`, separately with `--features insecure-bidders`.
These are library compilation checks, not artifact links or guest execution.

Parent logs are `/private/tmp/bidder-catalog-bindings-*-final.log`; the failed controls
and initial lint failures remain in separate scratch logs. Independent spec
and quality reviews approved this bounded increment after the inactive-row
and diagnostic repairs. Existing GVL deletion notices and the tracked
`proc-macro-error2` future-compiler warning remain; no exemption or warning
suppression was added. A fresh hosted CI run, complete workspace test run,
provider deployment, physical-memory qualification and schema qualification
were not executed or inferred from these checks.

## Remaining Work

This increment does not reconcile metadata predicates with real factory
declarations, consume these bindings in the live registry, remove its generic
fallback, approve schema or endpoint policy, or promote an imported identity.
Schema/alias/routing DTOs, generated target-layout bounds, catalog aggregation,
startup precharging, incremental lifecycle acceptance and full Task 6/7
completion retain their planned owners. resource qualification absolute fit and publisher pilot deployed
qualification remain open. No workload, budget coefficient, SDK/toolchain pin,
reference capture, dependency declaration or historical report was changed.

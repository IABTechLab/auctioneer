# Operator Configuration Foundations

Date: 2026-11-03

Base: `3b1b6d929bc99b800459fdafb3080655b8c77c29`.
Plan: [operator configuration implementation](../../superpowers/plans/2026-11-02-operator-runtime-configuration.md).
Pre-edit results: [selected baseline](operator-configuration-baseline-20261102.md).

This is an implementation checkpoint, not approval to change the serving
configuration reader. Strict deserialization establishes shape; it does not
admit a policy snapshot, prove storage bounds or make a deployment ready.

## Executed Foundation Checks

Commands used `--locked --offline -j2` and the existing shared target directory.
Builds ran serially. No source pin, workload limit or resource ceiling changed.

| Command selection | Executed result |
| --- | --- |
| `cargo test -p auctioneer-config-schema --lib --test root_shape` | 3 library and 12 integration cases passed. |
| `cargo test -p auctioneer-core --lib config::` | 170 passed, 0 failed, 475 filtered: existing configuration cases plus 24 field-override and 12 policy-model cases. |
| `cargo test -p auctioneer-core --test resolve_runtime_config --test runtime_config_failclosed --test config_funding` | 2 + 19 + 1 passed. These exercise the existing store reader, not the proposed snapshot API. |
| `cargo test -p auctioneer-bidder --lib --test registry_join --test test_isolation` | 1,188 library cases passed with 9 existing source-dependent cases ignored; registry integration 21 passed; workspace ownership/isolation 16 passed. |
| `cargo test -p auctioneer-bidder --test catalog_alloc -- --test-threads=1` | All 6 cases passed after final fixture cleanup. Its 13 current observations reproduce the matched comparison unchanged. |
| `cargo clippy -p auctioneer-config-schema --lib --tests -- -D warnings` | Passed, including the test build. |
| `cargo clippy -p auctioneer-core -p auctioneer-config-schema --lib --tests -- -D warnings` | Passed on the corrected policy/reset tree after fixing a new discovery-test directory-creation lint. |
| `cargo clippy -p auctioneer-bidder --lib --tests --test generated_settings -- -D warnings` | Passed after correcting test helper ownership, numeric types and Result-returning callback controls; no lint settings were relaxed. |
| `cargo test -p auctioneer-bidder --lib --target wasm32-unknown-unknown --no-run` | Passed: guest test code compiled, including compiler-evaluated 32-bit layout assertions. No guest test execution is claimed. |
| `cargo clippy -p auctioneer-bidder --lib --test catalog_alloc --target wasm32-unknown-unknown -- -D warnings` | Passed on the final native-helper gating/import tree. No guest tests executed. |
| Raw JSON fixture gate | Passed: 205 fixtures across 58 files. Five new lexical controls preserve duplicate or escaped-equivalent keys that `json!` cannot represent. |
| Raw JSON fixture gate self-tests | All 15 controls passed. |
| `cargo fmt --all -- --check` and discovered-bidder `rustfmt --edition 2024 --check` | Passed on the final tree. |

The opt-out regression was observed failing before its fix: supplying only
`value = "1"` incorrectly serialized an omitted name as `name = ""`.
`OptoutCookiePolicy` now preserves optional leaves and rejects explicit nulls;
JSON and TOML cases distinguish omitted and explicitly empty values. Both the
strict root and policy/reset slices received separate read-only spec review.
Separate read-only code-quality review found no remaining issues in that slice.

The bidder worker also executed the once-owned `generated_settings` target:
19 outer cases passed and its exact reconciliation verified 18 child contract
cases passed. Discovery's build-support selection passed 38 cases. Those
results precede the final layout/observation additions and are not a full
workspace, hosted or resource qualification run.
Existing vendor-deletion notices and the `proc-macro-error2` future-incompatibility
notice remain; they were not suppressed or repaired by this work.

## Registry Storage Change

The callback is not yet invoked by serving, but its descriptors already occupy
storage. The owner-local `configuration_callback_layouts_record_static_and_index_growth`
control compares actual current types with exact predecessor fields from the
base commit. Its first old-size expectation failed with compiler error E0080;
the corrected current rows passed native execution and guest test compilation.

| Per-row storage | 64-bit predecessor/current | 32-bit predecessor/current |
| --- | --- | --- |
| Static declaration | 32 / 40 bytes | 16 / 20 bytes |
| Static registration | 24 / 32 bytes | 12 / 16 bytes |
| Validated definition value | 64 / 72 bytes | 32 / 36 bytes |
| Factory-index value | 8 / 16 bytes | 4 / 8 bytes |
| Temporary feature-index value | 16 / 24 bytes | 8 / 12 bytes |

All five predecessor and current types align to 8 bytes on the tested native
target and 4 bytes on the tested guest target. These are compiler-observed
layouts, not hand-calculated B-tree node or allocator-overhead estimates.

At this checkpoint, structured parsing of executable bidder manifests found
17 declarations and 16 default-feature registrations, or 17 registrations with
`insecure-bidders`. The renderer gates factories only by `required_feature`;
`native_only` filters public definitions, not these factory tables. Static
declaration/registration growth is therefore 264 native bytes / 132 guest bytes
under default features, or 272 / 136 bytes with `insecure-bidders`. These totals
are derived source-table sizes; they are not binary-size, RSS or heap claims.

Factory and definition B-tree nodes grow immediately when a registry plan is
built. Its temporary feature map grows as well. The existing metadata census
does not automatically fund those changes.

### Matched Native Construction Observations

The existing `catalog_alloc` meter wrapped actual `ValidatedRegistryPlan::validate`
in two source trees: the current implementation and an archive of the base
commit. The added fixture/meter code was identical apart from the two new
descriptor-literal fields, which do not exist in the baseline API. Input names,
metadata and vectors were prepared before tracking; the returned plan remained
alive at interval end. Reporting and final destruction were outside tracking.
A constructor call would panic; neither run constructed a bidder.

Both successful commands used Rust 1.98.1, `aarch64-apple-darwin`, the debug test
profile, identical default features, locked/offline dependencies and the normal
shared target directory. Each executed one exact test with 13 populations:

```sh
cargo test -p auctioneer-bidder --test catalog_alloc \
  --target-dir "$CARGO_TARGET_DIR" --locked --offline -j2 \
  -- --exact tests::actual_registry_construction_peak_observation \
  --nocapture --test-threads=1
```

The baseline registry source Git blob was
`9a1f12f91fca956ca2dc62434c741c0c2d9a1a5e`; the measured current registry source
blob was `f3c2bc437c64926931e3e3d060880d807aa92498`. The respective lockfile blobs
were `b1c3934b2232ffee038db77824d8a251301e5a8f` and
`ec4adfc0f6e7eacc0da48633857c8ff2948f885e`; their only dependency change adds
the existing workspace `validator` dependency to the bidder package.

An initial baseline attempt failed before test execution because the shared
cache retained current codegen with fields absent from the baseline API. It
supplies no observations. Refreshing only two archived config-schema source
timestamps forced actual baseline codegen to rebuild; their source bytes,
registry blob and dependency pins remained unchanged. The retry passed. Current
codegen was rebuilt again before the final implementation checks. No target
cleanup, `RUSTFLAGS` change or alternate compiler output cache was used.

Each row has N implementations, 2N public identities and N source aliases.
Byte columns show **baseline -> current**. The last column is the common
allocation/deallocation/reallocation-call tuple; counts matched in every row.

| N | Cumulative requested bytes | Retained bytes | Peak bytes | Call counts |
| ---: | --- | --- | --- | --- |
| 0 | 0 -> 0 | 0 -> 0 | 0 -> 0 | (0, 0, 0) |
| 1 | 6614 -> 6878 | 2176 -> 2352 | 3912 -> 4088 | (21, 17, 0) |
| 11 | 21938 -> 22466 | 4960 -> 5400 | 11520 -> 11960 | (101, 94, 4) |
| 12 | 29552 -> 30432 | 7808 -> 8424 | 14944 -> 15560 | (122, 109, 4) |
| 13 | 32846 -> 33814 | 8704 -> 9408 | 16928 -> 17632 | (134, 120, 4) |
| 63 | 131658 -> 135266 | 30032 -> 32760 | 65792 -> 68520 | (580, 532, 8) |
| 64 | 132128 -> 135736 | 30032 -> 32760 | 65792 -> 68520 | (586, 538, 8) |
| 65 | 149046 -> 152654 | 30032 -> 32760 | 69056 -> 71784 | (595, 547, 10) |
| 127 | 269354 -> 276746 | 59280 -> 64824 | 129504 -> 135048 | (1153, 1057, 10) |
| 128 | 269888 -> 277280 | 59280 -> 64824 | 129504 -> 135048 | (1159, 1063, 10) |
| 129 | 300390 -> 307870 | 60176 -> 65808 | 137296 -> 142928 | (1170, 1073, 12) |
| 255 | 549786 -> 564570 | 119280 -> 130368 | 259264 -> 270352 | (2305, 2112, 12) |
| 256 | 550352 -> 565136 | 119280 -> 130368 | 259264 -> 270352 | (2311, 2118, 12) |

At 512 identities, cumulative requested heap increases by 14,784 bytes;
retained storage and construction peak each increase by 11,088 bytes. These
are observed requested-heap deltas, not allocator overhead, RSS, executable
size, exhaustive insertion-order bounds or guest heap measurements. The later
fixture cleanup changed only imports and native-helper gating, not the metered
operation or the registry implementation.

The immediate-growth finding passed separate spec rereview; a fresh read-only
code-quality review found no remaining issues in the staged bidder slice. Guest peaks,
callback-error allocation and production accounting remain Task 4 prerequisites.
No new charge, ceiling or budget equation is adopted.

## Pinned SDK Input Audit

`cargo metadata --locked --offline` resolved `edgezero-core` and `edgezero-cli`
to `https://github.com/IABTechLab/edgezero.git`, revision
`775366fdac923135e8430b1e3dffebedc648ef7c`. The resolved checkout's Git HEAD
matched that complete revision. Inspection used that checkout, not another
installed SDK version. Paths in the following table are relative to its root.

| Surface | Observed source behavior | Implication |
| --- | --- | --- |
| `crates/edgezero-core/src/config_store.rs`, `ConfigExtractionLimits` | The runtime extraction contract carries backend/blob/secret/total byte caps, JSON depth/value caps and a timeout. Its validator rejects zero/inconsistent limits. | Preserve all seven fields in the accepted extraction tuple. This is not an authoring TOML input limit. |
| `crates/edgezero-core/src/extractor.rs`, `AppConfig` and `extract_from_handle` | Typed runtime extraction uses bounded store reads and an extraction-scoped budget. | Retain this runtime path and its envelope/error behavior; it does not bound earlier standalone startup materialization. |
| `crates/edgezero-core/src/app_config.rs`, `AppConfigLoadOptions` and `load_app_config_raw_with_options` | Options expose environment-overlay selection. The loader calls `fs::read_to_string`, then parses a full TOML value. | Do not use this operation as a bounded startup loader. It has no input-byte or parser-structure reservation parameter at this revision. |
| The same file, `EnvLookup::from_process_env` | Collects `env::vars()` into an owned map before selecting values. | Filtering relevant variables after this collection would not establish a pre-materialization bound. |
| `crates/edgezero-cli/src/config.rs`, `load_validation_context` and the typed validate/push/diff entry points | The command loads a raw authoring table, then the typed path deserializes from the file again. Those contexts and loading helpers are private. | A preliminary file-size check does not bound the later reread or make both reads one immutable input. Do not claim an existing public preloaded-input hook. |

This is source inspection, not execution of an SDK resource qualification suite.
No SDK changes, new loader, alternate validator or dependency bump were made.

## Shared Bidder Validation Checkpoint

The later staged `config::validation::validate_bidder_configuration` accepts
explicit caller-owned bidder/alias maps and a validated catalog. It completes
shared structural, namespace and settings-binding checks before invoking an
owner callback. A borrowed-key union prevents duplicate invocation across known
enabled identities, configured aliases and explicit settings entries. Unknown
request-local settings retain `definition = None` and `enabled = false`.

The existing serving registry's `base_names()` includes compiled aliases and
disabled definitions; the registry plan's canonical-base index is narrower.
The subvalidator preserves the serving semantics, including exact-name host
shadowing, nonrecursive host targets and immutable/ancestry activation gates.
It does not introduce a stricter alias policy under the name of validation.

An `Ok(())` control implementation produced 18 passing existing child cases
and six failing new behavioral cases, with all 24 dispatched. An earlier
attempt did not reach assertions because the test fixture tried to clone a
non-Clone declaration; explicit field copying fixed the fixture, not production
types. The successful negative control establishes shared-dispatch sensitivity;
the failed compilation supplies no behavioral result.

| Additional command selection | Executed result |
| --- | --- |
| `cargo test -p auctioneer-core --lib config::validation::` | 16 passed. |
| `cargo test -p auctioneer-core --lib config::` | 186 passed, 475 filtered. |
| `cargo test -p auctioneer-core --lib` | 661 passed, no failures, ignores or filtered cases after lint corrections. |
| `.github/workflows/scripts/test/verify-schema-compiler-conformance.sh settings` | Passed: 19 outer cases, exact 24-child reconciliation and dedicated generated-settings Clippy. This preceded the core-only lint cleanup; the final core suite and test-build Clippy also passed. |
| `cargo clippy -p auctioneer-core --lib --tests -- -D warnings` | Passed after fixing ignored-error bindings, a redundant method closure, shadowed names and test-helper/numeric lints. No lint allowance was added. |
| `cargo test -p auctioneer-bidder --test test_isolation` | 16 passed, including ordinary modules, test ownership and fake-only shared identities. |
| `cargo fmt --all -- --check` and `git diff --check` | Passed on the final source and documentation tree. |

The six added synthetic cases exercise the actual shared function with generated
fake settings: exact dispatch/fallback, disabled and shadowed supplied values,
known-only requiredness, binding and size refusal before any callback, and fixed
failure diagnostics in the presence of a credential-reference canary. The 18
earlier child cases and their assertions remain. Independent read-only spec and
code-quality reviews reported no findings in this staged slice.

The pushed foundation/CI-repair commit `b41f57c` completed all 25 hosted checks
successfully. That result does not cover these later local additions.

This is not a complete `ConfigValidationContext`, policy resolver or admitted
snapshot. CLI/store/typed-host callers are not switched. Temporary namespace
folding, identity-set nodes and validation-error peaks still require Task 4
funding before activation; no zero-allocation or ready-target claim is made.

## Review Repairs

The five inline review threads were checked against the staged implementation.
These repairs do not activate the proposed policy reader or change resource
charges, dependency pins or feature eligibility.

- All 32 optional fields declared in `policy.rs` reject explicit null across
  48 tested paths. Absent fields are omitted when serializing, and keyed purpose
  maps serialize their numeric keys as strings for JSON/TOML round trips.
  Reused legacy nested DTO leaves are outside this change; their strictness
  remains part of the atomic policy cutover, not a completed guarantee.
- `FieldOverride::Value` rejects a root value that serializes as null, including
  optional, unit/newtype and non-finite cases. A one-pass serializer adapter
  preserves compound values and legitimate nested nulls without another JSON
  tree or a serialization prepass.
- The existing generated-settings child suite now supplies real typed settings
  for an implementation whose feature/factory is unavailable. Shared admission
  refuses both enabled and disabled supplied entries before callbacks or factory
  construction. Existing discovery eligibility is preserved, rather than
  generating references to absent modules.
- The 512-identity registry observations now have independent requested,
  retained and peak regression ceilings. Populating every callback slot yields
  the same complete allocation measurement as empty slots, with all 512 slots
  checked, no factory calls and a positive control for callback observation.
  These are requested-heap regression checks, not worst-case resource bounds.
- The two currently rule-free bidder configuration views are explicit unit
  structs. Their tests assert generated fieldwise projections independently;
  they do not claim a semantic rejection rule that those views do not contain.

Observed red controls included three policy failures (null acceptance,
serialization of omitted fields and TOML map keys), three field-override
failures (root null/newtype/null-producing floats), an empty allocation guard,
and the unlinked-settings case with only its availability check removed. The
last mutation dispatched all 25 child cases: 24 passed and only the new case
failed. The original production guard was restored byte-for-byte before the
green workflow run.

| Final command selection | Executed result |
| --- | --- |
| `cargo test -p auctioneer-core --lib` | 670 passed, no failures, ignores or filtered cases. The configuration selection contains 195 cases, including 16 policy and 29 field-override cases. |
| `cargo test -p auctioneer-bidder --lib` | 1,188 passed, no failures, 9 existing source-dependent cases ignored. |
| `cargo test -p auctioneer-bidder --test catalog_alloc --test test_isolation -- --test-threads=1` | 8 allocation and 16 ownership/isolation cases passed. |
| `.github/workflows/scripts/test/verify-schema-compiler-conformance.sh settings` | 19 outer and exactly 25 child cases passed, followed by dedicated generated-settings Clippy. One existing compiler owner, not a new duplicated child-build suite. |
| Core library/test and bidder library/allocation Clippy with `-D warnings` | Passed after correcting test-only import/counter lints. No allowance or lint policy was changed. |

Independent read-only specification and quality reviews found no remaining
in-scope issues in the seven source/test files. Guest compilation/execution,
hosted acceptance and complete resource qualification were not rerun for these
repairs. Known GVL deletion and future-incompatibility notices remain.

The maintainer reaffirmed one operator configuration document and one
load/validate pipeline. Startup and runtime remain scopes of that document,
not separate files, loaders or validators. Design section 9.1 records the
required bounded, reload-free SDK path and its remaining review prerequisites.

## Required Before Cutover

Tasks 4-10 remain uncompleted. In particular:

- Review the exact startup-input and parser/storage reservations and snapshot
  retention/admission ownership under #128/#130 before numeric adoption.
- Extend the single SDK authoring/startup pipeline under a reviewed contract.
  It must bound the actual read, parsing, relevant environment materialization
  and retained copies without a second loader, reread or post-allocation-only
  check.
- Implement the complete pure context-based semantic validation/resolution path
  and connect its staged shared bidder subvalidator. CLI, store and typed-host
  admission must use that same path.
- Complete product-default separation, optional user-sync state, funded
  snapshots and the atomic build/bootstrap/CLI/native serving cutover.
- Execute the real three-path parity, lifecycle/allocation and same-binary
  two-configuration acceptance checks. Compilation alone cannot replace them.

The existing reader remains active until that cutover group passes. No issue,
epic, provider port or host integration is closed by this checkpoint.

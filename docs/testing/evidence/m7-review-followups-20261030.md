# Catalog Review Follow-Up Verification

Date: 2026-10-30. Rust 1.98.1; Wasmtime 48.0.2. Base: merged main
`8a4e4ad6`. SDK stays at `9fe6c56c648f0a601b1c6c051ce71c09d3733ea9`.
This record covers the follow-up source, not a replay or rewrite of historical
qualification captures.

## Executed Checks

Commands ran from the isolated follow-up worktree. Cargo reused a retained
target directory with two build jobs and locked, offline dependencies. The
original and supplemental source variables identified Git repositories
containing `d7f8515b86258688304b0d9b6668c6a0e258bc9e` and
`f1e9d0d2164e76a8bcb5f10021ebb56e00dbfe9a`, respectively.

| Check | Result |
| --- | --- |
| `cargo test -p auctioneer-runtime --lib` | 254 passed, no ignored cases |
| `cargo test -p auctioneer-core --lib` | 607 passed, no ignored cases, without the retired configuration target or YAML dependency |
| `cargo test -p auctioneer-cli --no-default-features --test bidders_source -- --test-threads=1` | 24 passed, no ignored cases |
| `cargo test -p auctioneer-bidder --lib --test catalog_publication --test test_isolation` | 1,177 unit cases passed, nine existing ignores; two catalog cases and 16 isolation cases passed |
| Native `catalog_publication` and `test_isolation` with `--features insecure-bidders` | Two catalog cases and 16 isolation cases passed |
| `bash .github/workflows/scripts/test/execute-catalog-publication.sh` | Actual WASI execution: two secure and two development-feature cases passed; the script enforced one nonempty summary per lane |
| Each of the 21 extracted scripts, individually through `bash -n` | Passed |
| Raw-JSON fixture gate | 200 budgeted fixtures across 57 files |

Strict Clippy passed for the native catalog target,
archive-independent provenance target, all runtime targets and the catalog
target on `wasm32-wasip1`, each with `-D warnings`. Guest fallback bindings
needed explicit types; their fail-closed behavior is unchanged. Formatting and
`git diff --check` passed. Existing archive deletion notices and the
`proc-macro-error2` dependency future-incompatibility notice remain visible.

Application/protocol tests were not removed or newly ignored. The separate
workflow-configuration snapshot suite was intentionally retired below. The nine
bidder ignores retain
their existing environment/source prerequisites. The original source walk
already checked the six reported original-source bidders against Git objects;
this repair extends supplemental ownership and independently exercises changed
witnesses instead of claiming those six lacked all verification.

## Independent Regression Controls

Each temporary mutation was restored before clean verification:

- Remove the native-only publication predicate: the compiled WASI test aborts
  in its generated metadata `BTreeSet` equality assertion (exit 134), rather
  than failing compilation. Restoring the predicate passes both guest cases.
- Bypass the early raw-value newtype refusal: both borrowed/owned controls fail
  their zero-node assertion (one charged node versus zero).
- Bypass the map-key refusal alone: the flatten control fails after observing
  the hidden array expanded through an unwrapped `Value` parse.
- Remove only the Git digest comparison: the incorrect-digest control fails
  because the forged witness is no longer rejected. Other provenance and
  snapshot checks cannot supply that rejection.

AG explicitly removed the Rust workflow-configuration snapshot suite during
this review. It is not a current acceptance gate. The 20 pre-existing extracted
scripts were independently compared byte-for-byte with the merged base after
removing their new shebangs; YAML step metadata is unchanged. The twenty-first
script is the new executed guest catalog check. Actual CI builds, tests,
linting, artifact measurements and exact guest-result checks remain enabled.

## Boundaries

No bidder publication, schema reservation, workload limit, deployment ceiling,
SDK revision or historical capture changed. Guest execution checks publication,
not complete WASM resource fit or provider-host behavior. Download/install,
provider packaging and archive-refresh side effects were not run locally.

Hosted CI is not a local pass: the inspected
[Cloudflare job](https://github.com/IABTechLab/auctioneer/actions/runs/41168331725/job/124440632965)
on the merged base has no executed steps and reports failed account payments or
a spending limit. SDK Git-object availability remains an integration-source
dependency; the developer guide requires online acquisition before offline use.

## Remaining Native-Only Consumers

The follow-up review found two non-routing coverage gaps, not incorrect current
selection rules. The existing config-owned cold-allocation census now reads the
source `native_only` flag and executes in separate WASI secure/development
processes. The CI script runs two publication tests per lane and one cold census
per lane. All four commands passed with nonempty summaries and zero ignores.

The new build-harness case exercises portable/native-only alias bases on both
the compiler's native host target and `wasm32-wasip1`. It passes explicit target
flags while deliberately supplying the opposite `CARGO_BUILD_TARGET` default.
The four-build matrix passed; guest rejection checks the absent-base diagnostic
rather than accepting any unrelated build failure.

Independent temporary filter bypasses were detected:

- `auctioneer-config/build.rs`: the guest build accepted `fake_native_base`, so the
  alias test failed its success/refusal assertion after the valid controls.
- `codegen/storage.rs`: the actual guest census reported 17 active definitions
  against three source-selected definitions and failed in `assert_roster`.

Both production filters were restored without changing their implementation.
These checks concern metadata/aliases/requested allocation, not full physical
WASM residency or provider qualification.

The catalog plan now explicitly retires its last forward-looking workflow
snapshot instruction. One-off YAML validation confirms all 33 checkout/cache/
Rust-setup uses across eight workflows select v7/v6/v2, respectively; no Rust
configuration snapshot test was recreated. Official
[checkout](https://raw.githubusercontent.com/actions/checkout/v7/README.md),
[cache](https://raw.githubusercontent.com/actions/cache/v6/README.md) and
[Rust setup](https://raw.githubusercontent.com/actions-rust-lang/setup-rust-toolchain/v2/action.yml)
sources were checked. Rust setup v2 uses Cargo's warning policy rather than
injecting the former default Rust flags. A local config check passed under
`CARGO_BUILD_WARNINGS=deny` on the pinned toolchain; archive notices remained
visible. Cargo documents that this policy applies to local lint warnings, not
non-lint build notices in its [configuration reference](https://doc.rust-lang.org/cargo/reference/config.html#buildwarnings).

Final focused verification for this round: eight alias harness cases passed
(97 other cases filtered, not a full harness sweep); native binding/census
targets passed three plus one tests in each feature lane. The actual WASI
script passed two publication plus one census test per feature lane. Strict
Clippy passed for both changed native test targets and the WASI census target;
formatting, Bash syntax, raw-fixture budgets and diff whitespace checks passed.
No new ignored tests, workload changes, production filter changes or workflow
configuration snapshot tests were introduced.

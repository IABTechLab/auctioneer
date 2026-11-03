# Archive Verification Guards Implementation Plan

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans
> to execute these reviewed repair requirements with focused regression checks.

**Goal:** Make each retained archive boundary independently testable, share
release metadata policy, and reconcile the carried verification/lint findings.

**Architecture:** The raw pass owns expanded-stream, record, file and payload
upper bounds. One `ReleaseMetadata::validate` owns release identity, safe
declared members and exact membership at both archive and extracted-bundle
boundaries. The bounded filesystem walk remains an independent post-SDK
defense. No new archive format, qualification claim, dependency or SDK pin.

**Tech Stack:** Existing Rust 1.98.1, tar/flate2, serde, Tokio and Clippy.

## Ownership

- `auctioneer-cli/.../providers/archive.rs`: raw/preflight ownership and colocated tests.
- `auctioneer-cli/.../providers/artifacts.rs`: shared metadata policy, bounded extracted walk and colocated tests.
- `Cargo.toml` and affected impls: remove functional workspace suppressions, retaining explicit style policies.
- `scripts/raw-json-fixtures.allow`: explain the existing object-leading count without increasing it.
- `ISSUES.md` and the diagnostic qualification verification record: accurate disposition and scoped evidence, not historical rewrites.

Paths abbreviated above start at `crates/`; provider modules live under
`auctioneer-cli/src/qualification/harness/`. Changes remain on `diagnostic-hardening`;
bidder catalog specification work is independent.

## Task 1: Expose Independent Boundary Regressions

- [x] Add complete-archive controls with a hidden sixth member after the tar terminator, both inside one gzip stream and in a second gzip member.
- [x] Add clean archive and extra-zero-padding positive controls.
- [x] Run the new archive tests against the retained guard; all twelve archive tests pass.
- [x] Disable only the trailing-data predicate in an isolated source copy; the new test compiles and fails with `Ok(())` versus `release_invalid`.
- [x] Add direct raw-record/file, extracted-file, cumulative filesystem-node and exact member-set tests so later validators cannot mask the tested boundary.

## Task 2: Share Policy and Remove Redundant Counts

- [x] Move identity, safe member paths/digests, role uniqueness and exact member-set policy into `ReleaseMetadata::validate`.
- [x] Call it from preflight and extracted-bundle verification; remove both copied identity blocks.
- [x] Remove the raw pass's redundant minimum/exact-file check and the semantic pass's duplicate upper-file count. Keep the raw upper bound and metadata completeness as separate responsibilities.
- [x] Test format, lifecycle, adapter and source revision independently through each boundary.
- [x] Test member names, unsafe paths, duplicate roles and malformed digests directly on the shared policy.
- [x] Compile and kill isolated removals of every shared identity term, exact membership, raw file/entry ceiling and extracted file/node ceiling; also pin zero-only expanded-stream refusal. All fourteen removals compile and fail.

## Task 3: Narrow Functional Lint Exceptions

- [x] Remove workspace `unused_async_trait_impl` and `missing_trait_methods` allowances.
- [x] Scope justified exceptions to lazy async test-control impls, stable Drop impls and intentionally defaulted external trait impls; do not change behavior or add method stubs.
- [x] Keep `inline_modules` and `inline_trait_bounds` as explicit repository style policies; do not move colocated unit tests or annotate every ordinary generic bound.
- [x] Verify strict native workspace and actual CI WASM lint scopes on Rust 1.98.1.

## Task 4: Reconcile and Verify

- [x] Correct the evidence header: the initial matrix used the older SDK; later scoped checks use `596f9f4e23e43c8952a51ce6d92961d461032004`.
- [x] Explain why thirteen raw literals in `io.rs` yield twelve object-leading matches; leave the allowance unchanged.
- [x] Re-read the actual future-incompatibility report and dependency path. Keep E0365 open unless a compatible dependency repair is qualified; do not suppress it.
- [x] Run CLI all-target tests with the explicit pinned SDK checkout, strict workspace Clippy, feature/conformance lint scopes, formatting, whitespace and raw-fixture controls.
- [x] Independently review the final diff; record precise commands, failures and passing scopes in the verification record and `ISSUES.md`.

Publication follows these verification steps: commit and push the completed
diagnostic qualification repair without changing bidder catalog. The Git branch records publication, not a
second verification or qualification claim.

Focused commands use `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
CARGO_INCREMENTAL=0`, `--locked --offline -j2`. Core commands:

```sh
cargo test -p auctioneer-cli --bin auctioneer-cli qualification::harness::providers::archive::tests --locked --offline -j2
cargo test -p auctioneer-cli --bin auctioneer-cli qualification::harness::providers::artifacts::tests --locked --offline -j2
cargo test -p auctioneer-cli --all-targets --locked --offline -j2
cargo clippy --workspace --all-targets --locked --offline -j2 -- -D warnings
cargo fmt --all -- --check
scripts/check-raw-json-fixtures.sh
scripts/check-raw-json-fixtures-fixtures.sh
git diff --check
```

Guard-removal probes run only in a disposable source snapshot, not the shared
worktree. A compile error is not a killed mutation. Unexecuted live-provider,
resource-requalification and full regression sweeps are not credited here.

# Patched-Lock Catalog Continuity

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

Date: 2026-10-21
Source checkpoint: `7286bedfd1f1999edae9a5e3e5665b1f95117541`
Scope: matched accounting, not physical residency or catalog qualification.

## Captures

Both source-bound captures remain separate, byte-pinned files in
`auctioneer-core/tests/fixtures/resource_envelopes/`:

| Capture                              | Commit                                     | Raw SHA-256                                                        |
| ------------------------------------ | ------------------------------------------ | ------------------------------------------------------------------ |
| `catalog-baseline.toml`              | `e71e40aba611215bb9305f82e2449e13fb0f41ed` | `6da360d57f08267018035cedce6157ada2e955ab08913517a5d6020dee0b9e39` |
| `catalog-baseline-patched-lock.toml` | `7286bedfd1f1999edae9a5e3e5665b1f95117541` | `1d20dfa95f09a32a45f106f5503094efd71a718d794814fac353f83f357219d0` |

Each file is 131,237 bytes. The new file is actual output of the existing
`bidders resources --check --all-adapters --capture-audit` command, not an
edited historical record or a synthetic report. Two independent invocations
at the clean code checkpoint produced byte-identical output. Its capture log
is `/private/tmp/bidder-catalog-catalog-lock-continuity-capture.log`.

The SDK, compiler, tool declarations, features, ABI/layout labels, all resource
limits and the shared equation are unchanged. The lock moves from
`f20a05604c912c0d9946e7c86020f1e12549abbf6007bfe8d56883dda1a7a1c1` to
`aca2f5fc543402752f6e8c39aa79fa0c9269600c8ad08c2a617d81ec27ac6dbc`:
the approved local compiler patch, bidder's test `tar` edge and config's
runtime edge, without package-version changes. Source digests/counts and
dependent profile digests change. This is not proof that executable growth
or physical residency is zero.

## Preserved Growth

The transition is **not identity-only**. Count foundations increased the
auxiliary chooser charge from 397,312 to 462,848 bytes per permit. This
65,536-byte increase remains visible even though declared reservations and
isolate totals did not change. Its component receipt uses auxiliary plus
probe-config populations, not egress-handoff copies:

| Adapter    | Population | Chooser Component Before |     After | Preserved Delta |
| ---------- | ---------: | -----------------------: | --------: | --------------: |
| Axum       |          2 |                  794,624 |   925,696 |         131,072 |
| Cloudflare |          8 |                3,178,496 | 3,702,784 |         524,288 |
| Fastly     |          2 |                  794,624 |   925,696 |         131,072 |
| Spin       |          2 |                  794,624 |   925,696 |         131,072 |

Both complete auxiliary charge sums fit the same declared peak:

| Adapter                | Original Work |    New Work | Declared Peak |   Remaining |
| ---------------------- | ------------: | ----------: | ------------: | ----------: |
| Axum                   |    58,580,992 |  58,646,528 |   134,217,728 |  75,571,200 |
| Cloudflare/Fastly/Spin |   302,768,128 | 302,833,664 |   805,306,368 | 502,472,704 |

These are checked declared charges, not measured heap peaks. The unchanged
auxiliary isolate term already funds the full declared peak. Charging the
chooser increase again outside that reservation would double-count it.

## Admission Boundary

The new raw pin requires complete typed-audit continuity with the original.
The checker starts with that original audit, permits only the recorded
source/lock identity changes, recalculates dependent profile hashes and adds
the exact chooser increases above. The entire resulting audit must equal the
new capture. There is no ignore-list for other numeric owners: raw/effective
limits, target facts/constants, all costs, baked defaults, derived terms,
auction receipts, other auxiliary/component receipts, ownership decisions,
pending evidence and readiness remain unchanged. The new record also passes
the current audit validator, including full auxiliary funding.

Comparison output carries the original raw identity, both isolate totals
and preserved per-permit/component deltas under `baseline.continuity`.
The ordinary candidate-versus-selected-baseline delta stays separately named;
a zero there does not erase the earlier chooser increase. Comparing the old
file still refuses its lock mismatch. The new file can match only its recorded
ABI, toolchain and features; other builds must refuse rather than substitute
the native record as guest evidence.

```sh
cargo run --offline --locked -p auctioneer-cli -- bidders resources \
  --check --all-adapters \
  --catalog-baseline crates/auctioneer-core/tests/fixtures/resource_envelopes/catalog-baseline-patched-lock.toml
```

The report remains `matched_catalog_arithmetic_only`, with qualification
`not_evaluated`. All four absolute-fit flags remain false; `--require-fit`
cannot pass. Catalog C/I/V/K descriptors remain explicitly unchanged, not
qualified zero-cost bounds. Actual code/data/heap increments still require
qualified records against the retained original source basis, including work
introduced before the new capture. No serving registry, validator, bidder,
workload limit or runtime budget changes in this increment. Full Task 4,
schema qualification, guest observations and resource qualification readiness remain open.

## Verification

The compiling continuity stub first reported 18 passed and one failed: its
positive transition control. The new-pin loader control separately failed
with `ResourceInput`. Both were runtime failures, not compile failures.
An additional control independently changes limits, ownership and pending
evidence on each of the four adapter rows. Narrowing the production comparison
to the first adapter made it fail specifically at
`/adapters/1/report/raw_limits/baseline_bytes`. The complete audit comparison
is restored; that mutation is not part of the change.

Rust remains `1.98.1`. Final selected test invocations report zero failures
and zero ignored tests. Counts overlap and are not a summed workspace total:

| Scope                                                                                      | Result                                                                 |
| ------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------- |
| Secure CLI resource-unit selection                                                         | 138 passed                                                             |
| Complete secure resource-command target                                                    | 16 passed                                                              |
| Resource-unit selection with linked insecure-bidder/conformance features                   | 138 passed                                                             |
| Complete resource-command target with those features                                       | 16 passed                                                              |
| Actual Git checkout filters, including both accounting captures                            | 1 passed; ten protected members plus an unprotected conversion control |
| Workspace all-target Clippy with insecure-bidder/conformance features and `-D warnings`    | Exit 0                                                                 |
| Formatting, diff, raw-JSON gate and its negative controls, batch-rebase and roster scripts | Exit 0; 168 raw fixtures, 139 roster members                           |
| Standalone matching-build command                                                          | Exit 0, arithmetic-only, qualification not evaluated                   |
| Same standalone command with `--require-fit`                                               | Exit 1, byte-identical JSON; all four absolute-fit flags false         |

Tests use `CARGO_INCREMENTAL=0 cargo test --offline --locked -j2 -p auctioneer-cli`.
Unit invocations select `--bin auctioneer-cli bidders::resources::`; integration
invocations select the complete `--test bidders_resources` without a filter.
The feature runs add
`--features auctioneer-core/insecure-bidders,auctioneer-core/conformance-suite`. The
checkout control uses `--test bidders_import
checkout_filters_preserve_byte_pinned_catalog_and_resource_fixtures`.
Only incremental caching is disabled to limit disk growth: toolchain, profiles,
debug information, production limits and accounting coefficients are unchanged.

Final test logs are `/private/tmp/bidder-catalog-catalog-continuity-unit-verified-secure.log`,
`/private/tmp/bidder-catalog-catalog-continuity-cli-verified-secure.log`,
`/private/tmp/bidder-catalog-catalog-continuity-unit-verified-features.log`,
`/private/tmp/bidder-catalog-catalog-continuity-cli-verified-features.log` and
`/private/tmp/bidder-catalog-catalog-continuity-checkout.log`. RED/mutation logs are
`/private/tmp/bidder-catalog-catalog-continuity-red.log`,
`/private/tmp/bidder-catalog-catalog-continuity-loader-red.log` and
`/private/tmp/bidder-catalog-catalog-continuity-first-row-mutation.log`.
Standalone reports are scratch files at
`/private/tmp/bidder-catalog-catalog-continuity-verified-{report,strict-report}.json`;
`cmp` verified their exact equality. They are not captured deployment evidence.
The final matching-build command was compiled and run again after both selected
feature-mode suites; its build log is
`/private/tmp/bidder-catalog-catalog-continuity-verified-command.log`.

The final selected suites were rerun after representation-only hexadecimal
literal corrections required by the existing lint. No value, assertion,
production coefficient or allowance changed, and no suppression was added.
The full lint command is `CARGO_INCREMENTAL=0 cargo clippy --offline --locked
-j2 --workspace --all-targets --features insecure-bidders,conformance-suite --
-D warnings`; its log is
`/private/tmp/bidder-catalog-catalog-continuity-clippy-verified.log`. Existing GVL deletion
notices and Cargo's third-party `proc-macro-error2` future-incompatibility
notice remain; neither prevented the command's successful exit.

Read-only spec and code-quality reviews approved this arithmetic-only
increment; the parent executed verification. No full workspace test run,
guest execution, hosted CI success, credentialed deployment, physical-memory
qualification, completed bidder catalog or resource qualification readiness is claimed.

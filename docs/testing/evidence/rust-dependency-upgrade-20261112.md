# Rust Dependency Upgrade Checkpoints

This is the current implementation record for [#165](https://github.com/IABTechLab/auctioneer/issues/165) in existing [PR #166](https://github.com/IABTechLab/auctioneer/pull/166). It is not a deployment qualification, a complete all-latest claim, or permission to merge. Full hosted acceptance and independent implementation review remain required.

## Selection And Retained Sources

The 2026-11-12 requirement audit found 23 candidate requirement changes; the compatible lockfile audit found 148 updates. Those are selection observations, not test counts. Nineteen candidate requirements are updated below. `Cargo.lock` records the full resolved graph, including remaining transitive versions and sources. Its selected bytes have SHA-256 `9cd3a303b4cebde1619220ef8a3eb67a7d1540ec836d308bf2571f28040746fe`.

The implementation incorporates main's archive refresh at `7f6773e29d7657819a90dd53522055623b9fd6f2` and preserves the concurrent PR-branch refresh merge `c29c44d608b244994b1bcc05a3391fee85b53aed`; their integrated source trees matched. No separate upgrade PR is created.

| Workspace Dependency | Selected Requirement | Resolved Version |
| --- | --- | --- |
| hmac | `=0.13.0` | 0.13.0 |
| jsonschema | `=0.61.2` | 0.61.2 |
| jsonschema-macros-core | `=0.61.2` | 0.61.2, patched local source |
| getrandom | `0.4` | 0.4.3 |
| gimli | `=0.34.0` | 0.34.0 |
| ipnetwork | `0.21` | 0.21.1 |
| lru | `0.19` | 0.19.0 |
| mime | `0.3.18` | 0.3.18 |
| object | `=0.41.0` | 0.41.0 |
| png | `=0.18.1` | 0.18.1 |
| regex | `=1.13.1` | 1.13.1 |
| sha2 | `0.11` | 0.11.1 |
| zeroize | `1.9` | 1.9.1 |
| syn | `3` | 3.0.10 |
| thiserror | `2` | 2.0.21 |
| toml | `1.2` | 1.2.1+spec-1.1.0 |
| unicode-case-mapping | `=1.0.0` | 1.0.0 |
| wasmparser | `=0.263.0` | 0.263.0 |
| wasm-encoder | `=0.263.0` | 0.263.0 |

All six EdgeZero requirements and eight resolved EdgeZero packages retain the official repository and immutable PR #275 revision `598f0a2a490eb83dd31c06c024a0cefa8e049628`. No SDK checkout or exact pin is patched to force a newer shared tuple. Future coordinated tuple work belongs to [EdgeZero #467](https://github.com/IABTechLab/edgezero/issues/467).

| Retained Owner | Selected Version | Candidate Not Adopted | Constraint |
| --- | --- | --- | --- |
| Fastly | 0.13.1 | 0.13.2 | Retained SDK provider type identity |
| Worker | 0.8.5 | 0.9.0 | Retained SDK provider type identity |
| Brotli | 8.0.4 | 9.1.1 | Retained SDK codec tuple and single-copy contract |
| Maintenance reqwest | 0.12.28 | 0.13.5 | SDK pins 0.13.4; unifying maintenance native-TLS features into that client changes its implicit backend selection |
| Spin Git dependency | 7.0.0 at `6c59ed5b4fcb377072763aee0b27a3abdd8edb72` | Maintained version-8 candidate | Retained SDK's provider/fork identity and eager-progress repair |

The SDK's reqwest remains 0.13.4. Maintenance retains its existing native TLS, redirect, timeout, gzip and bounded-read behavior. Older versions remain under the explicit owner constraints below. A duplicate is not removed by silently changing its owner's features or public types. Tool versions, workload limits, reservations, resource ceilings and the additive budget are unchanged.

### Duplicate-Version Dispositions

The locked graph contains 468 packages and 26 duplicate package names (56 resolved versions in those families), versus 474 packages and 37 duplicate names at the baseline. The table accounts for every duplicate family, not only direct workspace requirements. It combines target, build and development dependencies: membership is not evidence of deployed-runtime inclusion or an all-target memory proof. Constraints below are Cargo requirements from the resolved owners; compatible additional owners do not justify deleting an incompatible older cohort.

| Family | Resolved Versions | Retention Owner And Constraint | Disposition |
| --- | --- | --- | --- |
| base64 | 0.22.1 / 0.23.1 | Both reqwest versions require `^0.22`; Auctioneer CLI/core/runtime select `=0.23.1`, hyper-util 0.1.21 accepts `^0.23`. | Upstream client cohort coexists with the project-selected version. |
| bit-set | 0.8.0 / 0.11.1 | fancy-regex 0.20.2 requires `^0.8`; proptest 1.12.0 requires `^0.11`. | Upstream schema and property-test cohorts. |
| bit-vec | 0.8.0 / 0.10.2 | bit-set 0.8.0 requires `^0.8.0`; bit-set 0.11.1 requires `^0.10.1`, proptest requires `^0.10`. | Follows the incompatible bit-set cohorts. |
| bitflags | 1.3.2 / 2.13.2 | fastly-shared/sys 0.13.1 require `^1.3.2`; current owners such as png 0.18.1 (`^2.0`), wasmparser (`^2.4.1`) and proptest (`^2.10`) use v2. | Upstream Fastly constraint; no direct project declaration. |
| block-buffer | 0.9.0 / 0.12.1 | sha2 0.9.9 requires `^0.9`; digest 0.11.3 requires `^0.12`. | Old Fastly crypto versus current crypto closure. |
| core-foundation | 0.9.4 / 0.10.1 | system-configuration 0.7.0 requires `^0.9`; rustls-platform-verifier 0.7.1 and security-framework 3.7.0 require `^0.10`. | Upstream Apple platform integrations. |
| cpufeatures | 0.2.17 / 0.3.1 | sha2 0.9.9 requires `^0.2`; sha2 0.11.1 and chacha20 0.10.2 require `^0.3`. | Old Fastly crypto versus current crypto closure. |
| darling | 0.20.11 / 0.23.0 | derive_builder_core 0.20.2 requires `^0.20.10`; validator_derive 0.20.1 requires `^0.23`. | Handlebars/derive-builder versus Validator upstream cohorts. |
| darling_core | 0.20.11 / 0.23.0 | Each darling and darling_macro cohort requires its exact matching version. | Exact upstream companion requirements. |
| darling_macro | 0.20.11 / 0.23.0 | darling requires its exact matching macro version. | Exact upstream companion requirements. |
| digest | 0.9.0 / 0.11.3 | sha2 0.9.9 requires `^0.9`; sha2 0.11.1 requires `^0.11`, hmac 0.13.0 requires `^0.11.2`. | Old Fastly crypto versus project-selected SHA2/HMAC. |
| getrandom | 0.2.17 / 0.3.4 / 0.4.3 | ring 0.17.14 requires `^0.2.10`; ahash requires `^0.3.1`; jsonschema/jsonschema-value/referencing 0.61.2 require `^0.3.4`; Auctioneer privacy, jobserver and quinn-proto accept `^0.4`, rand `^0.4.0`, tempfile `>=0.3.0, <0.5`. | Project's direct 0.4 declaration is WASM-dev-only; schema 0.3 edges are WASM-specific. Other incompatible cohorts are upstream. |
| matchit | 0.7.3 / 0.8.4 / 0.9.2 | worker 0.8.5 requires `^0.7`; axum 0.8.9 requires `=0.8.4`; edgezero-core requires `^0.9`. | Three upstream framework/provider requirements. |
| object | 0.39.1 / 0.41.0 | ar_archive_writer 0.5.5 requires `^0.39`; Auctioneer bidder's native-dev artifact inspection selects `=0.41.0`. | Upstream archive-building cohort versus project-selected inspection library; not a backtrace dependency. |
| r-efi | 5.3.0 / 6.0.0 | getrandom 0.3.4 requires `^5.1`; getrandom 0.4.3 requires `^6`. | Upstream EFI backend cohorts. |
| reqwest | 0.12.28 / 0.13.4 | Auctioneer CLI declares `^0.12`; edgezero-adapter-axum requires `=0.13.4`. | The older version is project-retained maintenance TLS behavior, not an upstream-only duplicate. |
| sha2 | 0.9.9 / 0.11.1 | fastly 0.13.1 requires `^0.9.3`; Auctioneer and EdgeZero SHA2 owners declare `^0.11`. | Old Fastly constraint versus current project/SDK crypto; baseline 0.10 removed. |
| strum | 0.27.2 / 0.28.0 | worker 0.8.5 requires `^0.27`; worker-macros 0.8.7 requires `^0.28`, jsonschema requires `^0.28.0`. | Upstream Worker runtime versus macro/schema cohorts. |
| strum_macros | 0.27.2 / 0.28.0 | Each strum cohort requires its matching minor series. | Follows the upstream Strum cohorts. |
| syn | 1.0.109 / 2.0.119 / 3.0.10 | fastly-macros and spin-macro require `^1.0`; the project-vendored jsonschema compiler and validator_derive require `^2`, derive-builder/Darling `^2.0.15`; workspace code-generation owners and updated upstream macros require v3 (`^3` for workspace owners). | Workspace selects v3; project retains the vendored compiler's upstream v2 manifest, not an unrecorded forced major migration. Provider v1 remains upstream-constrained. |
| thiserror | 1.0.69 / 2.0.21 | fastly 0.13.1 requires `^1.0.40`; Auctioneer/EdgeZero owners and Handlebars/JNI use `^2`, Quinn `^2.0.3`, Spin `^2.0.17`. | Project selects v2; Fastly retains v1. |
| thiserror-impl | 1.0.69 / 2.0.21 | Each thiserror version requires its exact matching implementation version. | Exact upstream companion requirements. |
| wasm-encoder | 0.262.0 / 0.263.0 | wasm-metadata and wit-component 0.262.0 require `^0.262.0`; Auctioneer bidder's native-dev fixture writer selects `=0.263.0`. | Upstream WIT generation versus project-selected fixtures. |
| wasmparser | 0.262.0 / 0.263.0 | wasm-encoder/wasm-metadata/wit-component/wit-parser 0.262.0 require `^0.262.0`; Auctioneer bidder selects `=0.263.0`, its matching encoder requires `^0.263.0`. | Same upstream WIT versus project inspection/fixture split. |
| windows-sys | 0.52.0 / 0.61.2 | ring 0.17.14 requires `^0.52`; Tokio/Mio/Schannel use `^0.61`, while other platform owners accept ranges including 0.61.2. | Ring's AArch64-Windows requirement retains 0.52; no direct project declaration, baseline 0.60 removed. |
| wit-bindgen | 0.51.0 / 0.57.1 / 0.63.0 | fastly-sys 0.13.1 requires `^0.51.0`; wasip2 1.0.4+wasi-0.2.12 requires `^0.57.1`; wasip3 0.10.0+wasi-0.3.2 requires `^0.63.0`. | Upstream Fastly, WASIp2 and WASIp3 cohorts. |

The actual older Object path is `pest 2.9.6 -> pest_stack_guard 1.0.1 -> stacker 0.1.25 -> psm 0.1.32 -> ar_archive_writer 0.5.5 -> object 0.39.1`. PSM's archive-writer edge is a build dependency; pest_stack_guard also directly requires PSM. There is no `backtrace` package in this lockfile. The previous backtrace attribution was incorrect. The graph audit reconciled all 468 package identities against current Cargo metadata after the unused bidder build-only Hex edge was removed; that removal changes neither the lockfile bytes nor any duplicate-version disposition.

### Explicit TOML Grammar Decision

On 2026-11-13 the user approved: **"Adopt TOML 1.1 and add explicit controls (Recommended)."** Auctioneer-owned build-time and CLI readers therefore intentionally adopt the selected TOML 1.1 grammar, matching EdgeZero's existing serving grammar; this is not a grammar-preserving patch upgrade. TOML table insertion-order preservation remains disabled. Typed validation, serving configuration, input limits and the one-runtime-config contract are unchanged.

The real CLI inventory-reader owner now pins `"\x41"` to `"A"` and `"\e"` to the escape character using literal expectations. It rejects incomplete/invalid hex escapes, an unknown escape, a surrogate escape, an unterminated string, duplicate keys, duplicate tables and invalid UTF-8 through the existing `InventoryInput` error. An independent same-source old/new library comparison rejects both valid 1.1 escapes under TOML 0.8.23, accepts them under selected TOML 1.2.1+spec-1.1.0, and retains all malformed-input refusals. This comparison is grammar-change evidence, not a claim that production code was changed from red to green in this review round.

## Compatibility Repairs

SHA-256 output now uses the existing workspace `hex::encode` rather than formatting the newer digest array with an unsupported `LowerHex` implementation. Expected signatures, capture digests and original reference bytes are not regenerated. HMAC's key construction imports its new `KeyInit` trait without changing the envelope or verification algorithm.

Syn consumers use the maintained trait-implementation, receiver, safety, function-pointer and guarded-pattern AST shapes. Object/DWARF readers use typed flags, checked writer offsets, fallible imports, decoded attributes and absolute tree depth while retaining their refusal predicates and input limits. The PNG decoder handles an absent bounded output size as `LimitsExceeded`.

Tokio's new descriptor registration API requires one user-approved, narrowly checked unsafe exception in the private stdout writer; the original explicit decision was **"Allow one audited registration call."** The review correction narrows that exception to socket and terminal output, where no safe Tokio constructor applies. FIFO output instead transfers the duplicated descriptor to the safe `pipe::Sender::from_owned_fd` constructor, which checks pipe type and write access and sets nonblocking mode. Its readiness/`try_write` loop retains the same two-second deadline as the socket/terminal path. The fallback's owned descriptor remains private and valid across readiness waits, cancellation and timeout. No dependency, feature or workspace unsafe policy is widened, and the existing full-pipe deadline test is byte-unchanged.

The added inline selection controls first compiled and reported `2 passed; 2 failed`: FIFO output still reached generic registration, and a read-only pipe was admitted. After the safe FIFO branch was added, the same four controls reported `4 passed; 0 failed`; the file/dev-null and socket controls remain positive witnesses for the unchanged paths. These targeted results establish the requested routing correction, not complete current-head acceptance; the broader verification and its limitations are recorded below.

The WASM parser now enables async components by default. The existing negative stream control explicitly disables the proposal, rather than relying on the superseded default; positive static component support and malformed-payload refusal remain tested. Local guest controls use the `.tool-versions` Wasmtime 48.0.2, not the unrelated version 45.0.0 found first on the shell's PATH.

## Compiler And Unicode Provenance

The authentic 0.61.2 compiler archive, source revision, all 280 published members, retained license and nine patched member identities are checked by the existing source owner. Three versioned patches replay separately and collectively; the original 0.58.6 archive and patches remain unchanged as historical evidence. See [vendor documentation](../../../vendor/README.md).

The new upstream repetition-count and collapsed-pattern paths bypass original engine construction. The rebase preserves original-pattern construction and its default/supplied limits in every generated mode. A real pre-correction run of `repeated_and_collapsed_patterns_cannot_bypass_engine_construction_limits` failed because an oversized pattern produced no refusal. After correction the same test and all existing initialization/cache controls passed. The source-compatibility failure in the unchanged constructor-observing target was also recorded; its wrapper was not expanded to hide the constructor substitution.

Unicode 16 uses an independent official C/S oracle, not the upgraded library to generate expected classes. It covers all 1,112,064 scalar values and retains the Go 1.23.12 / Unicode 15 capture unchanged. Exactly 30 pairs merge former singleton classes; all 1,424 old nontrivial classes are unchanged and no class splits. The new Cyrillic pair control failed against the old dependency and passed after upgrade. Source identities, attribution, complete delta and oracle boundaries are in the [fixture record](../../../crates/auctioneer-config-data/tests/fixtures/unicode/README.md).

## Executed Local Checkpoints

These are executed working-tree checkpoints, not blanket acceptance of the final commit. Earlier family checks precede the final compiler/source-reader changes. The final whole-tree and hosted results must be read separately.

| Command/Owner | Actual Result |
| --- | --- |
| Core signed-cookie units | `17 passed; 0 failed; 657 filtered out` |
| Privacy library | `291 passed; 0 failed` |
| Config library and root shape | `419 passed; 0 failed`, then `5 passed; 0 failed` |
| Macro library and conformance owner | `23 passed; 0 failed`, then `3 passed; 0 failed` (26 compiler cases) |
| Unicode/activity owners | `8 passed; 0 failed`, and `2 passed; 0 failed` |
| Pixel/reference owner | `18 passed; 0 failed` |
| Compiler constructor/cache owners | `15 passed; 0 failed`, `1 passed; 0 failed`, `2 passed; 0 failed` |
| Compiler compatibility and cold native allocation | `193 passed; 0 failed`, and `1 passed; 0 failed` |
| Exact vendor inventory/refusal controls | `16 passed; 0 failed; 314 filtered out` |
| Portable patch replay controls | `3 passed; 0 failed; 327 filtered out` |
| Artifact reader group with pinned Wasmtime | `180 passed; 0 failed; 150 filtered out` |
| DWARF layout group | `15 passed; 0 failed; 676 filtered out` |

An initial complete default-workspace lane exited 101: 6,533 passed, 39 failed, 19 ignored across 245 result summaries. All 39 failures were in three CLI source/evidence targets. The captured artifact bound `inspection.rs` to SHA-256 `04806a2ec28a655b545d586d0216ef4db955562e441cac0156172adcdfaff8ec`, while its subsequently edited file was `107bd610215e29760caf2957d8a8289f6002d671d2776b09cee418ed3d50e0ac`. This is a real nonpassing run, not a waived failure. The source guard remains unchanged; final verification must rebuild against frozen source and rerun the lane.

## Frozen Native Verification

The subsequent complete default-workspace lane exited 0, with 245 libtest summaries totaling **6,572 passed, 0 failed, 19 ignored**:

```sh
cargo test --locked --offline --workspace --all-targets --no-fail-fast -- \
  --skip tests::selected_schema_source_producer_records_exact_real_dispatch_without_qualification \
  --skip tests::controlled_native_acceptance_requires_all_producers_but_never_completes_the_full_catalog
```

The only subsequent source change simplified the artifact reader to positively admit ELF/Mach-O and reject other formats, without changing its predicates or adding a lint exception. After that cleanup, all 180 artifact-reader controls passed again. Strict workspace Clippy and explicitly selected `schema_build_controls` / `schema_source_conformance` Clippy both exited 0 under `-D warnings`. The latter targets have explicit CI ownership and are not covered by broad `--all-targets` selection alone.

The source-bound CLI targets were rebuilt and rerun after that final cleanup, not credited from an earlier artifact:

```text
test result: ok. 776 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 10.97s
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 49.15s
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.05s
```

Those are the CLI binary, `bidders_evidence` and `bidders_resources`, respectively. The two producer exclusions are the same ones owned by the existing mandatory producer lane, not new skips. `cargo fmt --all -- --check`, raw-JSON fixture checks and refusal fixtures, batch rebase, roster checks and refusal fixtures, and all 18 libtest-summary controls passed. The raw-JSON gate reported `203 raw JSON fixture(s) across 56 file(s), all budgeted`.

The scoped read-only author audit found no cryptographic, Unicode or compiler-patch semantic blockers. It did not execute tests and is not formal independent approval.

## Hosted Schema Repairs

The hosted matrix at `8c70466ea62ceca14dc173fafced7981c0aefe7b` completed with 23 successful checks and two schema failures. Both failures were reproduced locally before correction; they are not waived or relabeled as environmental failures.

The locked `wasm-bindgen` upgrade from 0.2.126 to 0.2.130 adds `__wbindgen_externref_xform__.__wbindgen_externref_statics_end`, the `() -> i32` fixed-reference-table-boundary intrinsic. Inspection of the actual linked bare artifact with the locked `wasmparser` established its namespace, name and signature; the locked upstream `src/externref.rs` declaration and its clone/drop uses were audited. The exact import policy now recognizes that one additional function. It still refuses other namespaces, names, non-function imports and duplicates. Recognition does not provide an implementation: the isolated runner retains `unknown-imports-trap=y` and cannot silently substitute a default value.

Before correction, the new exact-name policy row and expanded real-runtime control each failed with `unreviewed bare probe import`. After correction, all 13 bare controls and 18 guest-runner controls passed. The latter execute each of the four reviewed imports with its actual ABI, both unused (successful integer export) and invoked (required trap), including discarding imported return values so a default-returning stub cannot satisfy the negative control.

The second failure was a source-cohort fingerprint, not a serving reservation: the maximum reverse sparse entries changed from 292 to 336. The upstream 0.61.2 ECMA translator now expands wildcard dot to `[^\n\r\x{2028}\x{2029}]`. The imported `limelightDigital` pattern `^.+\..+$` therefore has 84 reverse states and four sparse slots per state. Pangle helper 1 also changes: emitted `.+` becomes `[^\n\r\x{2028}\x{2029}]+`, while Limelight helper 0 becomes `^[^\n\r\x{2028}\x{2029}]+\.[^\n\r\x{2028}\x{2029}]+$`. Existing anchors and escaped literal dots remain unchanged. The unchanged `yandex` helper still determines the largest reported cache-pair size (3,550 bytes on the native 64-bit host); the reverse stack maximum remains 31. Per-pattern diagnostics now precede the assertions so a future drift identifies its owner.

The original [baseline compiler-conformance job](https://github.com/IABTechLab/auctioneer/actions/runs/43282970105/job/131408834635) and [upgraded job](https://github.com/IABTechLab/auctioneer/actions/runs/43388973608/job/131764069481) report these 64-bit fixed-native-meta-cache bounds:

| Helper | Forward Live Bytes, Old -> New | Reverse Live Bytes, Old -> New | Composed Peak Bytes, Old -> New | Reverse Ordinary Graph States, Old -> New |
| --- | --- | --- | --- | --- |
| pangle helper 1 | 14,271 -> 16,683 | 14,119 -> 21,406 | 66,894 -> 76,753 | 21 -> 30 |
| limelightDigital helper 0 | 12,643 -> 15,939 | 16,435 -> 22,227 | 68,382 -> 77,790 | 17 -> 21 |

Comparison of all 25 layout-owner pattern/config digest bindings finds exactly these two changes, with no owner additions or removals. These are source-catalogue candidate bounds, not new serving reservations, allocation measurements or independent all-target qualification.

Only the source-cohort exact fingerprint changes to 336. The synthetic 292-entry ceiling, all checked storage formulae, the maximum cache-pair assertion, serving reservations, workload and resource ceilings remain unchanged. A new generated-pattern control pins the literal translation, rejects LF, CR, U+2028 and U+2029 separately, and preserves ordinary ASCII, a multibyte character, vertical tab and NEL. The executed secure/development selection manifest contains four generated regex helpers, all owned by OpenX or Rubicon; their dots are escaped literals, not wildcard atoms. This imported-catalogue correction does not introduce a new matching policy for the currently selected serving schemas or promote an unimplemented bidder.

The complete affected CI owners then passed, using the pinned Wasmtime 48.0.2 and the existing source-checkout prerequisites:

```sh
bash .github/workflows/scripts/test/verify-schema-compiler-conformance.sh schema
```

Exit 0, including strict explicit-target Clippy:

```text
test result: ok. 330 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 903.38s
test result: ok. 686 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 360.62s
```

The six filters are the existing separately owned application/layout/selected-probe checks, not new exclusions. The originally failing selected-probe owner also ran separately, executing the full native, bare-WASM, WASI 1 and WASI 2 cohort in both secure and development lanes:

```sh
bash .github/workflows/scripts/test/verify-schema-source.sh 1 \
  tests::selected_schema_probe_links_and_executes_the_source_bound_cohort \
  --locked -p auctioneer-bidder
```

Exit 0:

```text
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 691 filtered out; finished in 279.47s
```

`cargo test --locked --offline -p auctioneer-core --test codec_pins` also exited 0:

```text
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Strict workspace Clippy, ordinary Cargo formatting, dedicated discovered-bidder formatting and `git diff --check` each exited 0. The lockfile SHA-256 remains `9cd3a303b4cebde1619220ef8a3eb67a7d1540ec836d308bf2571f28040746fe`; the EdgeZero PR #275 revision and constraints above are unchanged. The scoped read-only author audit found no import-policy safety regression; it ran no builds and is not independent PR approval. The corrective commit still requires its complete hosted matrix and independent review.

## Review-Correction Verification (2026-11-13)

The repair adds one inline CLI grammar test, removes only bidder Hex's unused build dependency, and corrects the owner/provenance/current-versus-historical records above. It changes no parser implementation, serving path, workload limit, budget constant, captured expected value or SDK revision. The original seven review threads remain open for REVIEWER acceptance.

With executable sources frozen, the complete existing default-workspace lane was rerun:

```sh
cargo test --locked --offline --workspace --all-targets --no-fail-fast -- \
  --skip tests::selected_schema_source_producer_records_exact_real_dispatch_without_qualification \
  --skip tests::controlled_native_acceptance_requires_all_producers_but_never_completes_the_full_catalog
```

Exit 0: **245 actual libtest result summaries, totaling 6,574 passed, 0 failed and 19 ignored**. The two skipped producer cases retain their unchanged mandatory CI owner; these are not new skips. Actual result lines include the full CLI binary and its source-bound evidence and resource targets:

```text
test result: ok. 777 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 12.35s
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 54.16s
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.55s
```

The focused commands also exited 0:

```sh
cargo test --locked --offline -p auctioneer-cli --bin auctioneer-cli bidders::check::tests:: -- --nocapture
cargo test --locked --offline -p auctioneer-runtime --lib debug_budget::tests::
cargo check --locked --offline -p auctioneer-bidder --all-targets
cargo check --locked --offline -p auctioneer-bidder --all-targets --features test-support
cargo clippy --locked --offline --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

```text
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 776 filtered out; finished in 0.02s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 269 filtered out; finished in 0.00s
Finished `dev` profile [unoptimized] target(s) in 21.68s
Finished `dev` profile [unoptimized] target(s) in 3.92s
Finished `dev` profile [unoptimized] target(s) in 50.14s
```

The independent literal grammar comparison produced two expected old-library failures (`1 passed; 2 failed`) under TOML 0.8.23, versus `3 passed; 0 failed` under selected 1.2.1. A read-only standard-parser audit reconciled every duplicate-family/version row against Cargo.lock and all 468 package identities against Cargo metadata; three new documentation links resolve. Cargo.lock retains the SHA-256 above. The raw-JSON gate reported `203 raw JSON fixture(s) across 56 file(s), all budgeted`; its refusal controls, batch/roster gates and all 18 libtest-summary controls also exited 0.

Two local launcher attempts did not complete their intended checks: one omitted `asdf` from PATH (exit 127 before Cargo); another selected macOS Python 3.9 without `tomllib`. Restoring the existing PATH and using the already installed Python 3.14 standard parser completed the respective checks without changing repository code or adding dependencies. These attempts are not credited as passing checks.

A scoped read-only author audit found no introduced defects in the six repaired files; it executed no builds and is not formal PRfection approval. All 25 hosted checks passed at the preceding head `e3a0e22c2e9bd2d9938a14f1d939ad44c18accb4`. Those are historical results, not acceptance of this fixing revision. Its complete hosted matrix and independent re-review remain mandatory.

## Safe FIFO Review Verification (2026-11-14)

REVIEWER accepted and resolved the seven preceding findings at `d773e7fabb2cad740e924e7caa727ecbd4cb25ad`. This correction addresses the subsequent FIFO finding; its thread remains open for REVIEWER acceptance. The public `qualification_harness.rs` test file, dependency manifests, lockfile and workflows are byte-unchanged from that head.

The first strict lint run caught `clippy::similar_names` in the new socket control (`received` versus `receiver`). Renaming only the buffer to `payload` corrected it without an allowance or assertion change. The final executable sources then passed these commands, each with exit 0:

```sh
cargo test --locked --offline -p auctioneer-cli --bin auctioneer-cli qualification::harness::stdio::unix::tests:: -- --nocapture
cargo test --locked --offline -p auctioneer-cli --test qualification_harness
cargo clippy --locked --offline --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
git diff --check
```

Actual result lines:

```text
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 780 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.27s
Finished `dev` profile [unoptimized] target(s) in 35.88s
```

The broader local sweep ran before that final test-only buffer rename. Its commands and actual outcomes were:

```sh
cargo test --locked --offline --workspace --all-targets --no-fail-fast -- \
  --skip tests::selected_schema_source_producer_records_exact_real_dispatch_without_qualification \
  --skip tests::controlled_native_acceptance_requires_all_producers_but_never_completes_the_full_catalog
cargo test --locked --offline -p auctioneer-bidder -p auctioneer-exchange -p auctioneer-core --all-targets
```

| Local Lane | Exit | Actual Summaries | Passed | Failed | Ignored |
| --- | ---: | ---: | ---: | ---: | ---: |
| Default workspace | 101 | 245 | 6,577 | 1 | 19 |
| Secure bidder/exchange/core | 0 | 154 | 3,890 | 0 | 11 |

The complete CLI unit binary in the default lane reported `781 passed; 0 failed; 3 ignored`. The failed target was Axum's `usersync_transport`, not a passing or waived check:

```text
tests::real_binary_remains_available_after_partial_body_disconnect
assertion `left == right` failed: overloaded
  left: 503
 right: 200
test result: FAILED. 11 passed; 1 failed; 1 ignored; 0 measured; 0 filtered out; finished in 1.35s
```

The failure is at `crates/auctioneer-adapter-axum/tests/usersync_transport.rs:658`; this correction does not modify that file. Its cause has not been established or reproduced at the exact base with the same full command, so it is not classified as pre-existing. No retry erased this failed sweep, and no assertion or timeout was weakened.

The explicit insecure-workspace sweep was interrupted during compilation (process-group exit 130) as concurrent builds reduced free disk from 65 GiB to below 25 GiB; it produced no completed test result and is not a pass. The later standalone CLI, maintenance and insecure-Clippy steps in that batch were not run. No repository test, workflow, exclusion or feature lane was removed. The complete hosted matrix remains mandatory for the carrying commit, including the existing separately owned producer cases.

The unchanged raw-JSON gate reported `203 raw JSON fixture(s) across 56 file(s), all budgeted`; its negative controls, batch rebase, roster and roster-negative controls each exited 0. The roster enumerated 139 uniquely owned members, and libtest-summary controls reported `18 passed`. A read-only author-side audit found no issue in the two-file correction; it ran no builds and is not formal independent approval. Cargo.lock retains the SHA-256 recorded above. Overall acceptance remains blocked by the failed local target, incomplete current-commit matrix and required REVIEWER re-review.

## Outstanding Acceptance

The first hosted run at `651c0874a1bbf5a2b286cc3e3adc4ad88b68a797` passed ordinary `cargo fmt`, but its separate discovered-bidder source formatting step found one assertion layout in MSFT conformance that ordinary Cargo traversal does not visit. The correction changes only whitespace, not the assertion or fixture. The exact dedicated check (`find crates/auctioneer-bidder/src/bidders -type f -name '*.rs' -exec rustfmt --edition 2024 --check {} +`) then passed locally alongside `cargo fmt --all -- --check`. All bidder library tests were rerun after correction:

```text
test result: ok. 1229 passed; 0 failed; 9 ignored; 0 measured; 0 filtered out; finished in 0.50s
```

Any successful hosted checks on the earlier commit are not credited as completion of the corrective commit's matrix.

The full hosted matrix remains required: secure/insecure/maintenance/conformance lanes, fresh compiler/producer experiments, normal/staged source controls, actual guest execution and provider packaging. Current-head hosted results and independent implementation review remain pending; local default-workspace success is not a substitute. The two existing workspace producer exclusions retain their separate mandatory CI owner; this record does not create another exclusion. Missing tools, unavailable storage or unexecuted checks are not passes. No hosted acceptance, deployment readiness, issue completion or merge is claimed by this checkpoint. The SDK-constrained candidates above are not upgraded or certified by this work.

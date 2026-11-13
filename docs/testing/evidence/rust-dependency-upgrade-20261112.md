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

The SDK's reqwest remains 0.13.4. Maintenance retains its existing native TLS, redirect, timeout, gzip and bounded-read behavior. Older transitive major versions remain when their owners require them, including Syn 1/2 alongside workspace Syn 3, the compiler's own Syn 2, and the backtrace/tooling object and WASM parser versions. A duplicate is not removed by silently changing its owner's features or public types. Tool versions, workload limits, reservations, resource ceilings and the additive budget are unchanged.

## Compatibility Repairs

SHA-256 output now uses the existing workspace `hex::encode` rather than formatting the newer digest array with an unsupported `LowerHex` implementation. Expected signatures, capture digests and original reference bytes are not regenerated. HMAC's key construction imports its new `KeyInit` trait without changing the envelope or verification algorithm.

Syn consumers use the maintained trait-implementation, receiver, safety, function-pointer and guarded-pattern AST shapes. Object/DWARF readers use typed flags, checked writer offsets, fallible imports, decoded attributes and absolute tree depth while retaining their refusal predicates and input limits. The PNG decoder handles an absent bounded output size as `LimitsExceeded`.

Tokio's new descriptor registration API requires one user-approved, narrowly checked unsafe exception in the private stdout writer. The owned descriptor remains private and valid across readiness waits, cancellation and timeout; the existing full-pipe deadline control ran successfully. No workspace unsafe policy is widened.

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

## Outstanding Acceptance

The full hosted matrix remains required: secure/insecure/maintenance/conformance lanes, fresh compiler/producer experiments, normal/staged source controls, actual guest execution and provider packaging. Current-head hosted results and independent implementation review remain pending; local default-workspace success is not a substitute. The two existing workspace producer exclusions retain their separate mandatory CI owner; this record does not create another exclusion. Missing tools, unavailable storage or unexecuted checks are not passes. No hosted acceptance, deployment readiness, issue completion or merge is claimed by this checkpoint. The SDK-constrained candidates above are not upgraded or certified by this work.

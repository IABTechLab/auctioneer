# Complete Rust Dependency Upgrade

## Purpose And Authority

Deliver [#165](https://github.com/IABTechLab/auctioneer/issues/165), under the existing release-maintenance epic [#12](https://github.com/IABTechLab/auctioneer/issues/12). The user approved upgrading all Rust dependencies, including major releases, Git SDKs, vendored-patch rebases, compatibility fixes and refreshed current proofs on 2026-11-11. Preserve the full adapter acceptance matrix, independent tests and resource limits.

The requested outcome is one Auctioneer dependency-upgrade implementation PR, with reviewable commits by dependency family. Separate dependency-family PRs would simplify individual reviews but prolong convergence and repeatedly invalidate shared lockfile/proof identities. A compatible-only lockfile refresh is insufficient: it omits the explicitly requested major and exact-pinned upgrades. This document defines the common contract before those changes.

The user initially authorized a separate, coordinated EdgeZero upgrade first and approved this written spec, including the Unicode 16 proposal, on 2026-11-11. On 2026-11-12 the user directed implementation in existing Auctioneer PR #166 without a new PR, clarified that EdgeZero must stay on PR #275, and instructed CREATOR to continue. This later direction supersedes the separate SDK-first delivery gate for independent Auctioneer upgrades and the separate implementation-PR step. It does not authorize editing PR #275's checkout, forcing incompatible SDK dependencies, weakening acceptance, or merging either repository.

Retain the existing immutable PR #275 SDK revision and its shared provider/HTTP/JSON/codec constraints while upgrading the remaining graph. [EdgeZero #467](https://github.com/IABTechLab/edgezero/issues/467) remains the owner of future upstream tuple work, not a prerequisite to independent upgrades in #166. Record every version held back by that source explicitly; do not call a constrained package latest. The full all-latest outcome remains unavailable until the corresponding upstream constraints change.

## Baseline And Inventory

The planning baseline is `main` at `bc2193096920752cf3b149a1496f9deee1432f8f`, with Rust 1.98.1 and the EdgeZero revision selected by that commit's manifest/lockfile. Use an isolated worktree; preserve the user's modified Fastly configuration, untracked local configuration and unrelated worktrees.

PR #162 is parallel work, not part of this baseline. If it lands first, rebase on the resulting main, preserve its ownership fixes, refresh the inventory and rerun the affected acceptance. Do not duplicate its code or claim its unmerged proof index is already shipped.

The following are registry observations from 2026-11-11, not an upgraded build or a promise that upstream releases remain unchanged. `cargo upgrade --dry-run --incompatible allow --pinned allow --recursive false` reported these 23 workspace requirement changes:

| Dependency | Existing Requirement | Observed Candidate |
| --- | --- | --- |
| hmac | =0.12.1 | =0.13.0 |
| jsonschema | =0.58.6 | =0.61.1 |
| jsonschema-macros-core | =0.58.6 | =0.61.1 |
| fastly | =0.13.1 | =0.13.2 |
| brotli | =8.0.4 | =9.1.1 |
| getrandom | 0.3 | 0.4 |
| gimli | =0.32.3 | =0.34.0 |
| ipnetwork | 0.20 | 0.21 |
| lru | 0.12 | 0.19 |
| mime | 0.3.17 | 0.3.18 |
| object | =0.37.3 | =0.41.0 |
| png | =0.17.16 | =0.18.1 |
| reqwest | 0.12 | 0.13 |
| regex | =1.12.4 | =1.13.1 |
| sha2 | 0.10 | 0.11 |
| zeroize | 1.8 | 1.9 |
| syn | 2 | 3 |
| thiserror | 1 | 2 |
| toml | 0.8 | 1.2 |
| unicode-case-mapping | =0.5.0 | =1.0.0 |
| wasmparser | =0.247.0 | =0.263.0 |
| wasm-encoder | =0.247.0 | =0.263.0 |
| worker | =0.8.5 | =0.9.0 |

The unchanged-requirement lockfile dry run reported 147 compatible package changes, including updates hidden by broad requirements and dependencies inherited through the SDK. Inventory all resolved package IDs, not only unique crate names: valid different-major instances can coexist. Classify each as upgraded, already current, removed because its owner no longer needs it, or explicitly constrained by an identified upstream requirement. Do not force a transitive major across an upstream API contract or call a retained old instance current merely because another major was upgraded.

`cargo-outdated 0.19.0` failed because its temporary project omitted the local vendor paths. That failure is not evidence that dependencies are current or that Auctioneer is broken. Use the two successful Cargo dry runs, Cargo metadata and direct upstream registry/source inspection; do not alter production manifests to accommodate that audit tool.

Refresh the inventory once at implementation selection, then freeze exact registry versions, Git revisions, source archives and patch identities for verification. Report any newer release discovered after that freeze rather than repeatedly changing the tested graph during acceptance.

Audit versioned Git dependencies separately from the registry dry run. The Spin fork's default branch is not its pinned repair revision. Independent upstream inspection found [v8.0.0](https://github.com/spinframework/spin-rust-sdk/tree/acf428a09f516dd1b9fe6ddfd33038f5b0f3e7e0) at `acf428a09f516dd1b9fe6ddfd33038f5b0f3e7e0` and the required [eager-progress repair #169](https://github.com/spinframework/spin-rust-sdk/pull/169) merged on 2026-11-03. Treat that maintained upstream source as a candidate to replace the version-7 fork, subject to the coordinated interface/progress gates below; the registry-only report did not establish that Spin was current. A missing GitHub Release object does not mean a release tag or registry package is absent.

## Dependency Ownership

Keep declarations in `[workspace.dependencies]`, inherited by each crate. `Cargo.lock` binds the complete resolved graph, not just the workspace requirements. Do not add duplicate crate-local versions, legacy dependency aliases, new workspace crates, alternate loaders or unrelated refactors to make a migration easier.

Keep `.tool-versions` authoritative. Retain the current Rust and provider tools unless a selected dependency or packaging contract requires a reviewed tool change. A new Rust minimum requires checking the actual native and guest toolchain/ABI controls, not just changing the version string. Do not install prerelease dependency versions to satisfy this stable-upgrade request.

Keep features and platform ownership intentional. Native CLI fetches, artifact inspection and compiler qualification must not become guest serving dependencies. Preserve required worker/Spin/WASI features and the existing release/wasm-release packaging distinction. Resolve duplicate versions only when the actual owners permit unification; do not replace upstream constraints through an unreviewed global patch.

## SDK And Shared-Type Gate

The observed EdgeZero PR #275 head is `b97efec185abbaa901e633a980975c98ef2f6c3b`. This is an integration revision, not a published stable SDK release. Adoption must stay on the required retained-outbound/lifetime lineage and use an immutable official-repository revision; changing to an unrelated default branch is not an upgrade strategy.

A follow-up read during planning found head `14a6ca8bb5a5cbe055a1f725bdab8f42569a3bf0`, two commits ahead, changing Fastly lifecycle/progress-test files but not `Cargo.toml`. Direct inspection confirmed the shared constraints below remain. This is an audit update, not adoption or verification of those new commits. The separate SDK effort must select and review its carrying revision; do not chase a moving tip during downstream acceptance.

That source still exact-pins older versions in these groups:

| Group | Concrete Upstream Constraints | Required Decision And Evidence |
| --- | --- | --- |
| Provider interfaces | Fastly 0.13.1; Worker 0.8.5; the reviewed Spin version-7 fork/revision | A compatible SDK revision must own the newer provider ABI before Auctioneer entrypoints use its types. Qualify the upstream Spin version-8 candidate and its retained eager-progress repair before replacing the fork. |
| Shared HTTP/JSON | http 1.4.2; serde_json 1.0.150; bytes 1.12.1 | Bind interface types and raw-value/header storage controls to the selected tuple. Do not introduce incompatible duplicate boundary types. |
| Native HTTP | reqwest 0.13.4 | Verify resolver compatibility and native transport policy. The maintenance fetcher and SDK client have different owners; preserving a crate name does not preserve TLS, redirects, pooling or timeout behavior. |
| Codec stack | async-compression, compression-codecs/core, brotli/decompressor, flate2, CRC/adler helpers | Reconcile the actual decoding tuple and window/framing/cleanup contract. Auctioneer's archive encoder can have a separate owner; do not relabel the SDK decoder as upgraded when only the encoder moved. |

A newer requirement is not assumed resolvable while an SDK exact pin remains. Obtain concrete resolver/source evidence. Cargo [rejects conflicting exact compatible-version requirements and treats different-version public types as distinct](https://doc.rust-lang.org/cargo/reference/resolver.html). The original SDK-first sequence is superseded narrowly by the 2026-11-12 direction above: retain the existing PR #275 revision for this implementation and upgrade independently owned dependencies around its constraints. Any later SDK adoption must still retain PR #275's required outbound/lifetime contract and publish the shared version tuple, native and guest verification, and unresolved qualification boundaries. Upstream constraints remain blockers to an all-latest outcome; an issue, release tag or successful resolver run is not delivery evidence.

The current `codec_pins` owner also requires one resolved copy of each asserted codec. Do not weaken that check merely to admit a newer build-time encoder beside an old SDK decoder. Default to a coordinated tuple upgrade; any replacement with a graph-scoped check requires independent build/runtime ownership evidence and at least the same rejection coverage before cutover.

## Compatibility And Proof Migrations

1. **General API/tooling:** migrate `syn`/`thiserror`/TOML, randomness/cache and object/DWARF/WASM-inspection call sites using maintained APIs. Continue generating Rust with `quote!` and parsed `syn` trees. Preserve parser rejection cases, config null/absence/override semantics, CLI validate/diff/push behavior and archive safety. Compilation alone does not prove these contracts.
2. **Cryptography and browser state:** upgrade HMAC/SHA together with their trait family, preserving the signed-cookie envelope, MAC bytes, active-key selection, strength/version checks, constant-time verification and complete-cookie ceiling. Use the existing captured/independent expectations and real handler controls. Do not regenerate expected signatures with the implementation under test.
3. **Schema compiler and regex:** acquire authentic new published source, record archive/VCS/license identity, and replay/rebase each required compiler patch independently. Remove a patch only when an upstream implementation passes its original rejection/lifecycle/cache controls without it. Keep the selected engine, schemas, feature boundaries and explicit cache ownership. Recheck already-current patched packages too. Do not write another schema converter or regex engine.
4. **Source-bound resource controls:** update current dependency tuples, portable patch inventories, compiler-emitted layout controls, parse/visitor and allocator regressions against the new source and all relevant ABIs. Do not guess private layouts or change an independent bound to the measured result. If a selected version violates an existing charged bound, stop that adoption pending an independently justified correction or reviewed design decision; do not raise a ceiling or reduce workload.
5. **Unicode and semantic changes:** the existing `simple_fold` owner explicitly asserts Unicode 15. The published `unicode-case-mapping 1.0.0` source uses Unicode 16. The user approved adopting Unicode 16 simple folding as the one current behavior, not adding a compatibility mode. Before cutover, report merged/split equivalence classes against the old contract and validate every scalar against independent official Unicode 16 C/S case-fold data, excluding full/locale folding. Preserve the Go 15 capture bytes, unchanged applicable historical cases, ASCII and special-case protections; do not merely change the expected version literal, filter out changed scalars or derive the new oracle from the upgraded Rust library. Do not add a new production folding engine or CLI just to produce fixtures. Any additional TOML, HTTP, JSON, URL or error semantic change requires its own explicit decision rather than being hidden as API repair.

Use versioned replacement patch/evidence assets where existing bytes remain referenced. Preserve external/reference captures, original source archives needed by historical records, frozen profiles and historical verification documents. A dependency upgrade changes current source identity; it does not retroactively recertify old measurements.

OpenRTB recommendations take precedence over reference-server quirks. The proposed Unicode-data migration is an Auctioneer matching-policy decision, not an OpenRTB mandate. Apart from that explicitly reviewed version change, this upgrade does not authorize changing auction messages, no-bid signaling, privacy decision tables, bidder/alias attribution, SSRF/HTTPS policy or one-runtime-config behavior. Surface any unavoidable further semantic change separately before implementing it.

## Acceptance And Completion

The workflows and [test ownership guide](../../testing/development.md) remain authoritative for the full matrix. Required validation includes all four ordinary native feature graphs, secure package controls, macro/config and standalone compiler suites, patch replay, both heavy evidence producers, real boot/CLI parity, fanout/onboarding, all guest triples, guest execution where already required, and provider artifact packaging/size gates. Preserve their existing process isolation, environmental prerequisites, feature modes and dispatch-count checks.

Run focused red/green controls for changed failure/compatibility branches, then the complete matrix at the carrying head. Capture current source/dependency identities, actual result lines, package/version deltas and all unresolved blockers. `--all-targets` does not substitute for explicit `test = false`, required-feature or hosted owners. Missing prerequisites and jobs that never start are not passes.

Existing source/ABI/container/allocator/provider proofs that were incomplete remain incomplete. This task refreshes implemented proofs and controls; it does not silently absorb the separate full schema or host-fit qualification backlog. Preserve the current accounting, reservations, readiness flags and known limitations unless an independently reviewed correction is part of this change. No upgraded target becomes qualified merely because tests or package-size checks pass.

Do not mark the implementation ready or close #165 while selected upgrades, required semantic decisions, SDK prerequisites or mandatory acceptance remain unresolved. A draft may report partial progress honestly; it is not the completed all-dependency deliverable. No merge is authorized by approval of this design approach.

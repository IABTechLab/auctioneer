# Complete Rust Dependency Upgrade Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Upgrade the Auctioneer Rust dependency graph within the existing PR #275 SDK constraints, preserving protocol behavior, safety limits and all existing acceptance owners. Report retained upstream constraints rather than claiming an all-latest graph.

**Architecture:** EdgeZero owns its provider/shared-type/codec tuple; Auctioneer retains its existing immutable PR #275 revision while migrating independently owned dependency families. Workspace requirements, the resolved lockfile and independently replayable vendor patches remain the sources of dependency identity. Unicode 16 is the one explicitly approved matching-policy change; historical captures remain immutable.

**Tech Stack:** Rust and provider tools from `.tool-versions`, Cargo, the current EdgeZero outbound contract, existing `quote!`/`syn` generators, patched upstream schema/regex libraries and existing native/guest test owners.

---

## Authority And Sequence

Spec: [Complete Rust Dependency Upgrade](../specs/2026-11-11-rust-dependency-upgrade-design.md). Tracking: [Auctioneer #165](https://github.com/IABTechLab/auctioneer/issues/165). The maintainer approved the written spec, Unicode 16 and the initial separate SDK-first coordination on 2026-11-11. On 2026-11-12 the user directed implementation in existing PR #166 without another PR, required EdgeZero to stay on PR #275, and instructed CREATOR to continue. This supersedes the separate planning-merge/implementation-PR sequence and the SDK-first gate for independent upgrades. Neither repository has merge authorization. [EdgeZero #467](https://github.com/IABTechLab/edgezero/issues/467) still owns future upstream tuple changes, not independent Auctioneer work.

Use PR #166's existing owned branch/worktree under that explicit user exception; do not open another Auctioneer PR or edit the PR #275 implementer's checkout. Task 1 now freezes the retained SDK and its constraints rather than waiting for another SDK delivery. Refresh `origin/main` and reconcile merged changes before freezing the implementation baseline. An unmerged PR #162 remains separate work, not source to copy or accepted proof.

A family can require several small commits. Keep the final deliverable one Auctioneer implementation PR, not one PR per package. A compile failure does not justify changing privacy, workload, resource or test contracts. A newly discovered behavioral migration beyond the approved Unicode change returns to review before implementation.

## File Ownership

| Owner | Files And Responsibility |
| --- | --- |
| Dependency graph | `Cargo.toml`, `Cargo.lock`, inherited `crates/*/Cargo.toml`; `.tool-versions` only if a justified compatibility decision changes a tool |
| Current upgrade record | `docs/testing/evidence/rust-dependency-upgrade-20261112.md` records the implementation selection, upstream constraints, source/patch identities, actual commands/results and outstanding blockers; not a new backlog |
| SDK adoption | `crates/auctioneer-adapter-{axum,cloudflare,fastly,spin}/src/`, `crates/auctioneer-core/tests/codec_pins.rs`, current acquisition instructions in `docs/testing/development.md` |
| Config and generators | `crates/auctioneer-config/src/`, `crates/auctioneer-config-data/build.rs`, `crates/auctioneer-macros/src/`, existing generated syntax/compiler tests |
| Crypto and utility APIs | `crates/auctioneer-core/src/usersync/signed.rs`, `crates/auctioneer-privacy/src/gvl.rs`, `crates/auctioneer-config/src/url_safety.rs`, `crates/auctioneer-openrtb/src/prepare/implicit_fill.rs`, native CLI maintenance modules |
| Schema compiler | `vendor/jsonschema-macros-core/`, a new versioned original `.crate` archive and portable patches, `vendor/README.md`, existing schema compiler/provenance owners |
| Regex source/layout | `vendor/regex-automata/`, existing or versioned replacement archive/patch, existing `schema_build`/`schema_*` layout and allocator controls |
| Unicode oracle | `crates/auctioneer-config/src/activity.rs`, `crates/auctioneer-config-data/tests/simple_fold.rs`; add official Unicode 16 source and attribution under `crates/auctioneer-config-data/tests/fixtures/unicode/` |
| Artifact inspection | Existing `crates/auctioneer-bidder/tests/schema_build/` modules and `crates/auctioneer-cli/src/qualification/` artifact/provider inspection modules |
| Acceptance | Existing workflows and colocated scripts in `.github/workflows/`; existing test targets, no new workflow snapshots, broad skips or test-target proliferation |

Use `rg` to derive the complete call-site set for each changed package. The table identifies owners, not permission to skip additional real consumers. Colocate new unit cases; extend existing integration/conformance owners. Use synthetic fake bidders for shared controls and preserve bidder-local protocol/provenance data.

## Task 1: Freeze The Retained SDK Constraints

**External owner:** EdgeZero PR #275, with future tuple upgrades tracked in #467. No SDK/provider/codec source changes in this task.

- [ ] Verify all six SDK requirements and locked packages retain one official-repository revision on PR #275's lineage. Do not adopt unrelated main or a new provider SDK independently.
- [ ] Record the exact provider/shared-type/HTTP/JSON/codec requirements and resolved feature graph. Preserve the Spin fork until its owning SDK permits a coherent replacement.
- [ ] Classify each retained older package instance by its actual upstream owner and constraint. Independent package upgrades may proceed; conflicting shared-version upgrades may not.
- [ ] Preserve the codec single-copy, retained-outbound, deadline/cancellation and provider boundary controls. A resolver failure does not justify removing them or adding incompatible exposed types.
- [ ] Keep future #467 work and missing qualification explicit. This task does not deliver a newer SDK tuple or claim release stability or target fit.

## Task 2: Freeze The Implementation Inventory And Baseline

**Files:** `Cargo.toml`, `Cargo.lock`, current upgrade record. Read `.tool-versions`, `vendor/README.md`, [test ownership](../../testing/development.md) and current workflows.

- [ ] Fetch main into the existing PR #166 cycle and record the exact base and retained SDK commit. Preserve concurrent work, particularly PR #162 if it has merged. Confirm the locked base builds before changing requirements.
- [ ] Capture `cargo metadata --locked --format-version 1` and `cargo tree --locked --duplicates`, then run `cargo upgrade --dry-run --incompatible allow --pinned allow --recursive false` and `cargo update --dry-run --verbose`. These are audit commands, not permission to rewrite the manifest mechanically.
- [ ] Expand every workspace requirement to its owning call sites and every resolved package to its full Cargo package ID/source. Independently inspect Git release tags and relevant upstream repairs. Record selected stable versions and retained upstream constraints; do not depend on `cargo-outdated`'s broken vendor-copy path.
- [ ] Freeze candidates after this selection. The spec's 23 requirements and 147 compatible changes are dated discovery data, not asserted counts for a later resolution. Record new releases discovered afterward without moving a graph already under acceptance.
- [ ] Run the native, standalone compiler/producer, conformance and guest baseline under the current workflow owners, preserving exact commands, features, dispatch counts and result lines. If a baseline fails, report it and obtain a decision before coding; do not call it pre-existing without reproducing the same command on base and carrying head.
- [ ] Commit the inventory/selection record only; no dependency change may be described as tested by this unchanged-graph baseline.

## Task 3: Adopt The SDK And Shared Tuple

The original adoption checklist below is conditional on a future explicitly selected PR #275 revision permitting a coherent newer tuple. For this implementation, retain the current SDK/provider/codec pins and record constrained versions; this task does not authorize replacing the SDK, bypassing exact requirements or editing its checkout. Keep the executable SDK locator/provider pins unchanged when no revision is adopted.

**Files:** root and inherited manifests/lockfile; adapter source; `crates/auctioneer-core/tests/codec_pins.rs`; `.github/workflows/scripts/test/locate-pinned-sdk.sh`; `crates/auctioneer-cli/src/qualification/harness/providers/mod.rs`; current source acquisition instructions and current upgrade record.

- [ ] First run and retain the existing single-copy codec/source guard and adapter contract controls at the base. Read exposed SDK/provider HTTP/body types and the selected SDK migration notes before touching imports or conversions.
- [ ] Change all workspace EdgeZero requirements together to the accepted immutable official revision. Select matching Fastly, Worker, Spin, HTTP/JSON and codec requirements. Keep ordinary crate declarations workspace-inherited and platform features narrow; do not copy a provider version into an adapter manifest.
- [ ] Resolve online once with `cargo update`, then run `cargo metadata --locked` and inspect `cargo tree --locked --duplicates` plus inverse trees for each shared package instance. Check request/response/header/body interface identity, not only resolution success.
- [ ] Migrate necessary adapter calls using current maintained APIs. Preserve deadlines, eager progress, retained owners, one completion callback, cancellation/drop behavior, decoded-size/window/framing checks and safe diagnostics. Keep native clients native-only.
- [ ] Update `codec_pins` to the independently recorded accepted tuple, preserving its duplicate rejection and source binding. Run `cargo test --locked -p auctioneer-core --test codec_pins --test upstream_primitives --test edgezero_fanout`. A new encoder beside an old decoder must not be accepted by weakening uniqueness.
- [ ] Run native adapter tests and the three guest adapter compile/lint commands from CI before committing this family. Update the CI SDK locator and provider harness's executable source pin along with current acquisition instructions; search for other active uses of the superseded pin. Distinguish these current readers from historical captures, whose evidence pins must not be rewritten.
- [ ] Commit SDK adoption, necessary compatibility repairs and the selected tuple together. Preserve the old record as historical evidence, not a current reproduction recipe.

## Task 4: Migrate AST, Error And Configuration APIs

**Files:** workspace manifests; `crates/auctioneer-macros/src/{lib,field_metadata}.rs`; existing `auctioneer-config` generators; `auctioneer-config-data` syntax/build controls; all actual `syn`, `toml` and `thiserror` consumers.

- [ ] Inspect the selected major release migration documentation/source and run existing config/root-shape/field-metadata/generated-syntax controls before edits. Add literal negative controls first for any changed parser/error branch not already owned: duplicate or unknown config keys, null/absence, override order, malformed input, invalid generated syntax.
- [ ] Upgrade project-owned Syn to 3, thiserror to 2 and TOML to the selected 1.2 release. Preserve upstream Syn 2 if a dependency still owns that major; do not rewrite vendored upstream requirements just to eliminate a duplicate name.
- [ ] Repair AST construction/traversal and error conversions with maintained typed APIs. Continue `quote!` plus parsed `syn` trees, not generated Rust string concatenation. Preserve one runtime-config model and CLI validate/diff/push output/diagnostic boundaries.
- [ ] Run `cargo test --locked -p auctioneer-macros --all-targets`, `cargo test --locked -p auctioneer-config --features build-support --lib --test root_shape`, and `cargo test --locked -p auctioneer-config-data --test codegen --test disclosure_codegen --test product_defaults_snapshot --test usersync_config --test usersync_callback_validation`. Require actual named cases, not an empty filter.
- [ ] Run `bash .github/workflows/scripts/test/verify-schema-compiler-conformance.sh settings` and strict affected-target Clippy. Keep generated output parseable without deleting diagnostics or blessing unrelated snapshots.
- [ ] Commit API changes and their independent regression controls; update the package-ID inventory.

## Task 5: Upgrade Cookie Crypto Without Changing Its Wire Contract

**Files:** root requirements/lockfile; `crates/auctioneer-core/src/usersync/signed.rs`; its inline tests and existing `usersync_integrity`, `usersync_cookie`, `usersync_handlers` integration owners.

- [ ] Run the independent HMAC vector, authenticated unsupported-version, active-key/rotation, key-strength and every-MAC-byte rejection controls before changing HMAC/SHA. Preserve their literal expected signatures.
- [ ] Upgrade HMAC 0.13 and SHA2 0.11 with their matching trait families. Repair imports/construction using maintained APIs; retain constant-time MAC verification, key redaction, existing signed envelope/version and the complete-cookie ceiling. Do not recalculate expected MACs using the new code under test.
- [ ] Run `cargo test --locked -p auctioneer-core --lib usersync::signed::tests` and `cargo test --locked -p auctioneer-core --test usersync_integrity --test usersync_cookie --test usersync_handlers`. Confirm nonzero dispatch and real tamper rejection at both unit and handler boundaries.
- [ ] Run the existing auxiliary/cookie allocation owners to prove stronger alignment or changed dependencies did not exceed funded storage. An overshoot blocks adoption; it does not authorize a larger reservation or smaller workload.
- [ ] Commit crypto API migration and any independently derived new regression case, without changes to capture bytes or envelope semantics.

## Task 6: Upgrade Utility And Native Maintenance Dependencies

**Files:** actual users of `getrandom`, `ipnetwork`, `lru`, `mime`, `png`, `zeroize`, native `reqwest`, registry-compatible updates and root lockfile.

- [ ] Locate users with `rg` and inspect selected release APIs before batching compatible migrations. Use the resolved package-ID inventory to catch broad requirements already current in the manifest but old in the lockfile.
- [ ] Upgrade the project-owned requirements and implement only necessary API repairs. Preserve native HTTP TLS/redirect/timeouts/host restrictions, IP/subnet and implicit-fill behavior, GVL cache eviction, randomness failures, zeroization and diagnostic privacy, exact MIME admission and generated pixel behavior.
- [ ] Add targeted negative/boundary cases before each repair when the existing owner cannot observe that branch. For MIME, keep exact `application/json` acceptance separate from structured suffixes, malformed/double-plus types and parameters; library parsing success must not authorize additional capture.
- [ ] Run `cargo test --locked -p auctioneer-openrtb --all-targets`, `cargo test --locked -p auctioneer-privacy --all-targets`, `cargo test --locked -p auctioneer-core --all-targets`, and `cargo test --locked -p auctioneer-cli --no-default-features --all-targets`. Keep cryptographic/maintenance failure tests free of external credentials and network dependencies.
- [ ] Group commits by actual API/behavior owner, not one enormous incidental formatter diff. Record all remaining old package instances and the upstream owners that constrain them.

## Task 7: Rebase And Replay The Schema Compiler Patches

**Files:** `vendor/jsonschema-macros-core/`; new versioned original archive and patch files; `vendor/README.md`; root patch/requirements; `crates/auctioneer-bidder/tests/schema_vendor/`, `schema_replay/`, `schema_compiler_controls.rs` and their existing compiler owners.

- [ ] Acquire authentic selected published source and inspect its archive members, checksum, VCS and license. Keep old original archives and historical replay assets where existing records need their bytes. Do not use a mutable upstream checkout as the original package.
- [ ] Run the original numeric/type-keyword rejection cases against the unpatched candidate in a disposable workspace. Separately test constructor-once/error caching and explicit-cache ABI/context ownership without each corresponding patch. Record which controls fail before deciding a patch is obsolete.
- [ ] Rebase the type-check, initialization and explicit-cache patches in order, independently replayable against that version's authentic source. Preserve explicit caller-owned context, no implicit clone/default, formats/URI/reference ownership, constructor error behavior and native-only dependency boundaries.
- [ ] Update current archive/member/patch inventories and replay pins only after literal controls pass. Add rejection controls for a wrong version/source, omitted intermediate patch, unexpected added/deleted member and one altered patched member. Expected identities come from independently acquired source and diff assets, not from the expanded tree under test.
- [ ] Run `cargo test --locked -p auctioneer-bidder --test schema_compiler_controls --test schema_cache_controls --test schema_cache_initialization --test schema_cache_bounds`, then the mandatory schema owner: `bash .github/workflows/scripts/test/verify-schema-compiler-conformance.sh schema`.
- [ ] Run existing source-selection/selected semantics and normal/staged core/metadata owners for both feature graphs. Do not enable a serving callback, switch regex engines or claim an outstanding schema proof complete merely because new generated code compiles.
- [ ] Commit the authentic original, portable patch sequence, provenance and code changes as a reviewable family. If upstream removed the ABI needed for existing safety controls and no faithful rebase works, stop for design review rather than shipping a partial upgrade.

## Task 8: Refresh Artifact Inspection And Implemented Source-Bound Proofs

**Files:** actual object/gimli/wasmparser/wasm-encoder consumers; `vendor/regex-automata/` and its replay owner if source changes; existing bidder `schema_build`, compiler-layout, regex/source/allocator owners; current upgrade record.

- [ ] Upgrade root object, gimli, wasmparser and wasm-encoder requirements together with their consuming APIs. Preserve checked section/member bounds, trailing-data and identity rejection, exact generated-validator membership, instrumentation and guest-vs-native distinctions.
- [ ] Reconcile the regex library and all compiler/feature dependencies even if their requirement did not change. Replay the existing resource-layout patch against authentic selected source; if a version changes, use a separately versioned original and rebase, preserving the approved read-only query boundary.
- [ ] Obtain compiler-emitted layouts for the selected native and three guest ABIs from existing probes. Reconcile actual fields/owners against current source inventories; do not infer private layouts from a previous release or a native measurement.
- [ ] Exercise changed bounds using independent literals/source derivations and actual cold/retained/transient allocator owners. Vary unrelated caps independently; preserve fresh processes, exact populations and release assertions. An updated measurement is corroboration, not permission to replace an analytic bound with itself.
- [ ] Run `cargo test --locked -p auctioneer-bidder --test schema_source_conformance -- --test-threads=1`, plus `cargo test --locked -p auctioneer-cli --test bidders_evidence --test bidders_resources --test qualification_migration --test qualification_evidence`. The full source command includes actual application/native/guest probes; a cheap filtered pass cannot replace it.
- [ ] Retain reservation/readiness/limit values. If an implemented bound is invalid under the candidate, stop adoption until an independent same-contract correction is reviewed. Keep already-open complete heap/work/host-fit qualification separate; do not turn this upgrade into an unbounded certification effort.
- [ ] Commit API/proof-identity changes and checkable new observations, not rewritten historical qualification claims.

## Task 9: Adopt Unicode 16 With An Independent Oracle

**Files:** `crates/auctioneer-config/src/activity.rs`, `crates/auctioneer-config-data/tests/simple_fold.rs`; new official Unicode 16 source/attribution fixtures under that target's fixture directory; current upgrade record.

- [ ] Acquire the official [Unicode 16 `CaseFolding.txt`](https://www.unicode.org/Public/16.0.0/ucd/CaseFolding.txt) and its applicable attribution/terms, recording its versioned source and literal digest. The official file defines simple folding as C/S mappings, not full or locale folding. Preserve the original Unicode 15/Go reference capture byte-for-byte. Test the fixture's version, exact record shape, duplicate/status rules, digest and scalar validity in the existing `simple_fold` target.
- [ ] Build an independent test-only equivalence oracle from official C/S records only. Compute each equivalence class's canonical representative without calling `unicode_case_mapping` or `simple_fold_eq`; full (`F`) and Turkic (`T`) mappings must not become simple-fold permissions. No production folding implementation or fixture-generation CLI is added.
- [ ] Before changing the dependency, run the new full-domain Unicode 16 oracle against current Unicode 15 behavior and record the changed classes. Assert the loop covers exactly 1,112,064 Unicode scalar values. Compare old and new class partitions, not only version strings, and report added/merged/split classes explicitly.
- [ ] Upgrade to the selected `unicode-case-mapping` 1.0 release, preserving the single production matching path. Require the full-domain Unicode 16 oracle and ASCII/Kelvin/long-s/sigma/sharp-s/dotted-and-dotless-I/normalization protections. Keep historical unchanged cases; classify changed historical classes explicitly rather than silently filtering failures.
- [ ] Add a negative control proving a known newly recognized equivalence is denied by the old relation and accepted by the new one, plus a one-record mutation proving the independent oracle fails on a changed mapping. Check exact dispatched counts, no skip or empty renamed loop.
- [ ] Run `cargo test --locked -p auctioneer-config-data --test simple_fold --test activity` and affected config/privacy/handler case-insensitive name-resolution controls. Commit the approved semantic migration, oracle source/attribution and delta report together.

## Task 10: Complete The Locked Graph And Full Acceptance

**Files:** root requirements/lockfile, current source instructions, current upgrade record. Read current workflows, not a stale copied matrix.

- [ ] Reconcile every resolved package ID with Task 2's frozen selection. Record upgraded/already-current/removed/upstream-constrained disposition, including coexisting major versions and Git sources. Rerun dry-run audits without adopting later releases; no project-owned requirement or necessary SDK constraint may be silently omitted.
- [ ] Run `cargo fetch --locked` for native and all three guest triples; derive `EDGEZERO_SDK_ROOT` from locked metadata and assert its selected official source/revision. Supply the existing immutable bidder reference objects and tools from `.tool-versions`. No second reference checkout or legacy environment alias is introduced.
- [ ] Run `cargo fmt --all --check` and `cargo clippy --locked --workspace --all-targets -- -D warnings`; execute ordinary secure/default/insecure/CLI feature lanes exactly as owned in `test.yml`. The two existing heavy producer exclusions remain valid only because the mandatory producer owner below executes them separately.
- [ ] Execute all explicit compiler owners with `verify-schema-compiler-conformance.sh all`, `verify-config-build-failures.sh`, `verify-evidence-producers.sh`, the existing full source experiment or its exhaustive CI owners, `resource_boot_parity`, source-selection owners and four normal/staged metadata/core scripts. Preserve `test = false` and required-feature ownership; broad `--all-targets` is not full coverage.
- [ ] Execute the fanout and bidder-handler conformance targets with `conformance-suite`, controlled-native onboarding in secure and insecure feature modes, cold native schema allocation, browser shutdown prerequisites/tests, and real socket cancellation/deadline/privacy owners. Preserve isolated binaries and unchanged workload boundaries.
- [ ] Execute actual WASI catalog/storage controls in both feature modes through `execute-catalog-publication.sh`, all adapter-specific guest checks/lints/conformance builds, the existing Fastly guest runner and the provider workflows' packaging/size gates. Native artifacts cannot stand in for guest/provider evidence.
- [ ] Run the existing raw-JSON, roster, batch-rebase, secure-usersync, libtest-result and feature/dependency-purity gates. Preserve their failure controls. Use the existing scripts rather than adding Rust snapshots of workflow bodies.
- [ ] Record per-command exit status, real libtest result lines, skipped/ignored counts and environmental prerequisites at the exact tested head. Publish CI results separately from local results. Missing provider tooling, unavailable upstream objects, disk failures or jobs that never start remain blocked, not passing. Do not raise timeouts/budgets or remove cases to obtain green.
- [ ] Have the implementation independently reviewed in existing PR #166, with its phase/description updated to the complete version/source delta, compatibility decisions and current proof record. Do not create a second PR. Keep incomplete upgrade/acceptance gates explicit and do not merge without authorization.
- [ ] After approved merge and verified #165 acceptance only, reconcile superseded Dependabot PRs and close the task. Do not close #165 because this planning PR or the upstream prerequisite alone merged.

## Verification Discipline

Use @superpowers:test-driven-development for new compatibility repairs, @superpowers:systematic-debugging for failures, and @superpowers:verification-before-completion before claims or publication. Inspect current maintained APIs rather than guessing major-version changes. Record observed reds and greens; planned expected results in this document are not executed output.

Keep ordinary Cargo caches and serialize heavy local experiments. Compiler/measurement processes retain their existing freshness and independent feature graphs. Do not routinely clean unrelated targets, add result caches, skip slow checks or exceed local disk capacity to run the entire matrix concurrently. Where an existing workflow provides exhaustive isolated owners, preserve that ownership instead of repeating the same expensive child builds in every feature lane.

The initial planning change contained no upgraded production dependency and did not certify the discovery candidates. Implementation now follows the explicit 2026-11-12 same-PR/retained-SDK amendment above. Preserve the original safety, Unicode-oracle and full acceptance requirements; the later direction changes delivery sequence, not proof or merge gates.

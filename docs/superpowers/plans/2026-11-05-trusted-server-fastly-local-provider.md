# Trusted Server Fastly Local-Provider Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement the approved work package task-by-task. Steps use checkbox syntax. Do not execute downstream issues merely because they appear in this plan.

**Goal:** Complete #34's source-pinned integration contract and define reviewable tasks for whole-catalog reporting, qualification of every implemented bidder/alias across Axum, Cloudflare, Fastly and Spin, and the first actual Trusted Server host integration on Fastly.

**Architecture:** Reuse existing build-time bidder discovery for factories, public definitions, aliases, schema selection and acceptance coverage; retire target-dependent bidder publication after the proposed production artifacts qualify. Add typed local execution to Trusted Server's real compiled provider plan using one runtime Settings/configuration path, shared preparation/engine policy and existing host services. Standalone and embedded callers share implementation rather than communicate over HTTP.

**Tech Stack:** Existing Rust workspace crates, serde/validator, existing EdgeZero/Fastly services, current bidder/schema generation, Viceroy and the existing Rust qualification and host-integration harnesses. No new dependency, crate, runtime plugin, configuration loader or numeric limit is authorized.

**Contract:** [Trusted Server Fastly local-provider design](../specs/2026-11-05-trusted-server-fastly-local-provider-design.md). Both documents received maintainer approval and were merged through [PR #152](https://github.com/IABTechLab/auctioneer/pull/152), commit `9b83deaa680ad8e867169db3b4ebb195b8a1b33c`, under the head-specific independent-approval exception recorded in spec section 12. This approves the contract and decomposition, not unreviewed downstream APIs, changed limits or an integration-readiness claim. The exception does not apply to this closeout or later implementation PRs.

---

## Scope And Execution Rules

#34 delivers the reviewed contract, source/ownership reconciliation and issue decomposition. Packages 2-7 below are separately reviewed implementation work under #51, shared qualification #46/#47, adapter fit #72-#75, guest serving #88-#90, #76 and a matching Trusted Server-owned task. They are not production changes to land in the #34 planning PR or authorization to start another PRfection cycle automatically.

Use the source revisions in spec section 2. Read committed Trusted Server files at its pinned revision, not private local operator files. Before each implementation package, refresh its actual base and produce an explicit contract delta if interfaces or dependencies have changed. A branch-tip citation, another artifact's old measurements or an assumed SDK API is not evidence.

Enumerate the whole catalog from spec section 3's discovered manifests, import/provenance inventory and validated aliases, not a handwritten delivery list. Every adapter's report includes every entry; absent executable implementations are explicitly `skipped: not_implemented`, never omitted or counted as passing. Qualify every implemented identity on all four adapters and derive the first host's display cases from that same catalog. Cover execution across bounded batches; do not widen limits or require every identity in one auction. The host integration remains Fastly-first, while the bidder contract is portable.

### File And Ownership Map

The following new paths are proposed implementation locations, not files that already exist. Existing paths are identified separately. Use normal Rust module declarations; no handwritten `#[path]` or cross-crate `include!` source imports. Unit tests remain inline; independent conformance/allocation owners keep their existing process boundaries.

| Work Package                                   | Existing Files To Read/Modify                                                                                                                                                                                                                                                                                                                                                                              | Proposed New Files                                                                                       |
| ---------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| #34 contract closeout                          | This spec/plan; `docs/roadmap.md` only if an actual contradictory active statement is found                                                                                                                                                                                                                                                                                                                | None beyond these planning documents                                                                     |
| Shared application boundary (#51)              | Auctioneer `crates/auctioneer-core/src/lib.rs`, `src/handlers/auction.rs`, `src/config/{mod,resolve,validation}.rs`; `crates/auctioneer-exchange/src/run/mod.rs`, `src/response.rs`, `src/targeting/winner.rs`                                                                                                                                                                                             | `crates/auctioneer-core/src/auction/{mod,prepare,services,result}.rs`                                    |
| Production local execution (Trusted Server)    | `crates/trusted-server-core/src/auction/{plan,provider,orchestrator,routing,mod}.rs`; `src/config.rs`; `src/settings.rs`; `src/integrations/registry.rs`                                                                                                                                                                                                                                                   | `crates/trusted-server-core/src/auction/execution.rs`                                                    |
| Fastly host binding (#51 / host-owned changes) | Trusted Server `crates/trusted-server-adapter-fastly/src/{app,platform}.rs`, `src/platform/deadline.rs`; Auctioneer `crates/auctioneer-core/src/host_primitives/{edgezero_proxy,registry}.rs`                                                                                                                                                                                                              | `crates/trusted-server-adapter-fastly/src/auctioneer.rs`                                                 |
| Discovery/schema/publication cutover (#46/#47) | Auctioneer `crates/auctioneer-bidder/build.rs`, `src/bidders/*/bidder.toml`, `src/schema/{mod,native,staging}.rs`; `crates/auctioneer-config/src/{bidder_manifest,build_catalog,bidder_discovery,bidder_storage,schema_staging,schema_staging_build,schema_staging_codegen}.rs`, `src/codegen/{mod,storage}.rs`; `crates/auctioneer-config-data/build.rs`; core `src/host_primitives/{gates,bootstrap}.rs` | Replace native-only admission ownership within its current schema module; no second validator or catalog |
| Four-adapter qualification (#72-#75/#88-#90)   | All four `crates/auctioneer-adapter-*/src/envelope.rs` profiles, existing adapter/CLI qualification owners and current catalog/storage/schema conformance targets                                                                                                                                                                                                                                          | No new proof crate, bidder list or alternate budget implementation                                       |
| Results/delivery and actual host (#76)         | Trusted Server `crates/trusted-server-core/src/auction/{types,formats,orchestrator}.rs`, `src/publisher.rs`, `src/integrations/{registry,aps}.rs`; existing host/HTML runtime harness                                                                                                                                                                                                                      | No separate renderer or second HTTP application                                                          |

Do not restructure unrelated bidder modules, refactor all host protocol code or move configuration packages as part of this work. If one proposed module has no independent responsibility after the API review, keep it private within its owning module instead of manufacturing an abstraction.

## Package 1: Freeze And Review The Contract (#34)

**Files:** This spec and plan. **Remote tracking:** #34, #51, #46/#47, #72-#75, #88-#90, #76 and the existing integration epic #8. No new implementation files.

- [x] Read the current issue discussions, pinned source files and actual dependency graph. Record the source revisions, required identities, plan-backed execution gap, SDK mismatch, targeting reconstruction and HTTP-only pending-handle gap against the code, not an earlier review.
- [x] Review C1-C10 with the host owner. Confirm that the local provider offers Auctioneer's selected candidate per slot to outer competition; accepted local candidates remain available for their bounded existing purposes, but are not reranked by an unrelated CPM-only reducer. Explicitly distinguish engine equivalence from host mediation/floors/creative outcomes.
- [x] Search both repositories for existing tasks before creating any. Keep shared preparation/host binding in #51, production local execution Trusted Server-owned, common discovery/schema work in #46/#47, fit in #72-#75, guest serving in #88-#90 and the combined host artifact in #76. Reconcile their acceptance/dependency descriptions with the discovered all-adapter contract before implementation; preserve existing project fields. Do not recreate #135, a parallel registry or another configuration epic.
- [x] Propose independently reviewable implementation tasks/PR boundaries and native blocked-by links. Preserve existing issue fields. New tasks use the existing shared project/type taxonomy and proposed P1 priority only after authorization; leave Size/assignee/dates unset unless approved. Cross-repository changes remain with their repository owner.
- [x] Review the written spec and plan together, record findings in their original threads and obtain maintainer approval. Run the full prescribed current-head acceptance matrix before claiming a completed planning handoff. Keep #34 open when the planning PR merges. The independent-approval limb was excepted only for PR #152's reviewed head under the authorization recorded in spec section 12; this is not precedent for clearing later PRs' merge gates.
- [ ] Complete review of #34's contract/decomposition closeout and obtain merge authorization before integrating it and closing the issue. Do not make its contract acceptance depend on completing the downstream integration issues.

**Closeout progress:** PR #152 delivered the reviewed contract, with all 25 current-head checks passing before merge; that is planning evidence, not execution of C1-C10. The existing host coordination owner is [Trusted Server #1505](https://github.com/IABTechLab/trusted-server/issues/1505), so no duplicate host integration issue was created. The 12 existing Auctioneer ticket bodies in this package were reconciled and read back against the contract, preserving issue types, status, priority, size, assignment and milestones. #76 now has native blocked-by links to #51 and #88. Section 11 of the spec records the ownership and cycle constraints. #34 remains open pending closeout review and authorized integration; packages 2-7 remain unimplemented by this work.

**Expected outcome:** A reviewed contract and executable ownership/dependency map. No new guest-serving capability, resource certification or host artifact is claimed.

## Package 2: Share Typed Preparation And Results (#51)

**Files:** The Auctioneer shared-boundary paths in the file map. **Existing test owners:** `crates/auctioneer-core/tests/{handler_resolution,resolve_runtime_config,auction_config_lifecycle,auction_config_retention,privacy_config_parity}.rs`; `crates/auctioneer-exchange/tests/{run_auction/main,determinism,privacy_policy_composition}.rs`; inline tests in changed implementation modules.

- [ ] Freeze the small typed API in this package's reviewed design: admitted input, accepted policy/services, original request timing, owned prepared operation, typed local selection/results and terminal error. Derive ownership from current `auction_inner`, `prepare` and `run_auction`, not a new independent engine. Do not start code until the signatures and lifetime/resource owners are reviewed.
- [ ] Add failing equivalence cases to existing owning tests. Compare typed candidates, local deal preference, equal-price ordering, alias attribution, targeting and errors under independently varied account/privacy/settings inputs. Use capable fake bidders and deterministic clock/RNG. Assert fixtures actually contain each field/policy dimension and that the selected cases execute.
- [ ] Run the focused commands below and record the actual red result. A filter matching zero cases is a refusal to credit coverage, not a passing red/green cycle.
- [ ] Move common effective-policy/plan/context assembly into the proposed `auction` modules. Keep HTTP read/header/error rendering/terminal serialization in `handlers::auction`. Make both callers use the same canonical preparation and policy implementation; do not copy the handler or introduce an internal HTTP RPC.
- [ ] Expose selection facts from the same local winner reduction in the typed result. Preserve standalone wire output through its existing response projection; do not rerun winner selection in the bridge or discover the winner by parsing rendered targeting strings.
- [ ] Retain existing node/construction checks for programmatic typed input, original extension-shape provenance, request-bound secret references and snapshot lifetimes. Unrelated account overlays still die before fan-out; operator policy must not enter a global cache.
- [ ] Run the focused public/handler cases, unchanged standalone output cases, retention/cancellation controls and affected allocation owners. Record green results and verify that reverting the shared-boundary change breaks the new controls without rewriting their expected values. Commit only this package after review.

```sh
cargo test --locked -p auctioneer-core --lib
cargo test --locked -p auctioneer-core --test handler_resolution --test resolve_runtime_config --test auction_config_lifecycle --test auction_config_retention --test privacy_config_parity
cargo test --locked -p auctioneer-exchange --test run_auction --test determinism --test privacy_policy_composition
cargo clippy --locked -p auctioneer-core -p auctioneer-exchange --all-targets -- -D warnings
```

Expected after implementation: every named target executes; the new assertions fail on the original boundary and pass on the shared implementation. Counts and timing are recorded from the run, not copied into this plan as fictional results.

## Package 3: Add Real Local Execution To The Host Plan (Trusted Server)

**Files:** Host production-execution paths in the file map; inline tests in `auction::{plan,execution,orchestrator,routing}` and `config.rs`. **Prerequisite:** Reviewed #34 contract and the #51 API/service/result shape needed by the executor. This task can proceed independently of final Fastly qualification, but cannot make unqualified bidders callable.

- [ ] Add failing production-plan controls: local execution without an HTTP endpoint, rejection of a local/remote field combination, unknown/unregistered executor, disabled local module, duplicate bidder assignment, invalid typed local policy and unsupported target capability. Test live `from_plan` construction, not the legacy test registration map.
- [ ] Define a typed execution representation with common provider identity/routing/timeout and distinct remote/local requirements. Remote providers retain actual transport data; local providers refer to a compiled executor contract. No fabricated endpoint, hostname, backend name or HTTP response is used for the local executor.
- [ ] Bind the module-owned typed Auctioneer policy through the existing Settings/`TrustedServerAppConfig` flow, including nested validation and SDK secret metadata at authoring and resolved-runtime boundaries. The host must reject an invalid bidder policy through `ts config validate/push` and actual extraction. The exact typed registration/serialized shape is a reviewed host API change in this task, not an assumed existing extension hook. It cannot add another read/push pipeline or hide the policy in unvalidated JSON.
- [ ] Construct the local executor from the real compiled plan and injected host binding. Keep its implementation outside neutral orchestration types. Reuse the plan's single bidder-to-provider ownership map; unknown aliases or local/remote overlap fail before dispatch. Do not add a bidder-name switch.
- [ ] Extend the existing integration/capability registration to recognize the local declaration's actual delivery requirements. Register only approved typed capabilities; a sole local provider must not accidentally lose required delivery support because the old activation check looked only for a remote profile.
- [ ] Run the real plan/config/routing controls and the existing remote-provider regressions. Revert the new live construction branch to prove the controls fail. Commit the host task separately, linked to #51 and its approved contract.

```sh
cargo test -p trusted-server-core --lib auction::plan::
cargo test -p trusted-server-core --lib auction::routing::
cargo test -p trusted-server-core --lib config::
cargo test-fastly
cargo test-fastly-reuse
cargo clippy-fastly
```

Run from the Trusted Server root with its pinned tools and fixture setup. The native commands are focused feedback; `test-fastly` must actually execute the WASI/Viceroy cases. If the reviewed host shape creates a new unit module, select its exact test names with `--list` and record a nonzero dispatch count before crediting that additional filter.

## Package 4: Bind Fastly Services And The Owned Operation (#51 / Host Task)

**Files:** Proposed host `src/auctioneer.rs`, host `auction/execution.rs`, current Fastly app/platform/deadline code and Auctioneer's shared boundary. **Existing controls to extend:** Auctioneer `auction_config_retention`, `clock_domain`, `handler_wave`, `admission_holds_through_auction`; Trusted Server's inline orchestrator/platform deadline tests and the existing publisher abandonment guards.

- [ ] Reconcile the pinned SDK/service APIs before writing adapters. Identify the actual bounded outbound, clock/timer, secret, reference-data and admission handoff. Select and review any required SDK alignment/host hook; do not cast contexts between SDK versions, invent an available method or fall back to `DevFanout`. Keep this a named prerequisite rather than a surprise at final linking.
- [ ] Add red controls for direct and split start/collect/abandon behavior. The local operation must start eligible bidder I/O before origin work, avoid blocking unrelated provider launches, retain its one accepted snapshot/reservations, and collect without restarting work. Ordinary execution drives the same lifecycle.
- [ ] Implement a real owned local-operation representation in the host execution layer. Keep local work out of the Fastly HTTP-handle collector unless that collector is explicitly extended through a reviewed typed contract. No fake pending backend or arbitrary future downcast is accepted.
- [ ] Supply the original host deadline and monotonic mapping, existing service interfaces and funded owners. Secret lookup follows privacy/schema admission. An already-spent deadline performs no bidder I/O or newly funded parse/credential work. Do not initialize a second request scheduler/admission pool behind the host's back.
- [ ] Cover expired collection with available completed results, stalled headers, trickling/over-cap bodies, origin failure, stream abort and collection drop. Preserve the existing nonblocking completed snapshot with no further waits, distinguish logical from transport timeout and report Fastly abandonment rather than unsupported upstream cancellation.
- [ ] Extend independent retention/accounting controls through final host delivery; dropping the engine future must not free storage still owned by host serialization or egress. Record red/green output and exact-once permit release observations, then commit this package separately.

```sh
cargo test --locked -p auctioneer-core --test auction_config_retention --test clock_domain --test handler_wave --test admission_holds_through_auction
cargo test --locked -p auctioneer-core --features conformance-suite --test fanout_conformance
cargo test-fastly
cargo test-fastly-reuse
cargo clippy-fastly
```

The first two commands run from Auctioneer; the last three from Trusted Server. These local controls are not proof of the actual combined release artifact or hosted qualification.

## Package 5: Qualify The Discovered Catalog On Every Adapter

**Owners:** Common discovery/schema work #46/#47, actual fit #72-#75, and guest serving #88-#90. **Files:** The discovery/schema/publication and four-adapter rows in the file map; existing `catalog_publication`, `schema_selection`, `schema_source_conformance`, `bidder_storage` and adapter-entrypoint/qualification owners. Extend the Rust harness under `crates/auctioneer-cli/src/qualification/harness/providers/` where its boundary applies, not a new harness or crate. **Prerequisites:** Existing schema residency/work, auxiliary lifetime and SDK/decoder/platform facts. Investigate these alongside #34/#51 rather than waiting for the final host link.

### 5A. Make Discovery The Only Selection Contract

- [ ] Capture both complete catalog/alias inventory and implemented public-definition/factory closure from the existing discovery, provenance and binding contracts, independent of `native_only` or operator enablement. Derive explicit unimplemented skips for reference-only identities, including unported source aliases. A source capture sharing a Rust owner is not proof of implementation; conversely, missing generated wiring or qualification for a declared Rust public definition is blocked/failed, not skipped. Check canonical schema/feature ownership and bidder-owned parameter/provenance inputs. Do not freeze another list of names or turn baseline counts into hardcoded acceptance values.
- [ ] Add failing controls in the existing discovery/codegen/catalog owners using fake bidders: a reference-only entry appears as `skipped: not_implemented` on every adapter without counting as passing and cannot be activated by runtime configuration; adding its implementation and public declaration removes that skip and requires qualification automatically; its declared aliases enter registry/schema coverage without a central list edit. Reject missing, duplicate, extra, unvisited and stale rows, false skips and absent generated wiring for a declared implementation. Vary security features separately from adapter selection. Record actual red results and nonzero dispatch before changing generation.
- [ ] Replace delivery-list selection in `schema_staging_build.rs` and its `schema_staging` input model with the discovered closure. Retain per-bidder schema/source/format provenance and captured bytes, including each actual source origin, but stop using a capture/planning cohort to decide what compiles or gets validated. Update current consumers/oracles instead of rewriting historical captures or making the old list silently authoritative elsewhere.
- [ ] Reuse the existing schema compiler and typed `quote!`/`syn` generation. Generalize `schema/native.rs`, generated owner callbacks and core bootstrap/admission to accept the qualified adapter's descriptors and funded initialization/cache/work ownership. Remove guest `false`/unavailable stubs and the build-script staging restriction only as part of that working path. Do not copy the native reservation numbers to other ABIs, return true without validation, or add a second schema engine.
- [ ] Prepare the candidate cutover removing `native_only` from manifest parsing/models, metadata, discovery declarations, publication generation, registry bindings, alias reconciliation and storage census. Reject the retired input key. Update existing controls that deliberately require native-only rows to test uniform publication instead; keep their independent accounting and security-feature assertions. Do not retain aliases for the flag, a target list, bidder-name exceptions or another per-adapter eligibility field.

### 5B. Qualify The Candidate Artifacts Before Release

- [ ] Build the actual proposed production artifacts with the candidate cutover, rather than requiring the old filtered registry to prove impossible positive cases. Require exact whole-catalog report coverage, with verified unimplemented skips, separately from exact implemented adapter/identity/feature execution coverage. Keep this implementation PR unmerged/unpublished until all implemented cells pass; unimplemented skips are visible, not passes, and do not block the implemented set. The candidate source change is not itself approval to serve.
- [ ] Complete the independent fit/ownership work for Axum (#72), Fastly (#73), Cloudflare (#74) and Akamai Functions hosted Spin (#75). Include compiled registry/schema residency and initialization even when the operator disables bidders. Preserve the additive accounting, workload, ceilings and headroom; missing SDK/decoder/platform facts remain nonpassing. No measurement may set its own passing threshold.
- [ ] Extend the existing adapter/qualification test owners to drive each real production entrypoint with every applicable discovered identity/alias, parameter validation, privacy, error, cap and cancellation case. Use bidder-owned protocol fixtures and capable shared fake services, not per-name shared tests. Positive admitted responses, exact source/metadata/binding/registry equality and absent-row refusal must all be observed. The generic WASI publication test alone cannot qualify Cloudflare Workers, Fastly networking or a Spin component.
- [ ] Cover the complete implemented execution set across bounded deterministic batches using one source-bound artifact per adapter/feature graph, while all unimplemented catalog entries remain explicit report rows without bidder I/O. Exercise the installed concurrency boundary and one-over refusal without changing limits. An omitted report entry, execution batch, source-added declared alias or whole adapter must fail coverage; do not require all implemented identities in one request or accept a reduced set to make one batch fit.
- [ ] Install the qualified descriptors and actual initialization/schema-admission path through #88/#89/#90. Verify the final registry has no architecture-based bidder filter. Extend resource/retention controls to the combined Trusted Server Fastly application in #76, including host Settings, bridge/creative state and terminal output; do not add two standalone totals or release funding while delivery still owns storage.
- [ ] Record exact source/feature/ABI/SDK/config/profile/artifact identities and independently review all four adapter receipts. Remove the flag only in the accepted cutover, with no replacement target filtering. Future discovered bidder additions carry the same matrix before release. Keep unimplemented data-only entries non-callable and retain ordinary runtime enable/disable policy.

Run these existing feedback controls from Auctioneer with the pinned source/runner prerequisites; expected after implementation is actual nonzero execution and no missing discovered identity:

```sh
cargo test --locked -p auctioneer-config --features build-support --lib bidder_discovery::
cargo test --locked -p auctioneer-bidder --test catalog_publication
cargo test --locked -p auctioneer-bidder --test catalog_publication --features insecure-bidders
cargo test --locked -p auctioneer-config-data --test bidder_storage
bash .github/workflows/scripts/test/execute-catalog-publication.sh
cargo run --locked -p auctioneer-cli -- bidders resources --check --all-adapters --capture-audit
cargo run --locked -p auctioneer-cli -- bidders resources --check --all-adapters --require-fit
cargo test --locked -p auctioneer-adapter-axum -p auctioneer-adapter-fastly -p auctioneer-adapter-cloudflare -p auctioneer-adapter-spin --all-targets
```

The WASI script requires an existing disposable `RUNNER_TEMP` and pinned Wasmtime on `PATH`; its source-independent publication/storage comparisons supplement, not replace, actual adapter execution. The first resource command emits an audit; capture its stdout freshly and validate that same capture. Strict all-adapter fit remains nonpassing until every required fact and inequality passes. A prior native-only reservation or a successful package-size check is not fit evidence.

Before implementing each adapter-owned execution extension, freeze its exact existing/new test selection, features, runtime invocation and receipt assertions in that issue's reviewed package. Reuse the current `.github/workflows/ci-fastly.yml`, `ci-cloudflare.yml`, `ci-spin.yml`, `test.yml` and CLI provider harness owners. Current Fastly logging guests, Cloudflare dry-run and Spin packaging are only baseline controls; none already drives the complete discovered bidder set. Where a suitable executed-runtime case is absent, adding it is required work, not an implicit pass. No deploy, credentialed call or private-store mutation is authorized by this plan. Record missing execution/provider prerequisites explicitly rather than inventing a command or claiming the current CI matrix implements this future qualification.

## Package 6: Preserve Typed Results And Host Delivery (#76)

**Files:** Existing host result/formatter/publisher/integration files in the map, the shared typed local result and host binding. **Prerequisites:** #51's real operation/result contract and package 5's discovered-catalog qualification/cutover. Result projection may be developed earlier with fixtures, but cannot be labeled completed host integration from those fixtures.

- [ ] Add red cases with independently varied local deal preference, equal-price order and remote competition. The host receives the selected local candidate per slot as the outer contender; it must not rerank all local candidates by a different rule. Preserve the remaining local candidates/per-bidder outcomes only through their bounded existing purposes.
- [ ] Extend the typed host result/projection for local targeting and selection provenance. Preserve provider ID, requested bidder/alias, returned seat, slot and bid IDs, currency/price, dimensions, creative, notifications and applicable delivery metadata. Do not infer them by reparsing an HTTP response or a targeting string.
- [ ] Update both `auction::formats` direct output and publisher `build_bid_map_with_auction_id` consumption. Local selected targeting comes from Auctioneer; existing remote targeting remains unchanged. Test a remote winner to prove losing local winner keys cannot survive, and test engine equivalence separately from host floor/mediator/creative decisions.
- [ ] Verify APS and every other required display delivery form against actual bidder-owned response fixtures. Use the current Auctioneer bidder contract, not the host's different remote parser. Wire any required existing typed renderer/capability registration from the local plan; a bidder name or remote-profile presence check is not proof. Reject unsupported descriptors without reporting a required identity as accepted from refusal alone.
- [ ] Keep valid no-bid distinct from config/admission/execution error. Preserve bidder-local valid siblings and sanitized error attribution. Test creative rejection and existing no-runner-up behavior so a dropped winner cannot count as delivered.
- [ ] Run focused result/publisher/renderer controls and the real combined Fastly artifact/harness in package 7. Commit projection/delivery changes in an independently reviewable #76 implementation PR; preserve remote provider behavior.

```sh
cargo test -p trusted-server-core --lib auction::
cargo test -p trusted-server-core --lib integrations::aps::
cargo test-fastly
cargo test-fastly-reuse
cargo clippy-fastly
```

Add assertions to owning inline/host-integration tests rather than scattering real bidder names through shared unit fixtures. Each required positive bidder case remains catalog-driven or with its bidder owner.

## Package 7: Prove The Actual Fastly Host And Close Delivery (#76)

**Files:** The existing Trusted Server actual-runtime harness under `crates/trusted-server-integration-tests/tests/html_eof_runtime/`, its owning test target, `scripts/async-html-finalization-local-test.sh` and `scripts/README.md`; Auctioneer's current conformance/qualification owners and bidder-owned protocol fixtures. Keep harness changes in their owner; do not create another browser/HTTP test program to duplicate it.

- [ ] Build and package the actual non-test Trusted Server Fastly application with the approved Auctioneer dependency/features and eligible registry. Record code/component/package digests and source/config/profile identity. A logging-test guest, standalone Auctioneer component or native binary is not the subject.
- [ ] Drive real direct `/auction`, publisher split dispatch/collect and current page-bids flows through that artifact. Derive every required display identity/alias from the discovered declarations and capabilities; observe real admitted dispatch, valid results and compatible delivery. Cover the entire set across within-limit batches on the same artifact and exercise concurrency/refusal boundaries. Reconcile exact sent impressions and no duplicate local/remote assignment.
- [ ] Run the two isolated operator-policy snapshots against the same artifact and independently vary accounts, aliases, privacy, settings and secret references. Exercise invalid input, config changes, no-cross-host retention, original deadlines, partial failure, cancellation/abandonment, recovery and bounded diagnostics. Use synthetic authorized fixture stores; no live mutation or private credentials are authorized.
- [ ] Check final winner/targeting/creative behavior and the selected request/response ownership. Repeat affected resource/allocation owners after the final link. Bind final evidence to this artifact; do not upgrade another run's observations or infer a pass from compilation.
- [ ] Run the complete applicable acceptance suites in both repositories. Keep exact command lines, dispatched target/counts, exit codes and result lines. New targets/features must have an explicit existing CI owner; do not add Rust copies of workflow text. No skipped or ignored required row counts as passed.
- [ ] Complete independent review and maintainer authorization before merge. Verify actual default-branch integration and criterion-level issue closure. #34 contract acceptance, #51 bridge delivery, four-adapter catalog qualification/cutover and #76 actual-host evidence remain distinct outcomes; live hosted/publisher qualification stays with its existing selected-path issues.

```sh
./scripts/async-html-finalization-local-test.sh fastly --release --auction
cargo test --manifest-path crates/trusted-server-integration-tests/Cargo.toml --features html-eof-runtime --test html_eof_runtime
cargo clippy --manifest-path crates/trusted-server-integration-tests/Cargo.toml --features html-eof-runtime --test html_eof_runtime -- -D warnings
```

Run these from Trusted Server with its documented runtime prerequisites. Extend the existing harness selection to make the discovered local-execution cases mandatory and verify their actual dispatch; the unchanged existing command is not evidence that catalog-driven local cases already exist. The baseline control uses an unreachable remote bidder and cannot serve as the new positive local acceptance case.

## Full Verification And Evidence Policy

The implementation packages above describe future checks, not executed results. Every changed package gets red/green controls before its implementation and the full prescribed suite before review handoff. Do not invent exact counts for tests that have not been written, change a frozen threshold after measuring, weaken assertions, restore compiled-output caches, or repeat independent child-build measurements merely to increase a test total.

For this planning PR, run document/source/membership checks and available local native acceptance, then execute and inspect the complete current-head GitHub matrix. The repository's workflows remain authoritative; no workflow change or docs-only test exemption is proposed:

- `.github/workflows/test.yml`: secure, default/insecure workspace, CLI/maintenance, mandatory once-owned producers and compiler/config lanes, conformance, guest target checks and their existing explicit owners.
- `.github/workflows/format.yml`: formatting and each lint feature lane, including conformance.
- `.github/workflows/ci-fastly.yml`: vendor packaging, size and actual existing guest controls, not local Auctioneer host integration.
- `.github/workflows/ci-cloudflare.yml` and `ci-spin.yml`: current regression coverage, to be extended by their implementation owners for package 5's actual-runtime qualification; today's checks do not prove that future contract.

Before local commands, prepare the locked dependencies, source fixture commits, SDK root and pinned runner tools from [development and test ownership](../../testing/development.md). Keep separate Cargo invocations for feature graphs and one heavy build at a time. Existing explicit once-owned CI selections and their compensating owners stay intact; do not add new omissions or rely on a bare workspace invocation as the complete matrix.

Trusted Server changes require its full `AGENTS.md`/workflow acceptance, including applicable core/adapter, CLI, JS, format, target and actual-runtime owners. Its target aliases are not interchangeable with Auctioneer's; `cargo test-fastly` invokes its Fastly/Viceroy suite, not a native workspace test. This planning PR changes no Trusted Server files and makes no claim of having executed that application's new integration.

Record missing facts or failed/external checks as nonpassing, with exact output and next owner. An open, non-draft planning PR allows joint document review while checks run; it is not a claim of completed verification or merge readiness. Do not merge unless the current-head suite, review-thread acceptance, independent approval and maintainer authorization gates hold.

## Planning PR Handoff

Completed through PR #152 at the planning merge recorded above; these checks describe that historical handoff, not verification of a later implementation artifact.

- [x] Update the spec's status to joint spec/plan review without representing it as approved for implementation.
- [x] Run a whole-pair independent document review and address material findings in both documents.
- [x] Verify only these planning documents differ from main, commit them without generated/co-author trailers, and push the dedicated planning branch after the authorized checks.
- [x] Open one non-draft planning PR against `main`, with `PRfection phase: plan`, `Refs #34`, links to both documents, source baseline, actual verification receipts and unexecuted/blocked checks. Do not use an issue-closing keyword.
- [x] Read back the URL, open/non-draft state, intended head/base and check status. Ask for joint document review; do not implement, close #34, resolve reviewer threads, or merge merely because the PR exists.

# Trusted Server Fastly Local-Provider Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement the approved work package task-by-task. Steps use checkbox syntax. Do not execute downstream issues merely because they appear in this plan.

**Goal:** Complete #34's source-pinned integration contract and define reviewable delivery tasks for an actual Trusted Server Fastly artifact serving all 14 priority bidders, including APS.

**Architecture:** Add typed local execution to Trusted Server's real compiled provider plan. Reuse one runtime Settings/configuration path, shared Auctioneer preparation/engine policy, and existing host services; retain explicit request, deadline, resource, result and delivery ownership. Standalone and embedded callers share implementation rather than communicate over HTTP.

**Tech Stack:** Existing Rust workspace crates, serde/validator, existing EdgeZero/Fastly services, current bidder/schema generation, Viceroy and the existing Rust qualification and host-integration harnesses. No new dependency, crate, runtime plugin, configuration loader or numeric limit is authorized.

**Contract:** [Trusted Server Fastly local-provider design](../specs/2026-11-05-trusted-server-fastly-local-provider-design.md). Both documents are pending maintainer approval. The maintainer explicitly requested joint spec/plan review; this is not implementation approval.

---

## Scope And Execution Rules

#34 delivers the reviewed contract, source/ownership reconciliation and issue decomposition. Packages 2-7 below are separately reviewed implementation work under #51, #73, #88, #76 and a matching Trusted Server-owned task. They are not production changes to land in the #34 planning PR or authorization to start another PRfection cycle automatically.

Use the source revisions in spec section 2. Read committed Trusted Server files at its pinned revision, not private local operator files. Before each implementation package, refresh its actual base and produce an explicit contract delta if interfaces or dependencies have changed. A branch-tip citation, another artifact's old measurements or an assumed SDK API is not evidence.

The required bidder identities come from spec section 3 and the existing bidder-owned priority/supplemental manifests. All 14 must pass Fastly acceptance. Do not replace them with a smaller set or treat native execution as delivery.

### File And Ownership Map

The following new paths are proposed implementation locations, not files that already exist. Existing paths are identified separately. Use normal Rust module declarations; no handwritten `#[path]` or cross-crate `include!` source imports. Unit tests remain inline; independent conformance/allocation owners keep their existing process boundaries.

| Work Package                                   | Existing Files To Read/Modify                                                                                                                                                                                  | Proposed New Files                                                    |
| ---------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------- |
| #34 contract closeout                          | This spec/plan; `docs/roadmap.md` only if an actual contradictory active statement is found                                                                                                                    | None beyond these planning documents                                  |
| Shared application boundary (#51)              | Auctioneer `crates/auctioneer-core/src/lib.rs`, `src/handlers/auction.rs`, `src/config/{mod,resolve,validation}.rs`; `crates/auctioneer-exchange/src/run/mod.rs`, `src/response.rs`, `src/targeting/winner.rs` | `crates/auctioneer-core/src/auction/{mod,prepare,services,result}.rs` |
| Production local execution (Trusted Server)    | `crates/trusted-server-core/src/auction/{plan,provider,orchestrator,routing,mod}.rs`; `src/config.rs`; `src/settings.rs`; `src/integrations/registry.rs`                                                       | `crates/trusted-server-core/src/auction/execution.rs`                 |
| Fastly host binding (#51 / host-owned changes) | Trusted Server `crates/trusted-server-adapter-fastly/src/{app,platform}.rs`, `src/platform/deadline.rs`; Auctioneer `crates/auctioneer-core/src/host_primitives/{edgezero_proxy,registry}.rs`                  | `crates/trusted-server-adapter-fastly/src/auctioneer.rs`              |
| Fastly qualification/serving (#73/#88)         | Auctioneer `crates/auctioneer-adapter-fastly/src/envelope.rs`; `crates/auctioneer-bidder/src/schema/`; `src/bidders/*/bidder.toml`; `crates/auctioneer-config-data/build.rs`; existing qualification owners    | No new proof crate or alternate budget implementation                 |
| Results/delivery and actual host (#76)         | Trusted Server `crates/trusted-server-core/src/auction/{types,formats,orchestrator}.rs`, `src/publisher.rs`, `src/integrations/{registry,aps}.rs`; existing host/HTML runtime harness                          | No separate renderer or second HTTP application                       |

Do not restructure unrelated bidder modules, refactor all host protocol code or move configuration packages as part of this work. If one proposed module has no independent responsibility after the API review, keep it private within its owning module instead of manufacturing an abstraction.

## Package 1: Freeze And Review The Contract (#34)

**Files:** This spec and plan. **Remote tracking:** #34, #51, #73, #88, #76 and the existing integration epic #8. No new implementation files.

- [ ] Read the current issue discussions, pinned source files and actual dependency graph. Record the source revisions, required identities, plan-backed execution gap, SDK mismatch, targeting reconstruction and HTTP-only pending-handle gap against the code, not an earlier review.
- [ ] Review C1-C10 with the host owner. Confirm that the local provider offers Auctioneer's selected candidate per slot to outer competition; accepted local candidates remain available for their bounded existing purposes, but are not reranked by an unrelated CPM-only reducer. Explicitly distinguish engine equivalence from host mediation/floors/creative outcomes.
- [ ] Search both repositories for existing tasks before creating any. Keep shared preparation and host binding within #51's scope, the real local plan/executor change Trusted Server-owned, resource fit in #73, guest enablement in #88, and the actual combined artifact in #76. Do not recreate #135 or another configuration epic.
- [ ] Propose independently reviewable implementation tasks/PR boundaries and native blocked-by links. Preserve existing issue fields. New tasks use the existing shared project/type taxonomy and proposed P1 priority only after authorization; leave Size/assignee/dates unset unless approved. Cross-repository changes remain with their repository owner.
- [ ] Review the written spec and plan together, record findings in their original threads and obtain maintainer approval. Run the full prescribed current-head acceptance matrix before claiming a completed planning handoff. Keep #34 open when the planning PR merges; verify its own contract/decomposition criteria and obtain closeout authorization separately. Do not make #34's contract acceptance depend on completing the downstream integration issues.

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

## Package 5: Complete Fastly Fit And Guest Serving (#73 / #88)

**Files:** Existing Fastly resource, schema, generated registry and bidder publication owners in the file map. **Prerequisites:** Existing #73 dependencies for schema residency/work, auxiliary lifetime and SDK/decoder/platform facts. Resource work should be investigated alongside #34/#51, not postponed until the final host link.

- [ ] Capture the current source/profile/accounting gap using existing `bidders resources` and strict-fit commands. Preserve the native-only baseline. A known failure or unavailable SDK fact remains a named nonpassing requirement, not an ignored test or an external block falsely described as otherwise fitting arithmetic.
- [ ] Complete the applicable existing #73 proof/ownership work for Fastly, including the compiled application's separate stack, actual SDK/decoder facts and required headroom. Predeclare and review any proposed reserve or accounting correction before using its measurement as a passing threshold. No changed limit, ceiling, workload or equation is implied here.
- [ ] Extend that accounting to the actual combined host ownership: Settings, bridge input/operation, schema/registry/reference residency/init, bidder concurrency, host creative rewriting, response buffers and terminal egress. Do not add two standalone totals or omit shared/new live owners.
- [ ] Install only qualified descriptors, initialization/cache ownership and schema admission on the actual Fastly path through #88. Enable the approved definitions through the existing discovered publication mechanism only after their requirements pass. Removing a `native_only` flag alone is not the implementation.
- [ ] Execute the actual guest publication/registry, invalid-parameter, privacy, cap, cancellation and error controls. Require positive routed behavior for the exact 14 identities in spec section 3 and a combined within-limit case. Refusal-only, metadata-only, direct-factory or test-override evidence cannot qualify serving.
- [ ] Keep all unrelated/unqualified definitions closed. Record exact source, features, ABI, SDK, profile, config and artifact identities before claiming #73/#88 complete. Commit the fit and serving changes under their existing issues, not as part of the #34 planning PR.

```sh
cargo run --locked -p auctioneer-cli -- bidders resources --check --all-adapters --capture-audit
cargo run --locked -p auctioneer-cli -- bidders resources --check --adapter fastly --require-fit
cargo test --locked -p auctioneer-bidder --test catalog_publication
export CARGO_TARGET_WASM32_WASIP1_RUNNER="wasmtime run --dir $PWD"
cargo test --locked -p auctioneer-bidder --test catalog_publication --target wasm32-wasip1 -- --test-threads=1
cargo test --locked -p auctioneer-config-data --test bidder_storage
cargo test --locked -p auctioneer-adapter-fastly --all-targets
```

Use the pinned Wasmtime from the development guide. Capture the first command's stdout to a fresh audit artifact and validate that exact capture with the existing qualification owner; an audit is not a declaration of fit. The second command must remain nonpassing until Fastly's required facts and fit checks pass; success on other adapters is not this package's selected-target criterion. Guest publication controls do not replace the combined-application proof. No numeric result is preclaimed in this plan.

## Package 6: Preserve Typed Results And Host Delivery (#76)

**Files:** Existing host result/formatter/publisher/integration files in the map, the shared typed local result and host binding. **Prerequisites:** #51's real operation/result contract; #73/#88 for accepted all-14 Fastly serving. Result projection work may be developed earlier with fixtures, but cannot be labeled completed all-14 Fastly integration.

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
- [ ] Drive real direct `/auction`, publisher split dispatch/collect and current page-bids flows through that artifact. For every required identity, observe real admitted bidder dispatch, valid display results and compatible delivery; execute the complete set together within existing limits. Reconcile exact sent impressions and no duplicate local/remote assignment.
- [ ] Run the two isolated operator-policy snapshots against the same artifact and independently vary accounts, aliases, privacy, settings and secret references. Exercise invalid input, config changes, no-cross-host retention, original deadlines, partial failure, cancellation/abandonment, recovery and bounded diagnostics. Use synthetic authorized fixture stores; no live mutation or private credentials are authorized.
- [ ] Check final winner/targeting/creative behavior and the selected request/response ownership. Repeat affected resource/allocation owners after the final link. Bind final evidence to this artifact; do not upgrade another run's observations or infer a pass from compilation.
- [ ] Run the complete applicable acceptance suites in both repositories. Keep exact command lines, dispatched target/counts, exit codes and result lines. New targets/features must have an explicit existing CI owner; do not add Rust copies of workflow text. No skipped or ignored required row counts as passed.
- [ ] Complete independent review and maintainer authorization before merge. Verify actual default-branch integration and criterion-level issue closure. #34 contract acceptance, #51 bridge delivery, #73/#88 readiness and #76 actual-host evidence remain distinct outcomes; live hosted/publisher qualification stays with its existing selected-path issues.

```sh
./scripts/async-html-finalization-local-test.sh fastly --release --auction
cargo test --manifest-path crates/trusted-server-integration-tests/Cargo.toml --features html-eof-runtime --test html_eof_runtime
cargo clippy --manifest-path crates/trusted-server-integration-tests/Cargo.toml --features html-eof-runtime --test html_eof_runtime -- -D warnings
```

Run these from Trusted Server with its documented runtime prerequisites. Extend the existing harness selection to make the new local-execution cases mandatory and verify their actual dispatch; the unchanged existing command is not evidence that local/all-14 cases already exist. The baseline control uses an unreachable remote bidder and cannot serve as the new positive local acceptance case.

## Full Verification And Evidence Policy

The implementation packages above describe future checks, not executed results. Every changed package gets red/green controls before its implementation and the full prescribed suite before review handoff. Do not invent exact counts for tests that have not been written, change a frozen threshold after measuring, weaken assertions, restore compiled-output caches, or repeat independent child-build measurements merely to increase a test total.

For this planning PR, run document/source/membership checks and available local native acceptance, then execute and inspect the complete current-head GitHub matrix. The repository's workflows remain authoritative; no workflow change or docs-only test exemption is proposed:

- `.github/workflows/test.yml`: secure, default/insecure workspace, CLI/maintenance, mandatory once-owned producers and compiler/config lanes, conformance, guest target checks and their existing explicit owners.
- `.github/workflows/format.yml`: formatting and each lint feature lane, including conformance.
- `.github/workflows/ci-fastly.yml`: vendor packaging, size and actual existing guest controls, not local Auctioneer host integration.
- `.github/workflows/ci-cloudflare.yml` and `ci-spin.yml`: unchanged regression coverage; their completion does not gate the selected product's architecture or establish Fastly readiness.

Before local commands, prepare the locked dependencies, source fixture commits, SDK root and pinned runner tools from [development and test ownership](../../testing/development.md). Keep separate Cargo invocations for feature graphs and one heavy build at a time. Existing explicit once-owned CI selections and their compensating owners stay intact; do not add new omissions or rely on a bare workspace invocation as the complete matrix.

Trusted Server changes require its full `AGENTS.md`/workflow acceptance, including applicable core/adapter, CLI, JS, format, target and actual-runtime owners. Its target aliases are not interchangeable with Auctioneer's; `cargo test-fastly` invokes its Fastly/Viceroy suite, not a native workspace test. This planning PR changes no Trusted Server files and makes no claim of having executed that application's new integration.

Record missing facts or failed/external checks as nonpassing, with exact output and next owner. An open, non-draft planning PR allows joint document review while checks run; it is not a claim of completed verification or merge readiness. Do not merge unless the current-head suite, review-thread acceptance, independent approval and maintainer authorization gates hold.

## Planning PR Handoff

- [ ] Update the spec's status to joint spec/plan review without representing it as approved for implementation.
- [ ] Run a whole-pair independent document review and address material findings in both documents.
- [ ] Verify only these planning documents differ from main, commit them without generated/co-author trailers, and push the dedicated planning branch after the authorized checks.
- [ ] Open one non-draft planning PR against `main`, with `PRfection phase: plan`, `Refs #34`, links to both documents, source baseline, actual verification receipts and unexecuted/blocked checks. Do not use an issue-closing keyword.
- [ ] Read back the URL, open/non-draft state, intended head/base and check status. Ask for joint document review; do not implement, close #34, resolve reviewer threads, or merge merely because the PR exists.

# Stored Objects API Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Resolve stored OpenRTB templates and account policy through one configured API, using the same funded preparation path for standalone and embedded auctions.

**Architecture:** Keep one runtime operator policy and replace its inline named-account map in one hard cut. A shared resolver performs at most two identity-only batch reads, pins one snapshot, expands before canonical OpenRTB decoding, and projects account policy and response budgets from the same record. Existing host services, permits, receipts, deadlines and additive accounting remain authoritative.

**Tech Stack:** Existing Rust workspace, Serde/serde_json, validator, EdgeZero HTTP/secret-store APIs, existing test-support and qualification tooling. No new crate, dependency, SDK extension, database client or query engine is authorized.

---

## Authority and gates

- Design: [approved spec](../specs/2026-11-09-stored-objects-api-design.md).
- Tracking: [#35](https://github.com/IABTechLab/auctioneer/issues/35), with implementation owners [#53](https://github.com/IABTechLab/auctioneer/issues/53), [#77](https://github.com/IABTechLab/auctioneer/issues/77) and [#78](https://github.com/IABTechLab/auctioneer/issues/78).
- The user approved writing this plan and, on resumption, publishing the verified spec and plan together in one non-draft planning PR. Implementation and merge remain separately gated; merge the approved planning PR before starting an authorized implementation phase.
- This is one coordinated cutover, not three competing configuration mechanisms. Unwired, tested primitives may land first; no second public preparation function, compatibility mode, temporary feature flag, inline/API fallback or second loader may be introduced.
- Task 3 freezes the concrete source/storage/work package. Task 4's production funding changes and all activation wait for that package's independent resource review and explicit approval of any new reserve. A failed proof returns to that narrow design review; it does not authorize changing traffic limits, ceilings or the equation.
- Separate shared-resource and provider/schema qualification work remains separate. Adapt to its merged APIs when necessary; do not copy unmerged implementations or claim this plan closes those issues. Unknown provider facts block that target's lookup qualification.
- Every command below is a planned command and every stated outcome is expected, not executed evidence. The user's standing rule prohibits local Cargo/tests/fmt/Clippy/CI-script execution: execute validation in authorized hosted CI checkouts with the tools and source prerequisites in [development.md](../../testing/development.md). The owned local worktree is for source/document edits and read-only checks such as `git diff --check`. A command block does not override that rule.
- Preserve the complete current workflow matrix. Obtain tests-first failure and fixing-head success from separately authorized implementation checkpoints, recording exact heads, commands, case names and results. Do not add CI jobs, repeated full-suite owners, mutation PRs or new exclusions merely to execute this plan; any additional hosted experiments need their own authorization.

## Files and ownership

The initial draft inspected `499f0e540e95e1c86b730c1030aabef28de01d0a`; this resumption refreshes the inventory against merged main `d26fcf7f566db6d1dce2e1847d79c9f44581edb8`, including the shared resource work in #162 and recipient-root projection in #232. Reconcile the inventory again against the eventual implementation base before editing; material contract changes return to review.

| Owner | Files | Responsibility |
| --- | --- | --- |
| Shared configuration | Create `crates/auctioneer-config/src/auction_data.rs`; modify `src/lib.rs`, `src/policy.rs`, `tests/root_shape.rs` in that crate | Source settings, bounded JSON Pointer syntax/origins, one default selector list; eventual format-2 root |
| Portable resolution | Create `crates/auctioneer-core/src/stored_objects/{mod,protocol,budget,merge,account,resources}.rs`; modify that crate's `src/lib.rs` | Batch/record contract, two-stage resolution, cumulative counts, expansion, admitted account projections and resource facts |
| Common SDK attachment | Create `crates/auctioneer-core/src/host_primitives/auction_data.rs`; modify `src/host_primitives/mod.rs` | Capture existing HTTP/secret handles; bounded identity-only POSTs; no provider-specific resolver |
| JSON admission and arithmetic | Modify `crates/auctioneer-runtime/src/{bounded_json,budget,auction_budget,lib}.rs` only as needed | Reusable duplicate-aware admission and checked charges under existing owners; no OpenRTB policy in runtime |
| Canonical preparation | Modify `crates/auctioneer-core/src/auction/{prepare,services,operation,result,audit}.rs` and `src/handlers/auction.rs` | One settings-first async API, original time zero/funding, owned prepared state and fixed failures |
| Final OpenRTB invariants | Modify `crates/auctioneer-openrtb/src/validate.rs` with inline units | Nonempty final request/impression IDs, without requiring complete partial fragments |
| Account policy consumers | Modify core `src/config/{resolve,policy_account,budgets,validation,sdk}.rs`, `src/handlers/{cookie_sync,setuid}.rs` | Remove inline-map reads; reuse account validation/projection in auctions and sync |
| Late FPD boundary | Modify `crates/auctioneer-exchange/src/split/mod.rs` | Reject reserved controls after projection without another lookup |
| Shared fake and integration owners | Create `crates/auctioneer-core/src/test_support/stored_objects.rs`; modify its `mod.rs` and the existing targets named below | One fake source/SDK fixture; setup only, not duplicated expected results |
| CLI and current examples | Modify `crates/auctioneer-cli/src/application.rs`, affected current qualification fixture builders, `auctioneer.toml`, `crates/auctioneer-core/tests/config_reference_examples.rs` | Typed CLI/root cutover and exhaustive reference; preserve unrelated schema versions and historical bytes |
| Adapter capacity consumers | Each adapter's `src/limits.rs` and `src/envelope.rs`, Axum `src/resource_profile.rs`, their existing resource tests, core `src/host_primitives/gates.rs` and CLI `--adapter` resource readers | Carry the same reviewed cost inputs; no independent equation or invented provider facts |

New modules use ordinary `mod` declarations. Unit tests stay inline; integration tests remain in existing compatible binaries. Do not add handwritten source imports or cross-crate private-source compilation. Use `json!` for ordinary JSON controls and fictional `fake_` identities.

## Execution rules

For each task, add the named tests first, list them, observe a genuine failing branch, implement the smallest correction, then rerun the exact owner. New filters do not exist today: after adding tests, verify their names appear under `-- --list`; a zero-test exit is never a red or green result. Record actual result lines, not predicted counts.

Commit tested checkpoints using only the task's changed file paths, `git diff --cached --check`, and the suggested sentence-case title. Never stage the whole worktree, publish a checkpoint without authorization, or add bylines. Dependent cutover edits may be developed together and committed atomically rather than committing a broken intermediate tree.

### Task 1: Record the baseline and migration inventory

**Files:** Read the approved spec, `AGENTS.md`, `crates/AGENTS.md`, `docs/testing/development.md`, `.github/workflows/{test,format,ci-cloudflare,ci-fastly,ci-spin}.yml` and their colocated scripts. No production edits.

- [ ] Capture the implementation base, tool versions, installed targets and current source prerequisites. Resolve the SDK checkout from Cargo metadata using development.md, not a guessed cache path. Do not refresh or suppress an expired archive without separate authorization.
- [ ] Establish the full current hosted baseline, including the default-workspace command in Acceptance below and every dedicated owner. The initial draft's listener-shutdown failure is historical, not evidence that the refreshed base fails; its isolated pass did not waive that earlier failure. Record current-head executed results rather than carrying an old failure or pass forward. Do not fold an unrelated repair into this feature silently.
- [ ] Inventory `prepare_auction(` callers, `resolve_selected_account`/`resolve_account_from_*`, `policy.accounts`/`app.accounts`, SDK-wrapper validation, operator-schema literals and generated-App fixtures using `rg`. Classify each result by owner and retain its assertions. Record actual before-state test names/counts, including feature-gated targets.
- [ ] Distinguish operator format 1 from API protocol 1, qualification/profile/envelope versions and external captures. Only the operator format moves to 2. Identify current fixture builders needing migration; do not bulk-replace every `schema_version` or rewrite historical evidence.
- [ ] Record integration prerequisites and choose existing owners from Task 10. If a new target or dependency appears necessary, stop that change for review rather than bypassing test isolation or extending this plan silently.

**Expected:** A recorded baseline and exhaustive source-derived migration list, not a hand-picked list of four callers. No code, feature or acceptance claim yet.

### Task 2: Add the model and strict protocol primitives without activating them

**Files:** Config `src/auction_data.rs`, `src/lib.rs`; core `src/stored_objects/{mod,protocol,budget}.rs`, `src/lib.rs`; runtime `src/bounded_json.rs` if a reusable admission helper is needed.

- [ ] Add inline tests under `auction_data::tests` for endpoint/namespace/credential-reference validation, both JSON Pointer settings, default-versus-replacement ordering, decoded-token origin rules, escapes and forbidden-source rejection. Keep the six defaults in one owner. Validate with existing model/validator conventions; use existing structured JSON pointer lookup in core, not a query engine.
- [ ] Add inline `stored_objects::protocol::tests` for all three kinds, exact row coverage, versions/statuses, required/forbidden documents, unknown control fields, duplicate/trailing JSON and order-independent matching. IDs shared across kinds must remain distinct. Add `stored_objects::budget::tests` for cumulative body/node/key accounting and checked arithmetic.
- [ ] Keep `AuctionDataPolicy` standalone at this checkpoint: the active operator root still accepts only its current format. Do not temporarily accept both roots or add a second config API. Define strict wire envelopes with existing Serde and fixed error categories. Treat `document` as bounded structured data, never an unbudgeted raw capture.
- [ ] Establish duplicate-key refusal before map materialization for complete participating documents, including ignored wrapper/control subtrees. Extend the existing Serde admission owner rather than inventing a lexical parser. Preserve its private-raw-marker refusals, end-of-input, node and depth protections. Account for any per-object key-set storage in Task 3.
- [ ] Run the following, inspect nonzero dispatch and commit the tested primitives as `Add stored object policy and protocol primitives`:

```sh
cargo test --locked -p auctioneer-config --features config-model --lib auction_data::tests:: -- --list
cargo test --locked -p auctioneer-config --features config-model --lib auction_data::tests::
cargo test --locked -p auctioneer-core --lib stored_objects::protocol::tests:: -- --list
cargo test --locked -p auctioneer-core --lib stored_objects::protocol::tests::
cargo test --locked -p auctioneer-core --lib stored_objects::budget::tests::
cargo test --locked -p auctioneer-runtime --lib bounded_json::tests::
```

**Expected:** Added controls execute and pass; existing duplicate/raw-value/depth/node boundaries remain intact. The operator cannot activate the new source yet.

### Task 3: Freeze and review the exact resource package

**Files:** Core `src/stored_objects/resources.rs`, `src/auction/{services,audit}.rs`; runtime `src/{budget,auction_budget}.rs`; existing core `tests/{config_parse_factor,config_peak_charge}.rs` for changed config capture/projection; inspected adapter resource rows. Reuse compatible allocator processes, not a new binary.

- [ ] Enumerate every simultaneous owner from spec section 6: borrowed live input; accepted config/selectors; bounded secret/header/request buffers; original response and duplicate-check state; root/merged/final values; account overlay and both projections; repeated impression output; snapshot/dedup/IDs; traversal stack and SDK handoff. State when each is released and whether any provider storage is opaque.
- [ ] Derive checked formulas from existing B/N/I/T/D, config caps and compiler-owned layouts. Include merge/selector/duplicate-detection visits and overwritten/deleted subtrees. Distinguish actual source visits from retained output. Do not call bytes, nodes and visits interchangeable or use a passing measurement as the allowance.
- [ ] Build independent controls in existing allocator owners: vary byte/node/selector/impression/reference axes separately, use large inputs within accepted limits, and include a positive allocation control. Freeze exact literal expectations or independent derivations, one-byte-under funding refusals and overflow outcomes. Measure real SDK buffer ownership separately from a synthetic source.
- [ ] Put pure source/merge allocation units beside `stored_objects::resources`, using the core unit binary's existing counting allocator and its per-thread measurement API. Keep config extraction/model-capture measurements in `config_parse_factor`/`config_peak_charge`. Do not put lookup measurements in `serialize_allocation_bound`: that isolated target owns final-response serialization and remains a regression check. Preserve cold-state/process isolation; a needed incompatible measurement owner returns to review instead of silently mixing allocators or adding a binary.
- [ ] Produce one reviewed package of formulas, layouts, target facts, overlap diagram, unchanged workload/ceilings and intended existing receipt charges. New lookup charges enter the existing additive equation and original owners, not a new permit pool, independent source profile or catalog-cost disguise. Account-only auxiliary routes require their own original-owner derivation too.
- [ ] Obtain the spec's independent resource review and explicit approval for any new reserve before Task 4's production accounting changes. If an assumption fails, return the exact failing owner/formula to that review; independent protocol tests may continue, but activation and completion remain blocked. No zero-cost placeholder, lowered workload or ceiling increase is a remedy.

**Planned checks:**

```sh
cargo test --locked -p auctioneer-core --lib stored_objects::resources::tests::
cargo test --locked -p auctioneer-core --test config_parse_factor --test config_peak_charge --test serialize_allocation_bound
cargo test --locked -p auctioneer-runtime --lib budget::tests::
cargo test --locked -p auctioneer-runtime --lib auction_budget::tests::
cargo test --locked -p auctioneer-runtime --test term_exhaustiveness
```

**Expected:** An explicitly accepted package, or a named blocker. These commands alone cannot approve a reserve or prove a platform-private memory bound.

### Task 4: Integrate the accepted charges and refusal checks

**Files:** Core `src/stored_objects/{budget,resources}.rs`, `src/auction/{services,audit}.rs`, `src/host_primitives/gates.rs`; runtime `src/{budget,auction_budget}.rs`; existing adapter capacity consumers and their resource tests.

- [ ] Add tests that a source lookup cannot start with missing/underfunded costs, while an input needing no lookup performs no source or secret read. Pin original admission/aux ownership and zero resource leaks on every refusal.
- [ ] Carry the reviewed neutral costs through the existing resource input tuple. Update all constructors discovered in Task 1, including embedding and CLI paths, not just the standalone bootstrap. Require the same funded facts before each new allocation/I/O class; do not reacquire an auction permit or count one reserve twice.
- [ ] Preserve existing target clamping, the single additive total and one-byte boundary controls. A target with unknown or non-fitting prerequisites must refuse lookup activation with bounded diagnostics; default-only liveness behavior remains independent of the API.
- [ ] Run the exact resource tests plus original construction/funding controls below. Expected totals change only by the approved, independently pinned terms; never update expected and actual from the same new constant without an independent control.
- [ ] Commit as `Fund stored object preparation on existing resource owners` only after the checkpoint is accepted and the tested tree builds.

```sh
cargo test --locked -p auctioneer-core --lib auction::audit::tests::
cargo test --locked -p auctioneer-core --lib stored_objects::resources::tests::
cargo test --locked -p auctioneer-core --test boot_resource_charges --test config_funding --test auction_budget_concurrency
cargo test --locked -p auctioneer-runtime --test term_exhaustiveness
cargo test --locked -p auctioneer-adapter-axum --test resource_profile
cargo test --locked -p auctioneer-adapter-cloudflare -p auctioneer-adapter-fastly -p auctioneer-adapter-spin --test boot_envelope
```

### Task 5: Implement the bounded identity-only HTTP source

**Files:** Create core `src/host_primitives/auction_data.rs`; modify its `mod.rs`; core `src/stored_objects/{protocol,budget}.rs`; create `src/test_support/stored_objects.rs` and register it in existing `test_support/mod.rs`.

- [ ] Add inline `host_primitives::auction_data::tests` using an existing SDK outbound-client fixture pattern. Exercise the real SDK head/body finalization, not a fake already-decoded response. Pair each visible-coding negative with successful identity delivery of the same valid JSON.
- [ ] Pin the spec's normalized-header boundary, not raw-origin-header provenance. Pass a gzip label with `Connection: content-encoding` through real SDK normalization: fitting valid JSON is admitted as unlabeled with identical collected bytes, while actual gzip bytes fail JSON admission without decoding. Keep the exposed-gzip negative that rejects enabling automatic SDK decoding. Do not bypass the SDK or add a provenance capability.
- [ ] Capture HTTP/secret-store handles and their clock without I/O. When a lookup is admitted, read the referenced bearer token once, with bounded bytes and the original absolute deadline, and retain it only for this preparation. Reuse the existing credential loader's ownership pattern, not its bidder-specific payload parser. Never forward incoming headers/cookies or log a store/SDK error containing values.
- [ ] Serialize only the declared envelope under existing request raw/encoded/header bounds before dispatch; do not use an unbounded `OutboundRequest::json` allocation then measure it. Use existing counting/structured serialization facilities and a bounded writer owned here. Do not recompile or import the private user-sync writer.
- [ ] POST to the complete configured URL with cache bypass, identity headers, `ResponseDecoding::Original`, streamed mode, existing header/chunk caps, remaining encoded/decoded allowances and the original deadline. Apply section 7.1's typed SDK refusal precedence before inspecting a returned status/normalized coding, then use `into_bytes_bounded_until(remaining, deadline)`. Do not let a later builder setter silently restore buffered mode.
- [ ] Derive lookup T/D from the host row narrowed by the installed profile, before remote account resolution. Independently vary named-account, account-default and bidder/alias response-cap overlays while holding that host row fixed; assert that the lookup allowance does not change. Then vary each host cap against the installed ceiling and assert the exact narrowed allowance. Later scoped bidder-response budgets still use the selected overlay once, as Task 7 requires.
- [ ] Implement one SDK-error mapper for send and collection failures. Recheck the original deadline after await, preserve typed limit refusals, and never recover status/coding from diagnostic strings. Add independent SDK-backed collision controls for oversized 200/401/503 and gzip-labeled 200 heads, header limits and malformed framing. Assert exact categories and no application body polling on head preflight refusal; pair them with fitting returned-head outcomes and a separately expired original deadline. Do not enlarge caps to inspect an otherwise refused head.
- [ ] Implement the exact cumulative transition and no retry/redirect behavior:

```text
C = min(T, D); used starts at 0
before a required batch: remaining = checked_sub(C, used)
remaining == 0 -> funded resource refusal, no dispatch
after an admitted identity body: used = checked_add(used, body.len())
retain used, remaining node/key accounting and the snapshot for the next batch
```

- [ ] Verify two destinations/paths, exact method/envelope, status/secret failures, no-I/O paths, coding rejection, all cumulative boundaries and independent T/D axes. A valid gzip negative must fail if original mode is replaced with SDK decoding. Keep provider-opaque delivery unqualified rather than inventing original-byte evidence.
- [ ] Run and commit as `Add bounded identity-only auction data lookup`:

```sh
cargo test --locked -p auctioneer-core --lib host_primitives::auction_data::tests:: -- --list
cargo test --locked -p auctioneer-core --lib host_primitives::auction_data::tests::
cargo test --locked -p auctioneer-core --lib stored_objects::budget::tests::
cargo test --locked -p auctioneer-core --test edgezero_fanout --test upstream_primitives
```

**Expected:** SDK-backed negative/positive controls execute with exact call/body observations. No serving cutover or provider-fit claim yet.

### Task 6: Expand partial requests with one bounded merge owner

**Files:** Core `src/stored_objects/{mod,merge}.rs`; OpenRTB `src/validate.rs` and inline units; reuse runtime admission and canonical OpenRTB preparation; extend existing core `tests/{handler_resolution,original_request_extension,node_budget_on_the_auction_path,impression_limit_on_the_auction_path}.rs` when wiring becomes available.

- [ ] Add inline `stored_objects::merge::tests` with literal expected JSON for object recursion, null deletion, array/scalar replacement, absence, source/live IDs and sibling extension preservation. Include root-only, impression-only, combined, repeated and forbidden nested references. A root `imp` replacement never performs an ID-based array merge.
- [ ] Admit complete participating JSON before materializing maps. Discover root control first; validate a found fragment's required object/container/control shapes without demanding a complete typed auction. Keep the JSON representation until expansion is finished so absent/null distinctions survive. Do not deserialize an incomplete `BidRequest` or mint dummy IDs to make it parse.
- [ ] Implement RFC 7396 semantics with structured APIs, the approved byte/node/depth/work accounting and checks before allocating/growing destinations. Avoid cloning then deleting unneeded whole trees. Every repeated impression expansion consumes work/output allowance even when its fetch was deduplicated.
- [ ] Reject invalid live references as caller input and invalid source/control shapes as source data. Perform complete supported OpenRTB field/type validation only on the final expanded document through canonical ingress. Do not introduce a second general OpenRTB decoder or silently coerce malformed fields. If more than structural partial validation is proposed, return that design change for review rather than duplicating model validation.
- [ ] Add the missing final-ID invariants to `BidRequest::validate` in OpenRTB's existing validation owner: request ID and every impression ID must be nonempty, in addition to existing nonempty-array and uniqueness checks. Use fixed validation categories, with no backend-selected ID in the diagnostic. Add inline `validate::tests` proving each empty-ID check independently on an otherwise valid request, plus existing duplicate-ID/nonempty-array controls. Missing/null/wrong-type final IDs continue to fail canonical typed decoding; do not require an ID before partial expansion or invent one.
- [ ] In the merge units, assert that an omitted live ID can inherit a valid template ID and that a nonempty live ID wins. At Task 9 wiring, extend `handler_resolution` and the shared typed API controls to assert HTTP 400/the corresponding typed invalid-request reason for independently empty live, inherited and null-deleted final request/impression IDs, with no bidder dispatch. Test missing IDs that are successfully supplied by expansion separately from missing IDs that remain missing. These are required checks, not a claim that today's canonical validator already implements them.
- [ ] Run the complete OpenRTB package and current core preparation/handler owners after introducing these final-ID checks, before the async/root cutover. Inventory any existing fixtures that accidentally rely on empty IDs. Give otherwise-valid controls explicit `fake_` IDs so they still reach their own guard; preserve dedicated malformed-ID inputs and assertions. Do not relax validation, bulk-fill production input or rewrite an unrelated negative to make the checkpoint green.
- [ ] Consume only recognized reference members and preserve siblings; do not walk arbitrary bidder protocol fields as references. Admit the post-root impression population before building the second key set or expanding it, since impression templates cannot reduce that array's length. Respect I even for repeated references with one distinct key.
- [ ] Run the merge tests, inspect dispatch and commit as `Expand stored requests with bounded merge semantics`:

```sh
cargo test --locked -p auctioneer-core --lib stored_objects::merge::tests:: -- --list
cargo test --locked -p auctioneer-core --lib stored_objects::merge::tests::
cargo test --locked -p auctioneer-runtime --lib bounded_json::tests::
cargo test --locked -p auctioneer-openrtb --lib validate::tests:: -- --list
cargo test --locked -p auctioneer-openrtb --lib validate::tests::
cargo test --locked -p auctioneer-openrtb --all-targets
cargo test --locked -p auctioneer-core --lib --test handler_resolution
```

**Expected:** Structural expansion works without active config-root or public preparation changes; final canonical failures remain independently observable.

### Task 7: Project one remote account into policy and response budgets

**Files:** Core `src/stored_objects/account.rs`, `src/config/{policy_account,budgets,validation,resolve}.rs`; existing `tests/{response_account_resolution,privacy_config_parity,usersync_config_lifecycle}.rs` when the cutover is wired.

- [ ] Add inline `stored_objects::account::tests` for default/required/named/missing/denied/disabled/malformed cases. Exercise empty and nested account-policy pointers, strict selected objects, unselected bounded wrappers and no fallback. Keep sync's explicit account parameter separate from auction selectors.
- [ ] Project product defaults -> `account_defaults` -> the one admitted remote `AccountPolicy`, reusing existing nested validators and mergers. Keep false, empty collections, omission and explicit clears distinct. A found malformed record never turns into default selection.
- [ ] Change pure account/budget helpers to accept the selected overlay explicitly; retain it until both `AccountConfig` and scoped budgets are frozen. Preserve independent per-field bidder -> alias -> remote account -> defaults -> runtime/product host precedence and existing installed clamps. Do not reconstruct an operator policy or read the backend again.
- [ ] Add independently varied cap and privacy controls, including restrictive remote policy behind permissive defaults. Assert literal budgets and exact failure categories; do not couple the two cap fields or all privacy switches in one fixture. Prove one account fetch rather than two hidden lookups.
- [ ] Do not remove active inline-map readers until Task 9 can replace every consumer atomically. If helper-signature changes would break the interim tree, develop them with Task 9 and commit together; never add a compatibility API or default-only shim merely to make the intermediate tree green.

```sh
cargo test --locked -p auctioneer-core --lib stored_objects::account::tests:: -- --list
cargo test --locked -p auctioneer-core --lib stored_objects::account::tests::
cargo test --locked -p auctioneer-core --lib config::budgets::tests::
cargo test --locked -p auctioneer-core --lib config::policy_account::tests::
```

**Commit checkpoint:** `Project remote account policy and response budgets`, or include the dependent edits in the atomic cutover commit.

### Task 8: Close the post-FPD reserved-control boundary

**Files:** `crates/auctioneer-exchange/src/split/mod.rs`; core `src/stored_objects/merge.rs`; existing core `tests/privacy_outbound.rs` and feature-gated `tests/bidder_handler_conformance.rs`.

- [ ] Add early-admission cases for `imp.ext.prebid.imp.<fake_bidder>` patches carrying reserved stored-reference controls from live and stored data, including null and valid-looking IDs. Restrict inspection to actual shared FPD patch locations.
- [ ] Add an independent inline split test that supplies an already-projected synthetic request, bypassing only the earlier admission gate in test setup. Prove the final guard fails when the reserved root/impression control is present and succeeds for otherwise identical protocol data.
- [ ] Respect #232's select-before-copy boundary: `root::project` already drops an unrecognized root `storedrequest`. Check the grouped root's reserved path before that projection can erase it, and check each scoped impression immediately after `build_scoped_imp` applies FPD, before typed decoding or bidder construction. Use separate root/impression controls so either check's removal is observable. Do not add stored controls to the finite forwarding set or clone the complete root merely to inspect it; preserve the existing exact-recipient and alias/no-fallback assertions.
- [ ] Enforce absence after shared FPD mutation and before bidder construction/dispatch. Reuse the existing fixed `InvalidImpFirstPartyData` failure scope/mapping, without another fetch, silent forwarding or a production failure-injection hook. Do not let the early guard mask removal of the final guard.
- [ ] Run split tests and existing privacy/projection owners, then commit as `Reject stored controls reintroduced by bidder projection`:

```sh
cargo test --locked -p auctioneer-exchange --lib split::tests::
cargo test --locked -p auctioneer-exchange --test source_preflight --test disclosure_policy_diff --test projection_scrub_bound --test privacy_policy_composition
cargo test --locked -p auctioneer-core --test privacy_outbound
```

### Task 9: Make the single async/configuration cutover atomically

**Files:** Config `src/{policy,lib}.rs`, `tests/root_shape.rs`; core `src/auction/{prepare,services,operation,result,audit}.rs`, `src/handlers/{auction,cookie_sync,setuid}.rs`, `src/config/{resolve,policy_account,budgets,validation,sdk}.rs`; CLI `src/application.rs`; current examples and all affected callers from Task 1.

- [ ] Add new root-format, handler and bridge tests before changing readers. Then make format 2 the sole operator format, add top-level `auction_data`, remove `accounts`, and update CLI wrappers/validation and current fixture builders together. Keep SDK secret metadata reference-only, not inline bearer values. No legacy nested spelling or env alias is accepted.
- [ ] Replace, rather than supplement, the public preparation API:

```rust
pub async fn prepare_auction<'host>(
    settings: &AuctioneerAppConfig,
    input: AuctionInput<'_>,
    services: AuctionServices<'host>,
    funding: AuctionFunding,
) -> Result<PreparedAuction<'host>, AuctionFailure>
```

- [ ] Attach one borrowed host-provided source to services and capture accepted settings/handles once. The first optional root batch requests a fresh snapshot; the second batches account/impression keys at that snapshot. Exact matching, no retries, no third lookup, no cross-request cache and no request-selected URL remain mandatory. The source's configuration/secret session must not borrow the discarded full policy into `PreparedAuction`.
- [ ] Establish the original absolute deadline using existing `resolve_tmax` semantics before I/O, including zero/default/max behavior. A merged template can only shorten it. Carry the tightened absolute deadline into execution; no re-anchoring or whole-policy clone. Capture original JSON extension shape before expansion; typed input keeps unavailable-original-shape semantics.
- [ ] Resolve and validate the selected account and both projections with the exact spec stage/error order, then canonical request/privacy checks. Map fixed/redacted reasons through the common result projector. Exercise malformed envelope versus denied/missing/malformed-row precedence independently of result order.
- [ ] Convert `/cookie_sync` and `/setuid` to the same account-only resolver, original aux owner and route deadline. Keep opt-out/input/bidder checks, encoded callback-account propagation and existing route output shapes. No cookie mutation or sync URL on lookup failure; `/getuids` and `/status` gain no lookup.
- [ ] Update every old synchronous call/test/doc example from Task 1 to await or `block_on` the one API. Remove obsolete account-selection/map-resolution paths rather than retaining another authority. Re-run `rg` inventory; remaining inline-map references may only be explicit rejection tests or preserved historical data.
- [ ] Complete the following checkpoint before committing as `Resolve auctions and accounts through one runtime data API`:

```sh
cargo test --locked -p auctioneer-config --features config-model --lib --test root_shape
cargo test --locked -p auctioneer-core --lib
cargo test --locked -p auctioneer-core --test handler_resolution --test response_account_resolution --test original_request_extension
cargo test --locked -p auctioneer-core --test node_budget_on_the_auction_path --test impression_limit_on_the_auction_path
cargo test --locked -p auctioneer-core --test usersync_handlers --test usersync_precedence --test usersync_flow
cargo test --locked -p auctioneer-cli --bin auctioneer-cli application::config_contract:: -- --list
cargo test --locked -p auctioneer-cli --bin auctioneer-cli application::config_contract::
```

**Expected:** One supported runtime root and one preparation API. Default-only inputs need no lookup/secret read. All selected records either govern the operation or cause a typed refusal.

### Task 10: Prove lifecycle, real handler and ownership behavior

**Files:** Existing core `tests/{handler_resolution,auction_config_lifecycle,auction_config_retention,admission_holds_through_auction,response_account_resolution,usersync_config_lifecycle,usersync_routes,usersync_handler_alloc,serialize_allocation_bound}.rs`; test-support fake; core preparation/HTTP-source inline units. Extend the existing target modules normally, not with source imports.

- [ ] Compare representationally equivalent JSON and typed inputs through the one API with identical settings, service plan, snapshot and randomness. Compare semantic requests, account/caps, outcomes and funding; do not claim raw byte parity or regenerate historical captures from the implementation.
- [ ] Test API-record-only, operator-config-only and credential-only updates separately. Suspend between batches and prove one snapshot/endpoint/namespace/token stays pinned, while the next request observes the independent update. Snapshot mismatch/expiry must refuse without restart. Do not vary every input together and hide missing capture behavior.
- [ ] Drop while secret read, source body or second batch is pending; expire before/at/after each checkpoint and during expansion. Assert no late bidder dispatch, no retained request/aux permit, original ledger restoration and exactly-once completion. Verify caller-borrowed input remains unchanged and is still included in the peak while alive.
- [ ] Exercise the generated standalone App and the exposed library path with the real SDK source boundary, plus the named-account sync routes. A fake source tests policy, not physical transport; SDK-finalizer tests prove coding and scoped reads, not hosted provider facts. Preserve the existing socket/allocator/boot-global process isolation.
- [ ] Run each existing owner listed above explicitly, plus `bidder_handler_conformance` under its required feature. Inspect actual case names/result counts and commit as `Verify stored lookup lifecycle and shared handler behavior`.

```sh
cargo test --locked -p auctioneer-core --test auction_config_lifecycle --test auction_config_retention --test admission_holds_through_auction
cargo test --locked -p auctioneer-core --test usersync_config_lifecycle --test usersync_routes --test usersync_handler_alloc
cargo test --locked -p auctioneer-core --test handler_resolution --test response_account_resolution --test serialize_allocation_bound
cargo test --locked -p auctioneer-core --features conformance-suite --test bidder_handler_conformance
```

### Task 11: Exercise adapter delivery and preserve qualification boundaries

**Files:** Existing adapter `src/lib.rs`, capacity rows and `tests/{fanout,fanout_conformance,boot_envelope}.rs` as applicable; CLI `src/qualification/harness/providers/{preparation,sdk,artifacts}.rs` only where the existing proof format genuinely needs lookup coverage; `docs/testing/provider-handlers.md`.

- [ ] Verify each adapter captures its existing SDK HTTP/secret bindings into the same shared source path. Do not add provider-specific lookup logic, another loader, duplicate JSON contracts or a central bidder/adapter implementation-name switch.
- [ ] Extend compatible existing adapter conformance owners with positive/negative identity delivery, caps, original deadline/scope and drop behavior. Label native tests with controlled clients accurately; guest compile-only checks are not guest execution or original-wire evidence.
- [ ] Run the current cross-target check/lint/no-run commands from Acceptance for all three guest adapters, and native Axum source/handler controls. Use the existing Rust provider qualification command/format for actual hosted facts; no new script language, test binary or fake evidence row.
- [ ] Record the exact artifact, adapter, SDK and configuration bindings for any delivery proof. A provider that hides recoding/original bytes or cannot meet the frozen resource assumptions stays unqualified for lookup activation. Missing access is a blocker, not an ignored/pass row or permission to mutate an unapproved deployment.
- [ ] Check all source/merge/account acceptance criteria separately from complete target fit and Trusted Server host-service handoff. Do not close unrelated qualification/integration tickets or present compilation as feature completion.

**Expected:** Executed, correctly scoped adapter evidence or explicit target blockers. No deployment or live configuration mutation is authorized by this plan.

### Task 12: Update current documentation and run complete acceptance

**Files:** `auctioneer.toml`, core `tests/config_reference_examples.rs`, CLI `src/application.rs` reference tests; existing operator/embedding/provider docs that describe affected APIs. No historical evidence rewrite, `ISSUES.md`, new milestone labels or Rust workflow snapshots.

- [ ] Make the current config reference exhaustive for the new source fields, selector restrictions, identity protocol, required/default account behavior and secret references. Explain the account-store hard cut and external service responsibilities without claiming to provide its server or database.
- [ ] Update the embedding example to keep settings first and await the same preparation operation. Document source/schema/API versions separately and describe fixed failures, original deadline, provider readiness and no-cache freshness.
- [ ] Run all Acceptance commands and every current required CI workflow at the final implementation head, including dedicated heavy owners. Record command, SHA, exit, actual result lines and each not-run/blocked prerequisite. No new exclusions, widened expectations, artifact-result cache or weakened tests.
- [ ] Review the complete diff against the approved spec/plan. Reconcile Task 1's migration/test inventory, check the raw-fixture/source-import gates, and verify no obsolete active inline account reader, alternate config root or public preparation API remains.
- [ ] After review and separately authorized publication, open a distinct implementation PR linked to the merged plan commit. Close only issues whose actual criteria are delivered; retain provider/resource/host blockers explicitly. No automatic merge is authorized.

## Acceptance commands and evidence

All commands below identify required validation owners and reproducible inputs, not permission to run them locally. Use the existing authorized hosted matrix and inspect actual dispatch/results. Keep the complete matrix mandatory, including owners whose commands are invoked through their colocated workflow scripts. No local Cargo/CI run or extra hosted mutation batch is authorized by this plan.

### Fast checkpoint

Expected: every newly named group executes, no failures, and unchanged source/privacy/resource controls still execute. Counts are discovered and recorded, not hardcoded to today's baseline.

```sh
cargo test --locked -p auctioneer-config --features config-model --lib --test root_shape
cargo test --locked -p auctioneer-runtime --lib
cargo test --locked -p auctioneer-core --lib
cargo test --locked -p auctioneer-bidder --test test_isolation
cargo test --locked -p auctioneer-core --test config_reference_examples --test privacy_config_parity
cargo fmt --all -- --check
cargo clippy --locked -p auctioneer-config --features config-model --all-targets -- -D warnings
cargo clippy --locked -p auctioneer-core -p auctioneer-runtime --all-targets -- -D warnings
```

### Native graphs and dedicated owners

These reproduce existing selection rules, not a new smaller acceptance suite. The two producer skips below are the existing broad-lane exceptions with their mandatory owner; add none. `--offline` may be added only after locked sources and targets have been fetched.

```sh
cargo test --locked -p auctioneer-config-data --lib --test codegen --test disclosure_codegen --test gvl_archive --test smoke --test url_safety --test activity --test product_defaults_snapshot --test simple_fold --test usersync_config --test usersync_callback_validation --test bidder_bindings --test bidder_storage
cargo test --locked -p auctioneer-config --features build-support --lib --test root_shape
cargo test --locked -p auctioneer-bidder -p auctioneer-exchange -p auctioneer-core --all-targets
cargo test --locked --workspace --all-targets --no-fail-fast -- --skip tests::selected_schema_source_producer_records_exact_real_dispatch_without_qualification --skip tests::controlled_native_acceptance_requires_all_producers_but_never_completes_the_full_catalog
cargo test --locked --workspace --all-targets --features insecure-bidders --exclude auctioneer-config-data --no-fail-fast -- --skip tests::selected_schema_source_producer_records_exact_real_dispatch_without_qualification --skip tests::controlled_native_acceptance_requires_all_producers_but_never_completes_the_full_catalog
cargo test --locked -p auctioneer-config-data --features 'insecure-bidders test-harness-override' --lib --test codegen --test gvl_archive --test smoke --test url_safety --test bidder_bindings --test bidder_storage
cargo test --locked -p auctioneer-cli --all-targets -- --skip tests::selected_schema_source_producer_records_exact_real_dispatch_without_qualification --skip tests::controlled_native_acceptance_requires_all_producers_but_never_completes_the_full_catalog
cargo test --locked -p auctioneer-cli --no-default-features --all-targets
bash .github/workflows/scripts/test/verify-evidence-producers.sh
cargo test --locked -p auctioneer-cli --test resource_boot_parity
bash .github/workflows/scripts/test/verify-config-build-failures.sh
```

The dedicated compiler and feature-view owners are also mandatory. Set `RUNNER_TEMP` to an existing disposable log directory in the hosted checkout, as required by their scripts. The unsplit source target below identifies all its cases, including the six assigned independent hosted owners; reconcile those owners rather than omitting the six or adding a redundant full compiler run to CI. Retain fresh-process/fresh-child-workspace requirements and explicit lint coverage for Cargo targets excluded from broad selection.

```sh
cargo test --locked -p auctioneer-bidder --test schema_build_controls --test schema_source_conformance --test factory_binding_build --test registry_build --test generated_settings --test insecure_bidders_snapshot --test response_body_is_sealed --test feature_exports_source_conformance -- --test-threads=1
cargo clippy --locked -p auctioneer-bidder --test schema_build_controls --test schema_source_conformance --test factory_binding_build --test registry_build --test generated_settings --test insecure_bidders_snapshot --test response_body_is_sealed --test feature_exports_source_conformance -- -D warnings
cargo clippy --locked -p auctioneer-cli --test resource_boot_parity -- -D warnings
bash .github/workflows/scripts/test/verify-schema-source.sh 34 schema_selection --locked -p auctioneer-bidder -p auctioneer-exchange -p auctioneer-core
bash .github/workflows/scripts/test/verify-schema-source.sh 34 schema_selection --workspace
bash .github/workflows/scripts/test/verify-schema-source.sh 34 schema_selection --workspace --features insecure-bidders --exclude auctioneer-config-data
bash .github/workflows/scripts/test/verify-schema-core-secure.sh
bash .github/workflows/scripts/test/verify-schema-core-development.sh
bash .github/workflows/scripts/test/verify-schema-metadata-secure.sh
bash .github/workflows/scripts/test/verify-schema-metadata-development.sh
cargo test --locked -p auctioneer-bidder --test catalog_publication
cargo test --locked -p auctioneer-bidder --test catalog_publication --features insecure-bidders
bash .github/workflows/scripts/test/execute-catalog-publication.sh
cargo test --locked -p auctioneer-bidder --lib source_objects_ -- --ignored
bash .github/workflows/scripts/test/verify-supplemental-source.sh
```

The fixed dispatch count above belongs to the existing source-selection owner. If implementation legitimately adds cases there, update its source-derived enumeration and all owners together rather than weakening its dispatch guard. No supplemental source environment is reintroduced: both provenance groups use the one configured checkout.

### Conformance, guest and provider checks

```sh
cargo test --locked -p auctioneer-core -p auctioneer-adapter-cloudflare -p auctioneer-adapter-fastly -p auctioneer-adapter-spin --features conformance-suite --test fanout_conformance
cargo test --locked -p auctioneer-core --features conformance-suite --test bidder_handler_conformance --test native_bidder_onboarding
cargo test --locked -p auctioneer-core --features conformance-suite,insecure-bidders --test native_bidder_onboarding
cargo test --locked -p auctioneer-adapter-axum --test cancellation --test wave_transport
cargo test --locked -p auctioneer-adapter-axum --features insecure-bidders --test privacy_wire
```

For each existing matrix pair (`cloudflare`, `wasm32-unknown-unknown`), (`fastly`, `wasm32-wasip1`), (`spin`, `wasm32-wasip2`), execute these commands with `adapter` and `target` set to that pair:

```sh
cargo check --locked -p auctioneer-runtime -p auctioneer-openrtb -p auctioneer-config-data -p "auctioneer-adapter-$adapter" --target "$target"
cargo clippy --locked -p "auctioneer-adapter-$adapter" -p auctioneer-exchange -p auctioneer-privacy --all-targets --features conformance-suite --target "$target" -- -D warnings
cargo clippy --locked -p auctioneer-bidder --all-targets --target "$target" -- -D warnings
cargo test --locked --no-run -p "auctioneer-adapter-$adapter" --features conformance-suite --test fanout_conformance --target "$target"
```

Run the pinned Fastly/Viceroy, Spin and Cloudflare packagers, size checks and explicitly ignored environmental controls from their current workflows unchanged. Execute actual delivery qualification only with dedicated authorized bindings and fixtures. Compilation/package success cannot substitute for executed lookup identity/cap/cancellation evidence or prove whole-isolate fit.

### Final gates

```sh
./scripts/check-raw-json-fixtures.sh
./scripts/check-raw-json-fixtures-fixtures.sh
./scripts/check-batch-rebase.sh
./scripts/check-roster.sh
./scripts/check-roster-fixtures.sh
./scripts/check-secure-usersync.sh
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo clippy --locked --workspace --all-targets --features insecure-bidders -- -D warnings
cargo clippy --locked -p auctioneer-core -p auctioneer-adapter-cloudflare -p auctioneer-adapter-fastly -p auctioneer-adapter-spin --all-targets --features conformance-suite -- -D warnings
cargo fmt --all -- --check
```

The workflows remain the authoritative complete matrix. Read current job results, ensure runners actually executed the steps, and reconcile any newer required check before reporting acceptance. Do not recreate their YAML as a Rust gate.

## Completion record

- [ ] All spec behavior is implemented through the one runtime/preparation path; current examples and callers agree on format 2.
- [ ] Resource package and added charges are independently reviewed/approved; lookup activation refuses unqualified contexts without changing workload/ceilings.
- [ ] Before/after test inventory and actual complete-suite output are recorded; no guard was weakened or silently skipped.
- [ ] Provider and Trusted Server prerequisites are either satisfied by actual evidence or remain explicit blockers, not declared complete by this plan.
- [ ] User-authorized publication, independent PR review and separate merge decision are recorded. No implementation, publication or merge has occurred merely because this plan exists.

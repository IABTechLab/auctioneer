# Current Resource Proof Ownership

Date: 2026-11-09. This is a source-linked index for [#33](https://github.com/IABTechLab/auctioneer/issues/33), [#50](https://github.com/IABTechLab/auctioneer/issues/50) and [#48](https://github.com/IABTechLab/auctioneer/issues/48), not another issue tracker or a target certificate.

## Identity And Status

Discovery and SDK observations below are bound to planning head `b7910d05245103b352e2cbec943aa9e7974b3eaf`, whose code base is `499f0e540e95e1c86b730c1030aabef28de01d0a`. The implementation is stacked on that head at AG's direction while independent planning review remains pending. That exception authorizes local implementation, not approval, publication, issue closure or merging. Storage changes described below belong to this implementation checkpoint; they are not a recertification of that baseline.

- SDK: `598f0a2a490eb83dd31c06c024a0cefa8e049628`.
- `Cargo.lock` SHA-256: `c752e0bdc584920f09b4974155aae669063ccdb24075deaf7ca07928a27a00fa`.
- Toolchain: Rust 1.98.1; native observations use `aarch64-apple-darwin`.
- Locked HTTP/storage dependencies: `http 1.4.2`, `bytes 1.12.1`, `cookie 0.18.2`.
- The fresh default-workspace baseline executed 245 result-bearing targets: 6,522 passed, zero failed, 20 ignored. This does not include the separately owned compiler/producer, alternative-feature, guest or hosted-execution matrix.

**Current acceptance:** #33's closure reconciliation and #50's fact/owner mapping are supplied here for review. #48 is partial: bounded application storage and terminal controls do not establish complete container/allocator/host-copy bounds. No isolate charge, workload limit, headroom, schema selection, provider fact or historical evidence changed. No target is newly declared ready.

## Actual Executable Closure

The sources of membership are [factory discovery](../../crates/auctioneer-config/src/bidder_discovery.rs), [publication generation](../../crates/auctioneer-config/src/codegen/mod.rs), the generated `discovery.rs`/`registry.rs` and [schema staging](../../crates/auctioneer-config/src/schema_staging_build.rs). The table records observations at the stated baseline, not a new hardcoded membership gate.

| Population | Native Secure / Development | Guest Secure / Development |
| --- | --- | --- |
| Compiled candidate factories | 16 / 17 | 16 / 17 |
| Published executable bindings, including public aliases | 17 / 18 | 3 / 4 |
| Effective parameter schemas behind those bindings | 15 / 16 | 2 / 3 |
| Inspection-selected schemas | 15 / 16 | 15 / 16 |
| Controlled-native serving reservation | Installed when funded | Not available |

`native_only` constrains publication, not candidate factory discovery. Runtime-disabled definitions remain in their compiled lane. A public alias shares its resolved implementation and effective schema; schema-free bindings do not acquire an invented schema. Inspection selection is not the guest-serving population.

The selected schema closure has four generated regex helpers and one shared hostname program, plus hostname and UUID format callbacks. Equal patterns in distinct generated owners are not shared allocations. Native admission installs tested reservations of 16 MiB resident, 8 MiB initialization and 512 KiB validation scratch per auction through [the existing schema owner](../../crates/auctioneer-bidder/src/schema/native.rs); these are not certified worst-case bounds.

Imported inventory is a different population: 358 identities, 261 bases, 97 aliases, 259 implementation references, 261 main schema artifacts and two separately witnessed schema artifacts. There are 262 bidder manifests, 245 without an executable factory. Full-source controls remain required, but an unused imported schema is not a serving prerequisite. Some staging/source controls still select pinned priority/supplemental projections; replacing that selection with complete discovery is remaining [#46](https://github.com/IABTechLab/auctioneer/issues/46)/[#47](https://github.com/IABTechLab/auctioneer/issues/47) work, not delivered by this index.

## Application Proof Owners

In this table `K` is admitted concurrent auctions, `N` is the per-auction call bound, `S` is active body streams, `A` is auxiliary route permits and `P = max(A, PROBE_CONFIG_MIN_PERMITS)` is independent probe-configuration work. Units and populations are separate: one retained result per completed call is not bounded just by currently active streams.

| Surface | Owner, Units And Lifetime | Already Implemented / Remaining Evidence | Responsible Role And Next Action |
| --- | --- | --- | --- |
| Schema **C** | Compiler/library program owners; retained heap and linked code/data bytes; initialization through instance/execution teardown. Independently compiled owners coexist. | Library-owned layouts and component models exist; complete selected-program composition/allocator certificates do not. [Qualification owner](../bidders/schema-qualification.md), [source conformance](../../crates/auctioneer-bidder/tests/schema_source_conformance.rs). | Schema/target maintainers, [#46](https://github.com/IABTechLab/auctioneer/issues/46): compose actual generated owners, source certificates and linked artifacts without substituting observations for bounds. |
| Schema **I** | Constructor temporary bytes and source work; begins before construction, ends after success/failure cleanup, overlaps previously retained C. | Constructor/cache/failure models exist; aggregate target heap, work and stack remain incomplete. [Construction controls](../../crates/auctioneer-bidder/tests/schema_nfa_construction/mod.rs). | Schema/target maintainers, #46: prove cold/changed/failed construction with all already-live owners. |
| Schema **V** | Mutable cache/scratch bytes, evaluation work and stack; one bundle per active validation, charged per auction; hostname's per-call cache can overlap the outer bundle. | Explicit cache ownership and conditional component certificates exist, not whole-target worst-case qualification. [Generated dispatch](../../crates/auctioneer-config/src/schema_staging_build.rs), [conformance owner](../../crates/auctioneer-bidder/tests/schema_source_conformance.rs). | Schema/target maintainers, [#47](https://github.com/IABTechLab/auctioneer/issues/47): finish selected paths/format callbacks on applicable ABI, including simultaneous caches. |
| Configuration | Raw/parsed/converted/merged/retained bytes and JSON values; fund before extraction; auction, A and P work can coexist. | Pre-extraction funding and captured-cap guards exist. [Runtime extraction](../../crates/auctioneer-core/src/config/runtime.rs), [funding controls](../../crates/auctioneer-core/tests/config_funding.rs). | Config/resource maintainers, [#147](https://github.com/IABTechLab/auctioneer/issues/147), coordinated by [#130](https://github.com/IABTechLab/auctioneer/issues/130): complete phase/guest ownership proof without adding another config path. |
| Sealed response DTO | Delivered bytes/nodes and retained candidate storage; admission through decode/publication/drop; completed candidates remain after streams close. | One-shot claims, rollback, candidate credits and decode caps exist. [Decode session](../../crates/auctioneer-bidder/src/response_decode/session.rs), [node controls](../../crates/auctioneer-bidder/tests/response_node_budget.rs). | Bidder/runtime maintainers, [#150](https://github.com/IABTechLab/auctioneer/issues/150): complete target allocation/work proof, including retained results. |
| Ordinary Serde audit | Lexical nodes versus visitor allocations, work and stack during request/config/sync decoding. | Structural limits and raw-capture refusal exist; they are not a complete audit of every visitor. [Bounded parser](../../crates/auctioneer-runtime/src/bounded_json.rs). | Decoder owners, [#148](https://github.com/IABTechLab/auctioneer/issues/148): audit reachable visitors and adversarial structures; no new exploit is asserted here. |
| Auxiliary storage | Raw/escaped body, headers, complete cookie and construction scratch bytes; admission through terminal egress, A live slots. | This checkpoint bounds sync output reservation, detaches final sync headers and caps the existing cookie formatter. Complete physical container/guest/provider-copy certificates remain absent. [Storage owners](../../crates/auctioneer-core/src/handlers/sync_http.rs), [cookie owner](../../crates/auctioneer-core/src/usersync/cookie.rs), [allocation owner](../../crates/auctioneer-core/tests/usersync_handler_alloc.rs). | Core/target maintainers, [#48](https://github.com/IABTechLab/auctioneer/issues/48): fulfill the residual proof below; keep conservative charges until then. |
| Auxiliary/probe lifetime | A route slots/work receipts end at completion or abandonment; P configuration slots end at resolution/render/publication; probe output/cache, static output and auction populations stay independent. | Existing [admission](../../crates/auctioneer-core/src/ingress.rs), [real SDK controls](../../crates/auctioneer-core/tests/ingress_admission.rs), [probe pending-read controls](../../crates/auctioneer-core/tests/probe_pending_reads.rs) and [registered-route join](../../crates/auctioneer-core/tests/aux_routes_wrapped.rs) implement these owners. | Core/target maintainers, #48: retain actual-terminal coverage, saturation/recovery and zero-A probe controls; do not acquire another permit or release at handler return. |
| GVL backing | Distinct parsed lists, not Arc handles; cache residency plus K/A requests pinning versions evicted from cache. | Separate shared backing and retained-version population are already charged in [the runtime](../../crates/auctioneer-runtime/src/budget.rs). | Privacy/target maintainers, [#9](https://github.com/IABTechLab/auctioneer/issues/9): preserve distinct-version/last-handle accounting during target proof. |
| Combined host artifact | Original funding, host bridge/engine/egress bytes, stack and actual linked artifact; settings-first preparation through retained result and terminal handoff. | The portable API/owned receipts exist; standalone Auctioneer acceptance is not Trusted Server acceptance. [Services](../../crates/auctioneer-core/src/auction/services.rs), [preparation](../../crates/auctioneer-core/src/auction/prepare.rs). | Integration owners, [#76](https://github.com/IABTechLab/auctioneer/issues/76) and [Trusted Server #1510](https://github.com/IABTechLab/trusted-server/issues/1510): execute real host-service handoff and combined-artifact proof. |

## SDK Correspondence

All SDK references in this section point into [the exact pinned tree](https://github.com/IABTechLab/edgezero/tree/598f0a2a490eb83dd31c06c024a0cefa8e049628/crates). Existing payload or Auctioneer metadata charges do not automatically prove container, channel, task or SDK bookkeeping coverage.

| SDK Owner | Funding Correspondence And Lifetime | Missing Proof / Owner / Action |
| --- | --- | --- |
| Prepared contexts, preflight outcomes, batch slots | `edgezero-core/src/outbound/{dispatch,batch}.rs`; per-wave context vectors coexist with requests. Request backing maps to K times N call-backed auction work. | [#50](https://github.com/IABTechLab/auctioneer/issues/50), SDK/adapter maintainers: certify vector/context/layout bytes and map each separately; `outbound_meta_bytes` is bidder metadata, not a generic SDK allowance. |
| Callback/body scopes and registrations | `edgezero-core/src/outbound/scope.rs`; base scopes exist outside auction-only admission. Callback revocation and accepted-body handoff have different terminal points; empty handles can outlive payloads. | #50 then [#72](https://github.com/IABTechLab/auctioneer/issues/72)-[#75](https://github.com/IABTechLab/auctioneer/issues/75): bind scope/registration populations to ingress, A, P, streams and egress rather than K alone. |
| Tasks, channels and cooperative queues | Adapter outbound scheduling: Axum/Cloudflare batch tasks request `slots + 1` channel capacity; Spin's unbounded-channel API has a finite event producer; Fastly uses cooperative drivers/queues. | #50, SDK/adapter maintainers: bound task/future/channel allocations and concurrent queues. Finite event count is not a physical channel-capacity certificate. |
| Completed queues, state vectors, boxed receipts | `edgezero-core/src/outbound/batch.rs`; Auctioneer [consumes each `next()` item](../../crates/auctioneer-core/src/outbound/wave.rs) immediately, but retains completed results across waves until publication/drop. Payload demand therefore includes K times N, not only S. | #50 then target owners: certify queue/vector/box overhead and overlap. Unused SDK accumulator/merge/collect APIs are not additional serving prerequisites. |
| Scope teardown | `edgezero-core/src/outbound/{scope,task}.rs`; root gathering, cleanup vectors and detached boxed payloads can coexist with live callbacks/body owners. | #50, SDK/target maintainers: map cleanup peak and surviving wrapper/waker storage to the applicable population. Correct cancellation alone is not a byte bound. |
| Fastly recorder/configuration-key set | `edgezero-adapter-fastly/src/context.rs`; callback owns recorder and distinct configuration keys; numeric observation handles can survive callback exit. | #50 / [#73](https://github.com/IABTechLab/auctioneer/issues/73): certify set capacity, insertion overlap and separate observation lifetime. Do not assign one release point to every owner. |
| Fastly backend cache | `edgezero-adapter-fastly/src/outbound.rs`; static `BACKENDS` retains names, identities and backend handles for execution/instance lifetime. Successful registration precedes a later deadline check. | #50 / #73, SDK/provider and Auctioneer target maintainers: prove reachable persistent identity count and host storage; callback teardown is not backend-cache eviction. |
| Codec/framing/egress | SDK `edgezero-core/src/compression.rs`; per-layer decoder/windows and wrappers scale with S. Auction egress, A/P work, terminal output and probe output/cache retain separate owners in [the runtime](../../crates/auctioneer-runtime/src/budget.rs). | [#49](https://github.com/IABTechLab/auctioneer/issues/49) coordinates source/ABI/allocator/backend certificates; #72-#75 fulfill target facts. An application-header cap does not include provider-added framing. |

### Adapter Facts And External Actions

These are source declarations, not live deployment observations. `K/N/S/A` names the configured workload, not a provider-enforced inbound population.

| Adapter | Published Facts And Existing Enforcement | Remaining Owner / Action |
| --- | --- | --- |
| Axum | Ceiling, inbound population and host-ingress scope are operator-configured unknowns. The SDK has a finite pre-parser connection gate; default 256, while Auctioneer boot requires no more than 384 connections. K/N/S/A = 2/16/2/1. | Operator + SDK/Auctioneer target owners, [#72](https://github.com/IABTechLab/auctioneer/issues/72): bind quota, memory scope, actual ingress settings and allocation correspondence. SDK parser allocation APIs exist, but their Rust 1.95 audit is not a Rust 1.98.1 certificate. |
| Cloudflare | 128,000,000 bytes per instance; inbound population and host-ingress accounting are unknown. Auctioneer charges population 6; K/N/S/A = 6/32/32/4. | SDK/provider + target owners, [#74](https://github.com/IABTechLab/auctioneer/issues/74): obtain inbound enforcement and JS/host/Wasm scope correspondence. Six charged slots are not proof of six actual inbound requests. |
| Fastly | 128,000,000 primary bytes, separate 1,000,000-byte stack, one inbound request per execution; ingress accounting unknown. K/N/S/A = 1/32/32/1. | [#73](https://github.com/IABTechLab/auctioneer/issues/73): SDK/provider supplies ingress contracts; Auctioneer target owners prove the application's required stack against the linked artifact. Include recorder/cache owners above. |
| Akamai Functions hosted Spin | 134,217,728-byte hosted default, one inbound request per execution; ingress accounting unknown. Hosted metadata reaches the actual entrypoint/dispatch. K/N/S/A = 1/32/32/1. | SDK/provider + target owners, [#75](https://github.com/IABTechLab/auctioneer/issues/75): bind authorized deployment provenance and physical/host correspondence. No current generic-versus-hosted dispatch mismatch is established. |

[The existing evaluator](../../crates/auctioneer-cli/src/bidders/resources_platform.rs) has nine controls: scope mismatch, unexpected separate stack, nonunit execution population, once-scaled demand, stack boundary/proof shape, configured overflow independent of external unknowns, unknown facts/population coverage, minimum of independent ceilings/headroom, and numeric excess preserving external blockers. A configured-ceiling overflow is Auctioneer demand failure even if the provider ceiling is unknown. Missing facts remain nonpassing. A nonempty stack-proof string is only a shape check, not authentication of an artifact certificate. [Dispatch/layout checks](../../crates/auctioneer-cli/src/bidders/resources/mod.rs) remain separate.

## Storage Checkpoint And Accounting Decision

The sync writer now counts through a nonallocating cap-aware sink, reserves once, writes through the existing `CappedWriter`, refuses second-pass growth and shrinkage, and includes its final newline. HTML escaping counts all five substitutions before reserving escaped output. Raw and escaped buffers coexist during transformation. It changes neither successful bytes nor the existing cap. Info/health retain their existing distinct `serialize_json_exact` contract.

Sync final headers are rebuilt only after their current limits pass. Final names/values own bounded bytes, duplicates and sensitivity are retained, and removed fields cannot leave a large final map behind. The old map, new map, response body and cookie can overlap during copying; no claim is made that this also bounds arbitrary caller-owned input maps before the builder is called. `allowed_origin` already copied the matching configured origin and is not replaced with an ingress slice. Provider-added headers remain outside these application limits.

Complete cookie formatting still uses `cookie::Cookie`'s formatter, now writing to a fallibly reserved capped field instead of a growable string. The original preflight, complete 4-KiB field limit, dates, attributes, signed payload and byte-reference tests remain. Cookie-library date-format scratch, field bytes and the copied `HeaderValue` overlap and must be included in any physical proof.

Existing output funding includes two body buffers plus header/cookie storage; the transform and header-rebuild phases differ. That receipt is not a proof of the locked header map's physical layout or of an allocator/host copy multiplier. Native requested-allocation measurements are controls, not guest physical maxima.

**No accounting cutover:** retain `(A + P) * (H * D + T)`, with D the unchanged declared auxiliary work peak, H the existing handoff factor and T transmit state. The candidate `A * (D + (H - 1) * R + T) + P * D` is not implemented. A complete R is unavailable: header-map indices/entries/duplicate storage and allocator slack, cookie scratch/conversion overlap, fallback/info/health/probe shapes, guest layouts and provider handoff/copy contracts are not jointly certified. Do not fill R from a wire length or maximum observed allocation, set it to zero, or reuse a native measurement as guest proof. No savings or new fit is claimed.

#48 therefore stays unmet until core/target owners supply the complete route/ABI/container/allocator/lifetime proof and applicable SDK/provider facts in #50/#72-#75. The registered-route coverage join, actual SDK completion/abandonment controls, independent A/P/probe/static populations and real bootstrap/CLI parity remain required. A missing proof assigned to someone else is still a missing proof, not #48 completion.

## Reproduction Owners

Use [development setup and test ownership](development.md), the pinned source checkout and fresh processes. These commands are owner entrypoints, not claims that every matrix job has run at this checkpoint:

```sh
cargo test --locked -p auctioneer-core --lib
cargo test --locked -p auctioneer-core --test serialize_allocation_bound --test usersync_handler_alloc
cargo test --locked -p auctioneer-core --test ingress_admission --test egress_binding --test aux_routes_wrapped --test aux_route_write_deadline --test aux_request_body_bounds
cargo test --locked -p auctioneer-core --test probe_output_before_boot --test probe_pending_reads --test probe_before_boot
cargo test --locked -p auctioneer-cli --bin auctioneer-cli bidders::resources_platform::
cargo test --locked -p auctioneer-cli --test bidders_resources --test resource_boot_parity
cargo run --locked -p auctioneer-cli -- bidders resources --check --all-adapters --capture-audit
```

The real bootstrap/strict parity owner is the subprocess target above, not two calls to a shared helper. Historical [adapter envelopes](adapter-resource-envelopes.md), schema checkpoints and qualification captures retain their original identities and bytes; they are not current target certificates. GitHub retains priorities, blockers and final acceptance. Refresh this index when source/SDK, selection or ownership changes rather than silently recertifying old artifacts.

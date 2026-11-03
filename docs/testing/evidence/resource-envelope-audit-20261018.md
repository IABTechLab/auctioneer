# Resource Envelope Audit Notes

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

Source baseline: `b3f08e464b34dd69151c1975342637fe378421d4`.
SDK: `f6c708ccb5be19ba9fa92575db3c08c0f17e0d72`; Rust 1.98.1.
Date: 2026-10-18.

This records the initial reporting/lifetime slice, followed by commit
`2a38d931`. Later typed-record and Git-scope implementation is recorded
separately in [checkpoint controls](resource-review-controls-20261018.md).
The original counts and limitations below describe that earlier slice.

This is a source/lifetime investigation for the
[resource remediation plan](../../superpowers/plans/2026-10-17-resource-envelope.md),
not an approved remediation package, the canonical typed audit, a WASM
measurement or a fit sign-off. The required `audit.toml`, frozen change record
and AG approval checkpoint remain outstanding. No workload or allocation
coefficient changes are authorized by these notes.

## Ownership

The eleven terms below are the runtime-owned `IsolateTerms` entries. Their
population is applied in `derive_isolate_budget_with_baked_config`; they must
not be multiplied again by an exporter or SDK adapter.

| Term                     | Acquisition and retention                                                                                                  | Release and enforcement                                                                                                                                      |
| ------------------------ | -------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `baseline`               | Compiled defaults, registry and singleton storage, including existing initialization reserves                              | Instance/execution teardown; no global allocator byte gate. See core `bidder_registry::build_registry` and target envelope declarations.                     |
| `gvl`                    | Cache and request-held parsed vendor lists                                                                                 | Last cache/request handle; eviction does not free a pinned version. See privacy `GvlStore::get`. Cache, auction and auxiliary populations are distinct.      |
| `auctions`               | All six phase receipts prepaid before config extraction; parsed config, request projections, calls and results may coexist | Core `auction_response` drops work receipts after serialization or on cancellation. `AdmissionGate` bounds auctions; phase receipts bound funded work.       |
| `auction_egress`         | Egress permit acquired before serialization and bound to response completion                                               | Terminal egress/cancellation, not auction-work completion. See `auction::egress_leg` and `ingress_lease`.                                                    |
| `auxiliary`              | Aux admission/work receipt and independent probe-config work                                                               | Aux completion versus probe resolution/render/publication. See `ingress::admit_aux` and `probe_config::claim_config_work`; counters are not interchangeable. |
| `terminal_egress`        | Admission reserves a terminal slot; refusals/detached head rejection use their own static path                             | Terminal completion/cancellation, enforced by `StaticEgressGate`.                                                                                            |
| `probe_output`           | Independent pre-boot-capable probe output slots and cached verdict                                                         | Output completion, with cache residency separate. See `ingress::admit_probe` and the status handler.                                                         |
| `pre_admission_parser`   | Host head parsing before Auctioneer admission                                                                                  | Host/parser cleanup. No Auctioneer route gate proves this inbound population. Cloudflare's nominal six is not a published inbound bound.                         |
| `body_streams`           | Global wave lease around dispatch/decode                                                                                   | Wave settlement/EOF/cancellation. Completed slots remain retained outside this lease. See `wave::drive_fanout`.                                              |
| `catalog_resident`       | New catalog residency only; the current incremental descriptor is zero                                                     | Teardown. Existing registry/config residency remains funded elsewhere.                                                                                       |
| `catalog_initialization` | New initialization scratch only; current increment is zero                                                                 | Initialization/error cleanup. Zero incremental charge is not guest allocation evidence.                                                                      |

## Dominant Costs

- **Configuration:** the SDK extractor's version-precheck tree and verified
  envelope are sequential. Raw envelope storage is dropped before secret
  resolution/typed conversion. Typed config, resolved defaults and privacy
  state can subsequently coexist. A smaller charge needs a replacement bound
  covering verification, errors and these retained clones; observing a drop
  alone is not that proof.
- **Projection:** `split::materialize_per_bidder` returns an owned vector;
  surviving projections remain alive across fanout in `run_auction`.
  Sequential construction scratch does not make retained projections sequential.
- **Response:** completed slots survive across waves. `process_results` parses
  sequentially while accepted bids accumulate. The 32-by-256-KiB raw-body
  allowance alone is 8,388,608 bytes and cannot be scaled by stream width.
  Separating transient parse scratch from retained bids is a candidate, not
  an established smaller universal bound.
- **Auxiliary handoff:** Cloudflare, Fastly and Spin have different guest/host
  transfer paths. None demonstrates that all privacy/config/node work is
  copied to the host. Applying the handoff multiplier to the whole auxiliary
  peak deserves investigation; removing it requires an owning bound and
  overlap proof.

Changing only auction concurrency cannot repair Fastly or hosted Spin: both
already declare one auction. Reducing stream width changes wave/deadline
behavior and is not an unchanged-workload accounting correction.

## Rejected Shortcut

A scratch arithmetic investigation retained all fixed privacy/debug/sync-body
charges, but reduced bidders/calls to two, inbound/raw outbound/encoded
outbound/transport/decoded byte caps to 4,096, request/response nodes to 128,
backend/blob config to 1,024, config nodes
to 128, copied headers to 256, final response to 65,536, stream width to one,
and Cloudflare auction/auxiliary permits to one. The auxiliary reserve was
then derived, not guessed. Even this severe reduction did not fit any WASM
target under the existing equation. It is not a proposed production default.

The archived-baseline scratch run was independently repeated with the real
runtime derivation. Native construction costs were 800 fixed, 2,488 scratch
and 78 retained bytes per scope, with 1,584 serialized baked-config bytes.
These are native arithmetic controls, not guest layouts or measured heaps:

| Target                  | Shipped demand  | Reduced-profile demand | Reduced-profile gap including policy headroom |
| ----------------------- | --------------- | ---------------------- | --------------------------------------------- |
| Cloudflare              | 138,048,496,272 | 256,996,696            | 136,996,696                                   |
| Fastly                  | 23,589,646,360  | 198,014,296            | 78,014,296                                    |
| Spin hosted declaration | 23,589,646,360  | 198,014,296            | 72,185,176                                    |

The current four-target resource command independently reproduces the shipped
native-control totals. The scratch source was an archived baseline, not an
edit to any production envelope. This exploratory calculation does not replace
the committed typed audit/change package or confer approval.

Such changes would reject larger requests/configs, omit bidder work, introduce
oversize failures and alter concurrency/deadline outcomes. The useful next
package is an independently proved ownership/accounting improvement, not an
unreviewed cap reduction. No savings from overlapping alternatives may be
added against the same original term.

## External Prerequisites

| Missing fact or proof                                         | Owner                                                               | Next verification action                                                                                                                                                     |
| ------------------------------------------------------------- | ------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Host ingress/framing memory accounting on all four targets    | EdgeZero adapter maintainers/provider                               | Publish scope-bound accounting or an enforced bound with negative controls; exercise the existing SDK validator against it.                                                  |
| Cloudflare inbound population                                 | EdgeZero Cloudflare maintainer/provider                             | Publish/enforce a per-instance ingress bound. Six outbound connections is not a substitute.                                                                                  |
| Fastly separate-stack requirement                             | Auctioneer Fastly maintainer                                            | Establish an artifact/ABI/toolchain-bound upper bound covering cold init, admitted depth, validation, errors and cancellation against the distinct 1,000,000-byte allowance. |
| Spin hosted envelope versus generic dispatch                  | AG selects deployment; Auctioneer Spin maintainer aligns dispatch/proof | Select the actual deployment contract. Do not relabel generic dispatch as hosted; generic selection needs configured facts.                                                  |
| Axum ceiling and inbound population                           | AG/operator and EdgeZero Axum maintainer                            | Supply actual operator declarations and expose their enforcement/provenance through the existing platform metadata.                                                          |
| Guest layouts, baseline, parser/transmit and host-copy bounds | Auctioneer owning/provider maintainers                                  | Bind evidence to the selected target artifact and ABI. Native layout/allocator controls do not substitute for WASM evidence.                                                 |

Known configured-cap overflow remains a Auctioneer demand failure even if SDK
facts are unknown. Conversely, a fitting configured cap does not establish
platform fit. Both numeric failure and external blockers can coexist.

## Newly Executed Controls

- Runtime's real crate compiles in a temporary workspace, then a new
  unclassified `IsolateTerms` field fails both `checked_total` and `entries`.
  Structured compiler diagnostics are anchored to the actual function spans.
  The active worktree is never mutated by this control.
- Core's pure profile target checks all eight independent zero/nonzero config
  cap combinations against literal SDK defaults, serializes the actual baked
  host/account pair independently, and asserts no process-wide gates install.
- Each of the four catalog costs is varied alone and together at 1/3/7 auction
  populations, distinguishing once-only residency/initialization from
  per-auction validation/context charges.

Those controls passed 1 and 2 tests respectively; runtime's 170 unit tests also
passed. Focused runtime/core clippy checks passed with `-D warnings`.
These counts are not a complete workspace or target-execution result.

## Resource Command

`cargo run --locked -p auctioneer-cli -- bidders resources --check` inspects the
four real adapter envelopes using the core input resolver and runtime's
checked derivation. It reports raw/effective caps, the actual baked tuple,
all eleven named terms, six receipts, source/profile identities and linked
development features. Axum's binary and the inspector share its library
envelope and feature detection; the existing schema-2 wire export is unchanged.

The CLI build script captures the actual Rust compiler's verbose identity
and compilation target. A shared Git source snapshot detects changed, added,
deleted and moved inputs, including config generation data and owning layouts.
Non-Rust documentation and the resource qualification evidence directory are excluded to avoid
self-hashing captures; Rust remains source in either location. Source hashes
are streamed and only a digest/count is repeated in target rows, not file
contents or four complete source manifests. This is build-input provenance,
not deployment attestation or authentication.

The SDK adapter maps already-scaled total plus available selected headroom
once, without another request multiplier. Unknown ceilings leave effective
floor/gap pending, independently of any configured-cap overflow. External
unknowns remain visible even when SDK `Exceeds` short-circuits its own reason
collection. SDK validation errors retain typed nonpassing `Invalid` results;
Fastly's missing stack proof and Spin's dispatch mismatch cannot pass.

`--check` currently checks source/accounting completeness only. Matched catalog
baselines, component allowances, typed audit/change parsing and review-scope
checks are still unimplemented. `--check --require-fit` refuses the current
four-target result. Neither command claims deployment qualification.

Fresh complete CLI unit verification passed **544 tests, zero failures**, with
two existing child-process probe entries ignored by the top-level runner and
invoked by their parent tests. The earlier overlapping run had 532 passes,
five `fixture_changed` failures and two ignored entries: another focused build
replaced the executable while provider tests were hashing `current_exe()`.
The final run serialized compilation and execution with source/binary frozen;
no retry, weakened guard or ignore was added to the tests.

The resource tests separately exercise SDK invalid scopes/populations/stack
shapes, exact decimal stack boundaries, unequal ceiling/population cases,
all sixteen synthetic fit masks, positive-but-wrong baked sizes and isolated
source freshness changes. These synthetic all-known facts are adapter tests,
not published provider facts or live evidence.

## Final Verification

| Executed target/check                                                     | Result                                                        |
| ------------------------------------------------------------------------- | ------------------------------------------------------------- |
| Complete `auctioneer-cli --bin auctioneer-cli --no-fail-fast` unit target         | 544 passed, 0 failed, 2 existing child entrypoints ignored    |
| CLI `bidders_resources`, `help`, `qualification_cli` integration targets  | 4 / 4 / 15 passed                                             |
| Runtime unit target                                                       | 170 passed                                                    |
| Runtime `term_exhaustiveness` and core `catalog_resource_profile` targets | 1 / 2 passed                                                  |
| Axum library and native `resource_profile` integration targets            | 21 / 35 passed                                                |
| Actual `target/debug/auctioneer-cli bidders resources --check`                | Exit 0, accounting/source completeness only                   |
| Same command with `--require-fit`                                         | Exit 1, identical JSON, no qualified target                   |
| Strict all-target clippy for runtime, core, CLI and Axum                  | Passed with `-D warnings`                                     |
| Runtime `wasm32-unknown-unknown` library check                            | Passed; compilation only, not guest execution or memory proof |
| Formatting, diff whitespace and raw-JSON fixture gate                     | Passed; 169 fixtures across 50 files                          |

The 16 focused resource unit cases are included in the complete CLI count,
not additional coverage to add to it. Independent spec and quality reviews
approved only this bounded slice. Existing GVL deletion warnings and the
dependency future-incompatibility notice remain. No full-workspace, hosted
execution or absolute WASM-fit result is claimed.

## Projection Feasibility Follow-Up

Read-only investigation at code commit `3534d501` found no justified projection
savings to freeze into a package yet. This is a rejected shortcut, not approval
of an ownership or workload change.

`split::materialize_per_bidder` returns all owned survivor requests, and
`run_auction` retains them through construction, fanout and response decoding.
It drops the grouped shared JSON tree before scrubbing. Naively interleaving
materialization, scrubbing and adapter construction would extend that tree's
lifetime and alter the current fatal-FPD, deadline and audit ordering. No
streaming counter may replace the survivor population without preserving those
barriers and proving the new overlap.

Earlier release after construction is a narrower possible ownership change,
but does not eliminate the existing initial all-survivor peak. Moreover,
`Bidder::make_bids` explicitly receives the full post-FPD, privacy-scrubbed
request. Replacing it with IDs alone changes its contract. A compact response
and participation context needs its own typed boundary, independent size bound,
and adapter/media/correlation proofs before a later package can be proposed.

The existing construction descriptor is not a response-context allowance.
Completed response bodies still survive across waves; stream width does not
bound them. The current WASM response receipt alone is 4,037,678,240 bytes per
auction in the native-control accounting report. No earlier-release observation
alone reduces any term in the present additive equation.

The next design/proof decision is how to preserve all-survivor validation and
privacy/deadline ordering while bounding retained participation/response state
separately from transient projection work. Required controls include late
FPD/scrub failure, original-input immutability, aliases/order/cap refunds,
post-FPD media classification, cancellation, audit counts and overlapping
allocation peaks. A new descriptor belongs before the audited code-base freeze;
actual ownership changes and any accounting credit still need the concrete
package and AG approval. Current caps, charges and recorded gaps remain intact.

## Auxiliary Handoff Feasibility Follow-Up

Read-only inspection of baseline `5cc6d986` and SDK `f6c708cc` found a useful
ownership distinction, but not a proved smaller all-route bound. Auxiliary work
includes config, input parsing, signing, privacy, chooser state and retained
URLs. Provider handoff transfers response data, not that entire work graph.
Route completion owns auxiliary work through terminal egress; probe-config work
ends after rendering/publication and has independent probe-output ownership.

Taking credit from that distinction now would miss preboot extraction.
`configure::extraction_limits_for(None)` uses SDK defaults of 16,777,216 backend
bytes and 8,388,608 blob bytes. Limits are captured when App is built; installing
boot gates does not retroactively change an existing App. At 1,584 baked bytes,
the existing config-charge rule alone gives **3,238,205,440 bytes**, before other
work. None of the shipped auxiliary work slots covers this hypothetical
preboot/App-across-boot context individually. This is a missing accounting proof,
not an observed overrun or a finding that a production entrypoint builds App
before bootstrap.

Let `N = aux + max(aux, probe_min)`, `W = aux_route_peak_bytes`, `H = handoff`,
`T = transmit_state_bytes`, and `R` be a future proved all-route response-handoff
bound. The current term is `N * (H*W + T)`. A conservative candidate would be
`N * (W + H*R + T)`, retaining both work populations, the complete handoff factor
and transmit state. It is **not justified yet**: both once-only `W` and `R` need
owning proofs, including info/health output, sync cookies/headers, framework
fallbacks, config/secret failure, cancellation, recovery and cached verdicts.
Sync's own output ceiling is not a universal auxiliary ceiling.

Core/config/auxiliary maintainers own this prerequisite. The next concrete
partial package must first cover preboot and retained-App extraction without
narrowing accepted inputs, then prove all-route output/handoff storage on the
target ABI. SDK ingress, stack and host-retention facts remain separate. No
post-total, saved byte or reduced residual gap is claimed by this investigation.

AG selected **Akamai Functions hosted Spin** in an explicit user reply on
2026-10-18. The adapter-owned profile refactor preserves the currently generic
dispatch facts and reports the mismatch; deployment selection is not proof of
dispatch alignment. The future hosted dispatch/proof patch must use the selected
contract and must not treat metadata selection as a memory measurement.

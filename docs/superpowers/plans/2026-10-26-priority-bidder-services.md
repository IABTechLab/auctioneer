# Priority Bidder Services Implementation Plan

> **For agentic workers:** Use superpowers:subagent-driven-development and test-driven-development. Keep Cargo verification serialized in the existing M7 worktree. Units stay inline; cross-module controls use synthetic bidders.

**Goal:** Complete the shared floor-conversion and coherent-credential prerequisites needed by the remaining priority bidders, without bringing the full M9 currency product into M7.

**Architecture:** Borrow operator-configured rates from the existing validated configuration snapshot. The exchange requests a bounded credential bundle only for an admitted bidder that declares the need; core supplies the existing EdgeZero secret-store implementation. Bidder transforms remain synchronous and provider-independent.

**Tech Stack:** Existing Rust workspace, Serde, validator, bounded JSON, capped outbound metadata, existing async runtime interfaces and EdgeZero bounded secret reads. Reuse the locked base64 package for Basic authentication rather than implementing an encoder.

## Approved Boundary

AG approved these scoped prerequisites on 2026-10-26. All 14 priority flows
remain M7 requirements; moving other catalog migrations to M20 does not remove
Sharethrough, Rubicon, schema/resource qualification, or composed acceptance.

- Rates are explicit USD-per-unit values, not inferred market data. No external
  rate fetcher, bid-response currency conversion, or price relabeling is added.
  Those remain M9 work. Existing non-USD response rejection remains unchanged.
- A rate snapshot has an explicit expiry and bounded, strict currency-code/rate
  entries. Lookups borrow the snapshot, allocate nothing, and reject missing,
  stale, non-finite, overflowing or positive-underflowing conversions. USD
  identity conversion does not require a foreign-rate table.
- A credential reference is resolved from validated, implementation-bound
  settings, never from request parameters. One versioned bundle contains
  username, password and tracker. Rotation never mixes fields from two reads.
  No global secret cache or new config/signing path is introduced.
- The bidder-owned declaration uses the closed `optional_secret_reference`
  settings kind, with the existing borrowed string view and field-wise precedence.
  Config admission rejects present empty, over-128-byte or ASCII-control-bearing
  references using fixed text. Omitted/null remain absent; no implicit trimming.
- Privacy, schema/parameter admission, original auction deadline and available
  call capacity precede any secret read. A read completing late cannot create
  calls. Missing or unsupported storage fails only the affected bidder with a
  fixed diagnostic. Cancellation drops the provider future.
- Credential work is charged explicitly once per auction's sequential
  construction scratch, not once per catalog identity or outbound call. Existing
  workload limits, target ceilings, and the additive budget equation stay intact.
  Conservative demand can increase for real added work; missing fit is reported,
  never hidden by reducing workload or raising ceilings.

## Task 1: Finish The Previous Verification Batch

**Files:** `docs/superpowers/plans/2026-10-15-rust-qualification-harnesses.md`, `.prettierignore`, `ISSUES.md`.

- [x] Resume CLI targets that the earlier evidence failure prevented running.
- [x] Restore the additional historical ownership table without changing its guard.
- [x] Verify harness, ownership, reference and boot-parity targets: 8/5/2/5 passed.
- [x] Publish this small restoration separately from serving changes (`4b8d4f1a`).

The subsequently exercised roster gate also needed its historical M3b plan
restored/excluded. Its baseline and all planted rejection controls passed;
the repair shipped with the scope correction in `b49f2252`.

The preserved result is not a new all-target CLI run, current deployment
qualification, or completion of any priority flow.

## Task 2: Bounded Floor-Conversion Value And Configuration

**Files:** new `crates/auctioneer-config/src/currency.rs`, new `crates/auctioneer-bidder/src/currency.rs`; module exports; `crates/auctioneer-core/src/config/mod.rs`; `crates/auctioneer-bidder/src/request_info.rs`.

- [x] Write inline tests before behavior; record semantic failures against the
  default-empty implementation. Independently vary amount, source currency,
  configured rate, expiry and table membership; do not hold decision inputs equal.
- [x] Use strict typed configuration and existing validation/error plumbing.
  Bound the table to 256 entries, three uppercase ASCII code bytes, finite
  positive rates and an explicit expiry. Reject a USD override, invalid or
  duplicate known fields, and unknown fields. Global config byte/node caps apply.
- [x] Implement a borrowed conversion view with an empty default. Normalize a
  currency code in fixed stack storage; no copied map, generated rate, fallback
  multiplier or per-lookup allocation. Reject positive underflow to zero.
- [x] Run `cargo test --locked -p auctioneer-config --lib currency` and
  `cargo test --locked -p auctioneer-bidder --lib currency`; require nonzero dispatch
  and independent boundary assertions before committing.

## Task 3: Portable Credential Bundle And Read Contract

**Files:** new `crates/auctioneer-runtime/src/bidder_credentials.rs`, runtime exports;
existing workspace-managed dependencies if required for Basic encoding.

- [x] Write failing parser and redaction tests first. Define fixed errors and a
  redacting Debug implementation, not a derived Debug over credentials.
- [x] Admit at most 4096 source bytes and a small explicit node count before
  typed deserialization. Require schema version 1; reject unknown/duplicate
  fields, missing/empty username or password, colon in username, and overlong
  components. Bound username/password to 256 bytes each and tracker to 128.
- [x] Bound credential references and captured physical store names to 128 UTF-8
  bytes. Inspect the borrowed default binding before cloning its name; a missing
  or oversized binding remains a bidder-local fixed refusal, not a legacy-store
  fallback or a reason to poll before admission.
- [x] Produce Basic authorization through the existing base64 library within
  bounded storage. Do not publish the password or a serializable secret DTO.
- [x] Define an object-safe request-bound reader using the existing portable
  monotonic instant and async conventions. The reader accepts only the admitted
  reference and original absolute deadline, returning one owned validated bundle.
- [x] Test exact/over byte, node and field bounds; valid rotations; duplicate
  and malformed JSON; Debug/error output with canaries in every credential slot.
  Normal JSON uses `serde_json::json!`; only lexical controls use raw literals.
- [x] Independently measure complete cold parse/encoding peak and retained
  ownership, including rejected inputs, before accepting a scratch charge.

## Task 4: Shared Admission, Lifetime And Resource Wiring

**Files:** `crates/auctioneer-bidder/src/bidder.rs`, `request_info.rs`;
`crates/auctioneer-exchange/src/context.rs`, `run/mod.rs`;
`crates/auctioneer-core/src/handlers/auction.rs`, new `handlers/bidder_credentials.rs`;
resource-construction descriptor owners and their existing tests.

- [x] Add the default-no-credentials bidder capability and a borrowed credentials
  view; implement no bidder-name switch. Keep ordinary callers' defaults inert.
- [x] Write synthetic-bidder integration controls proving zero store polls for
  denied/parameter-invalid/deadline-expired/call-exhausted work. Independently test
  each gate, not combinations that mask one another.
- [x] Capture the request-bound secret handle before consuming RequestContext,
  but poll only in the exchange's admitted construction stage. Bound backend and
  accepted bytes with `get_bytes_bounded`, use the same instant domain, and
  recheck the original deadline after await.
- [x] Retain only one sequential credential bundle, finish synchronous capped
  construction, then drop it before the next bidder. Authorization headers remain
  in already-funded metadata; secrets never enter debug output or diagnostics.
- [x] Move the floor snapshot out before `RuntimeBidderSettings::from(app_cfg)`
  consumes the config, then borrow it through context into request construction.
- [x] Add the independently bounded logical credential scratch to the existing owner
  descriptor and reservation check. Pin its multiplicity with independent
  constants/measurements and boot-versus-strict equality; do not borrow unused
  privacy/config funds or rewrite old evidence to make a new artifact fit.
- [x] Verify current native and all three provider compilation/resource rows,
  preserving R0's explicit absolute readiness blockers.

These service controls exercise the bidder's pure parameter preflight, not the
unimplemented generated-schema survivor boundary. Parent Task 12 remains a
prerequisite for promotion and composed priority acceptance: its real schema
evaluation must precede credential lookup, with an independently invalid
parameter/schema control. A synthetic semantic refusal cannot discharge that gate.

## Task 5: Finish The Two Remaining Protocol Implementations

**Files:** bidder-owned `sharethrough/` and `rubicon/` only, plus shared sealed
decoder helpers only where a real protocol branch requires them.

- [x] Sharethrough: observed red/green units, untouched source captures and
  parameter literals; media splitting, cumulative block lists, source/version,
  bidder-fatal floor conversion rollback, exact sent-media correlation, status
  and mixed-bid behavior. Version is the compiled Auctioneer package version.
- [x] Rubicon protocol candidate: coupled credentials;
  request transforms, media selection/splits, source-specific conversion scope,
  bounded native/proprietary response projection, actual call correlation,
  metadata and price overrides. No request-body replay or raw response escape.
- [x] Complete the priority wave's `request_preflight` operation under AG's
  2026-10-27 canonical-ingress/consumed-field decision, now recorded in master
  section 8.1 and the same 113-operation priority manifest. Shared native decoding
  and legacy privacy rejection stay at ingress. Bidder-wide checks cover every
  consumed supply-chain location, legacy EIDs, DSA, first-entry multibid maxbids,
  rewarded flags, GPID and slot data, including shadowed or otherwise filtered
  inputs. Keep parameter, rp.target, media and floor failures impression-local.
  Validate borrowed values without another request representation; do not decode
  unused source-framework routing/storage/analytics fields inside the bidder.
  The earlier rewarded-only checkpoint did not satisfy this operation.
- [x] AG approved explicit-region, opt-in routing to the four source HTTPS
  destinations. Generated host settings accept exactly `us-east`, `us-west`,
  `eu` and `apac`; missing, empty or unknown settings refuse before credentials
  or sink work. Requested settings shadow the base, including invalid values.
  Routing and no-work controls pass; the owning manifest records the exact
  complete-host policy. No default region, user-geo routing or failover is added.
  Public definitions remain empty. Prospective activation stays disabled by
  default, and live provider authorization remains M7.6 work; a synthetic
  endpoint test is not that authorization or composed App acceptance.
- [x] For each: constructor and response allocation controls, immutable provenance,
  secure/feature builds and source review. Candidate code is not promotion.
- [x] Run affected shared controls with `fake_` bidders. Real protocol conformance
  stays under each bidder's own `tests/`, units inline, assets adjacent.

## Task 6: Review And Publish The Complete Priority Checkpoint

**Files:** existing priority wave/producer owners, selected handler acceptance,
schema/resource qualification records, `ISSUES.md` and the main M7 plan.

- [ ] Complete required schema/work/cache qualification and funded serving joins;
  a passing protocol unit cannot waive them.
- [ ] Drive all 14 selected identities through actual generated handlers with
  nonempty expected calls/bids, post-scrub wire checks, sibling isolation,
  runtime activation and bounded signed sync where supported.
- [ ] Bind fresh evidence to current code/config/schema/SDK/target identities.
  Keep priority completion separate from M20's full 358-source closure and from
  R0 fit, M7.5 embedding and M7.6 live acceptance.
- [ ] Freeze the publication tree, run secure and feature tests, strict Clippy,
  formatting/raw-fixture controls and matched resource checks, then obtain
  independent spec and quality reviews before commit/push.

Do not mark this plan or M7 complete because the roadmap moved the long tail.

## Verified Service Checkpoint: 2026-10-27

All implementation workers were frozen before the publication checks. Source
captures were not regenerated: only locally authored factory-manifest witnesses
changed with the adjacent TOML's candidate/boundary notes. Independent reviews
covered floor conversion, credential ownership/accounting, shared admission and
both protocols. Rubicon's four verified review defects were reproduced by five
failing tests before repair; its non-GAM adslot follow-up was also observed red.
Canonical shared DTO boundaries remain explicit, not new compatibility parsers.

| Scope | Result |
| --- | --- |
| Default bidder/config/exchange/core/runtime library suites | 1075/65/532/277/235 passed |
| Same suites with `insecure-bidders` | 1086/65/532/277/235 passed |
| All nine source-dependent library controls, explicitly run with both source-repository bindings | 9 passed; no foreign source execution |
| Bidder construction allocation / sealed responses / shared fake allocation | 67/32/8 passed |
| Exchange services / config overlay / construction bounds / construction deadline / outbound construction / provenance / response charges | 9/31/17/11/10/6/17 passed |
| Actual HTTP handler conformance, including operator-rate handoff | 4 passed |
| Config codegen / compiled secret-reference consumer | 187/1 passed |
| Core boot charges / boot-strict parity / config funding | 1/1/1 passed |
| Adapter envelope suites, compiled natively | Axum 23; Cloudflare 15; Fastly 14; Spin 16 passed |
| CLI historical-evidence integrity / actual boot-versus-CLI parity | 5/5 passed |
| Real Cloudflare, Fastly and Spin WASM target checks | All three compiled; no deployment execution claimed |
| Five changed shared crates' library/test Clippy, default and insecure builds; feature-gated HTTP conformance lint | Passed under `-D warnings` |
| Formatting / whitespace / raw-JSON gate | Passed; 189 budgeted lexical fixtures across 55 files |

The config codegen suite initially hit a sandbox denial binding its nonregular
Unix-socket fixture. It passed unchanged with that permission; no test was
ignored or weakened. Existing GVL deleted-vendor notices and the tracked
`proc-macro-error2` future-incompatibility notice remain separate from Rust
warning-denying Clippy results.

Two additional controls were mutation-proven. Substituting scrub-only runtime
costs killed the zero-poll funding assertion. Replacing core's operator-rate
handoff with the default view killed the HTTP test with `MissingSnapshot` versus
the literal expected conversion. Both production paths were restored before the
passing runs. Amounts and rates vary independently through the HTTP handler;
request/response currency is not converted or relabeled by this prerequisite.

The 128-KiB bank is paid once per admitted auction: two native auctions add
262,144 bytes to current demand. Isolated scrub/clone allocation controls retain
their smaller component-only limits rather than gaining credential slack.
Logical bounds, source provenance and these compilation/model tests are not
physical provider qualification or newly measured deployment evidence.

Task 6 remains open. Generated-schema admission, qualified schema work/cache and
catalog descriptors, regional/public-definition promotion, all 14 composed
priority flows and their current executable evidence still belong to M7. R0
absolute/provider readiness, M7.5 embedding and M7.6 live delivery remain distinct.

## Funded Handler And Rewarded-Signal Follow-Up

The existing conformance entry point now accepts the same six prepaid phase
receipts that `auction_inner` consumes in production. This changes only the
feature-gated conformance API, not serving dispatch, the additive equation or
any cap. Tests derive the complete construction descriptor using an explicitly
synthetic resource row, keep all receipts live through response construction,
and verify their release. They do not install process-global boot state.

The handler controls use the canonical fake and the request's actual default
secret registry. They exercise coherent rotations for alias/base reads of the
same key, distinct keys on independent requested identities, missing/malformed
bundles, privacy refusal with an allowed sibling, unrequested definitions, and
unfunded zero-read refusal. A shared request-monotonic source drives the handler
and SDK loader. These are in-process paid handler controls, not actual generated
App/config extraction, a provider deployment or a schema-admission proof.

The priority audit also found that malformed rewarded flags could select a
credential key and survive as ordinary inventory. Three semantic regressions
failed before repair. Rubicon now validates both standard and legacy signed-byte
values over every impression before selecting credentials or constructing calls,
including shadowed legacy values and both impression orders. One equivalent
legacy spelling is accepted; competing spellings, including equal/null pairs,
are refused. Valid legacy signed-byte values survive non-video projection;
rewarded video remains exactly flag 1. Fixed errors contain no input values.
At this rewarded-only checkpoint, the wider `request_preflight` operation
remained open; the consumed-field follow-up below closes its approved scope.

Follow-up verification against `c15a1556`, Rust 1.98.1, serialized
`CARGO_INCREMENTAL=0`, locked/offline two-job Cargo commands:

| Scope | Result |
| --- | --- |
| Default bidder/config/exchange/core/runtime libraries | 1080/65/532/277/235 passed |
| Same libraries with `insecure-bidders` | 1091/65/532/277/235 passed |
| Nine source-dependent controls explicitly driven with source bindings | 9 passed |
| Constructor allocation / sealed responses | 67/32 passed |
| Funded HTTP conformance, default and insecure feature lanes | 8/8 passed |
| Bidder/core library and test Clippy, including conformance in both feature lanes | Passed under `-D warnings` |
| Cloudflare/Fastly/Spin actual guest library checks | All three passed |
| Workspace and discovered bidder formatting, whitespace, both raw-fixture gates | Passed; 189 lexical fixtures across 55 files |

Dropping conformance receipt forwarding independently fails the positive secret
read assertion. Moving rewarded validation after construction independently
fails its transaction counter with two versus zero, despite refunded permits
and discarded calls. Both mutations were restored; the restored bidder library
and full HTTP target pass again. The other four library suites remain unchanged.
Source/spec and separate quality reviews found no blocking scoped issue; their
two requested assertion improvements were added and executed. Initial test-only
lint failures were corrected without allowances or weaker assertions.

Logs use `/tmp/auctioneer-services-followup-*`, with the two mutation controls in
`/tmp/auctioneer-http-receipt-mutation.log` and
`/tmp/auctioneer-rewarded-late-preflight-mutation.log`. No source capture, SDK pin,
workload cap, resource ceiling, production dispatch or qualification artifact
changed. Guest checks are compilation, not release-package, stack/RSS or hosted
execution evidence. Existing GVL notices and the dependency future-incompatibility
warning remain. This is not a whole-workspace test run or M7 closure.

## Consumed-Field Admission Follow-Up

AG's 2026-10-27 canonical-ingress decision closes Task 5's scoped
`request_preflight` operation, not complete reference-framework decoder
equivalence. Rubicon now checks the consumed raw fragments before selecting a
credential key or starting any sink transaction. These checks borrow the already
admitted tree; there is no parallel request DTO, clone/reparse, new dependency or
schema engine. Shared typed ingress stays authoritative. Impression-local
parameter, target, media and floor failures continue to preserve usable siblings.

Sixteen additional inline tests cover independent scalar types/widths, nulls,
case ambiguity, every shadowed chain location, legacy EIDs, DSA, first-entry
multibid, GPID and slot/adserver data. Negative controls require fixed code 2,
zero transactions, unchanged input and unspent permits with and without supplied
credentials. Positive controls exercise real outbound projection and retain
unconsumed opaque data. The initial slot and root negative controls were
observed failing before the corresponding guards were implemented.

Verification against `1ee2553c`, Rust 1.98.1, using the existing target directory
and serialized locked/offline two-job builds with `CARGO_INCREMENTAL=0`:

| Scope | Result |
| --- | --- |
| Default bidder/config/exchange/core/runtime libraries | 1096/65/532/277/235 passed |
| Same libraries with `insecure-bidders` | 1107/65/532/277/235 passed |
| Eight main-reference source controls plus APS's supplemental source control, explicitly driven | 8/1 passed |
| Constructor allocation / sealed responses | 67/32 passed |
| CLI wave parser / actual wave-command integration | 27/9 passed |
| Bidder library/test Clippy, default and insecure; CLI binary/wave integration lint | Passed under `-D warnings` |
| Cloudflare/Fastly/Spin actual guest library checks | All three passed |
| Workspace/discovered bidder formatting, whitespace and both raw-fixture gates | Passed; 189 lexical fixtures across 55 files |

The wave integration first rejected a stale local contract digest, then passed
after its witness was updated to the approved admission wording. Both CLI
readers pin the same amended local manifest: 13 source identities and 113
operations, still review-pending and zero promoted. The bidder manifest's local
factory/conformance witnesses were updated with its reviewed boundary notes.
External protocol captures, schemas, source witnesses, dependency pins, resource
limits and historical qualification artifacts are unchanged. Independent spec,
code-quality and final provenance reviews found no blocking scoped finding.

Logs are `/tmp/auctioneer-preflight-*`; earlier slot red/green logs are
`/tmp/auctioneer-consumed-extension-{red,green}.log`. Existing GVL notices and the
tracked dependency future-incompatibility warning remain. Guest checks are
compilation, not release-package, physical-memory or live-provider evidence.
Generated-schema survivor admission, complete schema/work/cache qualification,
regional/public-definition promotion, all 14 composed flows and Task 6 remain
open. No regex-engine change or M7 completion claim follows from this repair.

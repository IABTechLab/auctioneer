# Priority Bidder Parallel Execution Plan

> **For agentic workers:** Use subagent-driven-development and test-driven-development. The coordinator owns integration and publication; workers have disjoint bidder-owned write scopes.

**Goal:** Finish M7's full selected-flow acceptance for all 14 priority bidders,
including Sharethrough/Rubicon, APS and the existing PubMatic audit/repair,
without duplicating shared infrastructure or confusing protocol candidates
with accepted publisher delivery.

**Architecture:** One shared-capability owner and two bidder owners run concurrently
where dependencies permit, in the existing M7 worktree. Bidder owners each own
one `crates/auctioneer-bidder/src/bidders/<key>/` folder; the shared owner has an
explicit disjoint module scope. The coordinator owns contract integration,
documentation, serialized Cargo runs, independent reviews, commits and pushes. Automatic
adjacent discovery remains the only production registration mechanism.

**Tech Stack:** Existing Rust, borrowed Serde serializers, capped request sink,
sealed response API, shared media classifier and immutable source captures.
No additional workspace, build-target copy, crate, dependency or feature.

## Approved Scope

AG approved this parallel execution model on 2026-10-23. It changes scheduling,
not the [priority operation contract](../../bidders/waves/priority.toml) or its
[implementation and acceptance gates](2026-10-22-priority-display-bidders.md).
Specific unresolved protocol decisions still require review. Keep public
definitions empty for unpromoted candidates, preserve workload/resource limits,
and retain schema/resource, shared context/decoder and hosted-delivery gaps.

### 2026-10-26 Scope Correction

AG approved M7 = foundations plus full 14-priority selected-flow acceptance,
not full-catalog migration. Sharethrough and Rubicon remain required M7 work,
including their bounded FX, credential, decoder and resource prerequisites;
their source audits alone are not delivery. APS keeps its independent reviewed
contract/evidence. No required priority-local branch, schema/resource gate or
composed onboarding obligation is removed, weakened or transferred to M20.

M20 retains the parent plan's Tasks 20/21 terminal full-catalog gates;
M7 closure cannot wait for them. The 2026-10-27 amendment brings post-pilot
bidder expansion forward into M7.7, with residual migration under M20. Preserve the
358-source inventory, 261 bases/schemas/bindings, 97 aliases and 259 source
implementations. Imported metadata is not implementation. Default/normal
`bidders check`, wave checks and full `--require-complete` semantics stay
unchanged; full completion still refuses unfinished source rows/branches.
M7 closes only at the parent plan's complete Task 19 foundation/priority gate,
not a partial selected-flow check or protocol-candidate publication.

M7.7 follows selected M7.5/M7.6 Trusted Server/publisher testing, before M8
onward. Freeze a finite demand-led queue with reviewed source requirements and
reuse the parent's Task 20 wave procedure. Its full-catalog terminal gate stays
M20-only. Exact membership requires demonstrated requests/adoption, not guessed
rankings; see the [roadmap](../../roadmap.md#2026-10-27-post-pilot-bidder-expansion).
M7's 14-priority scope, M7.5/M7.6 and M8-M19 requirements, and R0 readiness
boundaries are unchanged. No selected wave can bypass schema/resource,
promotion, composed acceptance or its own rollout requirements.
Dated execution records below remain historical evidence, not acceptance of
this corrected scope; this documentation change claims no completion.

AG clarified on 2026-10-24 that OpenRTB is the baseline, with reviewed bidder
extensions, not exhaustive emulation of legacy source-language quirks. This
supersedes the earlier optional float-parser dependency approval: Sovrn keeps
its existing numeric/decimal-string extension handling and intentionally ignores
hexadecimal/underscore string overrides. No extra parser dependency is needed.
Decoded invalid UTF-8 is an approved fixed bid-local refusal that preserves valid
siblings. These decisions close those compatibility questions, not selected-flow
acceptance, and do not reduce malformed-input safety or resource-bound coverage.

The immutable reference remains
`d7f8515b86258688304b0d9b6668c6a0e258bc9e`. Read its Git objects; never execute
foreign source programs or derive expected protocol output with Auctioneer.
Follow [bidder rules](../../../crates/auctioneer-bidder/src/bidders/README.md):
inline units, optional separate conformance, adjacent metadata/schema/fixtures
and TOML provenance. No central production bidder-name list, copied test
framework, unnecessary `expect`, lint waiver or duplicate license.

## Ownership And Queue

Workers may inspect shared contracts but only edit their assigned write scope.
They do not run Cargo, commit, push or adjust budgets.
All Cargo commands use the single existing target directory, offline/locked
resolution, `CARGO_INCREMENTAL=0` and two build jobs. Disk pressure precludes
independent full Cargo target trees. Source inspection and editing remain
parallel while heavy verification is serialized.

Only missing implementations use the refusing-stub/promotion sequence below.
PubMatic retains its existing active factory and public definitions: add audit
regressions against the current implementation, then repair demonstrated gaps.
Do not replace it with a refusing stub or candidate-only metadata.

The coordinator freezes every input read by each red/green Cargo run, not just
the selected worker's file. Before publication, freeze all workers and verify
the exact proposed commit tree without unpublished drafts, captures or TOML
changes. The coordinator can temporarily move known unfinished files to
temporary storage and restore them after the gate; do not discard or revert
another worker's work. Only the tree actually verified may be published. AG approved
the [conversion test pipeline](2026-10-24-bidder-conversion-test-pipeline.md) on
2026-10-24: use affected fast checks during iteration, then run the unchanged
full acceptance checks once per frozen publication batch. Ordinary new bidders
own their conformance fixtures; shared rosters and protocol switches must not grow.

| Family                   | Execution State                                                                                                                                                              |
| ------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `rise`                   | Protocol implemented and locally tested; complete selected-flow acceptance open                                                                                              |
| `sovrn`                  | Protocol implemented and locally tested; OpenRTB-first numeric scope and bid-local invalid UTF-8 refusal approved; selected-flow completion open                             |
| `unruly`                 | Protocol implemented and locally tested, including canonical duration metadata; selected-flow completion open                                                                |
| `yahoo_ads` (`yahooAds`) | Protocol candidate implemented; 13 untouched captures and native construction controls pass; selected-flow completion open                                                   |
| `criteo`                 | Protocol candidate and shared bounded browser-config output implemented; original-envelope decoder boundaries and selected-flow acceptance remain explicit                   |
| `pubmatic` | Protocol repaired and locally verified; original captures and bounded native/browser output covered; selected-flow acceptance and schema/resource/hosted qualification remain open. See the bidder-owned README. |
| `msft`                   | Protocol candidate implemented with generated settings, bounded query replacement, sealed extension projection and native allocation controls; selected-flow acceptance open |
| `thetradedesk` (`ttd`)   | Protocol candidate implemented with typed SupplyId routing and original captures; selected-flow completion open                                                              |
| `openx`                  | Protocol candidate implemented and locally verified; OpenRTB-first media policy and browser-config output covered; selected-flow acceptance open                                                                              |
| `sharethrough`           | Protocol candidate implemented with borrowed floor conversion, compiled version, fatal rollback and untouched source captures; selected-flow/schema/resource acceptance remains open |
| `rubicon`                | Protocol candidate implemented with coherent credentials, floor conversion, proprietary native projection and untouched captures; explicit four-region opt-in routing approved and tested; public promotion and composed schema/resource acceptance remain open |

APS, Kargo and Triplelift already have protocol candidates; their uncompleted
wire/resource/configuration/delivery work remains required, not restarted or
declared complete by this queue. All 14 selected identities remain in M7 under
the [scope correction](#2026-10-26-scope-correction). Sharethrough/Rubicon's
implemented protocol candidates still require complete selected-flow acceptance.

## Next Shared Prerequisites

Read-only audits of the remaining families completed on 2026-10-24. They did
not create implementation files, enable factories or establish acceptance.
Before granting the next red-test slots, the coordinator must resolve:

- Typed settings and entrypoint/version ownership in `BidderRequestInfo`,
  without bidder-local configuration readers. MSFT needs HB-source settings,
  TTD needs its SupplyId default, and Sharethrough needs a frozen version value.
- A bounded FX interface for Sharethrough and Rubicon, and coherent,
  identity-owned rotating credentials for Rubicon. Preserve their different
  fatal-versus-call-local conversion failures; do not invent rates or defaults.
- Shared sealed response projection for provider extensions and browser-auction
  configuration, including actual consumers and retained-memory accounting.
  Missing output channels must not become write-only fields, raw-body access or
  copied bidder-local parsers. Keep required-field/null/case differences explicit.
  The [browser-output implementation](2026-10-25-browser-auction-output.md)
  supplies the common channel and Criteo consumer; it does not promote OpenX or
  PubMatic, erase shared-decoder gaps, or qualify complete bidder delivery.
- Exact call-local media and identity coverage for split requests. Source
  captures containing unsent IDs remain named boundary cases, not repaired
  reference output.

AG approved generated bidder-owned host settings on 2026-10-24: adjacent
`bidder.toml` declarations drive strict typed configuration, with an explicit
implementation discriminator and checked requested/base identity binding.
Do not replace this with a central union of family-specific fields, an
ambiguous untagged enum or a second runtime JSON decode. Preserve field-level
requested/base fallback, including an explicit MSFT zero overriding a base
value before source-default normalization. Sharethrough's server version stays
the compiled package version, not another runtime setting. Implementation and
allocation/lifetime evidence remain required before these paths can be used.

The shared caller-context slice now carries `Auction`, `Amp` and `Video` from
`AuctionContext` into bidder construction. Both constructors default to
`Auction`; the current HTTP auction route keeps that default. Caller context is
not inferred from root JSON hints or video inventory. Integration controls
cross both constructors, both host aliases, all three caller contexts and
conflicting payload hints after real normalization and privacy scrubbing. The
initial semantic red observed `Auction` where the trusted caller supplied
`Amp`. This adds no public AMP/video route and does not complete MSFT's port.
The final native run passed 588 bidder-library tests (one source-dependent
control not selected), 515 exchange-library tests, 271 core-library tests and
25 focused provenance/settings/construction-bound integration tests. Strict
Clippy for those three packages' libraries and tests, formatting, whitespace
and the unchanged 172-fixture raw-JSON gate passed. Spec and quality reviews
approved the slice after hint presence and video inventory were independently
varied. These checks do not establish provider execution or complete M7.

AG approved TTD's SupplyId disposition on 2026-10-24: use the first selected
nonempty impression value, otherwise the resolved typed host setting. Missing,
empty or traversal-unsafe values refuse with a fixed bidder-local error rather
than targeting the bare endpoint. Safe values use the shared bounded endpoint
builder's percent encoding. The pinned source's permissive empty default is an
intentional difference, not a new inferred default or endpoint approval.

The [typed endpoint-binding prerequisite](2026-10-24-bidder-endpoint-bindings.md)
records the approved refinement: adjacent `[endpoint.fields]` declarations
generate borrowed named values and stack pairs for the existing bounded builder.
Definitions and aliases use only their resolved implementation's contract;
no central bidder-specific placeholder list or runtime template engine is added.
This is distinct from TTD protocol integration. Its private draft still needs
coordinated red/green, original-capture comparisons with explicit typed-model
differences, allocation controls and the unchanged wave acceptance gates.

The TTD integration below supersedes that private-draft status. It does not
complete the remaining wave acceptance gates.

Preserve the existing factories and public definitions for PubMatic while
repairing its audited source-specific gaps. Newly audited families retain
`factory=false` and empty definitions until their own coordinated red slot.

## TTD Protocol Candidate (2026-10-24)

The implementation was integrated through an actually compiling refusing stub:
all 22 initial inline controls failed semantically, then all 22 passed with the
same tests. An additional control explicitly distinguishes fatal typed decoding
from bid-local semantic rejection. Registration is discovered from adjacent
`bidder.toml`; public `[definitions]` remains empty, so this is not promotion.

- The first nonempty impression publisher and SupplyId are selected independently.
  The generated settings view supplies only the fallback SupplyId, preserving
  requested/base field precedence. Empty and unsafe routes refuse with fixed
  diagnostics. Generated endpoint bindings feed the existing capped builder.
- Borrowed serializers preserve the scoped request while rewriting the first
  banner format dimensions and the selected inventory's publisher. All
  impressions share one POST; no caller-tree clone or second config decode exists.
- Sealed response parsing and actual sent-call coverage remain mandatory. Only
  banner/video/native `mtype` values are accepted; candidate-local errors retain
  valid siblings. Status, empty currency and exact uppercase price-macro behavior
  follow the recorded contract; notification fields do not trigger networking.
- Twenty original protocol captures total 69,721 bytes. Their twelve missing-ID
  responses remain named shared-parser refusals; one further response names an
  unsent impression. One complete original ORTB video response, four status cases
  and two parameter refusals remain distinct from those boundaries. Six source
  request omissions are compared explicitly after asserting input preservation;
  only comparison copies change, never captures or emitted bodies.
- All 46 TTD controls passed, including the separately selected immutable Git
  object and extracted-parameter check. The canonical schema stays byte-identical;
  three positive and nine negative source parameter strings remain source-bound.
- The request allocator target passed all 14 tests. TTD's new controls independently
  vary root/impression widths, impression count, slots, body cap and settings
  versus impression SupplyId. Real caller clones fail the same absolute and
  differential bounds. Zero slots allocate nothing; rejected bodies refund slots.
- The response allocator target passed all 22 tests. TTD's 24 admitted peak windows
  vary bid count, macro repetition and finite price, including the largest finite
  value and smallest positive subnormal. All three rewritten channels are checked
  against literal decimal oracles; case variants and `lurl` remain untouched.
  The over-byte-cap corner is explicitly excluded rather than widening admission.
  Measurements and separate content-growth reasoning use the existing charges.

The checkout-dependent source control is explicitly selected by CI using its
existing pinned reference checkout. The workflow guard first failed on its
missing command; exact execution, checkout ordering and disabled-consumer
controls cover the added step. No workload limit, coefficient, dependency,
source revision or destination approval changes in this protocol slice.

Final local verification after lint cleanup and fixture-helper refactoring:

- Secure-default bidder/core/exchange libraries: 637/515/271 passed. Both
  checkout-dependent bidder controls were explicitly included; none was skipped.
  The exact CI-selected TTD source test also independently passed one test.
- Secure-default bidder request allocation, registry, endpoint builder and
  expansion targets: 14/12/10/9 passed; response-factor controls: 22 passed.
- Feature-enabled bidder library/request allocation/registry: 648/14/12 passed,
  again including both checkout-dependent source checks.
- Selected secure config library/default/binding/storage/smoke targets:
  60/2/20/1/14 passed. This is not the complete config test suite.
- Maintenance-only CLI command/import/source/wave targets: 155/7/13/9 passed.
  The initial `--lib` invocation was invalid for this binary package and was
  corrected to `--bin auctioneer-cli`; it is not counted as a successful run.
- All 104 workflow guards passed with the actual source step installed.
  Config/bidder/core/exchange all-target and maintenance CLI all-target Clippy
  passed under `-D warnings`, without new lint suppressions. Test-only helpers
  separate fixture setup, measurement and assertions; request success compares
  the entire expected body and exact byte length. All original capture assertions
  remain present.
- Cloudflare, Fastly and Spin library checks passed for their existing compiler
  targets. These are compilation results, not provider execution or absolute-fit
  evidence. Existing GVL deletion notices and the third-party future-incompatibility
  warning remain distinct from new lint failures.

Endpoint/alias metadata promotion, shared schema/resource qualification, composed
privacy/deadline/gzip flows and publisher delivery remain open. Native unit,
capture and allocation evidence does not satisfy those gates or R0 readiness.

## Shared Query Replacement (2026-10-24)

`EndpointBuilder::expand_with_query_set_into` provides the shared prerequisite
for MSFT's nonzero member routing. It replaces every matching key after actual
macro expansion and one form-decoding pass. Literal `+` means space; encoded
`%2B` means plus. Matching is case-sensitive and does not recursively decode.
Unrelated pairs, repeated values, flags and their expanded encodings retain
their existing order. New keys and values use the same form encoder as append.

The endpoint builder owns query semantics. A crate-private transactional URL
view exposes only the current URL, bounded writes and checked UTF-8-relative
removal inside the existing metadata pool. It cannot edit earlier components;
the public `set_url_with` signature and its callback-error behavior are unchanged.
There is no map, second URL allocation, new stack array or retained context.
All returned errors discard the entire URL, including prior successful edits.
The method, headers, covered impressions and existing URL on duplicate attempts
remain intact.

Both the original expanded URL and the final replacement must independently fit
the available metadata region. Replacement cannot rescue an over-cap original.
Literal fragments refuse, like nonempty query append. This is not byte-for-byte
reference query normalization: unrelated malformed escapes and semicolon pairs
are preserved rather than silently dropped, flags are not rewritten, and the
existing form encoder retains its own `*`/`~` spelling. Equivalence to the source
member setter is limited to effective values in valid queries, not its parser
quirks. MSFT's decimal member value is unaffected by those encoding differences.

Removal and UTF-8 revalidation have quadratic worst-case byte work within the
fixed 3,328-byte metadata region, not a claimed linear pass. Each iteration
advances or removes at least one byte, with no unbounded query map or allocation.
No workload ceiling, memory coefficient, endpoint approval, factory declaration
or public definition changes in this prerequisite. MSFT adoption and its sealed
response-extension/metadata and composed resource controls remain open.

The initial two semantic reds retained old literal/encoded and macro-produced
keys under append-only behavior. Native controls now cover exact replacements,
independent original/output byte boundaries, encoded expansion, oversize keys
and values, missing/traversal macros even in discarded values, fragments,
duplicate-URL precedence and rollback after deletion. The allocator measurements
exclude fixture setup and assertions and retain a real allocation positive
control. Pointer checks follow the pool into the finished metadata instead of
mistaking `finish()`'s ownership transfer for a reallocation.

Final local verification: bidder/core/exchange libraries passed 643/515/271
tests, with both checkout-dependent bidder controls included. Secure request
allocation/endpoint/expansion/registry targets passed 14/10/12/12; the feature
build passed 654 library tests and the same 48 integration tests. Strict
all-target Clippy for bidder/core/exchange, all three existing guest library
checks, formatting, whitespace and the unchanged 172-fixture raw-JSON gate
passed. Independent source/spec and quality reviews found no remaining blocking
issue in this slice. GVL deletion notices and the existing third-party future
incompatibility warning remain. These are local checks, not hosted execution,
M7 completion, MSFT acceptance or absolute WASM memory-fit qualification.

## MSFT Protocol Candidate (2026-10-24)

The discovered MSFT factory now consumes the generated typed settings and endpoint
bindings plus shared query replacement. Public definitions remain empty; the
source HTTP endpoint is not approved by this implementation. A compiling refusing
stub produced 31 intended behavioral failures and two passing integrity controls
before the implementation passed the same controls.

- Valid impressions retain order in chunks of ten. Nonzero member disagreement
  is fatal before calls; malformed impressions retain valid siblings through the
  existing capped diagnostics. Generated requested/base fields preserve explicit
  zero before trusted Auction/Amp/Video default normalization. Input trees are
  borrowed, not cloned, and the shared query setter owns member routing.
- Sealed decoding preserves valid siblings for provider-extension failures and
  checks actual sent coverage plus offered media. Category arrays require strings.
  All 95 category pairs are independently source-bound. Signed deal priority uses
  the shared reserved `i64` slot without adding a ranking rule; final response
  assembly preserves it, video duration metadata and unrelated extensions.
- All 31 original protocol captures remain byte-identical: 117,400 bytes. The
  repeated-SDK capture names an impression absent from both actual sent IDs; its
  named test asserts that shared refusal, not source success. The 12 valid and 21
  invalid parameter literals preserve exact offsets, lengths and digests. Source
  controls independently inspect all 53 artifact witnesses and 34 retained files.
- SDK strings are decoded lazily only for an impression needing a default
  display version. A request-local cache retains successful or absent projection;
  plain strings borrow the scoped App extension. App serialization precedes SDK
  decoding, so a containing App over the raw body cap cannot allocate derived
  strings first. This ordering may alter gzip bytes and encoded-cap admission;
  source gzip parity remains unqualified.
- Native construction controls vary root/impression widths, count, call slots,
  body cap and independently resolved host settings. The early-refusal SDK cases
  keep input-independent bounds. Late escaped source/version/key cases initialize
  at impression 21 with two retained calls and also test final-body refusal.
  Whole-request clone controls fail both the ordinary and augmented bounds.
- For successful SDK decoding, let `R` be raw App-extension bytes. Serde scratch
  capacity is at most `2R`; owned decoded strings sum to at most `R`. Peak buffer
  ownership is bounded by `3R`. Cumulative scratch reallocations plus strings
  are bounded separately by `5R`, not represented as live funding. Unknown escaped
  keys participate in `R`. The source audit ties these bounds to the pinned Serde
  and Rust growth rules, not a next-power-of-two assumption.
- This live buffer work reuses the existing `4C` scoped-supplement wire portion
  of sequential materialization working copies (`C` is the raw outbound cap):
  App is emitted before SDK lookup, so successful lookup has `R <= C`, including
  SDKs supplied through merged scoped data. Grouped trees and transformation
  temporaries are dropped before sequential constructors,
  ingress funding remains held through completion, and retained bidder scopes
  have their separate projection charge. No production charge or limit changes.
  This is not whole-parser qualification: malformed SDK diagnostic construction,
  work bounds and physical/target allocation remain explicit open proof work.
- The response allocation grid covers 1/32/256 bids, all three supported provider
  media codes, signed priority extrema/zero and negative/zero/positive durations.
  Decode/projection and actual response assembly have separate measurement windows
  against unchanged charges. Mixed provider errors retain the valid sibling.

Source/spec and independent quality reviews found no blocking protocol defect.
Remaining priority implementation families are Criteo, OpenX, Sharethrough,
Rubicon and PubMatic's source-specific repair. Their acceptance, all candidate
metadata promotion, shared schema/resource qualification, composed privacy and
original-deadline/gzip controls, full catalog migration and hosted M7.6 delivery
remain open. Neither a discovered factory nor these native controls qualify R0.

Final local verification used offline/locked resolution and two build jobs:

- Secure-default bidder/core/exchange libraries: 713/515/271 passed. Both
  checkout-dependent bidder controls were explicitly included; none was skipped.
  OpenRTB's four targets passed 96/1/9/3, including its unchanged parse-factor
  guard over sixteen independent fixtures.
- Bidder request allocation, endpoint builder, endpoint expansion and registry
  targets: 26/10/12/12 passed. Response factors and real wire assembly: 24 passed.
  The final scoped-supplement funding clarification was followed by fresh secure
  and feature allocation runs, each passing all 26 tests without changed bounds.
- Feature-enabled bidder library and registry targets: 724/12 passed, including
  both checkout-dependent controls. Selected secure config library/default/
  binding/storage/smoke targets passed 60/2/20/1/14; this is not the full config suite.
- Maintenance-only CLI command/import/source/wave targets: 155/7/15/9 passed.
  These inspect immutable source objects without executing foreign programs.
- OpenRTB/bidder/exchange all-target Clippy and maintenance-only CLI all-target
  Clippy passed under `-D warnings`. The only new localized lint exception is the
  Serde-required reference predicate's explicit signature-based expectation.
- Cloudflare, Fastly and Spin library checks passed on their
  existing compiler targets. No provider execution, workload fit or R0 readiness
  is implied. Existing GVL deletion notices and the third-party future-compatibility
  warning remain distinct from new errors.
- Workspace formatting, explicit discovered-module formatting, whitespace and
  the unchanged 172-fixture/52-file raw-JSON gate passed.

The SDK/category review controls first failed on real borrowing and malformed
category behavior before their repairs. Oversized SDK allocation controls also
failed before lazy decoding. The later escaped-SDK test initially exposed a
test-model error: cumulative allocation counted freed scratch reallocations as
live memory. Its final cumulative bound is separately derived, not raised to an
observed number, and real caller clones exceed it. These are scoped local checks,
not a full-workspace result or selected-flow acceptance.

## Shared Construction And Yahoo (2026-10-24)

- The real test driver now uses production's semantic-failure discard path.
  Its observed regression had two accepted calls; the repair refunds both
  while retaining construction diagnostics, sink errors and rejection counts.
  The success control commits both instead.
- Shared `sent_media_type` reads the responding call's existing funded
  correlation rows by its actual covered-impression ordinal. Same-ID calls
  remain distinct, and unsent, unannotated and zero-coverage metadata refuse.
  Success lookup creates no storage; fixed refusals use existing bounded
  response diagnostics. The factory response smoke test now uses metadata
  produced by actual request construction instead of inferring sent context.
- MSFT's wave operations now use the pinned source names and transformations,
  including `placement_id`, `member` and `inv_code`. The owning CLI controls
  preserve all 13 identities and 113 operations; no routing gate changed.
- Yahoo had 34 intended semantic failures against its refusing stub, then all
  34 passed with the same tests. Independent review added a distinct-parameter
  two-impression success control and exact first-call coverage at exhaustion.
  One POST per surviving impression and banner-first response selection match
  the source; all original media remain on the wire. No per-format fan-out
  behavior was approved or introduced.
- All 13 original Yahoo protocol captures remain byte-identical (25,589 bytes).
  Their nine empty source URIs are preserved and compared separately from the
  fixed test-only HTTPS endpoint. Named unsupported-media and direct-adapter
  schema-boundary cases do not count as source delivery. Implementation/helper
  provenance uses immutable Git witnesses, not checked-in foreign programs.
- The native constructor allocation control independently varies one/three
  impressions, zero/one/four slots, 32/4096-byte caps, root/impression payload
  placement and 64/262144-byte inputs. Actual clone controls exceed the fixed
  bounds. Refused bodies refund slots, so cumulative size-growth comparisons
  normalize by observed allocating attempts; peak and largest-allocation
  comparisons remain separate. No funding coefficient or workload limit moved.

The owning native run passed 583 bidder-library tests (one separately driven
source-dependent control ignored by default), 11 constructor-allocation tests
and 10 registry-join tests. Formatting and the raw-fixture gate pass. Secure and
feature builds, source-object checks, strict lint and provider-target checks
remain publication requirements, not implications of those native results.
Public Yahoo definitions remain empty. This is protocol preparation, not
endpoint approval, schema/resource qualification or completed publisher delivery.

## Verified Native Slice (2026-10-24)

First-batch publication checkpoints:

- [x] Presence-aware request fields and affected capture comparisons.
- [x] Three source-specific factories, immutable captures and source review.
- [x] Shared bounded header construction with an observed allocation regression.
- [x] Secure and feature library controls, request/response allocation and source integrity.
- [x] Feature-enabled workspace strict lint and all three provider library checks.
- [x] Final explicitly selected conformance/source-dependent runs.
- [x] Publish the verified first batch.

The implementation batch was committed and pushed as `aac6ba82` to
`m7-bidder-catalog` for PR #21. This closes that integration round only;
the three new public definitions remain unpromoted.

The initial cohort run had 147 controls: 140 failed and seven passed, including
integrity and inherited-boundary characterization. One obsolete missing-seat
decoder expectation was corrected after the shared optional-seat repair; do not
interpret that total as 140 independent missing protocol behaviors.
Independent source review then added six
failing Sovrn regressions and eight failing Unruly controls before repairs.
Shared integer helpers separately had three initial and two canonicalization
failures before their five controls passed.

Initial completed local commands, all offline/locked with two jobs:

- `cargo test -p auctioneer-bidder -p auctioneer-exchange -p auctioneer-core --lib`:
  517/513/271 passed; the one APS checkout-dependent test passed separately
  with `AUCTIONEER_APS_SOURCE` and `--ignored`.
- `cargo test -p auctioneer-openrtb --all-targets`: 107 passed. The allocation
  target measures 16 independent fixtures; 29.405x remains the maximum and
  dense missing video MIME data measures 29.253x. The 30x/36x guards did not move.
- Bidder request allocation, endpoint expansion allocation, endpoint builder
  and registry join targets: 37 passed.
- Ten affected exchange integration targets, including response allocation,
  privacy, exact construction boundaries and config overlays: 120 passed.
- Six affected core integration targets, including privacy and allocation:
  33 passed.

The broader all-target pass subsequently completed every bidder target,
including real factory/discovery builds and native/three-target generated
schema linking. It then exposed an obsolete 256-name runtime-catalog fixture.
The continued core/exchange/model pass exposed a debug test relying on the old
missing-seat parse error. These failed commands are not counted as overall
passes: the repaired targets independently passed nine and sixteen tests.

Final follow-up checks completed so far:

- Final secure-default bidder/core/exchange libraries: 530/513/271 passed.
- Bidder library plus request allocation, endpoint builder/expansion,
  outbound construction and registry join: 580 passed, one source-dependent
  APS control selected separately. This includes thirteen metadata units.
- Feature library lane: bidder/core/exchange 541/513/271 passed.
- Immutable source-object and wave CLI targets: 13/9 passed. The 55 new
  source captures remain unchanged.
- Explicit source-dependent APS control: one passed with
  `AUCTIONEER_APS_SOURCE` and `--ignored`; fanout conformance: 35 passed;
  feature-enabled mocktioneer integration: two passed.
- Workspace all-target Clippy with `insecure-bidders,conformance-suite` and
  `-D warnings`: passed without added suppressions.
- Provider library checks for Cloudflare, Fastly and Spin on their actual
  WASM targets: passed. No live provider execution or fit qualification.
- Formatting, diff whitespace and the 172-fixture/52-file raw-JSON gate:
  passed.

The header follow-up first failed a real constructor guard at 6,909 peak and
7,016 cumulative allocated bytes against its unchanged 6,152-byte allowance.
Its eleven new primitive controls also failed against a compiling refusal stub before
implementation. Streaming Cookie/DNT output closes that regression; neither
the existing 2-KiB control allowance nor any production limit/coefficient moved.
Source-only independent review found no new P1/P2 issue in either the shared
presence changes or this bounded-header repair.

These are scoped local checks, not a whole-workspace test result, complete
schema qualification, publisher acceptance or readiness of any R0-blocked target.

The PubMatic audit identified missing request wrapper/allowed-bidder handling,
adslot/dimension and publisher selection, impression filtering, floors/keywords,
FPD/SDK relocation, exact status/required-media rules and response metadata.
Its existing factory and definitions remain intact until test-first repairs;
marketplace seat overrides, native/browser envelopes and shared bounded decoding
remain explicit shared work rather than bidder-local parser workarounds.

## Per-Family Implementation

- [ ] Reuse the completed pinned source audit where present; audit any new or
      changed source contract once. Cover the implementation, parameter contract,
      adjacent source tests and captures, status/currency/media rules and helpers.
      Report exact null/shape/case/shared-API differences before inventing policy.
- [ ] Write inline behavior tests and a compiling refusing factory stub as
      temporary `candidate.rs` in the assigned folder. `factory=false` forbids
      `mod.rs`, so only the coordinator atomically moves the ready draft to
      `mod.rs` and activates its declaration for a red-test slot. Do not commit
      the temporary draft filename. Pause edits during that measurement.
- [ ] The coordinator enables the candidate factory, observes nonzero intended
      semantic test failures and reports them to the worker. Compile errors and
      empty test filters are not red evidence.
- [ ] Implement borrowed bounded transforms and candidate-local response
      classification. Never clone caller trees, materialize an uncapped body,
      recover unscoped data or add provider clients. Keep source-specific fatal
      request errors distinct from per-impression filtering.
- [ ] Retain source capture bytes unchanged, with complete adjacent provenance
      and before-dispatch integrity checks. Exact capture membership/counts and
      shared-boundary controls must be independently identified. Report gaps;
      never relabel unsupported source branches as full migration.
- [ ] Vary independent permissions/parameters/limits independently. Cover
      mixed good/bad candidates, all-invalid input, exact sent coverage, status,
      currency, source-specific media, immutability and reservation refunds.
- [ ] Request a serialized green run, then address independently reviewed
      source/spec findings followed by engineering-quality findings. Repeat the
      owning tests after changes; no workstream certifies itself.

## Coordinator Integration And Publication

- [ ] Let independent manifest expectations and automatically generated factory
      conformance tests follow discovery. Keep protocol expectations in each bidder
      folder instead of adding central bidder lists, schema pins, witness counts or
      name-based protocol branches. Shared tests must not impose one protocol.
- [ ] Implement shared API prerequisites once, with focused red/green controls
      and separate review. Unavailable settings, credentials, currency or sealed
      decoding must remain explicit until their contract is implemented.
- [ ] Execute secure-default and feature test lanes separately; run source/
      retained-asset checks, allocation/boundary controls, guest library checks,
      workspace strict lint, formatting and raw-fixture gates for each frozen
      publication batch, after affected fast checks during iteration. Aggregate test
      counts only from completed, uncorrupted output. Shared-code changes expand the
      affected test set immediately; do not reuse results across changed inputs.
- [ ] Commit only reviewed, verified family changes plus their matching shared
      integration/evidence updates. Push each coherent completed round to the
      existing M7 branch/PR; leave concurrent unfinished folders unstaged.
- [ ] Reconcile the complete selected-flow set and actual original-deadline,
      privacy/context, runtime activation, schema/resource and delivery evidence
      before reporting priority completion. Require all 14 identities and all
      mandatory priority-local branches, plus current foundation/schema and
      four-target build/package/catalog-relative gates with native fit/headroom,
      at the parent plan's Task 19 M7 closure. Keep partial in_progress rows
      honest; do not shrink branch manifests to close M7. M20 Tasks 20/21 do
      not gate M7. Parallel preparation makes neither checkpoint nor R0 ready;
      M7.5 embedding and M7.6 credentialed delivery remain separate.

## OpenX Protocol Candidate (2026-10-26)

The automatically discovered owner now implements grouped banner/native calls,
per-impression video calls, borrowed capped field rewrites, exact sent coverage,
source metadata/video mapping and sealed browser-configuration output. Public
definitions remain empty. No shared bidder roster, crate, feature, schema,
dependency pin, workload limit or charge coefficient changed.

AG approved OpenRTB-first media handling on 2026-10-26: only absent `mtype`
uses call-local fallback. Supported codes 1/2/4 must match offered media;
unsupported Audio 3, zero, null, unknown/non-integer codes and ambiguous
spellings reject only their bid. Valid siblings and independent configurations
survive. This intentional difference is recorded in the adjacent manifest,
not hidden by rewriting source captures.

All ten captures retain their 37,709 original bytes. Adjacent provenance binds
27 source artifacts, three trees and all seven valid/fifteen invalid parameter
literals. Both original unsent Fledge configuration keys and the duplicate-ID
request remain named boundaries, not repaired original-flow success. Shared
typed-envelope limitations remain explicit; no raw-body access or second parse
was introduced. Numeric price comparisons retain an independent f64-bit check.

Coordinator verification used Rust 1.98.1 with locked dependencies:

| Check | Result |
| --- | --- |
| Default library / allocation / response / isolation | 836 / 54 / 19 / 11 passed |
| Feature library / allocation / response / public API / isolation | 847 / 54 / 25 / 4 / 11 passed |
| All ignored library source controls, explicitly driven | Six passed, including the new owner witness |
| Workspace all-target Clippy | Passed with `-D warnings` |
| Cloudflare / Fastly / Spin conformance guest library checks | All three passed |
| Workspace and owned-source formatting, whitespace, raw-fixture controls | Passed |

The feature compiler-child invocation initially failed on unavailable DNS.
Rerunning with `CARGO_NET_OFFLINE=true` passed both child feature selections;
no assertion or dependency was weakened. Counts above describe separate
invocations, not unique tests summed across feature graphs.

Independent review found a tiny-cap allocation control stopped before the
video serializer. Its replacement includes a larger cap with small-input
success, large-input refusal, both reward modes and a measured clone control.
An actual temporary clone in the serializer failed at 267,821 peak bytes
against the unchanged 7,144-byte allowance. Exact source restoration was
checksum-verified; the restored owner/factory filter passed 50 tests plus
all four owner allocation controls. No mutation hook remains.

Logs use `/private/tmp/m7-openx-*.log`. The existing generic `source_objects_`
CI selector discovers the new primary-source control without a provider roster.
This completes the native protocol slice, not endpoint approval, public
activation, full source parity, schema/resource qualification, absolute R0
readiness, hosted execution or the priority wave's selected-flow acceptance.

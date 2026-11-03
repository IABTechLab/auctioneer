# bounded fanout Implementation Review (2026-10-06)

> Editorial normalization (2026-11-02): delivery labels were replaced with capability names. Sibling-checkout citations below are archival source locations, not paths into the current checkout. Recorded line locations, findings and results are unchanged.

## Confirmed Findings

This section preserves the original source-traced findings. The user
subsequently authorized repairs; all six now have implemented fixes and
executed regressions. The original reproductions below were source-traced,
not executed by the recording reviewer. See Fix Disposition for the later
implementation decisions and verification. Existing dirty repairs were
part of the original implementation reviewed.

Severity is P2 for each finding: behavioral defects or contract/coverage
gaps requiring an explicit disposition, not provider-qualification blockers.
R5-R6 are confirmed resource contract/coverage gaps, not demonstrated
resource overflow. The parent owns reporting and the merge decisions.

### R1: Foreign-impression bids are admitted and can win (P2, FIXED)

**Location:** [run.rs:1168](../../../crates/auctioneer-exchange/src/run.rs#L1168).
The exchange appends every parsed USD bid without validating that its
`impid` belongs to the bidder's requested impressions or the originating
call. [run.rs:1155](../../../crates/auctioneer-exchange/src/run.rs#L1155) passes
the original auction request and retained call metadata to `make_bids`, but
[Generic's parser:51](../../../crates/auctioneer-bidder/src/bidders/generic/mod.rs#L51)
ignores both correlation arguments. Winner selection then scans every
original impression at
[run.rs:1339](../../../crates/auctioneer-exchange/src/run.rs#L1339).

**Source-traced reproduction:** Submit an auction with `imp-1` requesting
only Generic and `imp-2` requesting only Pubmatic, with targeting enabled.
Generic's outbound call covers `imp-1`. Return a valid USD Generic response
containing a bid with `impid: "imp-2"` and a price above Pubmatic's bid.
The bid enters the Generic seat and receives winner targeting for `imp-2`,
although Generic was not sent that impression. A nonexistent `impid` is
also admitted into the response, even though it cannot win an existing
impression.

**Impact:** An uncorrelated bidder response can participate in another
bidder's slot and displace its legitimate winner. Positional transport
correlation alone does not establish returned-bid impression membership.

**Missing tests:** A response mixing valid, foreign, and nonexistent
impression IDs; alias identities; and multiple calls for one bidder where
the returned ID belongs to another call. Cover both response admission and
winner targeting. The expected behavior for empty `covered_imp_ids` needs
an explicit decision rather than an unconditional reject-all assumption.

**Spec basis:** This is an inferred protocol invariant, not an explicit
foreign-`impid` rejection requirement in the supplied spec or plan. The
precise supporting references and the empty-metadata qualification are in
the spec-reference follow-up below.

### R2: Prefix fallback reserves 5 bytes instead of MIN_KEY_LENGTH (P2, FIXED)

**Location:** [keys.rs:113](../../../crates/auctioneer-exchange/src/targeting/keys.rs#L113).
`resolve_prefix_and_max` checks `prefix.len() + 5 > cap`, even though
`MIN_KEY_LENGTH` is 12 and the spec requires reserving that length before
accepting a custom prefix.

**Source-traced reproduction:** Enable targeting with the 15-byte ASCII
prefix `abcdefghijklmno` and the default 20-byte cap. The implementation
accepts the prefix without warning 10015. Both winner and per-bidder
`_bidder` keys truncate to `abcdefghijklmno_bidd`; the per-bidder identity
suffix has disappeared. The required behavior is fallback to `hb` with
the too-long-prefix warning. Prefix lengths 9 through 15 expose the wrong
acceptance interval at the default cap.

**Impact:** Keys can lose bidder identity and collide with unsuffixed
winner keys, without the prescribed fallback diagnostic. The bounded
allocation repair does not correct this acceptance threshold.

**Missing tests:** Default cap 20 with prefix length 8 accepted and 9
falling back; configured cap 16 with prefix length 4 accepted and 5 falling
back; UTF-8 byte-boundary cases; and an auction-level assertion for both
the resulting keys and warning 10015.

**Spec basis:** Explicit requirement in design Appendix E.3,
[lines 7891-7899](../specs/2026-05-21-auctioneer-openrtb-server-design.md#L7891).
The authoritative corrections retain the 20-byte hard cap and bounded
allocation requirement; this finding does not request removing either.

### R3: enabledonly lists a disabled alias shadowing a compiled name (P2, FIXED)

**Location:** [info_bidders.rs:171](../../../crates/auctioneer-core/src/handlers/info_bidders.rs#L171).
The list initially includes enabled compiled bidder names. Its subsequent
alias pass skips a disabled alias but cannot remove the already-added
compiled name. Sorting and deduplication preserve that name. By contrast,
[detail:230](../../../crates/auctioneer-core/src/handlers/info_bidders.rs#L230)
resolves the alias first, and the `all` response overwrites the compiled
entry with the alias entry.

**Source-traced reproduction:** Configure host alias
`pubmatic = "appnexus"` and disable Appnexus through
`[bidders.appnexus] enabled = false`, leaving compiled Pubmatic enabled.
`GET /info/bidders?enabledonly=true` still includes `pubmatic`.
`GET /info/bidders/pubmatic` and `/info/bidders/all` describe it as a
disabled alias of Appnexus. The repaired auction resolver/splitter denies
the dropped shadowing alias instead of falling through to compiled
Pubmatic.

**Impact:** Discovery advertises an enabled observable identity that the
detail endpoint and auction path treat as disabled. This is a residual
catalog-list defect, not a request to undo the dropped-alias denial repair.

**Missing tests:** A host alias colliding with a compiled name, with its
base disabled by an overlay and separately by baked metadata; compare
enabled-only list, detail, `all`, and auction eligibility. Preserve the
explicitly base-only semantics of `baseadaptersonly=true` in separate
assertions.

**Spec basis:** Design section 8.1,
[lines 5171-5177](../specs/2026-05-21-auctioneer-openrtb-server-design.md#L5171),
requires `enabledonly=true` to filter to enabled bidders. Section 5.9's
[observable alias contract](../specs/2026-05-21-auctioneer-openrtb-server-design.md#L3318)
and the plan's
[Current Review Corrections](2026-07-10-multi-adapter-production-fanout.md#current-review-corrections-2026-10-05)
establish the alias identity and dropped-shadow routing behavior.

### R4: Alias prebid loses diagnostics to auction-wide entries (P2, FIXED)

**Location:** [response.rs:149](../../../crates/auctioneer-exchange/src/response.rs#L149)
and [response.rs:152](../../../crates/auctioneer-exchange/src/response.rs#L152).
`collect_errors_and_warnings` first inserts bidder diagnostics, then uses
`insert("prebid", ...)` for auction-wide errors and warnings, replacing
any array already belonging to an alias named `prebid`.
[alias.rs:132](../../../crates/auctioneer-exchange/src/alias.rs#L132) accepts
`prebid -> generic`; its identifier validator checks length, not reserved
names.

**Source-traced reproduction:** Declare request alias
`ext.prebid.aliases = {"prebid": "generic"}` and request it through
`imp.ext.prebid.bidder.prebid = {}`. Make GDPR inapplicable so the bidder
is not privacy-denied, and advance a fixture clock until `tmax` expires
before dispatch. The short-circuit path creates bidder timeout code 1 at
[run.rs:665](../../../crates/auctioneer-exchange/src/run.rs#L665) and an
auction-wide code 12 at
[run.rs:720](../../../crates/auctioneer-exchange/src/run.rs#L720). Response
assembly leaves only code 12 under `ext.errors.prebid`, discarding code 1.
The warning map has the same overwrite when bidder and auction-wide
warning arrays are both nonempty.

**Impact:** Diagnostics for a currently accepted alias are silently lost.
Rejecting a reserved alias name versus preserving both diagnostic arrays
is a policy/contract decision; this review does not automatically choose
or implement a new namespace restriction.

**Missing tests:** Request and host aliases named `prebid` with simultaneous
bidder/global errors and simultaneous bidder/global warnings; an expired
`tmax` auction exercising the observable output; and explicit validation
tests once the reserved-name policy has been decided.

**Spec basis:** Section 5.9 requires alias-keyed errors and warnings, while
Appendix F reserves the `prebid` output key for auction-wide issues. Neither
the supplied alias validation rules nor the effective schema explicitly
forbids this alias name. See the follow-up below for exact references.

### R5: Runtime bidder-config keys bypass the identifier bound (P2, FIXED)

**Location:** [config/mod.rs:507](../../../crates/auctioneer-core/src/config/mod.rs#L507).
`validate_bidder_map` validates each override value but not its map key.
An invalid value also formats the unchecked name into its validation error.
[handlers/auction.rs:973](../../../crates/auctioneer-core/src/handlers/auction.rs#L973)
copies map keys into `BidderName` before filtering the disabled overlay.

**Source-traced reproduction:** Supply a runtime `[bidders.<name>]` entry
whose quoted name is 65 ASCII bytes, with `enabled = false`. The value
passes validation and the oversized name is copied when constructing the
request-scoped disabled set. Separately use an invalid override value,
such as `max_transport_bytes = 0`, under that oversized name: the error
format includes the unchecked name instead of rejecting the key first.

**Impact and action:** This bypasses the already-agreed 64-UTF-8-byte
bidder identifier contract before resolution/diagnostic construction.
Validate map keys first and use a generic error for oversized keys, without
echoing the name. This is a confirmed validation/resource-contract gap,
not an executed demonstration of envelope overflow.

**Missing tests:** Runtime bidder keys at 64 and 65 UTF-8 bytes, including
multibyte names; an oversized key with `enabled = false`; and an oversized
key with an invalid value, asserting the key-bound error wins and does not
include the supplied name. Exercise production config validation before
auction/catalog disabled-set construction.

**Spec basis:** The first identifier-bound bullet in the plan's
[Current Review Corrections](2026-07-10-multi-adapter-production-fanout.md#current-review-corrections-2026-10-05)
requires bidder/alias bounds before resolution or diagnostic construction.
This finding applies that existing contract, not a new identifier policy.

### R6: Config allocation guard bypasses production resolution (P2, FIXED)

**Location:** [config_parse_factor.rs:173](../../../crates/auctioneer-core/tests/config_parse_factor.rs#L173).
The guard directly deserializes TOML into `AuctioneerAppConfig` and asserts the
final retained allocation against the charged factor. Cumulative allocation
is recorded for reporting, but neither production resolution nor the peak
overlap of its extraction, integrity, and typed-conversion stages is tested.

**Source-traced reproduction:** Inspect `config_parse_factor_guard`: its
measurement window calls `toml::from_str` directly, then reads the retained
live-byte delta. No production resolver/extractor is invoked. Passing this
test therefore cannot establish the production-resolution allocation claim.
No test was executed and no production overflow was demonstrated.

**Impact and action:** This is a confirmed acceptance/coverage gap, not
proof that the configured resource budget is insufficient. Add a guard
through the production resolution path, measuring the relevant peak/live
overlap, including extraction, integrity handling, and typed conversion,
against their funded terms. Retained-only direct TOML coverage can remain
as a narrower test but must not substitute for that acceptance guard.

**Missing tests:** Production-resolution allocation coverage with bounded
adversarial config envelopes and the simultaneously live stage buffers;
assertions tied to the configured extraction and parse charges. Measure
actual overlap rather than treating cumulative allocated bytes as peak
retention.

**Spec basis:** The bounded fanout plan's allocator-harness requirement at
[line 6838](2026-07-10-multi-adapter-production-fanout.md#L6838)
requires the real resolve path, not just the struct. The resource formula
at [lines 6982-6990](2026-07-10-multi-adapter-production-fanout.md#L6982)
separately funds raw extraction and deserialized config; the acceptance
guard must cover the production work and relevant overlapping lifetimes.

## Spec-reference Follow-up

### Foreign-impression correlation

- Design section 4.1,
  [lines 626-638](../specs/2026-05-21-auctioneer-openrtb-server-design.md#L626),
  defines retained `OutboundRequestMeta`, including `covered_imp_ids`, as
  the response parser context. It does not prescribe membership rejection.
- Design section 5.2,
  [lines 2118-2122](../specs/2026-05-21-auctioneer-openrtb-server-design.md#L2118),
  requires each positional result to be correlated to its owning adapter
  and request before `make_bids`. This is call correlation, not an explicit
  returned-bid validation rule.
- Design Appendix E.4,
  [lines 7907-7915](../specs/2026-05-21-auctioneer-openrtb-server-design.md#L7907),
  specifies per-impression winner selection. R1 demonstrates how the
  unvalidated response crosses the bidder's participation boundary.
- bounded fanout plan
  [lines 2861-2883](2026-07-10-multi-adapter-production-fanout.md#L2861)
  requires retained per-slot parser context and a fixture that reads it.
  However,
  [lines 6263-6266](2026-07-10-multi-adapter-production-fanout.md#L6263)
  explicitly permit empty `covered_imp_ids` when an adapter did not
  pre-correlate. Do not turn that into an undocumented reject-all policy.

### MIN_KEY_LENGTH prefix fallback

Design Appendix E.3,
[lines 7891-7899](../specs/2026-05-21-auctioneer-openrtb-server-design.md#L7891),
explicitly requires the prefix to fit alongside `MinKeyLength` 12 or fall
back to `hb` with a warning. Within the authoritative 20-byte ceiling,
the acceptance check is therefore prefix byte length plus 12, not plus 5.

The protocol reference behavior also reserves 12 bytes for bidder-specific
keys and falls back to the default prefix when that reserve would not fit.
Reference behavior does not override bounded fanout's authoritative 20-byte bound.

### Is alias prebid explicitly allowed or forbidden?

**Spec answer: neither.** Design section 4.2,
[lines 721-732](../specs/2026-05-21-auctioneer-openrtb-server-design.md#L721),
enumerates alias validation failures without a reserved-name exclusion.
Section 5.9,
[lines 3320-3333](../specs/2026-05-21-auctioneer-openrtb-server-design.md#L3320),
requires alias identity in bidder errors and warnings. Section 5.6,
[lines 2840-2841](../specs/2026-05-21-auctioneer-openrtb-server-design.md#L2840),
and Appendix F,
[lines 8036-8038](../specs/2026-05-21-auctioneer-openrtb-server-design.md#L8036),
reserve `prebid` as a diagnostic output key, but do not explicitly prohibit
it as an input alias name.

**Effective schema/validation answer: currently accepted, not expressly
documented as an allowed special case.** The wire type is an unrestricted
`BTreeMap<String, String>` at
[request_prebid.rs:10](../../../crates/auctioneer-openrtb/src/ext/request_prebid.rs#L10).
Request resolution checks identifier lengths, base registration/disable
state, and no-op aliases, but has no reserved-name guard. Host overlay
validation at
[config/mod.rs:149](../../../crates/auctioneer-core/src/config/mod.rs#L149)
checks count and identifier lengths; runtime validation at
[config/runtime.rs:461](../../../crates/auctioneer-core/src/config/runtime.rs#L461)
and baked configuration validation at
[auctioneer-config/build.rs:652](../../../crates/auctioneer-config/build.rs#L652)
check base registration, not alias namespace reservation.

Reference behavior distinguishes configured alias registration from
request-scoped aliases: the former rejects reserved diagnostic names,
while the latter does not establish the same blanket prohibition. That
distinction does not define Auctioneer's request-alias policy. R4's confirmed
overwrite remains actionable regardless of the explicit policy adopted.

## Descendant Inspection

Source inspection found R1-R4 still present in privacy enforcement, user sync, and diagnostic qualification
at the time of this review. The table identifies the relevant modules and
exact admission/check/overwrite lines; no descendant files were edited.

| Finding                            | Privacy enforcement source | User-sync source | Diagnostic qualification source |
| ---------------------------------- | ------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| R1: foreign-bid admission          | `run/mod.rs:1081` | `run/mod.rs:1017` | `run/mod.rs:1017` |
| R2: prefix reserve +5              | `targeting/keys.rs:113` | `targeting/keys.rs:116` | `targeting/keys.rs:116` |
| R3: compiled-name enabled filter   | `info_bidders.rs:171` | `info_bidders.rs:171` | `info_bidders.rs:171` |
| R4: prebid error/warning overwrite | `response.rs:149`, warnings at 152 | `response.rs:149`, warnings at 152 | `response.rs:149`, warnings at 152 |

For R1, the descendants already pass a bidder-scoped sanitized request to
`make_bids` at privacy enforcement `run/mod.rs:1068` and user sync/diagnostic qualification `run/mod.rs:1004`. That does
not resolve the finding: Generic still ignores `_request` and `_sent` at
`crates/auctioneer-bidder/src/bidders/generic/mod.rs:32`, and the exchange
admits its returned bids without an impression-membership check.

For R4, request alias acceptance remains in
`crates/auctioneer-exchange/src/alias.rs:132` in privacy enforcement and `:129` in user sync/diagnostic qualification, without
a reserved-name check. None of the inspected descendant rules supersedes
the prefix contract or resolves the reserved diagnostic-key collision.

## Separate Update And Test-gap Work

The EdgeZero PR 275 update to
`950cb81d406d0dc269baf94314547329a4c1cef0`, integration changes, and their
test-gap fixes are parent-owned work, tracked separately in
[EdgeZero Update And Capability Sync](2026-10-06-edgezero-update-and-stacked-branch-sync.md).
The current review corrections govern the pin and batch/EOF integration
contract. Parent work on those changes must not be reported as automatic
resolution of R1-R6, or as authorization for additional auction policies.

The prior dirty repairs tracked in the
[auction review follow-up](2026-10-06-auction-review-followup.md) were
included as implementation, not reverted or treated as absent. This review
does not repeat those resolved bookkeeping, scoped-cap, dropped-alias,
logging, or acceptance-roster repair tasks as new findings. Verification
claims for a prior pin do not independently verify the new pin; the parent
owns current integration verification and downstream synchronization.

The parent's latest reported verification status is recorded separately
from this review's unexecuted reproductions:

- Seven EOF regression tests were added and passed in the 45-test wave.
- Strict Clippy was reported green in run `33522`.
- The full default suite was reported green in run `41261`.
- The insecure suite was still running in run `65260` at that update.

These are parent-reported results, not commands run or independently
verified by this reviewer. They do not close R1-R6; capability sync was
still underway when the parent supplied this status.

## Accepted Qualification Deferrals

The following remain accepted boundaries, separate from R1-R6 and from the
integration/test-gap work:

- Budget-over-ceiling remains warn-only under the authoritative corrections.
  This review does not make that accepted mode a blocker or require a new
  hard-rejection policy.
- Hosted-provider qualification is not established by local checks or WASM
  builds. Existing hosted-provider deferrals are not new review blockers.
- Raw-ingress head/framing limits and BestEffort response-egress completion
  retain their documented qualification boundaries. New upstream EOF and
  batch checks do not automatically upgrade those guarantees.

## Fix Disposition

The user authorized fixes after the original review. The governing spec
and plan now state these decisions explicitly:

- R1: retain only bids belonging to the surviving observable bidder group
  and, when populated, the originating call's impression metadata. Empty
  metadata falls back to group membership. Mixed responses retain valid
  bids; one fixed-size code 4 diagnostic per affected call does not echo
  supplied IDs. The existing bid vector is filtered in place, without a
  new membership index or request clone. Seven integration regressions
  cover foreign, unknown and cross-call IDs, aliases and empty metadata.
- R2: reserve `MIN_KEY_LENGTH` (12 bytes) within the existing 20-byte cap.
  Boundary, UTF-8 and auction tests enforce fallback warning 10015. A
  second regression caught expansion of escaped control bytes: the quoted
  diagnostic prefix is now bounded both before and after escaping.
- R3: in non-base-only discovery, accepted catalog aliases shadow compiled
  names before enabled filtering. Seven regressions compare list, detail,
  aggregate and auction eligibility, including baked/overlay disablement,
  invalid-base alias dropping and base-only behavior.
- R4: keep the accepted alias `prebid` valid and append auction-wide entries
  after bidder entries at their shared wire key. Two unit and four auction
  regressions preserve simultaneous errors and warnings for host/request
  aliases, including expired `tmax` without dispatch.
- R5: validate all bidder map keys against the 64-byte limit before any
  values. Manual root validation also prevents the custom-field derive
  from serializing whole rejected bidder, account or alias maps into error
  parameters. Field keys, codes, nested validation and `ValidateArgs` remain
  covered. The focused config suite passes 39 tests; real resolver tests
  reject malformed config before catalog/auction dispatch.
- R6: measure peak live logical heap through the SDK raw config store and
  real runtime resolver, including envelope integrity, typed conversion,
  and retained host/account clones. Valid and rejected adversarial fixtures
  are checked against the combined production-funded config terms. The
  former direct TOML test remains a separately named retained-only guard.

Behavioral regressions were run red before repairs and green afterward.
Independent follow-up review found the R2 escaping and R5 hidden-map-copy
gaps; these were also reproduced and fixed. Final commands, measurements
and downstream integration status are recorded in
[bounded fanout Review Fixes](2026-10-06-production-fanout-review-fixes.md).

The original recording reviewer edited only this document and ran no
commands. Later source repairs, tests and Git operations are parent-owned.
The master spec filename is retained in bounded fanout and renamed in user sync onward.

## Coverage And Residual Risks

The read-only source review covered the exchange auction pipeline, alias
routing/catalog, scoped response budgets, privacy/config precedence, bidder
transforms, and response shaping/targeting/bookkeeping, against the supplied
design and bounded fanout plan, especially Current Review Corrections. Follow-up source
inspection checked the same four defects in privacy enforcement-diagnostic qualification and the protocol reference
prefix and alias-validation behavior.

The completed Helmholtz resource review, reported by the parent, checked
all gates, probes, budgets, parsers, cancellation, and source guards. Its
only additional findings were R5 and R6; no other findings were reported
from that coverage. R5-R6 were also checked against the cited local source
for this record. Their descendant persistence was not separately traced
here; the descendant table records only the confirmed R1-R4 inspection.

The original reproductions were source-traced by the recording reviewer;
later executed verification is distinguished in Fix Disposition. Neither
review nor local fixes establish exhaustive absence of defects or hosted
qualification. The allocation guard measures logical heap on the native
resolver's measuring thread, excluding provider-resident startup data,
allocator overhead, internal physical realloc overlap and other provider
runtimes. Its finite adversarial fixtures validate combined funding, not
each factor independently or every possible input.

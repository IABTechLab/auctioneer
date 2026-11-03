# privacy enforcement: Auction Privacy Completion

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

Date: 2026-09-26
Status: Implemented; local resource acceptance closed; provider deployment and physical-isolate qualification remain separate
Worktree: `.claude/worktrees/privacy-enforcement` (`privacy-enforcement` branch)

## 1. Purpose and Scope

privacy enforcement completes auction-path privacy: GPP section selection, US-Privacy/CCPA,
activity controls, LMT, COPPA, and the remaining request scrubs. It extends
the GDPR enforcement GDPR implementation; it does not replace it with a new privacy stack.

The approved architectural direction is one privacy enforcement design document, followed
by a separate implementation plan after document approval. Add modules to
existing crates, not new workspace crates. This document specifies technical
behavior, not a legal-compliance certification.

Included:

- GPP envelope parsing and sections 2 (TCFEU2) and 6 (USPV1).
- Opaque preservation, but no policy enforcement, of other GPP sections.
- Legacy privacy-field validation, source precedence, conflict warnings,
  and bidder-specific legacy projection.
- CCPA enablement, sale opt-out, and bidder no-sale exceptions.
- All eight activity names and their evaluator; the four live auction
  activities enforce decisions in privacy enforcement.
- LMT/COPPA enforcement and every remaining Appendix C.2 scrub row.
- Retention of TCF Disclosed Vendors membership, as promised by the master
  spec, without creating an additional permission-granting path.
- Baked and runtime configuration, resource accounting, and end-to-end
  tests across the existing target builds.

Not activated here:

- `/cookie_sync`, `/setuid`, `/getuids`, cookie storage, syncer selection,
  and endpoint-specific sync privacy errors: user sync.
- Enforcement for GPP sections 3 and 7-12 or additional US state policies.
- Analytics delivery, enrichment modules, new bidder integrations, or
  enforcement of TCF Publisher TC contents.
- EdgeZero transport redesign, new runtime signing, or provider
  certification. privacy enforcement consumes the bounded fanout runtime contracts.

## 2. Baselines and Authority

| Source                           | Revision / role                                                                                                                                                    |
| -------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Auctioneer starting code             | `d1f1bde25b02169b8eba04ce0dca23061c757203`, committed bounded fanout worktree state                                                                                           |
| Master design                    | [Auctioneer design](2026-05-21-auctioneer-openrtb-server-design.md), sections 7.1-7.7, 14, 15, Appendices A, C, D                                                          |
| Transport dependency             | [bounded fanout plan](../plans/2026-07-10-multi-adapter-production-fanout.md)                                                                                             |
| EdgeZero dependency at this base | `cff8a85ed6e91a077e700edc9eae28e0b18b46c4`                                                                                                                         |
| Behavioral reference             | [Pinned source snapshot](https://github.com/prebid/prebid-server/tree/d7f8515b86258688304b0d9b6668c6a0e258bc9e), commit `d7f8515b86258688304b0d9b6668c6a0e258bc9e` |
| Reference GPP library            | `github.com/prebid/go-gpp v0.2.0`, as pinned in that upstream revision                                                                                             |

The local upstream checkout HEAD is not the compatibility reference. Generate
reference fixtures from the exact revision above, with its locked library
versions. Record revision, input, configuration, observable output, and any
intentional difference in each fixture family.

Section 12 resolves master-spec ambiguities and records explicit compatibility
decisions. Implementation was authorized on 2026-09-28 after re-review.
Copy these decisions into the master spec and its divergence table during
implementation; do not leave conflicting completion criteria behind.

The starting revision is not a claim that bounded fanout is complete or certified.
Independent parsing, configuration, evaluation, and scrub work can start
on this baseline. Before integrating shared auction/runtime paths,
incorporate reviewed committed bounded fanout changes affecting config, resource limits,
bidder projection, or runtime construction. Do not copy uncommitted changes
out of another worktree.

## 3. Ownership and Existing Gaps

| Crate                       | privacy enforcement responsibility                                                                                                                           |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| `auctioneer-privacy`            | Bounded signal parsing, immutable normalized state, activity evaluation, CCPA/LMT/COPPA decisions, warning taxonomy, retained DV membership |
| `auctioneer-exchange`           | Bidder-local request projection, fetching gates, scrub primitives, legacy projection, final serialized-request assertions                   |
| `auctioneer-config`             | Effective typed privacy settings, baked defaults, build-time lowering and validation                                                        |
| `auctioneer-core`               | Runtime override DTOs, validation, account resolution, handler integration                                                                  |
| `auctioneer-openrtb`            | Wire-field representation and structural parse validation only                                                                              |
| `auctioneer-runtime`            | Existing budget accounting extended for measured privacy enforcement allocations; no consent policy                                                          |
| Existing adapters and tools | Configuration plumbing, portable build checks, transport-capture integration fixtures                                                       |

Keep `auctioneer-config-schema` and `auctioneer-gvl-schema` independent of privacy
evaluation. They are existing schema/build boundaries, not places to move
the new policy engine. A third-party parser dependency is not a new Auctioneer
workspace crate.

Relevant starting-code gaps:

- `activity_allow` and CCPA decisions are permissive stubs. The current
  `Activity` enum lacks the transmission activities.
- GDPR-off and NSP paths can return a globally permissive context. That
  construction cannot represent independent privacy enforcement policies.
- Host CCPA/LMT baked settings exist, but corresponding runtime overrides,
  account CCPA channel settings, and activity rules are incomplete.
- `scrub/mod.rs` currently implements only GDPR identifier and geo/IP scrubs.
- TCF DV framing is validated, but its membership is discarded.

## 4. Processing and Decision Contract

### 4.1 One Normalization Pass

The auction pipeline is:

1. Perform existing structural validation and account
   resolution, alias validation, inventory-channel selection, and iOS ATT
   preparation. Establish the validated bidder-name namespace for `nosale`.
2. Select one authorization timestamp, preserving GDPR enforcement's GCL/GVL time model.
3. Parse/normalize privacy inputs once. Produce source selections, bounded
   warnings, and immutable request-wide policy state.
4. Resolve bidder identity using the existing registry and alias rules.
5. Evaluate `fetchBids` and GDPR `AllowBidRequest`. A failed gate prevents
   adapter request construction and all outbound sends for that bidder.
6. Complete permitted bidder-specific input projection, FPD merging, and
   identity injection. Derive the bidder's scrub decisions from the
   immutable context, never from fields already scrubbed for another bidder.
7. Apply scrubs and legacy privacy projection to that bidder's owned request.
8. Pass only that sanitized request to the bidder adapter and enforce the
   existing transport/header/endpoint restrictions on every outbound request.

Survivor FPD must preserve each original `imp.id`. Replacing or deleting it
returns `InvalidImpFirstPartyData` (code 15) before scrubbing, adapter hooks or
dispatch, including on an expired auction. Repeating the same ID and changing
context are permitted. Denied, sampled-out and cap-dropped bidders remain
unmaterialized; their FPD cannot poison an allowed sibling. Response hooks keep
the sanitized bidder-local request, not a restored unsanitized original.

Request construction and transport finalization share the original absolute
auction deadline. Check it before every queued finalization, including multiple
calls from one bidder and sibling aliases. If a finalization exhausts that
deadline, release the partial batch and all remaining accepted bodies without
encoding another call or dispatching. Preserve original participation and prior
errors/warnings in the existing timeout response.

No later phase may restore prohibited fields from the original request,
raw body, stored request, cookies, account defaults, or bidder parameters.
An adapter that produces multiple HTTP requests must satisfy the same
privacy decisions for each request. The transport must not add the caller's
raw IP or identifiers through headers after body scrubbing.

Preserve `prepare()` rejection of OpenRTB 2.5 privacy locations such as
`regs.ext.gdpr`, `regs.ext.us_privacy`, and `user.ext.consent`. privacy enforcement does not
promote them into native fields or add a second accepted privacy dialect.
Here, "legacy" consent/USP means the native non-GPP fields `user.consent`
and `regs.us_privacy`; supported legacy EID locations are a separate scrub
concern, not permission to accept legacy privacy-signal locations.

### 4.2 Independent Policies

The resolved context contains independently evaluated GDPR, CCPA, activity,
LMT, and COPPA state. GDPR disabled/out-of-scope means permissive **GDPR
permissions**, not a permissive overall privacy context. The same applies
to an NSP GDPR bypass.

Preserve the current resolver's request/account/host/GVL/GCL/time inputs.
Exchange-owned NSP recognition must reach the GDPR subresolver as a narrow
exemption, through an explicit internal resolve-options value or equivalent
constructor; it must not bypass normalization or the other policy engines.
Keep privacy error types in `auctioneer-privacy`, not `auctioneer-exchange`.
The exchange supplies the validated bidder-name namespace to the no-sale
normalizer. Pass its validated result into resolution through the same
internal options/input boundary; do not make privacy depend on the bidder
registry crate or have it accept an unchecked exemption list.

Per bidder, expose a cohesive decision with:

- whether fetching is permitted, and the reason if not;
- existing `AuctionPermissions`;
- `transmitUfpd`, `transmitPreciseGeo`, and `transmitTid` outcomes;
- CCPA enforcement, LMT enforcement, and COPPA enforcement flags.

Do not introduce a single `privacy_allowed` boolean. Sending a contextual
bid request, passing identifiers, and passing precise geo are distinct
decisions. Activity allow does not override GDPR or another policy's scrub.

### 4.3 Trust and Identity

Derive channel from validated inventory/route, not
`ext.prebid.channel.name`: `dooh`, otherwise `app`, otherwise `web` for the
auction route. Do not let request-controlled channel text select a less
restrictive configuration.

Retain both the requested bidder name and resolved core bidder name. GVL
lookup and alias GVL overrides retain GDPR enforcement semantics. Activity/no-sale
matching must use the identity specified in sections 6 and 7, not whichever
name is most convenient at the call site. An account-id sentinel must never
acquire an NSP exemption merely because it matches configured text.

## 5. GPP and TCF Selection

### 5.1 Separate Envelope, Section, and Policy States

Represent these states distinctly:

- Envelope: absent, valid, or malformed/unsupported. A bounds/work limit
  violation is a terminal error, not a recoverable envelope state.
- Selected section: absent, present-empty, present nonempty raw value, or
  unusable inner value. Empty-section handling is policy-specific below.
- Effective value: selected GPP value or legacy fallback, with provenance.

Do not reduce them all to `Option<String>`. An absent authoritative section
permits fallback; an authoritative section with empty/invalid TCF does not.
An empty USPV1 section does permit fallback, unlike empty TCF.

Parse only the GPP envelope, then inspect sections 2 and 6 as needed. Other
sections remain opaque. A malformed inner payload in an unenforced section
must not make a valid section 2 or 6 unusable. Preserve the original `gpp`
and integer `gpp_sid` on outbound requests; do not re-encode the envelope,
sort the wire list, or synthesize an envelope from legacy inputs.

Membership tests may deduplicate SIDs internally, but may not change wire
ordering. A typed JSON failure in `gpp_sid` remains a request parse error.
Unsupported integer SIDs do not activate a policy and are preserved.

### 5.2 Auction Precedence

| Input                          | Effective GDPR scope                                              | Effective TCF / USP                                                                     |
| ------------------------------ | ----------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| Nonempty SID list containing 2 | `Yes`, before existing GDPR enablement gates                      | Active section 2 if available; otherwise legacy consent                                 |
| Nonempty SID list without 2    | `No`                                                              | Section 2 is inactive; legacy consent is retained but not parsed for GDPR authorization |
| Missing/empty SID list         | Legacy `regs.gdpr`, otherwise GDPR enforcement ambiguous/geo/default resolution | Legacy consent; no GPP section active                                                   |
| SID includes 6                 | Unchanged GDPR rule                                               | Nonempty active section 6 if available; otherwise legacy USP                            |
| SID does not include 6         | Unchanged GDPR rule                                               | Legacy USP                                                                              |

Additional cases:

| Condition                                                                           | Required auction behavior                                                                                                                          |
| ----------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| GPP absent/empty                                                                    | No GPP envelope warning; use permitted legacy fallback                                                                                             |
| Malformed envelope or unsupported envelope version, within parsing limits           | Nonfatal GPP warning; values fall back to legacy; independently parsed SID still controls scope                                                    |
| GPP byte/count/ID-range/work limit exceeded, or checked decode arithmetic overflows | Terminal `PrivacyError::GppLimitExceeded` -> auction `400`; no legacy fallback, adapter calls, or outbound sends                                   |
| SID names a section missing from a valid envelope                                   | Legacy fallback for that value; SID remains authoritative for scope                                                                                |
| Section present but not active in SID                                               | Ignore it for local selection and legacy backfill                                                                                                  |
| Active section 2 present but empty/malformed TCF                                    | Unusable consent under GDPR enforcement defaults when GDPR enforced; never retry legacy TCF to obtain a grant                                                    |
| Active section 6 present-empty                                                      | Treat as absent USP and select legacy USP; no empty-value or conflict warning; validate the selected legacy value normally                         |
| Active section 6 present-nonempty but invalid USP                                   | Nonfatal USP warning and no usable effective USP; retain selected-malformed state for enabled CCPA ID/geo scrubbing; do not retry the other source |
| Selected nonempty GPP value differs from a nonempty legacy value                    | GPP wins locally; bounded nonfatal conflict warning                                                                                                |

Within-limit malformed/unsupported-envelope fallback retains the existing
master Appendix D compatibility behavior; it is not a claim of fail-closed
behavior. Limit errors are not malformed-envelope warnings and cannot
reactivate a legacy grant. For example, adding one opaque section to move
from 64 to 65 total sections, with restrictive TCF and granting legacy
consent unchanged, must produce `400`, not different permissions. This holds
even when GDPR/CCPA are disabled or the limited section would otherwise be
inactive.

Tests must distinguish these cases from invalid selected TCF. Preserve GDPR enforcement's
rule that disabled/out-of-scope GDPR does not parse TCF for authorization
or emit malformed-TCF warnings from that inactive authorization path.

### 5.3 Existing TCF Security Applies to GPP TCF Too

Send selected section-2 bytes through the same GDPR enforcement parser, temporal checks,
GCL validation, exact `(gvl_specification_version, vendor_list_version)`
lookup, publisher restrictions, and permission logic as legacy TCF. No
second TCF implementation with weaker acceptance rules is allowed.

For Full and Basic enforcement, a matching `NotAllowed` or conflicting
`RequireConsent` and `RequireLegitInterest` records for the same purpose/vendor
deny that purpose. Matching includes overlapping ranges in separate records.
These denial checks precede disabled enforcement, vendor exceptions,
basic-list bypasses, and the LI-transparency override; record order cannot
pick a granting basis. Duplicate matches of the same type and restrictions on
a different purpose/vendor do not create a conflict.

Basic's Purpose 2 LI-transparency override requires a nonzero resolved vendor
ID. Its other allow rows in master Appendix A.3 remain unchanged, including
disabled-enforcement, vendor-exception, basic-list, and `!enforce_vendors`
allows at vendor ID 0 when reached. Basic still does not consult GVL
declarations or remap the legal basis for a single `RequireConsent` or
`RequireLegitInterest`; only Full does. The owner approved these Basic
safeguards on 2026-10-12 as an explicit Auctioneer policy tightening, not a
normative IAB mandate. Implementation and permission tests must follow the
revised Appendix A.3, replacing the old Basic table/reference behavior
without a legacy compatibility path.

Retain DV membership in a bounded bitset during the existing validated
trailing-segment pass. Do not expand ranges into per-vendor heap objects
or reparse once per bidder. A disclosed vendor has no implicit consent or
legitimate-interest grant. DV membership must not turn on `AllowBidRequest`,
`PassID`, or `PassGeo`; their GDPR enforcement definitions remain unchanged.

Any special-purpose authorization helper must require a usable TCF, the
selected nondeleted GVL vendor declaring that purpose, the purpose existing
in the selected GVL, and membership in DV. Do not wire such a helper as an
OR bypass around Purpose 2 or Purposes 2-10. privacy enforcement adds the membership/checking
contract, not a new special-purpose-only bidding mode. Publisher TC remains
framing-validated but not semantically enforced.

### 5.4 Legacy Projection for Non-GPP Bidders

Use `BidderInfo.openrtb.gpp_supported` from the resolved bidder metadata.
For a GPP-supporting bidder, do not add legacy fields solely for downgrade.
For other bidders:

- Fill `regs.gdpr` only when absent and the SID list is present: `1` when
  it contains 2, otherwise `0`. A present empty list yields `0`, matching
  the reference downgrade even though scope resolution treats it as absent.
- Fill empty/missing `user.consent` only from a present, active section 2.
- Fill empty/missing `regs.us_privacy` only from a valid, nonempty active
  section 6.
- Never overwrite populated legacy fields, even if they conflict with the
  effective policy source. Existing invalid-USP normalization runs first.
- Consent backfill copies the active raw section value, not a new TCF
  serialization. It does not parse TCF when authorization is inactive. A
  copied malformed value stays malformed and never grants local permission.

Local scrub decisions always use normalized policy inputs, not these
projected fields. No GPP field is deleted just because a bidder lacks GPP
support. Legacy projection may add only these privacy signal fields; it
cannot reintroduce identifiers, geo, FPD, or transaction IDs.

### 5.5 Parser Dependency Qualification

Use a qualified parser behind a small `auctioneer-privacy::gpp` module.
The initial candidate, `iab_gpp 0.1.2`, was audited on 2026-09-28 and rejected
for direct adoption; no dependency was added. Its
[published source](https://docs.rs/iab_gpp/0.1.2/src/iab_gpp/v1/mod.rs.html)
uses a closed section-ID enum and materializes decoded header ranges.
The audit also reproduced an incorrect cumulative section-ID result for
header `DBADNb`: expected 2/6/7, actual 2/6/5. See the
[candidate audit](../plans/2026-09-28-privacy-enforcement-verification.md#gpp-candidate-audit)
for exact versions and probe results. Passing its existing examples is not
sufficient evidence of the privacy enforcement contract.

On 2026-09-29 the user selected an internal reference-based envelope module,
with a clean boundary for later extraction into its own crate. The
[module design](2026-09-29-gpp-envelope-module-design.md) records that decision,
the parser/policy separation, and reference-padding qualification. Independent
spec review and self-review passed; the user authorized implementation on
2026-09-29. No new workspace
crate, general-purpose section library, or fork is required now.

The parser workstream must qualify this exact implementation and references:
native/WASM support, supported envelope version 1, raw section extraction,
opaque unknown-ID handling, duplicate rejection, allocation limits, and
bounded range decoding. Do not silently relax the limits, reject
all unknown IDs, or decode every section to fit a convenient library API.
The explicit module decision replaces the earlier preference for an upstream
bounded API or pinned patch; it does not waive parser qualification. This is
a parser workstream gate, not a reason to defer independent scrub/config/activity work.

Initial Auctioneer envelope limits are 256 KiB total raw bytes, 4 KiB encoded
header, 64 section entries, section IDs in `1..=65535`, and 524,288 charged
header-decode operations. Charge before range expansion, iteration, or
allocation. Check raw/header lengths and the raw section count before
semantic header decoding, and cap the decoded ID count before materializing
it. Exceeding a limit, a positive section ID above 65535, or checked decode
arithmetic overflow yields the terminal limit error in section 5.2. Within
those limits, section ID zero, duplicate/out-of-order IDs, and section-count
mismatch are malformed-envelope errors. A dependency must preserve this
distinction instead of mapping all failures to one parse-error category.
Preserve opaque payload slices without decoding/copying them. TCF retains
its existing separate byte, range, and operation limits. These are new
Auctioneer resource limits, not claimed upstream defaults; boundary tests and the
memory derivation must accompany them.

## 6. CCPA, LMT, and COPPA

### 6.1 US-Privacy Validation and Enforcement

USP is exactly four ASCII characters: version `1`, followed by three
characters from `Y`, `N`, `-`. Empty means absent. Do not trim, lowercase,
truncate, or accept an otherwise valid prefix. Only index 2, the sale
opt-out position, triggers the CCPA scrub for a valid value; notice and LSPA
positions do not. A malformed selected nonempty value also triggers enabled
CCPA ID/geo scrubbing under the approved conservative Auctioneer policy.

For section 6, present-empty and absent both select legacy USP. With
`gpp_sid=[6]`, an empty section-6 payload, legacy `us_privacy="1YYN"`, and
CCPA enabled, enforce sale opt-out unless that bidder has a no-sale
exception. If legacy USP is also absent/empty, there is no effective USP.
If legacy USP is nonempty and invalid, emit its validation warning and
clear it. Do not emit a GPP/legacy conflict for an empty section-6 value.
Whitespace is nonempty and invalid, not a fallback trigger. Preserve the
raw GPP payload in all these cases; do not fill section 6 from legacy USP.

Validate nonempty legacy USP even when CCPA is disabled. Clear an invalid
legacy value on bidder-visible requests and emit a nonfatal warning. Parse
selected GPP USP by the same rules without rewriting the GPP string. A
conflict warning never contains either raw value.

Resolve CCPA enablement from the trusted channel:

`account.ccpa.channel_enabled[channel] -> account.ccpa.enabled ->
host.ccpa.enforce -> false`.

An explicit account false wins over host true; an unset account value
inherits. Account-wide `enabled` is also optional/tri-state, not a default
false that masks the host fallback. This middle fallback is present in the
pinned reference's account settings but omitted from master section 7.5;
section 12 records the corresponding master correction. There is no implicit GDPR/geo
dependency or separate sale-opt-out switch.

`ShouldEnforce(bidder) = enabled && (valid_sale_opt_out || selected_usp_malformed)
&& !no_sale_exception(bidder)`.

Keep selected absence/empty, validated value, and malformed nonempty value
distinct. An invalid unselected legacy value cannot override a valid active GPP
value; its sanitation warning still applies. Malformed outer GPP framing retains
legacy fallback and does not itself create malformed selected USP. Never invent
a valid opt-out signal or backfill invalid bytes. Disabled enforcement and exact
no-sale exceptions remain effective. This is Auctioneer enforcement policy, not an
IAB-mandated treatment of invalid syntax.

Only `ext.prebid.nosale` provides the request's no-sale exceptions. It does
not grant activity permission, bypass GDPR, override LMT, or override COPPA.
Do not confuse an exception from a CCPA scrub with consent to all uses.

Match the observable bidder/alias name exactly and case-sensitively, without
core-name fallback. Exempting core `appnexus` does not exempt requested alias
`ax`. Only the singleton array `["*"]` exempts all; a mixed wildcard list
is invalid. Duplicate valid names collapse for lookup. Validate names
against registered bidder names and successfully resolved host/request
aliases, not just bidders participating in this auction. Unknown names,
including wrong-case spellings, are invalid.

Include validated alias identities that alias resolution removes only for
disablement (`ResolvedAliases::dropped_alias_names`), not only the live alias
map. An operator disabling a known alias or its base must not turn an otherwise
valid no-sale list into an auction-wide error. Preserve existing alias-validation
errors, including request aliases targeting baked-disabled bases; this rule
does not make an invalid alias valid. Host aliases dropped for unknown bases
remain outside the namespace. Test disabled-alias and disabled-base cases with
an allowed sibling, including CCPA disabled.

An invalid no-sale list is an auction `400` through `PrivacyError` ->
`AuctionError::BadInput`, even when CCPA is disabled or USP is malformed.
Validate it before permissive policy short-circuits. This deliberate
validation order also covers a malformed USP combined with invalid
exemptions; see section 12 for the narrower compatibility distinction.

### 6.2 LMT and ATT

LMT enforcement is `host.lmt.enforce && prepared_device.lmt == 1`; the host
default remains false. Preserve the existing app-only, case-insensitive
iOS ATT mapping in preparation: ATT 0/1/2 sets LMT 1; ATT 3 sets LMT 0;
other/missing leaves it unchanged. ATT wins over supplied LMT for 0-3.
Do not duplicate that transformation in bidder adapters or infer LMT from
an unrelated activity denial. Web requests do not acquire the app override.

### 6.3 COPPA

`regs.coppa == 1` always enforces its scrub, including when GDPR and host
CCPA/LMT are disabled, no-sale exemptions apply, or the request is NSP.
There is no COPPA toggle, account exception, or bidder exception. COPPA
removes `user.data`, `user.keywords`, `user.kwarray`, and `user.ext.data` even
when `transmitUfpd` is allowed. This stronger user-FPD removal is an approved
Auctioneer policy, not an IAB-prescribed field list. Unrelated site/app/imp context
and transaction IDs retain their independent gates. COPPA also removes every
user/device geo field, rather than only coordinates. Existing
geo objects serialize as `{}`; absent geo stays absent.

## 7. Activity Rules

### 7.1 Model and Consumers

| Activity                   | privacy enforcement consumer                                               |
| -------------------------- | --------------------------------------------------------- |
| `fetchBids`                | Exchange gate before adapter construction                 |
| `transmitUfpd`             | Bidder-local FPD scrub                                    |
| `transmitPreciseGeo`       | Bidder-local geo/IP scrub                                 |
| `transmitTid`              | Bidder-local transaction-ID scrub                         |
| `syncUser`                 | Evaluator covered in privacy enforcement; live sync endpoints remain user sync    |
| `reportAnalytics`          | Config/evaluator only; no analytics delivery added        |
| `enrichUfpd`               | Config/evaluator only; no enrichment implementation added |
| `transmitUniqueRequestIds` | Config/evaluator only; no invented auction-ID scrub       |

Represent all component types: `bidder`, `analytics`, `rtd`, and `general`,
with an explicit name. Update the existing `Component` representation and
callers together. Do not assume the current `Module` variant distinguishes
analytics from RTD or that `Global` is a bidder wildcard.

### 7.2 Evaluation

For one activity, walk configured rules in array order. A rule matches when
all populated clauses match; entries within a clause are alternatives.
Empty/absent name or type lists match all. Matching is case-insensitive.
The first matching rule returns its `allow`, including an explicit allow
before a later deny. No rule match returns the activity default, which is
true when unset. There is no deny-override pass and no GPP condition syntax.

For auction activities, the component name is the observable requested
bidder/alias name, not the core bidder. A core-name rule does not implicitly
cover its aliases; operators can list aliases or use a bidder-type rule.
Use Unicode simple case folding for component-name and component-type matching,
consistent with the pinned behavioral reference. Do not substitute ASCII-only matching,
lowercasing, locale-sensitive folding, full case folding, or Unicode
normalization. In particular, U+212A (Kelvin sign) and ASCII K/k compare
equal, while U+00DF (sharp s) and `ss` do not. Preserve original names for
registry lookup and outbound identity; folding belongs only to activity
comparison. `nosale` remains exact and case-sensitive.

Use a portable simple-fold implementation and record its Unicode data
version and the reference fixture generator's toolchain version. Fixture parity
must cover non-ASCII aliases, not just canonical ASCII bidder names. This
does not introduce an ASCII-only restriction on existing alias inputs.

The canonical effective config shape is:

```text
account.privacy.allowactivities.<activity>:
  default: optional boolean
  rules: ordered list of:
    condition:
      componentName: list of strings
      componentType: list of component types
    allow: required boolean
```

An omitted condition means match-all. A missing `allow` is a configuration
error, not an implicit allow. Unknown activity names, condition keys, or
component types are configuration errors, not ignored policy statements.
Recognize component-type values using the same simple-fold comparison
against the four canonical types, so `BIDDER` is valid and canonicalizes to
`bidder`. Do not broaden case acceptance for activity keys or field names.
Component names are not restricted to currently installed bidders because
the evaluator also serves future consumers and aliases.

### 7.3 Configuration Merge and Bounds

Maintain the existing baked/runtime split and account resolution order.
Add host runtime `ccpa.enforce` and `lmt.enforce`, account CCPA `enabled` and
channel overrides, and account activity overrides. Defaults keep CCPA/LMT
off and activities allowed. Existing GDPR permission rules remain unchanged;
privacy enforcement intentionally replaces the temporary GDPR enforcement GPP-selection behavior with
section 5's source-selection contract.

Reuse the existing `TriState` encoding for account CCPA settings:
`"enabled"`, `"disabled"`, and `"unset"`. An omitted runtime field inherits
its baked value; explicit `"unset"` replaces that value and restores the
channel/account/host fallback at that level. Host switches and activity
`default`/`allow` remain booleans. For example, these are the privacy enforcement runtime
tables in `auctioneer.toml`, not accepted by the starting implementation:

```toml
[runtime.ccpa]
enforce = true

[runtime.lmt]
enforce = true

[runtime.accounts.publisher-1.ccpa]
enabled = "enabled"

[runtime.accounts.publisher-1.ccpa.channel_enabled]
app = "disabled"

[runtime.accounts.publisher-1.privacy.allowactivities.transmitUfpd]
default = false
rules = [
  { condition = { componentName = ["trusted-alias"], componentType = ["bidder"] }, allow = true },
]
```

The corresponding defaults use `[baked.host.ccpa]`, `[baked.host.lmt]`,
`[baked.account.ccpa]`, and `[baked.account.privacy.allowactivities...]`.
CLI push validation and runtime deserialization must use the same DTO
contract; no second schema or signing step is introduced.

Overlay an activity by key. An absent activity inherits the baked value;
within a present activity, absent fields inherit. Explicit `default=false`
must survive deserialization. A supplied `rules` array replaces the entire
inherited array in order; `rules=[]` clears inherited rules. Do not append,
sort, deduplicate, or deep-merge individual rules. CCPA channel fields merge
independently: omitted override fields inherit their baked values; an
explicit `"unset"` replaces the baked value and falls through to the next
enablement level during policy evaluation.

Validate at both baked generation and runtime override boundaries and again
on the effective merged object where cross-field limits apply. Reject an
invalid config through the existing config error path; never fall back to
allow-all because rule parsing failed. The last accepted config remains
governed by existing bounded fanout activation behavior, not a new privacy enforcement cache.

Use initial ceilings of 128 rules per activity, 128 combined name/type
entries per rule, and 128 UTF-8 bytes per component name. Existing global
configuration byte/node limits also apply. Evaluate borrowed/compiled
configuration; do not clone the full rule list per auction or bidder.
Document these new rejection limits in operator configuration references.

## 8. Scrub Contract

The [classified disclosure design](2026-10-09-gdpr-scrub-classification-design.md)
supersedes the original blanket-clearing inventory in this section. Its pinned
standard-field catalogue and supported-extension policy are the field authority;
the policy composition below remains unchanged unless explicitly revised.

All mutations are on a bidder-owned request. The original prepared request
and sibling bidders remain unchanged. Combine decisions monotonically:
an allowed policy never restores a field removed by another policy.

### 8.1 Field Inventory

| Primitive                 | Required fields and treatment                                                                                                                                                                                                                                                 |
| ------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `scrub_gdpr_id`           | Remove classified identifiers and unclassified protected fields from user/device objects and extensions; retain reviewed well-formed context/control fields, including supported device interstitial metadata                                                                 |
| `scrub_user_fpd`          | Identifier/demographic fields above plus `user.data`, `user.keywords`, `user.kwarray`, and `user.ext.data`; preserve unrelated contextual site/app/imp data                                                                                                                   |
| `scrub_geo_and_device_ip` | Keep reviewed well-formed contextual Geo fields and coarsen finite user/device coordinates; remove precise, unclassified, and malformed protected fields in typed and flattened representations; mask `device.ip` and `ipv6` using account keep bits; invalid IPs are removed |
| Combined CCPA/LMT scrub   | Identifier/demographic/EID removal plus geo/IP scrub; not the broader user-FPD scrub unless activity also requires it                                                                                                                                                         |
| Combined COPPA scrub      | Identifier and user-FPD removal plus IP masking, with every field of existing `user.geo` and `device.geo` cleared; existing Geo objects serialize as `{}`, absence stays absent                                                                                               |
| `scrub_tid`               | Remove `source.tid` and every `imp.ext.tid`, including catch-all representation where the field is not typed                                                                                                                                                                  |

The six legacy device keys are `dpidsha1`, `dpidmd5`, `didsha1`, `didmd5`,
`macsha1`, and `macmd5`. Audit both typed fields and flattened `other`
representations. Native OpenRTB fields and their supported legacy aliases
must not provide an alternate route around the same removal decision.
Both EID locations are cleared by every identifier-removing primitive.
This extends GDPR enforcement's native/legacy hardening to privacy enforcement rather than copying the
reference primitives' inconsistent EID coverage (section 12).

Preserve GDPR enforcement's exact floating-point rounding formula
`floor(value * 100 + 0.5) / 100`, not Rust's different tie behavior in
`round()`. Masking uses account IPv4/IPv6 keep bits and retains GDPR enforcement boundary
behavior. COPPA removal dominates all coordinate rounding.

Identifier denial drops all unclassified user fields, including shadow-only
containers, while preserving independently permitted reviewed consent, contextual
data, and geo. Geo denial preserves reviewed context such as country, region,
city, metro, and time-zone offset while removing precise fields and coarsening
coordinates. Classification applies equally to typed and flattened fields;
malformed recognized context cannot shelter identifiers.
Other extension maps are not cleared indiscriminately: preserve unrelated
protocol and bidder metadata outside these denied user/geo surfaces.
This field inventory is not a claim to recognize arbitrary identifiers
hidden in opaque vendor payloads elsewhere.

The [coverage ledger](../plans/2026-09-29-privacy-enforcement-coverage.md#resource-integration-status)
records closure of RES-1 and local integrated resource acceptance. This does not
claim provider deployment certification or physical-isolate memory safety.
Bidder integration review must account for any additional adapter-specific
identity projection. Every supported FPD projection path is scrubbed after
projection and before adapter serialization.

### 8.2 Decision Composition

1. `fetchBids=false` or GDPR `AllowBidRequest=false`: no adapter invocation.
2. `transmitUfpd=false`: apply the FPD scrub; otherwise apply the GDPR ID
   scrub when `PassID=false`.
3. CCPA enforcement adds its combined scrub regardless of an activity
   branch already taken. Do not implement mutually exclusive branches that
   accidentally suppress IP/geo scrubbing.
4. `transmitPreciseGeo=false` or GDPR `PassGeo=false`: apply geo/IP scrub.
5. LMT adds its combined scrub. COPPA adds user-FPD removal and the stronger full-geo scrub.
6. `transmitTid=false`: apply the TID scrub independently.

Every primitive must be idempotent. Applying multiple policies must equal
the union of required removals with the most restrictive geo treatment.
Keep required OpenRTB request/imp IDs: `transmitTid` is not permission to
destroy request correlation or silently activate `transmitUniqueRequestIds`.

## 9. Errors, Observability, and Resources

Reuse `PrivacyWarning` collection and the existing response mapper.
Within-limit malformed/unsupported GPP envelopes use warning code 10001
with `regs.gpp` field scope. GPP limit failures instead propagate a typed
`PrivacyError::GppLimitExceeded` into `AuctionError::BadInput` and the existing
HTTP `400` handler path before bidder construction. Use the fixed diagnostic
`regs.gpp exceeds privacy parsing limits`; keep the limit kind as a closed
internal enum, not request-derived text. Do not downgrade the error to a
warning because legacy consent is available or enforcement is disabled.
Invalid USP and GPP/legacy conflicts use the corresponding nonfatal
validation-warning category (10001). Existing TCF/GVL/GCL warnings keep their GDPR enforcement
codes and fail-closed decisions. A privacy denial is not an HTTP failure
of an otherwise valid auction; unaffected bidders can still return bids.

Use the existing privacy-blocked response representation for either fetch
gate: bidder warning 10017, `dispatched=false`, and seat-nonbid status 300
per covered impression, subject to the existing response visibility rules.
If both gates deny, record one bidder exclusion, not duplicate rows. Evaluate
activity first for the internal reason; do not label an activity-only block
as a GDPR enforcement metric. Invalid `nosale` is also a hard input error
and has a fixed field-scoped diagnostic without echoing arbitrary text.

Warnings are request-normalization outputs, not one duplicate per bidder.
Deduplicate by bounded reason and field, with stable ordering. Do not emit
consent strings, IPs, user IDs, raw GPP payloads, or adapter response bodies
in diagnostics. Use closed error enums and fixed templates, not arbitrary
third-party parser error strings. Do not add high-cardinality metric labels
for component names, consent versions, or raw request fields.

Carry source provenance into warning construction. Legacy TCF diagnostics
use `user.consent`; legacy USP diagnostics use `regs.us_privacy`. For selected
GPP TCF/USP failures use scope `regs.gpp` and fixed messages naming section 2
or 6; do not blame a valid unselected legacy value. GPP/legacy value conflicts
also use `regs.gpp` with distinct section-specific reasons. Preserve
`regs.gpp_sid` for the existing GDPR-scope conflict and scope-free GCL freshness
diagnostics. Deduplicate by source/section/reason, not just numeric code.
`PrivacyContext` debug output contains only bounded structural summaries,
not decoded consent, alias values, configuration rules, or warning messages.
Malformed-consent warning constructors must not accept arbitrary error text
for interpolation; normalized source provenance selects a fixed diagnostic.
Emit normalization warnings in fixed groups: wire GDPR signal, envelope,
legacy USP validation, value conflicts (TCF then USP), selected-value validation
(TCF then USP), and remaining GDPR authenticity/GVL diagnostics. Within each
group use fixed reason order, never bidder iteration order. Fixture assertions
must cover simultaneous legacy/GPP failures and conflict warnings.

privacy enforcement must extend bounded fanout's memory accounting for retained DV membership, decoded
GPP header state, selected-value storage, warning storage, activity config,
and scratch used by transformations. Account for simultaneously live
original/bidder-projected/serialized requests rather than adding a nominal
parser limit and declaring the whole auction bounded.

Runtime-config reservations must use the extractor's effective caps. A nonzero
declared byte or JSON-node cap is applied to extraction; zero selects the SDK
default (8,192 nodes). Boot resolves these defaults before charge/floor validation
and retains the same effective row for per-App extraction and diagnostic
rederivation. Lower node caps must reject at their declared inclusive boundary;
higher caps must admit valid envelopes above the default without exceeding the
installed charge. Shipped rows retain their existing cap.

All four shipped adapters currently declare the SDK's 8,192-node cap, with no
operator TOML override. Honoring a different declaration is a host-embedding
contract, not a change to today's adapter configuration surface. Deliberate
warn-and-boot behavior above the isolate ceiling remains in place; accounting
tests must include a realistic ceiling where the node population alone changes
whether boot warns, and pin the node unit cost independently of the derivation.

Do length/count/work checks before dependency allocations. Reuse borrowed
raw slices where ownership permits; retain at most one authorization TCF
parse per auction. Checked arithmetic and pre-allocation reservations are
required on native and 32-bit WASM. Auction resource exhaustion must use the
terminal runtime refusal contract, not masquerade as a harmless parse error
and resume with a less restrictive policy. The inspected `4d830f0` basis has
concurrency admission and boot-envelope checks, but no general per-auction
byte-reservation API; that integration contract must be completed before
RES-1 can pass.

The baseline does not yet prove these resource guarantees. bounded fanout owns wiring
the scheduler and bounded construction sinks into the handler, retaining the
admission lease for the complete auction lifetime, and correcting retained-node
memory charges. privacy enforcement adds its measured state to those corrected paths and preserves
their terminal refusal mapping. RES-1 must exercise the integrated handler,
including concurrent auctions and failed reservations, not only parser helpers.
This is an integration/completion dependency, not a gate on independent privacy enforcement work
or a new provider-certification requirement.

## 10. Verification Matrix

Fixtures must assert both the decision and captured outbound bytes. Tests
that only call the policy evaluator do not prove enforcement.

| ID      | Owner                 | Required coverage                                                                                                                                                                                                                                                                                                                                          |
| ------- | --------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| GPP-1   | privacy               | Absent/empty/valid/bad envelope; version/type/count/padding/duplicate/range errors; distinguish within-limit format errors from terminal limit/overflow errors                                                                                                                                                                                             |
| GPP-2   | privacy               | SID absent, empty, 2, 6, both, unsupported; missing section; inactive section; malformed envelope with valid legacy; malformed active TCF with granting legacy                                                                                                                                                                                             |
| GPP-3   | privacy/exchange      | Conflicts and warning deduplication; unknown opaque sections; malformed unused section does not poison 2/6                                                                                                                                                                                                                                                 |
| GPP-4   | exchange              | GPP-capable versus noncapable bidders; missing/empty/populated legacy fields; present-empty SID downgrade; raw TCF copying cannot change local permissions; invalid USP is not injected                                                                                                                                                                    |
| GPP-5   | privacy/core/exchange | Every envelope cap at/over its boundary; over-limit/overflow is 400 with zero adapter calls/sends despite granting legacy consent; 64-to-65 total-section transition with unchanged restrictive TCF cannot reactivate legacy grants; inactive-policy and disabled-enforcement controls                                                                     |
| TCF-1   | privacy               | Legacy versus GPP-selected TCF yields identical permissions; every existing GDPR enforcement hardening gate still applies                                                                                                                                                                                                                                                |
| TCF-2   | privacy               | DV bitfield/range membership, upper vendor ID, malformed/overlapping ranges, unknown/deleted/undisclosed vendor, no implicit grant from disclosure                                                                                                                                                                                                         |
| CCPA-1  | privacy               | All USP flag combinations, lowercase/Unicode/length errors, only index 2 effective, host/account/trusted-channel precedence                                                                                                                                                                                                                                |
| CCPA-2  | privacy/exchange      | Empty/name/wildcard no-sale lists, aliases including validated disabled identities, case behavior, unknown names, no-sale does not override other policies; malformed USP plus invalid no-sale is 400                                                                                                                                                      |
| CCPA-3  | privacy/exchange      | Conflicting active GPP sale-opt-out versus legacy USP still enforces from GPP; conflict plus invalid no-sale is 400; warnings cannot disable enforcement                                                                                                                                                                                                   |
| CCPA-4  | privacy/exchange      | Active empty section 6 plus legacy sale-opt-out enforces and scrubs outbound bytes; absent/empty and unselected invalid USP do not trigger CCPA; selected nonempty invalid/whitespace USP triggers enabled ID/geo scrub without fallback or fabricated projection; disabled/channel/account and exact no-sale controls; warning order and counts unchanged |
| ACT-1   | privacy               | All eight activities, all four component types, default/match-all/AND/OR, case folding, first allow before deny and reverse order                                                                                                                                                                                                                          |
| ACT-2   | core/config           | Missing versus false, inherited versus empty rule array, key/type rejection, baked/runtime equivalent lowering, limit validation                                                                                                                                                                                                                           |
| ACT-3   | privacy/core/config   | Unicode upper/lower and Kelvin/K/k equivalence; sharp-s/ss non-equivalence; deny rule cannot miss a simple-fold-equivalent alias; allow-rule and default-deny controls; mixed-case component types canonicalize; nosale remains exact                                                                                                                      |
| SCRUB-1 | exchange              | Every inventory field in typed and supported legacy locations; missing objects, retained unrelated extensions, idempotence, sibling isolation                                                                                                                                                                                                              |
| SCRUB-2 | exchange              | Cross-policy combinations, CCPA under both activity denials, GDPR off, NSP, LMT off/on, COPPA unconditional ID/full-Geo/four-branch user-FPD removal despite activity allow and no-sale, full geo beats rounding                                                                                                                                           |
| SCRUB-3 | exchange              | IP keep-bit edges, malformed IPs, floating-point ties and negative-coordinate reference divergence, repeated scrubbing, multiple impressions, preserved ordinary request/imp IDs                                                                                                                                                                           |
| ATT-1   | openrtb/exchange      | App iOS casing and ATT 0-3, missing/other, contradictory LMT, web/non-iOS controls, capture after preparation                                                                                                                                                                                                                                              |
| E2E-1   | exchange/core         | Denied fetch has zero adapter calls/sends; one denied bidder plus allowed sibling; all bidders denied uses existing empty-auction response                                                                                                                                                                                                                 |
| E2E-2   | exchange/bidder       | FPD/buyeruid/legacy projection ordering, multi-request adapter, no raw-IP header resurrection, per-bidder outbound capture                                                                                                                                                                                                                                 |
| RES-1   | privacy/runtime       | Bounded parser work/allocation under hostile ranges, retained-state budget charge, config evaluation bounds, concurrent-auction budget regression                                                                                                                                                                                                          |
| PORT-1  | CI/adapters           | Existing native/WASM build matrix, unit/integration tests, target dependency features and no new runtime/network dependency in privacy                                                                                                                                                                                                                     |
| SYNC-1  | core                  | Sync routes remain unavailable in privacy enforcement; evaluator support is not endpoint activation                                                                                                                                                                                                                                                                         |

Reference-comparison fixtures use the pinned upstream revision for overlapping
behavior. Differences in section 12 have separate explicit expected
outcomes; never update a reference snapshot simply to bless a discrepancy.
Fuzz the GPP wrapper and existing TCF parser separately, then the complete
normalization path. Assert no panics, unbounded expansion, duplicate warning
growth, or alternate-source grant after invalid authoritative TCF or a GPP
limit refusal. Use a reduced injected operation ceiling to exercise work
exhaustion without huge fixtures. Runtime reservation exhaustion must remain
a terminal runtime error, distinct from input-limit `400` and format warnings.

## 11. Delivery and Acceptance

Implementation can start without waiting for additional provider
certification. Keep the following workstreams separately reviewable inside
the existing crates:

1. Qualify the GPP parser and capture pinned-reference fixtures.
2. Add typed config/overrides and the complete activity evaluator.
3. Add GPP/USP normalization, independent CCPA/LMT/COPPA state, and DV retention.
4. Integrate the exchange gates, scrub matrix, and legacy projection.
5. Add outbound-capture, adversarial/resource, and cross-target verification.

This list is not a task-by-task implementation order. See the
[privacy enforcement implementation plan](../plans/2026-09-28-privacy-enforcement.md) for dependency order
and independently testable checkpoints.

privacy enforcement is complete only when all matrix rows have automated evidence, unaffected
GDPR enforcement regressions remain green, config defaults preserve behavior outside the
explicit privacy enforcement changes, no privacy stub controls a live auction activity, and
privacy enforcement state is included in resource accounting. Replace temporary GDPR enforcement GPP
expectations with the section 5 precedence matrix: specifically,
`gpp_sid_section2_with_granting_user_consent_yields_passid_false` currently
nullifies legacy consent solely because SID 2 is present and decoding is
unimplemented. Its absent-envelope case must now exercise legacy fallback;
retain separate negative controls proving empty/invalid authoritative TCF
and GPP limit errors cannot reactivate that grant. Do not weaken unrelated
GDPR enforcement authorization tests to accommodate the new parser.

Update the master spec's privacy enforcement status and divergence table, operator config
examples, and relevant crate/module documentation.

Do not claim privacy enforcement compliance merely because `cargo test` passes, or wait for
user sync to finish an auction-path scrub. Provider deployment evidence remains an
bounded fanout concern unless privacy enforcement changes invalidate that evidence; such changes must
rerun the affected provider checks, not invent a new signing requirement.

## 12. Explicit Compatibility Decisions

The approved design and re-review use these explicit decisions:

1. **Inactive GPP section 2:** follow the master's inactive-section rule:
   ignore it for selection/backfill. The pinned reference selects decoded
   section 2 and backfills legacy consent without requiring active SID
   membership. Auctioneer requires active membership for both. This is an
   intentional Auctioneer difference, not exact parity.
2. **Unenforced sections stay opaque:** do not parse sections 7-12 just to
   validate the container. The pinned reference decodes those payloads and
   may retain a partial container when a payload fails validation.
   Auctioneer separates envelope validity from inner policy validity and does
   not infer US state-policy support from successful pass-through.
3. **Account-wide CCPA fallback:** add `account.ccpa.enabled` between channel
   and host settings, matching the pinned reference's account model. The
   master summary omitted this layer; omission must not become a permanently
   incompatible configuration model.
4. **Hard bounds and validation:** the envelope/activity limits and strict
   unknown config-key rejection are explicit Auctioneer restrictions. Only
   within-limit malformed/unsupported envelopes retain the master's legacy
   fallback. GPP limit violations and decode arithmetic overflow instead
   reject the auction with `400`, even if legacy consent could grant; these
   are not permission decisions. Resource reservation exhaustion separately
   retains the runtime's terminal error behavior. Update master Appendix D
   to name this distinction, with GPP-5 as its regression contract.
5. **DV does not widen permission grants:** retaining disclosed vendors
   fulfills the privacy enforcement storage/checking prerequisite; special-purpose-only
   bidding is not silently added to the master's defined permission matrix.
6. **No-sale validation ordering:** the pinned reference validates CCPA
   before exchange: an ordinary invalid no-sale list already
   produces a fatal request error. Its parser, however, checks consent
   before no-sale and returns early on invalid consent. Auctioneer validates
   the list independently and returns `400` even for the combined
   malformed-USP/invalid-no-sale input. The ordinary `400` is parity, not
   new hardening; the combined-input error precedence is the intentional
   difference. Host aliases are accepted when Auctioneer's registry has
   successfully resolved them; fixtures must not assume identical alias
   namespaces between projects.
7. **Consistent EID removal:** the reference omits native EIDs in its
   GDPR/CCPA/LMT/COPPA projection and legacy EIDs in
   its FPD primitive. privacy enforcement removes both representations for each applicable
   identifier scrub, extending an already adopted GDPR enforcement hardening decision.
8. **USP conflicts do not disable CCPA:** Auctioneer warns and continues using
   the selected active GPP value, as intended by master Appendix D. The
   reference warns on conflict and skips CCPA enforcement on that warning.
   Auctioneer does not reproduce this warning-induced loss of enforcement.
   No-sale validation remains independent: conflict plus invalid no-sale
   returns `400`. CCPA-3 locks down both observable differences.
9. **Empty USPV1 is not invalid nonempty USPV1:** follow the pinned reference's
   US-Privacy selection behavior: an empty active section 6 falls back to legacy USP,
   without a conflict warning. A nonempty invalid value does not. This
   clarification must appear in master Appendix D as well as CCPA-4; it
   does not relax the no-fallback rule for empty/invalid active TCF.
10. **Activity comparison matches Unicode-aware reference behavior:** simple
    case folding applies to names and type values, without changing registry
    identity or no-sale equality. ASCII-only comparison is not an accepted
    shortcut. ACT-3 verifies both equivalence and non-equivalence cases.
11. **Retain GDPR enforcement's negative-coordinate rounding:** Auctioneer uses
    `floor(value * 100 + 0.5) / 100`. The pinned reference truncates toward
    zero instead. For `-1.238`, the
    respective results are `-1.24` and `-1.23`. Preserve Auctioneer's idempotent
    behavior and include separately labeled reference/Auctioneer expectations
    for negative values and repeated scrubbing; do not report exact parity
    or change the established GDPR enforcement formula to reproduce the reference drift.
12. **COPPA removes user FPD:** approved on 2026-10-12. The ID/full-Geo scrub
    additionally removes all four named user-FPD branches, even with activity
    permission or no-sale exceptions. Preserve reference output as evidence;
    label the stricter Auctioneer expectation explicitly.
13. **Malformed selected USP is restrictive when enabled:** approved on
    2026-10-12. Invalid nonempty selected legacy/GPP USP warns and activates
    enabled CCPA ID/geo scrubbing, subject to exact no-sale exceptions. Absent,
    empty, valid non-opt-out, and unselected invalid values remain distinct.
    Neither this policy nor decision 12 is asserted to be an IAB requirement.

## 13. Verification at Draft Time

The clean `privacy-enforcement` worktree was created from the revision in section 2.
Before writing this document, the existing privacy library suite was run:

```sh
cargo test -p auctioneer-privacy --lib --locked --offline \
  --target-dir /private/tmp/auctioneer-privacy-enforcement-baseline-target
```

Result: 217 passed, 0 failed, 1 ignored. This is a baseline regression check,
not verification of the unimplemented privacy enforcement requirements. No production code,
dependencies, or implementation plan are changed by this document.

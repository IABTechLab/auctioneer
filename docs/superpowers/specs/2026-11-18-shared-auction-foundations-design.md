# Shared Auction Foundations

Date: 2026-11-18
Status: Written specification approved by the user for implementation planning on 2026-11-18. This is not implementation, publication or merge authorization.
Issue: [#36](https://github.com/IABTechLab/auctioneer/issues/36)
Related delivery: [#54](https://github.com/IABTechLab/auctioneer/issues/54), [#55](https://github.com/IABTechLab/auctioneer/issues/55), [#56](https://github.com/IABTechLab/auctioneer/issues/56), [#57](https://github.com/IABTechLab/auctioneer/issues/57), [#79](https://github.com/IABTechLab/auctioneer/issues/79), [#59](https://github.com/IABTechLab/auctioneer/issues/59)

## 1. Purpose And Scope

Freeze the common contracts for response-currency conversion, multibid, stored bidder-UID projection and one host-wide protocol mode. The deliverable for #36 is a reviewed specification and ordered implementation plan, not four simultaneous implementations. Existing child issues retain their implementation and acceptance ownership. A planning merge does not close those issues or make a provider qualified.

Use one validated runtime operator policy, one settings-first auction API, one bidder registry and one implementation per bidder. Keep parsing and policy decisions shared between standalone and embedded execution. Do not add a configuration loader, a crate, bidder-name switches, a second engine or a legacy compatibility implementation.

The mode's compatible behavior is a small, enumerated normalization policy inside the same implementation. It is not a format-1 reader, an old preparation wrapper or permission to revive removed configuration paths.

The user's hard-cut instruction applies to the readers and protocol behavior changed here: replace the old path and its active expectations atomically; do not preserve it behind a legacy parser, alias, feature or fallback. Immutable external inputs remain evidence, not a promise to keep accepting their old behavior. Section 9 defines how a deliberately tightened contract changes a fixture's checked outcome without dropping that fixture.

### 1.1 Verified Baseline

Source baseline: Auctioneer `d26fcf7f566db6d1dce2e1847d79c9f44581edb8`.

| Surface | Present code | Change proposed here |
| --- | --- | --- |
| Rates | `auctioneer-config/src/currency.rs` defines `FloorCurrencyConfig`; `auctioneer-bidder/src/currency.rs` borrows its expiring USD-per-unit table for floors only. | Share one coherent rate owner with response conversion; do not call floor conversion proof a response-price proof. |
| Response currency | `exchange/src/run/mod.rs::process_one_result` drops non-USD responses; `exchange/src/response.rs` emits USD when bids exist. | Convert surviving candidates into one auction currency before comparing prices or producing targeting. |
| Multibid | `openrtb/src/ext/request_prebid.rs` retains an untyped value. `exchange/src/split/root.rs` currently copies that entire control. | Validate once, select the recipient's control before copying and rank/limit candidates deterministically. |
| Identity | The signed cookie reader, keyring and owner-local UID exclusions exist under `core/src/usersync` and discovered bidder metadata. The auction API's `RequestHeaders` has no cookie input. The flattened `user.ext.prebid.buyeruids` map is copied to recipients without protected disclosure. | Strip that request-wide map in the early containment unit below, then separately add bounded auction identity input and recipient selection; do not claim sync endpoints already perform auction ingestion. |
| Mode | The roadmap defines `strict` and `reference_compatible`; there is no resolved production mode in `AuctionContext`. | Add exactly one validated host setting and capture it once per operation. |
| Result lifetime | The public API owns prepaid funding through preparation, execution and response handoff. | New retained state remains under those same owners; no unfunded result or detached snapshot. |

The existing `UidCookie::uid` applies owner exclusions but does not check expiry. Auction ingestion must use `is_live(family, captured_now)` before selecting that UID; merely calling `uid` is insufficient.

### 1.2 Standards And Reference Authority

The normative OpenRTB source remains the repository's [2.6-202606 capture](../../../crates/auctioneer-config-data/data/openrtb-2.6-source.toml), commit `1debba8001f40eb14597102dec3205afddf811d4`. The pinned behavioral reference remains Prebid Server `d7f8515b86258688304b0d9b6668c6a0e258bc9e`. This design changes neither pin.

[OpenRTB 2.6](https://github.com/InteractiveAdvertisingBureau/openrtb2.x/blob/1debba8001f40eb14597102dec3205afddf811d4/2.6.md) specifies accepted request currencies, a response-wide currency, CPM prices and a buyer-specific user identifier. It does not specify an exchange-rate feed, freshness policy, the Prebid multibid extension, callback ownership authentication or an Auctioneer protocol-mode setting. Those are explicitly product contracts below, not IAB requirements.

Reference implementation anchors are [currency handling](https://github.com/prebid/prebid-server/blob/d7f8515b86258688304b0d9b6668c6a0e258bc9e/exchange/bidder.go), [multibid normalization](https://github.com/prebid/prebid-server/blob/d7f8515b86258688304b0d9b6668c6a0e258bc9e/openrtb_ext/multibid.go), [ranking](https://github.com/prebid/prebid-server/blob/d7f8515b86258688304b0d9b6668c6a0e258bc9e/exchange/auction.go) and [UID selection and extraction](https://github.com/prebid/prebid-server/blob/d7f8515b86258688304b0d9b6668c6a0e258bc9e/exchange/utils.go). The pinned source, not a changing documentation example, settles a claim of reference equivalence. Do not copy its interpolated diagnostics, ambiguous-map ordering or unrelated implementation quirks.

### 1.3 Non-Goals

This design does not deliver external creative/VAST caching, new bidder protocols, bid-floor rules or bid adjustments, FPD merging, additional GPP policies, callback authentication, provider deployment or full schema/resource certification. It specifies the interfaces those features must preserve. Do not block the independent #244 planning review or implement its stored-object changes on this branch.

## 2. Ownership And One Operation Snapshot

| Owner | Responsibility |
| --- | --- |
| `auctioneer-config` | Strict operator models, the mode enum and a single rate-snapshot model; static validation through existing `validator`/custom validation patterns. |
| `auctioneer-openrtb` | Typed wire controls and field metadata; presence-aware types where absence changes behavior. No rate fetching or cookie key access. |
| `auctioneer-core` | Capture accepted settings, request time, identity input and rate ownership once; bounded signed-cookie reading through existing secrets; settings-first standalone/embedded orchestration. |
| `auctioneer-exchange` | Resolve controls against accepted aliases and participants, recipient projection, privacy composition, shared response conversion and deterministic ranking/targeting. |
| `auctioneer-bidder` | Existing protocol encoding/decoding, source currency and owner-local UID exclusions. No duplicated auction-wide rate selection or winner policy. |
| Existing runtime/adapter owners | Original deadlines, host cooperation, admission, receipts and provider bindings. No adapter-local business policy. |

Extend the existing plan/context/request-info fields only where their consumers need them. A new public service trait is not justified by a single pure transformation. Raw JSON input and typed input use the same admitted controls. Typed callers cannot skip byte/node/depth validation, duplicate-sensitive controls or resource ownership by constructing a Rust value directly.

One operation captures one mode, one effective policy, one wall-clock instant, one selected rate table and one admitted identity state. Hot updates affect the next operation; they do not change an active auction between aliases, source responses or targeting. No helper rereads configuration, a rate service or cookies per bidder. Owned state needed after the full policy is released is projected once under funding, not borrowed through an invalid lifetime or cloned per bidder.

This specification follows the current shared preparation/execution boundary. If #244 lands first, integrate into its single asynchronous preparation function; otherwise do not create a second public preparation function in anticipation of it. Rebase and review the ownership/layout changes before activation, not merely rename a call to keep both paths.

## 3. Ordered Pipeline

### 3.1 Requests

1. Admit input and configuration under the existing byte/node/depth limits, original deadline and receipts. Retain existing error precedence.
2. Perform supported stored expansion, then canonical OpenRTB normalization/validation. Unsupported features remain unsupported; this item does not add expansion before #244's implementation exists.
3. Resolve aliases and actual participants. Parse currency and multibid controls once; reject or normalize only the reviewed differences in section 7. No control creates participation.
4. Capture the operation's effective currency and rates. Resolve bounded UID candidates from the admitted input. Cookie parsing/key access is conditional on actual eligible use, not performed for an already refused operation.
5. Build each recipient's request, selecting its control and identity before copying. Apply existing FPD behavior, then all composed privacy/activity decisions. Identity resolution never runs after the final scrub.
6. Run the bidder's normal validation/construction and bounded transport. Recheck the original deadline at existing host-cooperation checkpoints; no new I/O receives a fresh auction allowance.

### 3.2 Responses

1. Decode through the existing sealed byte/node-budgeted response path and classify media. Preserve valid siblings on bid-local errors.
2. Check actual sent-impression membership and candidate viability: finite nonnegative CPM, zero allowed only with a nonempty deal ID, and the required identity/media fields. Preserve #192's explicit zero-price-deal exception. An invalid high-price candidate cannot win and suppress a valid sibling. Coordinate with [#192](https://github.com/IABTechLab/auctioneer/issues/192) at this owner; do not add a second viability implementation.
3. Convert each candidate's CPM into the single auction currency, retaining source-price provenance. Drop an unconvertible candidate rather than relabeling it or rejecting unrelated successful bidders.
4. Rank and apply the multibid rule per requested public bidder and impression. Reconstruct output from retained candidates; never retain stale winner indices into a truncated/reordered vector.
5. Select overall winners from those candidates, then build price buckets and targeting in the auction currency. Browser-auction configuration remains a separate bounded channel, not synthetic monetary bids.
6. Move retained output through the existing response builder and funded handoff. Do not produce a second deep copy of creative/extension data for ranking.

Floor enforcement and adjustments remain their own issues. When implemented, eligibility and monetary adjustments must occur in the reviewed common-currency order before ranking; neither may compare differently labeled values or restore a rejected candidate.

## 4. Currency And Freshness

### 4.1 One Currency For The Auction

Select USD when request `cur` is absent or empty. Otherwise validate every supplied code and choose the first entry as the auction currency, before any bidder returns. This first-entry policy is Auctioneer's deterministic selection from the caller's acceptable list, not an assertion that OpenRTB limits the list to one item. Do not select a different response currency for each bidder or silently fall back to USD when the selected currency cannot be funded/converted.

Codes use the existing three-uppercase-ASCII-letter representation. The rate map's shape check is not a new ISO currency registry; a code with no usable conversion is unsupported even if syntactically valid. Request-shape errors are request errors. An absent response currency uses the OpenRTB default USD. The existing adapter result's empty currency also continues to mean USD in both modes; this normalization is an explicit Auctioneer policy, not a newly claimed standards requirement. Whitespace, arbitrary text and case folding do not turn a foreign code into USD.

Adapters supply the currency of the source response, not the requested currency guessed from the request. Different calls from the same bidder may have different source currencies; converting each call before merging prevents loss of that fact.

This is an end-to-end hard cut, including decoders that currently erase a declared response currency. Remove those resets and their fixed-USD active expectations in the same delivery as exchange conversion; they are not grandfathered by reference fidelity or section 7's otherwise-unchanged protocol rule. Preserve every nonempty declared code through the adapter boundary for shared validation/conversion, including invalid codes for fixed refusal rather than silent USD substitution. Only the absent/empty default above remains. Bidder-owned tests prove the decoded code and unchanged source amount; shared fake tests independently prove conversion. A fake that already supplies the right currency cannot prove a real decoder stopped discarding it.

When bids are returned, response `cur` equals the selected auction currency. Preserve the existing empty-`cur` convention on a response containing no bids. No numeric bid survives with an unconverted source price under the selected response label.

### 4.2 Reuse And Generalize The Existing Snapshot

Use one USD-per-unit table, implicit USD rate 1, finite strictly positive foreign rates and exclusive `expires_at`. Preserve the existing 256-rate limit and the enclosing configuration extraction limits. A table is fresh only when `captured_now < expires_at`; equality is expired. Retain the existing distinction between static shape validation and time-aware operational admission. Do not claim that CLI shape validation establishes freshness forever.

At response-conversion activation, replace the operator `floor_currency` field with one `currency` field using the generalized snapshot model. Do not accept both names, create precedence between two operator rate tables or retain a legacy reader. The application-format change and examples are one coordinated hard cut from the then-current schema; #244's proposed format number is not reserved or reused by this document. The implementation plan must name the actual base format and new value after rebasing. Shared floor-conversion callers use the same captured table and retain their protocol-local refusal scope.

An explicitly configured stale snapshot remains an operational policy-admission failure under the existing time-aware validator. A low-level/programmatic operation that reaches conversion without a usable table cannot convert foreign amounts. Identity conversion from a currency to itself remains exact and requires no rate; it does not override an earlier invalid-policy refusal. These two boundaries need different tests, not one fixture that fails before reaching conversion.

External rates are data, not another operator configuration. In #54, a bounded operator-side import/refresh producer may obtain an external table, validate it and publish a complete replacement through the existing config validate/diff/push path. Serving operations do not fetch rates, start a background refresh task, maintain an independently authoritative cache or combine a published table with live feed entries. The producer must preserve existing URL/secret protections and receive its own reviewed input/work/deadline bounds before implementation. This spec approves no new importer command, endpoint or numeric reservation. Until that producer is delivered, external-feed acceptance in #54 remains open; manual snapshots do not count as its implementation.

Request-supplied `ext.prebid.currency.rates` and `usepbsrates` are not accepted in this initial contract in either mode: a present unsupported override returns a fixed request error before dispatch. This is an intentional limitation relative to the reference, not silent host precedence or a false claim of full currency parity. Consumed currency controls are removed from recipient requests; bidders receive the selected request `cur`, not a competitor/global rate map.

Enforce that presence rule before generic-map insertion can discard a duplicate. The existing bounded ingress must reject repeated decoded control-bearing slots along root `ext`, `ext.prebid`, `ext.prebid.currency` and its `rates`/`usepbsrates` members, including equal values, nulls and escaped-equivalent keys. In particular, an earlier currency object containing `rates` cannot be hidden by a later empty currency object. Use the same admitted result downstream; checking only a deserialized `other` map or the later recipient projection is insufficient. Integrate with the existing direct typed decode or #244's admitted tree builder, whichever is actually present, without another whole-request parser. Typed callers validate their supplied representation but cannot recover duplicate syntax already discarded outside Auctioneer.

### 4.3 Arithmetic And Provenance

For source `S`, target `T` and USD-per-unit rates `rS`/`rT`, conversion is `price * rS / rT`, with USD rate 1. Source-equals-target is bit-preserving identity after price admission. For foreign conversion, check each operation for nonfinite output and reject positive underflow to zero. The initial implementation may conservatively refuse an overflowing intermediate even if arbitrary-precision reassociation could represent the final real number; record and test that fixed policy rather than add an unreviewed numeric dependency.

This draft proposes retaining the admitted `f64` wire model and checked arithmetic already used for floors. The captured OpenRTB specification recommends integer/decimal money arithmetic even though its wire price type is float. Retaining binary floating point is therefore an explicit proposed departure from that recommendation, not a claim that float multiplication is exact settlement arithmetic. User approval of this choice is required before the #54 plan freezes it. No new decimal library, implicit quantization, precision loss hidden by bucket rounding or hand-written arbitrary-precision engine is authorized here. Zero-price deals remain zero through conversion when the required foreign rates are usable; positive-underflow rejection must not reject an admitted zero-price deal.

Compare full admitted converted CPMs. Do not round to the targeting bucket before winner selection. The existing `price_bucket` policy remains the final formatting/rounding owner. Example exact controls: 4 units at USD-per-unit 1.25 becomes USD 5; USD 6 to a target rate of 2 becomes target 3; 4 units at source rate 1.5 and target rate 2 becomes target 3. Include reverse directions; USD-to-foreign is not multiplication by the foreign rate.

The exchange, not a bidder, owns `origbidcpm`/`origbidcur` for converted candidates. They record the admitted pre-conversion value/code and replace conflicting provider-supplied annotations. For identity conversion, clear untrusted conversion annotations instead of presenting them as exchange evidence. The output `price`, comparison value and targeting all use the same converted CPM. Original-price annotations are not an authenticated commercial offer or a permission grant.

### 4.4 Failure Scope

| Failure | Outcome |
| --- | --- |
| Invalid static operator snapshot or mode | Existing configuration refusal; no bidder/source dispatch. |
| Explicitly expired configured snapshot at operational admission | Existing time-aware configuration refusal, not a last-known-good fallback. |
| Malformed request `cur` or unsupported request rate override | Fixed invalid-request category before bidder work. |
| A source/target rate is missing or unusable at conversion | Drop affected monetary candidates, preserving other calls/bidders and diagnostics. |
| Invalid source price, conversion overflow or positive underflow | Bid-local refusal; never clamp to zero or infinity. |
| Deadline/resource refusal | Existing typed deadline/resource path; do not recast as a missing exchange rate. |

Use existing bounded wire error/warning categories where they apply; any new numeric allocation needs the owning code-registry review in the child plan. Messages are fixed categories and do not echo rate tables or untrusted strings.

## 5. Multibid

### 5.1 Input And Recipient Ownership

Replace the untyped control with a typed list of `bidder` or `bidders`, required integer `maxbids` and optional `targetbiddercodeprefix`. Use server wire spellings, not the JavaScript API's camelCase names. An absent/null control or an empty array means no rules; other non-array controls fail. Strict mode requires exactly one selector field to be present: either a nonempty bidder string or a nonempty list of nonempty bidder strings. It also requires a unique accepted public name per expanded rule and `maxbids` in 1 through 9. The upper bound is the reference extension's protocol limit, not permission to enlarge any existing response/work capacity.

An absent/null/empty prefix means no extra label prefix. A nonempty prefix and every resulting `prefix + rank` label must fit the existing `MAX_BIDDER_NAME_BYTES` byte bound. Use the shared UTF-8-safe key builder for final truncation, not a second string-truncation implementation. The section 7 group-prefix rule concerns nonempty prefixes. Missing/null/noninteger `maxbids`, invalid selector types and unknown fields in a multibid rule are fixed request errors in both modes unless explicitly normalized by that table.

Separate structural decoding from semantic normalization without retaining a legacy representation: the one typed wire reader requires the integer `maxbids`, rejects unknown/duplicate rule fields and invalid member types, and preserves valid selector presence/array order for the mode-aware normalizer. Range, selector conflicts, name ownership and group-prefix normalization remain in that single later pass. Neither successful wire decoding nor a Rust constructor establishes semantic admission. Do not add a permissive old-`Value` reader just to retain earlier roundtrip expectations; migrate the model corpus contract as specified in section 9.

Resolve a rule against the public identity actually requested, after aliases. An alias does not borrow a base rule or its targeting prefix. A group rule expands only to its explicitly listed accepted names; it cannot activate another bidder. A valid but nonparticipating known name is inert and consumes no recipient state. Unknown names, malformed members and duplicate/ambiguous decoded JSON keys fail admission before map conversion; bounded compatible normalization is limited to section 7.

Before copying the recipient root, select only that recipient's normalized rule and serialize it in the extension's single-recipient array shape. Never clone the full list and remove competitors later. Alias and parameter selection from #232 remain unchanged. A malformed rule must not disappear through root projection and become apparent absence.

Without an applicable rule, preserve the current valid-bid retention behavior and ordinary targeting semantics. This change does not silently introduce a default one-bid truncation or an account-level bid limit. An explicit rule limits wire monetary candidates for that public bidder/impression to its `maxbids`; unrelated bidders/impressions remain independent.

### 5.2 Ranking And Reconstruction

Combine candidates from all completed calls for a public bidder, after viability and conversion. Rank within each `(public bidder, impid)` group by the existing deal preference when enabled, then descending converted CPM, then original call slot and original bid ordinal. The final two keys give deterministic stable ties independent of completion order. Preserve the existing overall-winner tie order between public bidders. Mixed supported formats compete within the same impression; this feature does not create a hidden per-format multiplier.

For an explicit rule, retain at most `maxbids` candidates from that group and rebuild its output in rank order. Never synthesize a creative, rewrite a bid ID merely to make it unique, transfer `source_seat` to a different bid or reuse removed indices. Existing exact sent-call coverage is checked before grouping; another call's impression coverage cannot rescue a forged response. Invalid siblings are removed before ranking and never consume a slot.

A prefix creates targeting labels only, not aliases, registry entries, privacy scopes, output `seat` owners or sync families. Rank 1 uses the public bidder's existing targeting label; rank 2 and above use `prefix` plus the decimal rank. Set `targetbiddercode` from this exchange-owned calculation, overwriting provider claims. Without a prefix, extra retained bids get no additional bidder-specific targeting labels; the primary and overall-winner rules still apply.

Before emitting targeting, test generated labels against all participating public labels and other generated labels, including the actual configured key truncation. If the mapping would collide, refuse that multibid control before bidder dispatch rather than overwrite a map entry or invent a replacement name. This check must use the same label plan later consumed by targeting. Do not rename privacy/registry identities to solve targeting collisions.

Output byte/node limits and response refusal behavior still apply. The semantic limit 9 is not an allocation bound: admission accounts for the full bounded source list, normalization, candidate bookkeeping and output growth before any allocation it authorizes. A request can exhaust existing funding below 9 and must receive the existing resource refusal, not bypass it in compatible mode.

## 6. UID Sources, Trust And Privacy

### 6.1 One Bounded Input Boundary

Add explicit optional identity input to the shared auction input boundary, usable with both raw JSON and a typed request. Keep operator settings first in the exposed API. An embedder may omit cookie input and still supply request UID hints. Do not invent a second host-only preparation path or let a public `BTreeMap` masquerade as authenticated cookie state.

Reuse the signed cookie reader/keyring, configured names, key rotation, owner-local exclusions and existing cookie/header byte/node caps. A signed UID remains an unverified identifier hint: neither the MAC nor `/setuid` proves UID ownership, consent, account authorization or caller identity. The current cookie envelope does not bind an account ID; never describe it as authenticated tenant/account state or use it to choose an account. Account selection remains the shared policy boundary's responsibility.

Negative state and positive cookie fallback are different decisions. After existing fatal input/configuration/funding refusals, process the following ordered states once, before selecting any UID source:

| State | Result and next work |
| --- | --- |
| Bounded cookie/header admission fails | Existing typed resource/input refusal. Do not assume opt-out is absent. |
| Existing separately configured host opt-out signal matches | Suppress UID projection from all sources; do not read keys or decode the signed cookie. Continue only the otherwise-permitted auction work. |
| No actual recipient can receive any UID under existing eligibility/privacy decisions | No cookie secret/decode work is needed. Those decisions already forbid every positive UID source; they are not relaxed by skipping this read. |
| No UID-cookie input was supplied | No key read; request hints remain subject to their ordinary recipient privacy decisions. |
| UID-cookie input exists, some recipient could receive a UID, but host storage-read permission denies verification | Do not read the secret or payload. Suppress UID projection from every source, including explicit hints: an unread cookie might contain negative state. This is an identity suppression, not an authentication grant or an instruction to block contextual bidding. |
| Supplied UID cookie may permissibly be examined | Read the coherent keyring once and verify/decode once, even when an explicit UID would win or no recipient has a sync family. Higher-priority hints never justify skipping negative-state discovery. |
| That key read fails | Missing/invalid configured key material is a fixed configuration failure; backend unavailability is a fixed transient failure; deadline/resource failures keep those typed categories. No fallback to explicit hints or an old keyring. |
| Integrity/payload validation fails | Fixed invalid-request refusal; no fallback and no cookie-clearing write. |
| Integrity-valid state contains opt-out | Suppress UID projection from every source. |
| Integrity-valid non-opt-out state | Apply source precedence; a family entry at or beyond its expiry contributes no cookie UID. |

Evaluate storage-read permission against the auction's effective privacy signals and host vendor, not the sync endpoints' separate signal ladder or a bidder-only exception. The no-eligible-recipient shortcut is about permission to disclose any UID, not merely whether a lower-priority cookie fallback would be used. Suppression is an operation-bound decision consumed by every recipient; a late adapter cannot repopulate a denied field. Suppressed, unrelated or absent cookie state cannot select a URL, account, key namespace or public bidder identity.

These are explicit proposed auction policies to reconcile with [#180](https://github.com/IABTechLab/auctioneer/issues/180) and #57 before #79 activation, not claims that the present auction handler enforces them. Keep secret/error diagnostics fixed; key references, backend text and cookie bytes are not public messages. The child plan must map the typed secret outcomes to the existing HTTP/configuration categories without collapsing an unavailable key store into an invalid cookie.

### 6.2 Source Precedence

Select before per-recipient privacy scrubbing, in this order:

| Priority | Source | Matching rule |
| --- | --- | --- |
| 1 | Nonempty request `user.buyeruid` | Explicit request-wide hint; it remains the caller's responsibility to supply an appropriate buyer identifier for every intended recipient. Never manufacture it from `user.id`. |
| 2 | Present `user.ext.prebid.buyeruids[public_name]` | Exact accepted public bidder/alias identity; a nonempty value selects a hint and an empty string explicitly clears it. Neither falls through to a cookie or base/alias/sibling entry. |
| 3 | Signed cookie's live entry for the recipient's sync family | Family comes from accepted discovered/resolved metadata, not a request-selected family or an implementation-name guess. |
| 4 | None | Leave `buyeruid` absent. Do not insert an empty replacement or a host cookie ID as a bidder UID. |

An empty top-level `user.buyeruid` means no request-wide hint, so evaluation continues to the per-recipient map. By contrast, a present empty string at the exact map key is a terminal clear when that map priority is reached; do not restore a stored UID beneath it. A nonempty top-level hint still wins over the lower-priority map. Non-string/null/object values and duplicate decoded keys in these controls are malformed input, not permission to fall through. Empty cookie-entry values never become a UID. Apply existing identifier-length/input bounds without logging rejected bytes. These rules are identical in both protocol modes; broad case-folding of arbitrary alias names is not added as an unreviewed compatibility behavior.

A shared sync family may supply the same cookie UID to several declared family members, including an accepted alias where that is its resolved metadata. Sharing a family does not share GDPR vendor permission, activities, account scope, request parameters or explicit UID-map entries. A bidder with no declared family cannot use another bidder's entry. Honor discovered `accepts_sync_uid` exclusions without a central real-bidder switch.

Consume and remove the global `buyeruids` map before recipient copying, including the typed/flatten-backed path. Forward only the selected `user.buyeruid`, never a map containing competitors' identifiers. Preserve unrelated reviewed user context under the existing disclosure policy. No mutation of the caller's request or cookie is allowed.

Deliver the removal first as an independently reviewable containment unit under #79, before the full UID feature. Strip the entire exact `user.ext.prebid.buyeruids` member while constructing recipient roots, before copying it; do not start selecting an own entry, parsing cookies or manufacturing a `buyeruid` in this unit. Preserve the existing explicit top-level `buyeruid` and unrelated context. This requires no new public setting, identity model, key reader, trust decision or reserve; prove the changed projection remains within its existing admitted bounds. It must not wait for #57, #180, currency/multibid delivery or the full UID resource package. Any actual new charge requires review of that exact delta, not assumed free capacity.

The early unit needs an independently failing fake-transport control: two admitted recipients, distinct UID-map canaries present before dispatch, all identifier disclosures allowed, no cookie/key work, and exactly one captured outbound request per recipient. Neither body may contain the map or either canary; unrelated user context and an explicit top-level hint remain correct, and the caller's input stays unchanged. Raw and typed entrypoints must reach the same barrier. This is a requirement to execute during implementation, not a claim that the source-traced disclosure has been demonstrated by a wire test in this planning PR. Later #79 consumes admitted hints before this same stripping barrier; it must not reintroduce pass-through or duplicate the projection mechanism. #79 stays open until its full scope passes.

### 6.3 Privacy Wins After Selection

The effective per-bidder scrub still combines GDPR pass-ID, transmit activities, COPPA, LMT and applicable US-policy decisions. Any composed identifier denial removes the selected UID regardless of source or mode. UID selection precedes that scrub, and no adapter/finalization stage may subsequently repopulate it from retained maps or cookies. Host cookie-read permission is not a replacement for the recipient's disclosure permission.

No UID source bypasses `fetchBids` refusal, disabled bidder/account, unsupported inventory or registry eligibility. New typed buyer-UID extension fields must be classified through the existing field-metadata/disclosure ownership, not put on an unknown-field allowlist. No raw identifier, signed cookie, key text, consent or selected source value appears in errors, logs, `Debug`, metrics labels or debug body capture.

#57 must record whether intended consumers need callback ownership/replay authentication beyond integrity. Until a separate bounded authentication contract is approved and delivered, expose no authenticated-UID type or claim. #79 can implement the reviewed hint boundary; it cannot claim to implement callback authenticity by projecting an integrity-checked string.

## 7. One Global Protocol Mode

Add `host.protocol_mode` with exactly `strict` (default) and `reference_compatible`. Resolve it through the existing validated policy path, not a Cargo feature or adapter flag. Unknown values fail operator validation. One captured value governs every public name in an operation, standalone and embedded; it is not serialized into a bidder request as configuration.

Reserve the following exact override locations. Their presence is invalid, including null, an empty value or a value identical to the host setting:

| Input owner | Forbidden paths | Refusal boundary |
| --- | --- | --- |
| Operator publication | Root `/protocol_mode`, `/features/protocol_mode`, `/account_defaults/protocol_mode`, `/bidders/<public-name>/protocol_mode`, and `/accounts/<account>/protocol_mode` if that account-overlay format is still the actual base | Existing strict configuration validation; do not reintroduce `accounts` if #244 has removed it. |
| API-backed account policy, once that format exists | Account-policy root `/protocol_mode` and `/host/protocol_mode` | Account-policy validation; no override of captured operator mode. |
| OpenRTB request | Root `/protocol_mode` and `/ext/prebid/protocol_mode` | Invalid request before root projection can erase the field, for raw and typed input alike. |

These are structural paths, not a recursive name blacklist. A bidder parameter or unrelated first-party JSON member named `protocol_mode` is not an engine override. Do not inspect every arbitrary extension for the same word or invent a camelCase/environment alias. Both modes apply the same presence-based guard, including when flatten storage would otherwise preserve or discard the supplied field.

Both modes retain one implementation and all common safety invariants. The initial mode-dependent differences are only these pinned multibid normalizations:

| Control | Strict | Reference-compatible |
| --- | --- | --- |
| Integer `maxbids` below 1 or above 9 | Invalid request. | Clamp to 1 or 9, emit a fixed warning, then apply unchanged resource admission. |
| Both selector forms with nonempty `bidder` | Invalid request. | Keep `bidder`, drop `bidders`, emit a fixed warning. |
| Repeated recipient in otherwise valid normalized rows | Invalid request. | Keep the first valid occurrence in wire array order; warn and ignore later occurrences. |
| Prefix on a `bidders` group | Invalid request. | Drop the prefix with a fixed warning; retain the valid group. |

Missing `maxbids`, invalid member types, no usable selector and invalid names are request failures in both modes in this initial subset. The reference can discard some such rows and continue; that extra tolerance is deliberately not promised. Ordinary JSON admission, duplicate-key refusal and alias ambiguity are also unchanged. A compatible setting is not general Go struct-key matching, unknown-value coercion or support for every Prebid feature.

Reference array first-wins normalization is deterministic and distinct from duplicate object keys: the latter are rejected before map conversion, not given last-write-wins semantics. A malformed earlier row does not become an owner of a name by being partially parsed. The normalizer and the later recipient projection consume the same admitted result, not two independent parsers.

All unlisted current bidder protocol choices remain unchanged. A future bidder-owned difference needs its own pinned source, paired controls, error scope and review; it must not add a central bidder-name branch or a second implementation. The initial #56 activation must include at least one of the real paired behaviors above with #55, not publish a mode knob whose values have no exercised semantic difference.

The following proposed differences from pinned Prebid Server apply in both modes and must be recorded with their fixtures, rather than hidden under the compatible name:

| Contract | Pinned source behavior | Proposed Auctioneer boundary |
| --- | --- | --- |
| Request multibid limit | `exchange/targeting.go` uses request `maxbids` for targeting; `exchange/auction.go` uses `Account.DefaultBidLimit` for wire truncation. | An explicit request rule also bounds retained wire candidates for that public bidder/impression. No new account bid-limit knob is introduced. |
| Explicit buyer-UID map names | `exchange/utils.go` lowers the map keys and requested bidder name. | Match the accepted public name exactly; no case-folded or base/alias fallback. |
| Request-supplied currency rates | The reference supports request-provided conversion tables. | Initial serving conversion uses only the accepted operator snapshot and rejects a supplied rate override. |
| Unusable multibid rows | The reference can warn, skip rows and keep processing. | Unsupported malformed rows are request errors; only the four enumerated normalizations differ by mode. |

Selecting the first requested currency globally and retaining deterministic stable ties are also Auctioneer policies; the mode does not promise per-bidder currency selection or the reference's map/sort ordering. The initial paired matrix is intentionally a subset, not blanket parity.

### 7.1 Invariants In Both Modes

Privacy and activity enforcement, account authorization, cookie integrity, HTTPS/SSRF protections, secret handling, structural/byte/work limits, original deadlines, sent-impression coverage, canonical bidder attribution and target qualification are never optional. Unsupported native/guest factories remain unavailable. No normalized request can increase an installed limit or return capacity that it did not own. This mode does not override #244's hard-cut schema or re-enable a legacy public API.

## 8. Resource And Failure Contracts

This planning package authorizes no new numeric reservation, concurrency increase, workload reduction, raised ceiling, dependency, SDK extension or replacement accounting equation. Use the existing additive budget and ownership ledger. Before a child activates changed serving behavior, freeze and review its changed layouts, storage multiplicities and bounded work, then independently verify every affected target. A native functional pass does not certify a guest or prove memory fit.

| New state/work | Owner and multiplicity |
| --- | --- |
| Mode | One scalar in captured operation state; no map per account or bidder. |
| Rate snapshot | One bounded operation owner, or an accepted immutable owner retained under that operation's funding; not a table per bidder/candidate. |
| Currency metadata | Bounded scalar/code annotation per retained candidate; annotation JSON growth and serialization still charged. |
| Multibid controls | At most the admitted input width plus normalized accepted-recipient state; group expansion and prefix planning checked before allocation. |
| Candidate ranking | Reuse/move candidate storage with bounded ordinals/index scratch; no creative-tree clone. Stable-sort scratch, if used, must be charged. |
| Cookie state | One admitted decode owner plus selected recipient strings under existing construction growth; no copy of the complete map for each bidder. |
| Secrets | One bounded coherent keyring read when needed; never one read per UID or alias. |

Preserve the existing precedence between malformed input, policy/privacy refusal, resource refusal and original deadline expiry. Do not invent a fallback successful auction to hide a failed snapshot, key read or incomplete candidate transformation. Bid-local conversion/decoding failures preserve valid siblings; auction-fatal input/admission failures dispatch no bidder. Every cancellation/failure path retains and releases the correct receipts exactly once, including when a prepared or completed result is held by an embedder.

Host cooperation must occur at the existing bounded work checkpoints on frozen-clock runtimes. Parsing, group expansion, rate/cookie admission and ranking must not consume an unbounded synchronous slice or dispatch after the original absolute deadline. Resource bounds must cover refused as well as successful cases and concurrent operations, not merely a steady-state valid fixture.

## 9. Verification And Test Ownership

Use existing owner-local test targets. Keep pure unit cases inline and conformance under the owning crate; reuse the capable `fake_bidder` for shared pipeline/handler tests. Real bidder protocol expectations stay beside their implementation. No new integration binary, source include trick, local workflow snapshot or parallel oracle copied from production is implied.

Every implementation chunk supplies its own tests; later deployment acceptance does not defer them. The implementation plan must resolve full commands from the current workflow and `docs/testing/development.md`, including secure/insecure feature dispatch and all four adapter artifacts. Under the user's standing rule, execute acceptance on hosted GitHub CI, not local Cargo/Clippy/fmt, and label expected commands versus actual hosted result lines honestly. Base checks are not evidence that these proposed behaviors exist.

### 9.1 Required Independent Controls

| Area | Controls that must genuinely distinguish the branches |
| --- | --- |
| Currency selection | Absent/empty list versus one and multiple valid entries; an unconvertible first entry cannot be rescued by a later entry. Vary source and target independently. |
| Arithmetic | Literal expected cross-currency values in both directions, identity bit parity, bad source CPM, missing rate, expiry immediately before/at/after, overflow and positive underflow. Do not derive expectations using the converter. |
| Ranking integration | Source prices ordered opposite to converted prices; prove the converted winner, `cur`, price annotations and price bucket on the real shared handler output. |
| Snapshot lifetime | Update policy/rates/mode while an operation is retained; the active operation stays coherent and the next one observes the change. No private cache-hit claim. |
| Multibid | Zero/one/two/nine/ten candidates with 1, 2 and 9 requested; multiple calls, impressions, aliases and formats varied independently; valid lower-price sibling beats invalid high-price input. |
| Early UID-map containment | Two recipients with distinct present input canaries, no identifier restriction and no cookie input; exact successful outbound counts and no map/canary in either body, preserved ordinary context/top-level hint and unchanged caller input. Protected-only rows cannot prove this barrier. |
| Projection | At least two competitors with distinct rules/UID canaries; assert before-state presence, selected output equality and absence of every nonrecipient control on the actual fake transport body. |
| Target labels | Base/alias and generated-prefix collisions, truncated-key collisions, rank changes and provider-forged `targetbiddercode`; no identity reassignment. |
| UID precedence | All three sources carry different values; remove each higher source independently. An empty global hint permits map/cookie selection, while a present empty per-recipient map value suppresses cookie fallback. Include malformed values, alias/base separation, declared shared families, missing families and owner exclusions. |
| Cookie state | MAC tampering, unsigned input, rotation, host and signed opt-out, expiry immediately before/at/after and absent input. Include a secret-read positive control; prove no secret read for ineligible operations and denied host reads. Put explicit winning UID hints and no declared sync family in signed-opt-out rows, so the negative-state guard cannot be hidden by fallback arithmetic. |
| Privacy | Separate ID denial, activity denial, COPPA, LMT and applicable US-policy rows across each UID source; shared family members with different decisions. Assert the UID existed before scrub and never returns afterward. |
| Mode | A paired valid/normalizable case for every listed difference, invalid mode configuration, every reserved override path with null/host-equal/different values, unchanged safety negatives and a hot-update snapshot control. Prove a same-named bidder parameter is not recursively rejected. |
| Funding/deadline | At/below/above independently frozen changed charges, retained success/failure results, cancellation during preparation and frozen-clock cooperation. Do not make caps equal to fixture demand at every boundary. |

Reference comparisons use immutable external captures and recorded intentional divergences. Do not generate expected results with Auctioneer or merely compare a corpus against its own manifest. Every corpus operation has a shape/count owner and every filtered driver asserts a nonzero/exact exercised count. Hostile lexical fixtures use the existing raw-fixture gate only where `json!` cannot represent the condition.

The hard cut explicitly replaces the OpenRTB sample corpus's unconditional deserialize/roundtrip expectation for newly invalid controls, not its fixture population or source bytes. Every discovered fixture must have exactly one exercised outcome: unchanged JSON-value parity when structurally representable, or a specifically reviewed refusal at the newly tightened boundary. Refusal cases are enumerated by source revision, relative path, digest, offending field and exact failure category; unknown failures never become accepted negatives. Pin accepted/refused populations separately, reject skipped/unclassified rows, and use an otherwise-valid control to prove each refusal comes from the named field. Keep missing-root, unreadable-root and committed-population guards. This is a test-contract migration for rejected input, not a legacy serving mode. The plan names the initial missing-`maxbids` capture and the synthetic camelCase fixture to migrate.

### 9.2 Acceptance Of This Planning Package

The implementation plan may be written after the user approves the written specification and the advisory document review loop completes. Then publish the spec and plan together for PR review, subject to publication authorization. Production implementation waits for that planning PR's approval and merge; the PR review is not a prerequisite for writing the plan it must contain. The plan must map each contract to an existing child owner, actual files and commands, test-first evidence, an activation/resource checkpoint and a separate implementation PR. Planning approval does not authorize endpoint publication or speculative provider qualification.

The plan must specifically preserve #57's trust decision, #180's recovery/privacy policy review, #192's shared viability ownership and #59's later FPD work. No new unconditional issue dependency is installed by prose alone. Change native issue links or acceptance scope only with authorization and readback.

## 10. Delivery Boundaries And Sequence

| Delivery | Required result | Not claimed by its planning predecessor |
| --- | --- | --- |
| #36 | Approved common contract and ordered plan, with explicit intentional reference differences. | Currency, multibid, mode or UID serving implementation. |
| Early #79 containment | Remove the request-wide buyer-UID map before recipient copying, with unprotected two-recipient wire and projection-resource controls, independently of the later UID prerequisites. | Positive map/cookie UID selection, trust/recovery decisions, new key reading or full #79 closure. |
| #54 | Shared response/floor rates, currency conversion/provenance and reviewed external import/publication, with resource and handler acceptance. | Automatic feed ingestion from a manual table; guest readiness from native execution. |
| #55 + #56 | One typed multibid path and genuinely exercised global mode behavior, atomically activated without a no-op public selector. Separate implementation commits may share one delivery PR if reviewed together. | General reference parity, different bidder registries or raised response capacity. |
| #57 | Explicit callback authenticity/replay decision and any reviewed follow-up scope; keep integrity-only hints truthful. | UID ownership proven by a signed cookie. |
| #79 | Shared bounded auction identity input, precedence, declared-family mapping and post-selection privacy-safe recipient projection. | Browser third-party-cookie acceptance, callback authentication or account authorization. |
| #59 and its children | Subsequent FPD/EID precedence and permission contract consuming the same public identities and captured mode. | Forwarding unresolved request-wide `data` or `bidderconfig` in anticipation of that feature. |

#54 and the #55/#56 work are separate implementation units. Their ordering must not cause an intermediate response builder to label unconverted prices with the selected currency. The early map-stripping unit lands first after its own targeted baseline, tests and full acceptance; it does not wait for these other deliveries. UID trust review can proceed independently; activation of positive UID selection/cookie ingestion still requires #79's reviewed privacy/recovery and resource contracts. No production work is stacked onto this planning branch.

## 11. Approval Record And Open Readiness

On 2026-11-18 the user approved drafting one shared contract covering currency/freshness, multibid ordering/limits, UID precedence/trust and the global mode, with implementation split across existing child issues. This approval permits this draft, not its proposed policy choices or their implementation.

The user subsequently approved the written specification at `b9c6d158cd8d30ed0e667edd2e713cd1dc728e18` with "Approve written spec; write the plan". That approval includes the explicitly proposed checked-`f64` policy, operator-published rates without per-auction fetching/request overrides, per-rule multibid wire limits and fail-closed all-source UID suppression when supplied cookie negative state cannot permissibly be verified. The earlier draft-only approval is superseded for planning, not for implementation.

On 2026-11-18 the user also requested addressing PR #246's feedback with no backward compatibility: hard-cut changed paths and remove legacy implementations where needed. The review revision therefore makes decoder currency preservation, pre-map control admission and the explicit model-corpus rejection migration part of this contract. The subsequent review correction separates the already-required UID-map stripping into an early unit instead of blocking it on the rest of #79. These are planning corrections, not implemented behavior or authorization for a second parser to preserve obsolete behavior. Merge still requires current-head reviewer acceptance and checks, including the separate one-account exception when applicable.

This specification deliberately leaves numerical implementation proof packages and the external producer's detailed design to their existing delivery owners; it does not claim them complete. The next artifact is the ordered implementation plan. Publication, issue changes and merge need their respective approval. PR #244 remains a separate stored-object planning cycle.

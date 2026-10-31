# Bounded Browser Auction Output

## Decision And Scope

AG approved this shared prerequisite on 2026-10-25 after clarification. It
preserves an optional bidder output; it does not execute a browser auction,
fetch configuration URLs, introduce an engine or enable a provider definition.
Ordinary server-side display bidding remains independent of this extension.

The pinned reference has two producer shapes: Criteo's `ext.igi[]` selects the
first `igs[].config`, while OpenX and PubMatic use an impression-keyed
`ext.fledge_auction_configs` map. Their output contract is
`ext.prebid.fledge.auctionconfigs[]`, with `impid`, `bidder`, `adapter` and
`config`. These are technical extension identifiers, not core OpenRTB fields.
The source revision remains `d7f8515b86258688304b0d9b6668c6a0e258bc9e`;
source objects are read, never executed.

## Ownership

`auctioneer-bidder` gains `BrowserAuctionConfig { imp_id, config }` and an
`auction_configs` vector in `BidderResponse`. There is deliberately no
provider-controlled bidder or adapter identity. Debug formatting reveals only
counts, not configuration contents or impression identifiers.

Each bidder moves its protocol's values out of the already node-budgeted
`BidResponse.ext` tree. No raw-body accessor, second JSON parse, request clone,
generic path interpreter or new dependency is added. Optional extension shape
errors follow that bidder's documented policy; exceeding the sealed parser's
node limit remains the existing whole-response failure.

The exchange validates every returned configuration against both its
bidder-scoped request and the exact call's nonempty sent-impression coverage.
Empty coverage does not authorize an extension output. Invalid entries are
dropped individually, valid siblings survive, and at most one fixed code-4
diagnostic per call records uncorrelated configurations without their values.
This validation is independent of ordinary bid validation and currency.

An accepted per-bidder browser result owns the resolved canonical adapter name
once and the moved configuration records. The public bidder identity comes
only from the exchange's existing bundle key. An alias therefore emits its
requested identity as `bidder` and the resolved base as `adapter`, regardless
of any names inside the opaque provider configuration.

## Response Assembly

The final response builder moves payload values and impression IDs into
`ext.prebid.fledge.auctionconfigs`. It does not serialize and reparse the
payload or clone its tree. Only the fixed wrapper and exchange-owned identity
strings are allocated during assembly. Empty output omits `fledge` entirely.
Configuration-only responses remain supported without invented bids, currency,
targeting keys or prices. Non-USD bid refusal does not erase independent
browser configurations. An HTTP/decode failure contributes no configurations.

Bundle order remains lexicographic public-bidder order. Within a bundle,
original call order and producer record order are preserved. No map traversal
or deadline is used to invent result order. Provider maps must supply a stable
key order; array producers retain array order.

## Funding And Limits

Keep all existing byte/node ceilings, resource coefficients and the shared
additive budget equation unchanged. The attached sealed-response node budget
bounds parsing, source records and configuration trees. Existing parse and
assembly charges must cover the moved records and transient vector capacity;
this is an acceptance condition to measure, not an assumption of qualification.

Repeated public/adapter names can grow independently of a provider's short
configuration bytes. Extend the existing response-output preflight to count
those serialized names per returned configuration, separately from ordinary
bid targeting. Check the held response reservation before allocating expanded
wire wrappers. If it cannot fund the output, use the existing resource refusal;
do not raise a limit, borrow debug capacity or silently trim configurations.

Retain only one canonical identity per bidder result, not one clone per
configuration before preflight. The existing request-derived bookkeeping
allowance must still cover the larger fixed bundle shape. Allocation controls
must vary configuration count, payload bytes, nodes, call count and identity
length independently, including configurations with no ordinary bids. Existing
response-finalization caps still bound the complete serialized result.
Use an unchanged declared profile's runtime-derived response reservation,
excluding debug funding, as the independent allocation bound, preserving the
existing twenty-percent safety margin. Include fixed bundle bookkeeping and
config-only alias amplification. A receipt sized from the new output formula
alone is not proof that the existing reservation funds the feature. Absolute
deployment-fit blockers remain distinct from this response-phase proof.

## First Consumer And Boundaries

Criteo is the first real producer. Its ordinary requests and response metadata,
original protocol captures and optional IGI output exercise the complete shared
path. OpenX and PubMatic use the same result type when their source-specific
ports are integrated; this change does not fabricate their implementation or
promote their pending definitions.

Required controls cover config-only output, multiple calls/aliases, forged and
unsent IDs, empty sent coverage, currency independence, omitted empty output,
malformed optional IGI, node-budget refusal, redacted Debug and proof that a
large payload moves rather than clones. A real exchange/fanout integration must
check the final wire extension and authoritative identities, not just type
construction. Resource and final-byte refusal paths need direct assertions.

This closes an output-channel prerequisite only. Full source migration,
schema/cache qualification, absolute target fit and hosted delivery remain
separate acceptance gates.

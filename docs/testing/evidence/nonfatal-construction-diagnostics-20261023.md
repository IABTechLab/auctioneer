# Nonfatal Construction Diagnostics

This record covers the shared bounded channel, not migrated bidders, generated
schema qualification, hosted execution or physical memory fit. Historical
resource captures are unchanged.

## Ownership

`auctioneer-bidder::ConstructionDiagnostics` retains 32 payload-free enum rows in
33 inline bytes. A 33rd entry replaces the final row with a fixed suppression
message. No input values, impression identifiers or parser messages enter the
channel. Code 2 and at most 64 ASCII message bytes are pinned independently.
The existing 16-entry sink-error window is unchanged and independent.

Both sink terminal paths move the buffer. Fatal semantic errors still discard
accepted calls and refund reservations. The exchange retains only nonempty
buffers in a map keyed by the observable bidder name. The map survives zero
calls, fatal rollback and both post-construction deadline exits. Response
assembly serializes borrowed ordinary/diagnostic/global sequences before moving
bids; it does not copy existing error strings into a second error vector.

## Charge

The predeclared context reserve is zero for no participants, otherwise
`4096 + B * 98304`, where B is the post-cap bidder-name population, including
aliases and zero-call bidders. It is not multiplied by calls or impressions.
Four fixed/map envelopes, bounded row objects/strings and serializer roots are
covered by the conservative owner reserve documented in Task 14.

All four adapters feed their supplied bidder limit into the same function
through the core composition API. Boot and CLI inspection use that descriptor.
Only `CatalogResourceCosts.context_per_auction_bytes` changes; catalog residency,
initialization and schema-validation increments remain unchanged. The existing
additive equation funds context once per auction, then applies auction
concurrency. No workload, ceiling, dependency or toolchain pin changes.

Before construction, the exchange checks request projection, retained scope
growth, reusable scratch and the diagnostic reserve as one sum against the
held Projection receipt. Two individually fitting quantities cannot share one
receipt without funding their combined peak. Projection ownership remains held
through response serialization by the existing auction lifecycle.

## Controls

- Colocated buffer/sink tests cover 31/32/33 rows, repeated overflow, fixed
  messages, independent error windows, both terminal paths and fatal refunds.
- `response.rs` tests preserve ordinary rows, diagnostic rows and global rows
  in that order for the accepted alias named `prebid`.
- `outbound_construction` covers accepted siblings, zero calls, fatal refunds,
  two zero-call aliases, exact/one-short/individually-fitting/wrong-phase
  receipts and 32 diagnostic rows alongside 16 sink errors in final JSON.
- `construction_deadline` checks final diagnostic rows after construction
  expiry and after gzip-finalization expiry, with no transport calls.
- `response_charge` starts its allocation window before constructing owners
  and names. It varies B over 1/16/50 and row attempts over 0/1/32/33/1000,
  uses maximum-length names, asserts no retained allocation while recording,
  inspects actual wire row counts and measures assembly/serialization against
  the separately declared reserve. Layout/no-drop checks guard inline storage.
- Adapter tests vary the supplied bidder limit independently of each adapter's
  default and pin native/guest context charges to literal expected values.

## Verification

Commands use `CARGO_INCREMENTAL=0`, `--offline --locked -j2` and the named
targets, on Rust 1.98.1 with the existing EdgeZero pin unchanged.

| Invocation                                                                                                                                                                                    | Outcome                                                |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------ |
| Runtime/bidder/exchange/core `--lib`, secure invocation                                                                                                                                       | 1,200 passed                                           |
| Exchange `bidder_call_correlation`, `construction_deadline`, `outbound_construction`, `projection_scrub_bound`, `response_charge`, `response_factors`, `run_auction`, `sparse_request_copies` | 77 passed                                              |
| Four adapter `--lib` suites                                                                                                                                                                   | 79 passed                                              |
| CLI `bidders_resources`                                                                                                                                                                       | 16 passed                                              |
| Axum `boot_envelope`, `resource_profile`, with localhost permission                                                                                                                           | 63 passed                                              |
| Bidder/exchange `--lib --features insecure-bidders`                                                                                                                                           | 503 passed                                             |
| Core `cargo check --lib --features insecure-bidders,conformance-suite` for all three guest ABIs                                                                                               | Passed; compilation only                               |
| Workspace all-target Clippy with `insecure-bidders,conformance-suite` and `-D warnings`                                                                                                       | Passed                                                 |
| Formatting, diff checks, raw-fixture gate and its negative controls                                                                                                                           | Passed; 168 raw fixtures in 50 files, no new exemption |

The response-row and cost controls first failed on missing behavior. Removing
the combined diagnostic charge subsequently fails the zero-call-alias receipt
test. Dropping the short-circuit diagnostic handoff fails the two-deadline-exit
test. Both mutations were restored before the final successful runs.
All four supplied-population controls failed when envelopes used their adapter
default instead of the supplied row; the corrected envelopes pass.

The live native total is 2,536,789,752 bytes with 1,758,177,544 bytes of declared
margin. This is accounting, not a new physical measurement. Guest ceilings and
readiness blockers are unchanged. No historical evidence/capture is rewritten,
and none of these results imply provider readiness or complete M7 delivery.
Existing deleted-vendor build notices and the `proc-macro-error2` future-
incompatibility warning remain; no new lint suppression was added.

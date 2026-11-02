# Call Correlation Controls

Scope: the correlation portion of Task 14, using Rust 1.98.1 and the existing
locked dependencies. This is not completion of the diagnostic channel, bidder
migration, schema qualification or M7-R0 readiness.

## Ownership

`OutboundRequestMeta` owns one optional 32-byte binary header and optional
eight-byte rows indexed by its own ordered covered-impression list. Media is a
closed enum; tokens are `u32`. Neither request JSON nor sent body bytes are
retained. Metadata Debug output omits protocol bytes and tokens.

Construction rejects oversize/repeated headers, unknown/repeated ordinals,
duplicate coverage and invalid media codes before the body closure. Sink tests
observe that the closure never runs, the reservation is refunded immediately
and both earlier and later accepted siblings survive. Existing bidder-fatal
rollback tests continue to pass.

The same immutable Arc owns context after finalization, including gzip refusal.
The exchange fixture completes two calls in reverse order but returns results
in their original slots, as the transport contract requires. Both calls cover
the same impression with different media; response bodies have no media hints.
The fixture checks metadata pointer identity, tokens, bid attribution and
targeting, then independently fails each slot.

## Funding

The existing scalar adds `64 + 8 * maximum_impressions` bytes per call.
Compile-time assertions check the row, context and total metadata layouts;
provider tests independently pin the owning arithmetic.

| Adapter                                     | Metadata Per Call | Additional Bytes Per Auction | Additional Isolate Bytes |
| ------------------------------------------- | ----------------: | ---------------------------: | -----------------------: |
| Axum, 8 impressions / 16 calls / 2 auctions |             4,192 |                        2,048 |                    4,096 |
| Cloudflare, 100 / 32 / 6                    |             5,248 |                       27,648 |                  165,888 |
| Fastly, 100 / 32 / 1                        |             5,248 |                       27,648 |                   27,648 |
| Spin, 100 / 32 / 1                          |             5,248 |                       27,648 |                   27,648 |

The additive equation, workload limits, pool sizes and deployment ceilings are
unchanged. Frozen resource captures are unchanged; an older capture with a
different metadata scalar cannot serve as a matched-input catalog baseline.

## Executed Controls

All Cargo commands used `CARGO_INCREMENTAL=0`, `--offline --locked -j2`.
Native results are macOS controls, not guest execution or allocator proofs.

- Bidder library plus `outbound_construction`: 211 + 12 passed under secure
  defaults; 222 + 12 with `insecure-bidders`. No failed or ignored tests.
  A final secure rerun also passed all six AppNexus construction-allocation
  controls with the changed owning metadata scalar.
- All four adapter libraries: 22 + 19 + 13 + 21 passed.
- Exchange `bidder_call_correlation`, `outbound_construction` and
  `sparse_request_copies`: 2 + 7 + 8 passed.
- Thirteen affected core boot, admission, budget and handler targets: 61 passed.
  This includes `boot_strict_parity` using real boot and CLI paths.
- CLI `bidders_resources`: 16 passed. Frozen metadata mismatch remains a refusal.
- Axum binary, `boot_envelope` and `resource_profile`: 4 + 25 + 36 passed outside
  the filesystem/network sandbox. The initial sandboxed run refused the existing
  localhost bind; no test was weakened. Review also found a stale native export
  total, reproduced its exact 4,096-byte mismatch, then verified the repair.
- Bidder library checks compile layout assertions for `wasm32-unknown-unknown`,
  `wasm32-wasip1` and `wasm32-wasip2`, in both feature modes. No guest artifact
  execution is claimed. The two exchange correlation cases also pass with the
  insecure feature enabled.
- Strict Clippy passed for the bidder library/construction target, exchange
  correlation target and all four adapter libraries. The raw-JSON gate remains
  168 fixtures across 50 files; no exemption was added.

The initial implementation stubs failed four correlation tests. Metadata
integration then failed the scalar, frozen-context and duplicate-coverage
controls before their implementations were wired. No blanket lint suppression,
new dependency, engine change or new runtime catalog is part of this slice.

The later workspace-wide Clippy run included library test modules and exposed
an inferred integer case index in the new sink regression. Giving the first
case an explicit `u8` type repaired that lint without a suppression. The fresh
`cargo clippy --workspace --all-targets --features insecure-bidders,conformance-suite -- -D warnings`
run then passed with the same offline/locked/job settings. This is a full
workspace lint check, not execution of the full workspace test suite.

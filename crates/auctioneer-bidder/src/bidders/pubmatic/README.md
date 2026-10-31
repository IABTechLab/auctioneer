# PubMatic Protocol Port

## Scope

This repair preserves the existing active factory and independently maintained
public definitions. It does not approve a new endpoint, complete selected-wave
acceptance, qualify serving schemas/resources, or establish hosted readiness.

Registration, endpoint safety, deadlines, privacy, request/body/node budgets and
browser-output attribution remain owned by the existing shared contracts.
No dependency, feature, workload limit or funding coefficient changes here.

## Source Evidence

`bidder.toml` records the immutable implementation, schema, metadata, license
and capture witnesses from revision
`d7f8515b86258688304b0d9b6668c6a0e258bc9e`. Attribution and licensing remain in
the workspace's shared reference acknowledgements rather than copied licenses.

- 23 original protocol captures: 104,454 unchanged bytes.
- 26 original parameter literals: six valid and twenty invalid.
- 32 artifact witnesses and three tree witnesses.
- The source-dependent Rust control compares these with pinned Git objects;
  it does not run foreign source code or compute expected output with Auctioneer.
- `fixtures/factory.json` is separate, locally authored discovery evidence.

## Implemented Behavior

Requests aggregate surviving banner/video/native impressions into one capped
POST. Borrowed serializers select publisher/wrapper values, rewrite ad slots,
apply finite floors, SDK metadata, keyword/direct/FPD precedence, and retain
reviewed impression extensions without copying caller trees. Invalid
impressions keep valid siblings; all-invalid input leaves call funding intact.

Keyword selection emits a key before its next selection pass, retains the last
nonempty value, and keeps deterministic order without a retained index. This
prevents duplicate blocks from doing quadratic work before the first output.
It is not a claim of linear work for a complete large output. SDK extraction
uses one Serde decoder with borrowed raw keys/values and reusable scratch.

Responses enforce status, required standard media, actual sent coverage and
offered formats. They partition bad bids, keep source currency, truncate
categories, map existing metadata channels, unwrap native ADM in place through
the shared bounded helper, and move node-budgeted browser configurations.
Provider defaults clear conflicting destination deal priorities.

## Intentional Boundaries

The manifest records every reviewed difference: requested identity beats an
arbitrary payload alias; marketplace fields cannot become exchange identities;
unsupported audio and malformed/ambiguous bids refuse locally; native failures
cannot publish invalid creatives; and the shared response ID remains required.
FPD scalar formatting uses finite Rust f64 display, not complete emulation of
legacy scientific-exponent spelling. Original duplicate keys and generic
envelope case/default behavior remain shared-decoder boundaries.

## Verification

Native checkpoint on 2026-10-26:

- Bidder library: 912 default tests and 923 feature-enabled tests passed.
- All seven normally ignored source-object controls were explicitly driven.
- Construction/response allocation conformance: 59 passed; sealed decoding: 19.
- Discovery: 87 passed; registry: 23; test isolation: 12.
- Core library: 516 passed; exchange library: 276.
- Strict all-target bidder Clippy and workspace formatting passed.
- Raw-JSON fixture gate: 178 fixtures across 52 files; planted controls passed.

The discovery filesystem controls require local Unix IPC permission; the
sandbox-denied setup was rerun successfully with that permission. These are
local protocol/build checks, not provider-network or absolute WASM-fit evidence.

The CLI's 728 active primary tests and bidder import/source/wave integration
checks passed. Its complete all-target run initially stopped at the separate
historical evidence-formatting gate (two passed, three failed). The affected
byte-pinned records and profiles were subsequently restored; all five evidence
integration tests now pass unchanged. Other documentation formatting remains,
with frozen assets excluded by `.prettierignore`. Later CLI targets were not
reached in that original all-target run.

Unit controls remain inline in `mod.rs`. Protocol, source, allocation and
sealed-decoder conformance live under this bidder's `tests/` directory and are
automatically discovered without adding a central bidder roster.

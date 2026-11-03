# APS Priority Integration

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

Status: implementation decisions approved by AG on 2026-10-23; Rust protocol
candidate and local fixture/raw-admission controls implemented. Metadata
promotion, complete qualification and delivery acceptance remain pending.
This is a supplemental source contract, not
implementation evidence or provider authorization. APS remains the fourteenth
priority identity; it does not alter the original 358-identity catalog pin.

## Source Boundary

AG supplied the updated local reference checkout on 2026-10-23. Read immutable
Git objects at `f1e9d0d2164e76a8bcb5f10021ebb56e00dbfe9a`, not dirty worktree
files. The original catalog remains at `d7f8515b86258688304b0d9b6668c6a0e258bc9e`.
No bulk catalog revision update or automatic source approval is implied.

| Witness                                     | Git Object                                 |
| ------------------------------------------- | ------------------------------------------ |
| APS implementation tree, 19 recursive blobs | `a8d550dbcee0ab4ef95d6478d1aeafdf4e6404d8` |
| APS implementation                          | `6c9b88a872c1835bf77cba7a4623cdead9d58c23` |
| APS protocol test owner                     | `80f92f7524fab6ae2a37124e3022f5d4f9a1e153` |
| APS parameter test owner                    | `74e0358851e73cad3744f600271a68abb52ba62f` |
| APS parameter model                         | `f84a3e9a8fb3c2b6578923e5f3b2dc872fd4b1da` |
| APS metadata                                | `dded94f05e4cf60c327877721dc2ce96cb7abfc4` |
| APS Draft-04 schema                         | `48f5de753f4175e671f0d584ab532120c8c9cd5d` |
| Four exemplary fixture files                | `bcf872996ca20ccc80a9808aac3344b4ca4b5211` |
| Twelve supplemental fixture files           | `9abd725a7784bd551b565cf9602b121a2939e791` |

The 19 implementation-tree blobs plus model/metadata/schema are 22 APS-owned
blobs. Registry and shared-wrapper/harness objects are additional witnesses,
not part of that count. Record their paths, modes, raw lengths and SHA-256 in
the eventual capture inventory. Do not check in foreign-language programs or
generate expected protocol output by executing Auctioneer itself.

## Protocol Contract

Use `src/bidders/aps/` and its adjacent declaration in `auctioneer-bidder`.
The approved [single-folder contract](2026-10-16-bidder-catalog-design.md#catalog-storage)
also puts effective metadata, schema associations and source/capture provenance
in APS's owning TOML. The coordinated asset cutover removed the former JSON
provenance consumer. Under the approved 2026-10-24 metadata boundary, APS keeps
its metadata source witness but no local metadata YAML or `retained` record;
preserve JSON schemas, protocol/parameter capture bytes and both source pins.
Registration remains discovery-based. Metadata declares vendor 793, site
banner/video, gzip and no usersync. Shipping remains opt-in and depends on the
same schema, resource, routing, privacy and factory gates as other new bidders.

Parameters have required string `accountID` and optional `region` in
`na/eu/fe`. Preserve the distinction between Draft-04 acceptance and runtime
trimming: empty/whitespace accounts can satisfy the schema but fail protocol
construction. Invalid impressions retain valid siblings; conflicting valid
account IDs refuse the whole bidder. Do not coerce malformed types.

Regional HTTPS destinations are the exact source-declared origins. A request
may select only a reviewed region, never a host/path. Preserve source routing:
the last valid impression's region wins; an omitted region defaults to `na`.
An invalid trailing impression must not change it. Test both mixed-region
orders and the omitted-region override explicitly.

The Rust candidate represents this routing with the existing
`https://{{.Host}}/e/pb/bid` mechanism. The region enum selects exactly the
three `s2s.prebid.bid-{na,eu,fe}.ads.aps.amazon-adsystem.com` hosts. This is
an exact representation change from the source's region-containing authority,
not a scheme/host equivalence assumption or a new URL policy. Host allowlisting
and the standard exchange validator remain mandatory; no metadata binding has
been promoted by adding the factory.

Preserve covered impression order, `test=1` query behavior, missing-or-empty-currency USD
default, banner first-format backfill (including partial-dimension overwrite),
unrelated request-extension keys and source-defined user/device removals.
Never mutate the caller's request or restore fields removed by shared privacy
enforcement. Fund construction before retaining bodies, context or diagnostics.

Use the existing bounded request sink, sealed response decoder, shared media
classification and per-call correlation path. Before shared inference, enforce
APS's explicit response `mtype` restriction: only `1` (banner) and `2` (video)
are accepted. Missing/zero/other types refuse that bid, never infer a format
from request or extension hints. Cover 200/204/400/other statuses,
malformed or empty JSON, response currency, multi-seat responses and candidate-
local media refusal without discarding valid siblings. Separate source status
semantics from stricter shared safety admission.

## Explicit Differences And Open Decisions

- A JSON-null or non-object request extension must refuse safely with typed
  bad input and zero APS calls. The source's null-map assignment hazard is not
  behavior to reproduce. Exercise the actual raw admission path so fixtures
  cannot normalize null to absence before the guard.
  `BidRequest.ext: Option<BidRequestExt>` normalizes null to `None`. Shared
  request admission now retains the bounded original-extension presence/shape
  in immutable request context before typed deserialization and supplies it
  through `BidderRequestInfo.original_request_ext_shape`. The APS candidate
  refuses unsafe shapes and refuses unavailable provenance with a typed absent
  extension. A typed object can be extended safely without claiming wire
  absence. The real raw handler checks all three regions across absent, null
  and object extensions; protocol units separately cover other unsafe shapes.
  Do not introduce an APS-only handler, reparse retained raw requests per bidder,
  or infer original null from a later privacy-transformed request.
  Retain only a small funded shape value, not a raw body or second JSON tree.
  Typed-only callers need an explicit unavailable-provenance state, never a
  default claiming wire absence. APS's null policy is bidder-local; it does
  not authorize global null rejection for other bidders.
- Auctioneer must not silently identify itself as the reference implementation.
  AG approved provider-visible SDK source `auctioneer`, with version from the
  built package, on 2026-10-23. Do not ignore SDK keys or accept both identities
  in tests. Provider acceptance still requires the separate live checkpoint.
- No adapter-specific credential lookup, Authorization header or SigV4 flow
  is evidenced by this source. `accountID` is a routing/account parameter, not
  proof of live access. Do not invent authentication; hosted acceptance remains
  in the separately authorized publisher-pilot capability.
- Source mock fixtures establish compatibility expectations, not live endpoint
  acceptance. Preserve the original fixtures and explicitly record Auctioneer
  expected outputs for reviewed intentional differences.

## Required Evidence

Retain all 16 source protocol fixtures byte-for-byte and all 14 parameter
expectations (five accepts, nine refusals) as data with exact provenance.
Parameter expectations transcribed into JSON are source-test expectations,
not executed captures. Freeze exact member/operation sets and reject missing,
extra, duplicate, reordered-identity or altered fixture witnesses.

Add independent cases for null/wrong-shaped extensions, SDK replacement,
preserved unrelated keys, all three regions, both mixed-region orders,
default-region override, invalid trailing impressions and account conflicts.
Also require method/header/gzip, metadata site/app/media admission, malformed
responses, mixed valid/invalid bids, exact call coverage, privacy isolation,
deadline/cancellation, limits and input immutability. The source harness omits
method comparison and conditionally skips headers; importing its 16 rows alone
does not cover those properties.

Inline `aps::tests` own units and protocol assertions; the native
`aps/conformance.rs` owns captured-source dispatch and provenance checks.
The original parameter schema lives at `src/bidders/aps/parameters.json`;
the other 19 capture documents live under `src/bidders/aps/fixtures/`, with
unchanged raw schema, metadata, protocol and parameter-case bytes. Capture
descriptors use bidder-root-relative paths. The source license remains an
immutable witness; the identical shared license and attribution are linked
from the root README rather than duplicated in APS's directory. Follow the shared
[bidder layout](../../bidders/README.md#bidder-layout), not an APS-specific
top-level fixture/test convention. The shared fixture macro and bounded sink
driver are reusable test infrastructure, not APS-local copies. Actual exchange/transport integration
tests own composed privacy, gzip and correlation. No generic fallback, stub,
no-bid-only implementation, source-file inventory or pending contract can
count as APS delivery. Freeze exact producers/counts and obtain contract review
before adding it to migration evidence or declaring the priority wave complete.

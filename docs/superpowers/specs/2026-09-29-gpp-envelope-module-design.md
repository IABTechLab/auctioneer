# Extractable GPP Envelope Module

Date: 2026-09-29
Status: Independent review passed; user authorized implementation after
self-review on 2026-09-29. Implementation and qualification in progress.

## 1. Decision and Scope

Implement a bounded, reference-based GPP envelope parser as
`auctioneer-privacy::gpp`, not as a new workspace crate or an `iab_gpp` fork.
The user approved the internal-module approach with a future extraction
requirement. This is the explicit replacement decision required by section
5.5 of the [M4 design](2026-09-26-m4-privacy-design.md).

The module must be independently understandable and testable. Moving it into
a library later should require relocating its files and updating imports,
not separating auction policy from parser code. No new workspace crate,
publication, general plugin API, or dependency is required now.

In scope: envelope version 1, numeric section IDs, bounded header decoding,
borrowed section lookup, closed errors, and portable parser tests.
Out of scope: encoding, canonicalizing outbound strings, section codecs,
GDPR/CCPA decisions, legacy fallback, `gpp_sid`, and all network/runtime work.
This decision does not reduce any M4 policy or integration requirement.

## 2. Ownership and Extraction Boundary

The public-facing module API owns its types and depends only on standard Rust
facilities. No references to other Auctioneer modules/crates, OpenRTB, config,
bidder registries, HTTP statuses, warnings, logging, clocks, or environment.
Do not reuse the TCF bit reader by creating a dependency on the TCF parser.
Do not add generic codec traits or move existing TCF/USP implementations.

Intended API shape, with final naming settled in implementation planning:

```rust
pub fn parse<'a>(raw: &'a str, limits: Limits)
    -> Result<Envelope<'a>, ParseError>;

// Envelope fields and construction are private.
impl<'a> Envelope<'a> {
    pub fn section(&self, id: u16) -> Option<&'a str>;
    pub fn sections(&self) -> impl Iterator<Item = (u16, &'a str)> + '_;
    pub fn raw(&self) -> &'a str;
}
```

`section(id)` distinguishes missing (`None`) from present-empty (`Some("")`).
It returns the original payload slice,
including any inner `.` segments; no payload validation, copying, trimming,
case folding, or decoding. Unknown numeric IDs are ordinary opaque sections.
ID 0 is never present in a successful result.

Absent/empty input normalization belongs to the caller. `parse("")` is a
format error; Auctioneer handles missing/empty GPP without calling the parser and
without issuing a warning. A valid zero-section header is a valid envelope
with no sections, distinct from missing input.

`Limits` is a small immutable value with private fields, documented defaults,
and validated construction for lower limits. Defaults are the M4 ceilings
below. Raising a limit beyond these supported ceilings requires a deliberate
future API/design change, not silently larger storage. Invalid caller limits
are rejected when constructing `Limits`, before request parsing. A zero work
budget is useful for negative controls and fails before any charged action.

Use private fixed-capacity metadata for at most 64 `(ID, borrowed payload)`
entries and a length, with no unsafe code or request-sized heap allocation.
The result borrows the original input; it does not retain another owned copy.
Avoid automatically deriving `Debug` for payload-bearing types. Debug output
and errors contain only fixed labels, version/count information, and bounded
numeric metadata, never consent or payload bytes.

Auctioneer's resolver remains responsible for active-section selection, error to
warning/400 mapping, fallback, and dispatch to the existing TCF/USP parsers.
Tests for those policies stay outside the extractable parser test suite.

## 3. Reference and Compatibility Contract

The format authority is the IAB GPP version 1.0
[Consent String Specification](https://github.com/InteractiveAdvertisingBureau/Global-Privacy-Platform/blob/2e9d322e2b7873f0438f9ba83afc3206f5304876/Core/Consent%20String%20Specification.md),
including its 2023-11-03 encoding clarifications, pinned to document revision
`2e9d322e2b7873f0438f9ba83afc3206f5304876` (retrieved 2026-09-29).
The interoperability reference remains pinned
[go-gpp v0.2.0](https://github.com/prebid/go-gpp/tree/v0.2.0), as used by M4's
pinned upstream revision. Preserve provenance and applicable attribution for
any translated implementation or fixtures; do not copy an entire parser.

Decode the header's six-bit URL-safe alphabet directly with a checked cursor.
Accept `A-Z`, `a-z`, `0-9`, `-`, and `_`; reject whitespace, `=`, `+`, `/`, and
non-ASCII header characters. Read type 3, version 1, the 12-bit range-entry
count, then cumulative Fibonacci-coded singleton/range offsets. Payloads are
split only on `~`; they are not subject to the header alphabet rules.

The general format uses six-bit right-zero-padding, rather than ordinary
byte-aligned base64. Auctioneer additionally accepts the pinned reference encoder's
byte-aligned zero padding. Let `b` be the number of semantic header bits:

- Canonical encoded length: `ceil(b / 6)` characters.
- Reference-encoder encoded length: `ceil(8 * ceil(b / 8) / 6)` characters.
- Accept only either exact length, with all remaining bits zero. Reject
  nonzero padding or additional trailing zero characters beyond these lengths.

This is a named interoperability decision, not permission for arbitrary
trailing data. Probe results on 2026-09-29 use the pinned reference encoder
and decoder. Fixture-generation provenance: offline with Go 1.23.12.

| IDs     | Canonical header, accepted by reference decoder | reference encoder output |
| ------- | ----------------------------------------------- | ------------------------ |
| 2       | `DBABM`                                         | `DBABMA`                 |
| 2, 6    | `DBACNY`                                        | `DBACNYA`                |
| 2, 6, 7 | `DBADNb`                                        | `DBADNbA`                |

These are header probes, not complete valid TCF/USP auction fixtures. Retain
the cumulative-singleton regression: the rejected `iab_gpp` 0.1.2 candidate
returned 2/6/5 for `DBADNb`. Its audit remains in the
[verification record](../plans/2026-09-28-m4-verification.md#gpp-candidate-audit).

Reference agreement is required only in the reference's valid representable
domain. Do not copy the reference's unchecked integer arithmetic, recursive large-weight
fallback, unbounded range materialization, or missing tail validation. The
reference encoder rejects Fibonacci integers at 6765 and above; boundary vectors
outside that domain must be specification-derived and independently checked,
not labeled Reference-encoder parity. reference decoder acceptance alone cannot establish
that hostile input or padding is valid.

## 4. Bounds and Failure Contract

| Resource                                   | Maximum |
| ------------------------------------------ | ------- |
| Raw UTF-8 input bytes                      | 262,144 |
| Encoded header bytes                       | 4,096   |
| Raw payload slots, including empty slots   | 64      |
| Decoded section IDs after expanding ranges | 64      |
| Positive numeric section ID                | 65,535  |
| Charged header-decode operations           | 524,288 |

An encoded range entry is not a decoded section: one range can exceed the
64-ID cap. Check inclusive range cardinality and total output count before
iterating or writing any IDs. The encoded entry count can also be rejected
above 64 since each valid entry contributes at least one ID.

Preflight raw length, header length, then raw payload count before semantic
header decoding. Count separators without collecting a request-sized vector;
retain empty slots and stop at the first excess slot. Structural scans are
bounded by the raw-byte cap; they are separate from the header work budget.

Then decode left to right, validating each encoded character once when the
cursor first reaches it, rather than prevalidating the whole header. An
invalid later character must not replace an earlier type/version error.
Charge before each alphabet-character validation,
bit read, range-entry step, checked arithmetic step, and output-ID write.
Padding bits use the same charged reader. Arithmetic/loop steps have fixed
cost; Fibonacci weights and sums use iterative checked `u64` arithmetic, not
platform-sized integers, recursion, or a request-sized table. Detect weight,
sum, endpoint, and count overflow before
mutation. A failed parse publishes no partial envelope. No global state or
reusable input-dependent state is needed.

Use wider checked accumulators before narrowing IDs. A positive decoded ID
above 65535 or arithmetic overflow is terminal, not wrapped/clamped or
reclassified as malformed input. At end, require strict ascending unique
IDs, valid padding, and exact agreement between decoded IDs and raw payload
slots. Empty payloads remain valid outer framing.

Closed error classes:

- `LimitExceeded`: raw/header/count/ID-range/work limits or decode arithmetic
  overflow. Auctioneer maps this to `PrivacyError::GppLimitExceeded` and HTTP 400,
  even with disabled policies or available granting legacy consent.
- `Malformed`: unsupported type/version, invalid alphabet, truncation,
  invalid within-limit coding/order/zero ID, nonzero/excess padding, or
  decoded-count mismatch. Auctioneer maps this to its bounded envelope warning
  and only the legacy fallback permitted by the M4 source-selection rules.

Structural cap errors take priority over format errors. During decoding,
the first encountered error wins; do not continue after an invalid type or
format error to discover hypothetical later limit errors. Fixed regression
tests pin mixed-error ordering. No raw string appears in errors.

## 5. Tests and Resource Evidence

Keep parser-only fixtures, provenance, and tests together under the GPP module
and its dedicated test-data directory. They may use test dependencies but no
Auctioneer policy types. Preserve this suite when extracting a crate. Full M4
source-selection and outbound tests consume the public module API separately.

Required parser coverage:

- Both accepted padding forms; invalid/excess padding, invalid alphabet,
  truncated fixed fields/Fibonacci integers, unsupported type/version.
- Singleton deltas, three or more cumulative singletons, mixed ranges,
  first/last IDs, gaps, adjacent ranges, numeric unknown IDs, ID overflow.
- No sections, missing/extra payloads, present-empty sections, unchanged
  opaque Unicode/inner-dot payloads, borrowed slice identity and wire order.
- Every cap at and above its boundary; 64 versus 65 expanded IDs and raw
  payloads; hostile large ranges rejected before expansion; reduced-budget
  controls proving no writes after failed charges; deterministic mixed errors.
- Arbitrary-input no-panic properties, checked arithmetic, bounded work and
  result size, and reference fixtures generated outside the Rust decoder.
- Native and existing WASM target checks, including identical overflow versus
  truncation classification; no native/network/FFI dependencies.

Measure parser scratch and result sizes on native and WASM-relevant layouts,
including coexistence with the raw request, normalized state, and TCF data.
Fixed storage still consumes live memory. Feed those measurements into M4
Task 10 admission accounting; parser bounds alone do not close that task.

## 6. Delivery and Future Extraction

1. Review this decision and the detailed contract before codec implementation.
2. Refine Task 4's parser qualification and Task 7's normalization plan around
   this module; preserve all existing reference and security acceptance gates.
3. Implement parser-only tests first, then bounded decoding, then independent
   spec and code-quality review. Do not activate auction policies in this step.
4. Integrate source selection and enforcement in the existing M4 tasks.

If extraction later has a concrete consumer, move the module and its isolated
tests/fixtures into a new crate, retain their attribution, and optionally
re-export the API from `auctioneer-privacy` for migration. Auctioneer keeps its policy
adapter and exact operational limits. Publishing, broader limits, additional
versions, encoding, or section codecs require separate decisions. No crate
name, release process, or general-purpose framework is reserved prematurely.

## 7. Review Record

Independent spec review on 2026-09-29 approved the document against M4 section
5 and plan Tasks 4/7. Its fixed-width-arithmetic advisory was incorporated
above. Approval covers the design only, not implementation, fixture
qualification, live policy activation, or full M4 acceptance.

Self-review on 2026-09-29 found no scope or extraction blocker. It clarified
lazy alphabet validation so mixed-error precedence is reproducible. The user's
instruction to self-review and continue finishing M4 authorizes the planned
implementation; it does not waive the reference, test, or resource gates.

# Original Request Extension Provenance

Scope: the shared ingress prerequisite in the approved APS integration design,
not an APS implementation, schema qualification, catalog promotion or target
readiness claim. Dependencies and Rust 1.98.1 remain unchanged.

## Ownership

`bounded_json::count_json_nodes_with_field` admits the same syntactic nodes as
`count_json_nodes`, then inspects one root field during complete syntax
validation. It uses Serde for escaped keys and temporarily borrows selected raw
values. Neither keys, raw values, the request body nor a second tree escape the
call. Huge opaque numeric tokens are not converted to floating-point values.

`JsonFieldShape` distinguishes unavailable provenance, absent fields, null,
objects, non-objects and duplicates. Repeated keys never acquire first/last
precedence. Non-object roots retain the ordinary later typed rejection rather
than being mislabeled as having an absent field.

The HTTP handler captures root `ext` before account discovery and typed parse,
and passes it through `AuctionContext` to each `BidderRequestInfo`. A typed-only
constructor explicitly uses `Unavailable`. Provenance is independent of later
normalization, alias scoping and privacy transformations, and contains no
request-controlled text. Shared admission continues accepting null; any future
bidder-specific refusal must be implemented by that bidder.

## Funding

Core's existing fixed construction contribution adds 32 bytes per auction:
four eight-byte owner allowances for the parser result, handler local, auction
context and sequential bidder info. This does not depend on accepted calls.
Compile-time assertions require a heap-free one-byte marker and alignments no
larger than eight on every compiled ABI. The existing additive equation,
workload limits, C/I/V/K values and deployment ceilings do not change.

The isolate delta is 64 bytes for Axum's two auction slots, 192 bytes for
Cloudflare's six, and 32 bytes each for Fastly and Spin. Historical captures
remain unchanged. Live resource exports include the new construction charge;
older evidence is not silently rewritten or requalified.

Serde already uses a byte stack when skipping nested containers. The fixed-
depth width sweep observes eight transient bytes, not zero allocations, and
zero retained bytes at widths 64/4,096/65,536. Escaped-key controls separately
require scratch allocation and release at widths 64/1,024/16,384, with peak
below twice the input bytes and cumulative requests below four times input.
Source inspection of the locked `ignore_value` identifies its shared scratch
owner. This scratch belongs to the existing input/parse allowance and is gone
before the funded typed parse, not an extra retained tree or catalog owner.
These native controls are not a physical guest allocation qualification.

## Executed Controls

Commands use `CARGO_INCREMENTAL=0` and `--offline --locked -j2`.

- Runtime library: 197 passed, including shape, escaping, duplicates, opaque
  numerics, unselected malformed content, trailing input and exact/one-over
  node controls. The initial new API controls failed to compile before the API
  existed.
- Bidder library and `catalog_alloc`: 212 + 5 passed. The initial allocation
  assertion exposed the existing eight-byte transient stack and was corrected
  against the locked implementation, not by allowing input-sized retention.
- Exchange library, `request_provenance` and `bidder_call_correlation`:
  268 + 3 + 2 passed. Two aliases observe the original marker after COPPA and
  impression scoping; removing typed root `ext` does not replace its provenance.
  Typed-only construction remains unavailable.
- The final secure rerun executed 1,215 tests across runtime/bidder/exchange/
  core libraries and the selected allocation, provenance, correlation, HTTP,
  config-lifetime, boot-parity and pure catalog-resource targets. Core's full
  library passed 511 tests, including the new raw-handler spy. No failures or
  ignored tests occurred in that selection.
- Bidder/exchange feature-mode controls: 223 + 5 + 268 + 2 + 3 passed with
  `insecure-bidders`. All four adapter libraries passed 22 + 19 + 13 + 21 tests;
  the CLI's actual resource subprocess target passed 16.
- Axum binary/library, `boot_envelope` and `resource_profile`: 4 + 22 + 25 + 36
  passed outside the sandbox for their existing localhost listeners.
- Core library checks passed for `wasm32-unknown-unknown`, `wasm32-wasip1`
  and `wasm32-wasip2`, compiling the shared marker and reserve assertions for
  Cloudflare, Fastly and Spin respectively. These are compilation checks,
  not guest execution, physical-memory measurements or provider acceptance.
- Workspace all-target strict Clippy passed with
  `insecure-bidders,conformance-suite`; formatting and diff checks passed.
  The raw-JSON gate remains 168 fixtures across 50 files, with no exemption
  added. Existing GVL deletion warnings and the dependency future-incompatibility
  notice remain; no workspace lint policy was relaxed.

Review identified that status/call-count HTTP controls alone could not detect
a dropped context assignment. A colocated raw-handler test now supplies a
fixture registry to the same private handler leg used in production and
observes all three original shapes at `make_requests`. It passed, then failed
with `Unavailable` versus `Absent` when the handler assignment was disabled;
the correct assignment was restored. This is not an additional public test
hook, bidder-specific handler or duplicated parsing path.

The HTTP integration cases separately prove null/absent/object admission and
pre-dispatch refusal of wrong-shaped/duplicate extensions. The shared controls
do not implement a production APS guard: its implementation and bidder-local
raw-handler refusal control are still required. No full workspace execution,
hosted acceptance or complete M7 gate is claimed by this increment.

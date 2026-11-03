# User Sync Reference Corpus

Pinned reference: [Prebid Server](https://github.com/prebid/prebid-server), by
Prebid.org and its contributors, at
`d7f8515b86258688304b0d9b6668c6a0e258bc9e`, Apache-2.0. `LICENSE.prebid`
is the unchanged reference license. `pixel.png` is the original
`util/httputil/pixel.png`, not a re-encoding: 86 bytes, SHA-256
`c2ecff291918a3caf0b7e470323e89f2a1f05b92e12a10649e598cacebe62acf`.

The supplied archive identifies the revision; it has no Git metadata.
`sources.json` fingerprints the inspected source, tests, bidder metadata,
license, and `go.mod`/`go.sum`. Rust validation can optionally verify all
35 existing source fingerprints without executing reference code. This is not
a claim of independently verifying the archive against a Git commit object.

## Files and Schema

| File | Cases | Scope |
| --- | ---: | --- |
| `cookies.json` | 48 | Decode/re-encode, expiry, opt-out, HTTP cookie reading, exact Set-Cookie |
| `wire.json` | 42 | Actual bare `/cookie_sync`, `/setuid`, `/getuids` handler outputs |
| `chooser.json` | 25 | Four ordering modes and ordered candidate checks |
| `templates.json` | 12 | Original AppNexus redirect, PubMatic iframe and redirect metadata |
| `divergences.json` | 8 | Separate approved Auctioneer expectations, not upstream outputs |
| Privacy `reference.json` | 71 | Endpoint privacy extraction and signed-int8 SID parsing |
| Privacy `divergences.json` | 7 | Separate Auctioneer privacy selection/validation expectations |

Privacy files are in `crates/auctioneer-privacy/tests/data/sync`. Total: 198
source-call cases and 15 design cases. Every case file has this envelope:

```json
{
  "schema_version": 1,
  "reference": {
    "revision": "d7f8515b86258688304b0d9b6668c6a0e258bc9e",
    "license": "Apache-2.0",
    "generator_version": 1,
    "go": "go1.23.12",
    "now": "2026-09-30T12:00:00Z"
  },
  "cases": [{
    "id": "unique-within-file",
    "operation": "decode_cookie",
    "provenance": {"kind": "source_call", "paths": [], "notes": "function and harness scope"},
    "input": {},
    "expected": {}
  }]
}
```

`source_call` paths are relative to upstream and must be in `source_sha256`.
`auctioneer_design` paths are relative to Auctioneer, point to the approved design,
and deliberately are not in the upstream fingerprint map. The reference
header on design files identifies the baseline being diverged from.

- `decode_cookie`: input `encoded`, `decoded_json_utf8` (null for raw base64
  edge cases); expected `encoded`, `json_utf8`, `uids`, `allow_syncs`,
  `has_live_syncs`. Re-encoding uses the actual upstream encoder.
- `read_cookie`: ordered `cookie_headers`, `host_cookie`; the same expected
  cookie fields. `uids` maps family names to stored UID strings, not only
  live entries. Entry expirations remain in exact `json_utf8`.
- `write_cookie`: `encoded`, `ttl_days`, `domain`,
  `secure_same_site_none`, `now`; expected exact `set_cookie` field value.
- `http_response`: method/target/body/cookies, fixed permission mode and
  configuration inputs; expected status, exhaustive recorder `headers`
  (arrays of field values), `body_base64`, `body_utf8`, `set_cookie_present`.
  Base64 is standard padded base64 for response bytes; cookie payloads use
  padded URL-safe base64. PNG has null `body_utf8`.
- `bidder_order` and `choose`: explicit candidate/control inputs and
  ordered results. Privacy callback traces pin short-circuit precedence.
- `template`: trusted origin, callback template, actual bidder metadata,
  selected privacy values, simulated UID; expected URL, family, type,
  literal macro, decoded callback and simulated round-trip fields.
- `auctioneer_divergence`: approved expectations linked to source cases where
  available. Source-inspected nondeterminism has no invented exact winner.

## Validation And External Recapture

The original generator version 1 used Go **1.23.12**, with pinned upstream
module dependencies, `-mod=readonly`, `GOPROXY=off`, and `GOSUMDB=off`.
The Go standard-library provenance is Go 1.23.12: `net/http` and
`net/http/httptest` supply cookie/header formatting and recorder behavior;
`encoding/base64`, `encoding/json`, `net/url`, `time`, and `image/png` supply
the referenced encodings. These are not files in the upstream archive.

From the Auctioneer worktree root:

```sh
cargo run --locked --offline -p auctioneer-cli --no-default-features -- reference check --root .
cargo run --locked --offline -p auctioneer-cli --no-default-features -- reference check --root . --source /path/to/extracted-pinned-reference
cargo run --locked --offline -p auctioneer-cli --no-default-features -- reference check --root /path/to/staged-worktree
```

The utility verifies all ten originally generated captures across usersync,
privacy and GPP against committed byte counts and SHA-256 pins. It also checks
capture version/metadata, unique IDs, source-call provenance
and fingerprint coverage. The original PNG and license are checked against the
source lock. Optional `--source` checks only the existing upstream fingerprints;
it does not authenticate a Git commit or re-execute the reference. The check
itself otherwise needs no archive, Go, network, or cached Go modules. This
maintenance-only CLI mode does not build the embedded application archive.

`reference check --root` also reads a staged tree with the same relative capture
paths. It compares against committed pins and never writes, refreshes, or blesses results.
Future reference recapture happens externally; retired generators and Go
overlay fragments are not included in this fresh-history repository. Captured expectations must never be
regenerated by Auctioneer. Separate hand-authored design divergence files are not
upstream captures and are checked by the existing Rust consumers.

`usersync_reference.rs` pins exact chooser input/expected key sets for all
21 choice rows and four ordering rows, plus corpus/operation counts. The real
cookie drivers in `usersync_cookie.rs` require all 36 decode and seven read
rows to run. Encoded-body consistency, cookie negative controls, independent
codec qualification, PNG decoding/CRC checks and divergence cross-references
remain in the existing consumers. `codec-qualification.md` records library
compatibility limits. The utility does not port these codecs or the reference
implementation, and its operation-field boundary test does not prove consumer
coverage.

The bidder catalog automatic-selection policy adds a separate design case, driven by
`usersync_choose.rs`: disabled automatic additions cannot reserve a shared
family or consume shuffle draws, while explicitly requested disabled identities
retain their diagnostic and family-attempt behavior. The original 25 chooser
capture rows are unchanged; their raw ordering stage does not apply this new
catalog eligibility filter.

## Harness Boundaries

The historical `generate.go` created temporary Go overlays without modifying
the archive:

1. Replace exactly five `time.Now()` calls in `usersync/cookie.go` with
   `2026-09-30T12:00:00Z`. Serialization and expiry logic are unchanged.
2. Expose private endpoint extraction helpers through thin wrappers.
3. Expose the source chooser with an identity shuffler and sorted available
   names. This pins ordering/precedence, not production random permutations
   or cross-language seeded RNG parity.

Endpoint capture calls real handlers. Analytics/metrics are inert; a local
account fetcher and explicit permission outcomes mirror upstream endpoint
test strategy. They do **not** qualify TCF/GVL/GCL authorization. No router,
CORS middleware or HTTP transport is executed: headers are recorder result
headers, not Date/Transfer-Encoding added by a server. Error bodies preserve
the pinned Go diagnostics, even when Auctioneer must redact them.

Notable baseline facts are intentionally preserved: empty explicit `f=`
defaults in upstream; `/getuids` HTML-escapes JSON while `/cookie_sync` does not;
empty `buyeruids` is omitted; expired stored IDs are returned; upstream base64
accepts some noncanonical spellings. Do not change source outputs to make a
Rust codec pass. Any compatibility decision belongs in qualification or
separate divergence cases.

Base64 edge cases derive from padded `{"optout":true} `, not an empty cookie:
accepted CR/LF and nonzero unused bits retain opt-out, while missing/extra
padding, spaces and an invalid suffix reset to the fresh cookie. Repairing
each rejected variant recovers valid opt-out JSON, making overpermissive
acceptance observable. The alternate-alphabet case standard-base64 encodes
`{"optout":true,"marker":"~~~~~~"} ` with literal `+`; it is valid JSON under
the wrong decoder, not an unrelated JSON failure. The generator verifies
that its nonzero-bit mutation decodes to unchanged bytes in Go but fails
strict base64 decoding. `trailing_json` starts with `{"optout":true}` before
a second object, so ignoring trailing JSON also produces an observable error.

Templates use original YAML through `BuildSyncers`: AppNexus family `adnxs`
but callback bidder `appnexus`, macro `$UID`; PubMatic omitted family resolves
to `pubmatic`, iframe appends to terminal `uid=`, redirect uses `#PMUID` and
`f=i`. Simulation replaces an encoded UID after outer decoding and before
parsing the callback. Named-account cases configure a custom host callback;
they do not claim automatic account propagation by upstream. Browser execution
and provider substitution behavior are not tested.

All twelve captured template cases use form-safe privacy values; GPP and
GPPSID are empty. They are not hostile-value encoding evidence. The separately
identified Rust-authored `template_controls.rs` varies all five privacy macros
with nonempty delimiter-bearing values at callback and direct bidder splice
points, asserting literal escaped values and complete URLs, and decoded value
preservation without extra query parameters. Those controls are Auctioneer tests,
not upstream captures.

The corpus is bounded evidence acquisition, not complete endpoint or privacy
implementation coverage. Full policy authorization, exhaustive cookie merge
and eviction, framework/resource/CORS integration, and bidder browser behavior
remain later implementation-test responsibilities.

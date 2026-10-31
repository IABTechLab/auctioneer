# Bidder Owner Template

This directory is outside `crates/auctioneer-bidder/src/bidders` discovery. Files
ending in `.template` are deliberately incomplete, not executable examples or
approved metadata. The manifest defaults to `factory = false`, has no public
definitions and contains invalid replacement markers. The Rust entrypoint
fails compilation until deliberately replaced. Do not remove these guards
merely to make discovery or compilation pass.

## Copy And Review

1. Read `crates/AGENTS.md` and the bidder implementation README first. Copy only
   into a **new** owner directory, replacing the implementation-key markers and
   stripping `.template` suffixes. Never overlay an existing reference package:
   preserve its manifest, source pins, schema and capture bytes instead.
2. Keep reference packages disabled. Fill provenance from actual immutable
   source evidence, including reviewed implementation differences and separate
   schema, metadata, capture, helper and license witnesses where applicable.
   There are no sample revisions, digests or parameter schemas to reuse. Use
   `origin = "local"` only for genuinely local work, never to bypass attribution.
3. Implement the real `Bidder` contract before replacing the Rust blocker.
   `mod.rs` contains factory and trait delegation with ordinary
   `mod request;` and `mod response;` declarations. Copy the matching request and
   response templates as well; their owner-local files hold construction
   and sealed decoding respectively. Add parameters/native/projection helpers
   only where they own a real responsibility. Do not add handwritten module-path
   attributes, source includes, cross-crate private imports or extra dependencies.
4. Keep unit tests inline beside their owning implementation, including request
   and response modules. The root inline test block can declare native-gated
   `mod conformance;` for `tests/conformance.rs`. Only conformance belongs under
   `tests/`; do not relabel or move ordinary units there. Use shared
   `bidders::harness` and `protocol_fixtures!`, not copied test infrastructure.
5. Author independent owner-local `fixtures/factory.json` cases from the reviewed
   protocol. Replace every fixture marker with its required typed value. The
   skeleton intentionally fails fixture deserialization until this is done;
   neither empty cases nor placeholder output is evidence. Bind the final
   reviewed manifest SHA-256, keep cases nonempty, and pin actual ordered calls,
   headers, coverage, bodies and sealed response outcomes. Do not generate
   expectations from the bidder under test or copy another bidder's parameters.
6. Add `parameters.json` only when a genuine canonical parameter contract exists.
   Captured parameter cases belong in `fixtures/parameters.json`, not the schema.
   Retain external protocol fixtures byte-for-byte with their original witness;
   authored factory cases are not upstream captures. Add other conformance files
   only when the corresponding resource or protocol responsibility exists.
7. Review registration separately: enable a completed factory explicitly, declare
   any required feature, then separately review HTTPS endpoints, endpoint macro
   fields, settings, aliases and public definitions. Copying files, imported
   source enablement or a passing unit test is never public activation approval.

## Shared Contracts

Use the real `OutboundRequestSink` and its existing `write_json` convenience
method. Metadata remains inside its reserved closure, followed by endpoint
validation and bounded serialization. Never create an uncapped body or parse
the sealed response twice. Reuse `url::json_request_meta` only when POST,
standard endpoint expansion, the exact no-charset JSON/Accept pair and all
impression coverage match. The narrower `json_headers` and
`json_utf8_headers` helpers preserve header bytes and ordering without imposing
endpoint, filtering or correlation behavior.

Opt into `status::standard_status` only for 204 as an empty USD response, 200
as sealed decode, and every other status as a single protocol error. Keep
wording, punctuation and 400 classification exact. `bad_input_or_server` shares
the 400 bad-input / otherwise bad-server classification; `debug_status_error`
also shares the capitalized debug wording without a terminal period. Protocols
accepting other success statuses or using body-empty no-bid rules must opt out.

Follow the existing development and publication lanes in the bidder README:
locked dependencies, the existing target directory, two jobs, secure defaults
and applicable feature lanes. Check the selected unit/factory filters are
nonempty, run owning shared regression targets, and format discovered modules
directly as well as through the workspace check. No new target, fixture
framework, limit, budget coefficient or schema mechanism is part of this template.

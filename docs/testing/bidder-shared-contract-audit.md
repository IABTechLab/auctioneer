# Bidder Shared Contract Audit

This audit covers all 17 executable request constructors and response status
paths. It records reuse boundaries, not provider acceptance or resource
qualification. Protocol transformations, limits, budgets, SDK pins, manifests,
schemas and captured fixture bytes are not changed by the shared extraction.
Verification separately rewrapped two pre-existing overlong authored notes in
Rubicon and Unruly manifests, with parsed-value equality against the prior tree.
Only their local manifest checksum witnesses changed; factory cases, external
captures and historical qualification artifacts did not.

## Status Contracts

`status::standard_status` owns only 204 as an empty USD result, 200 as permission
to continue into the owner's sealed decoder, and every other status as exactly
one caller-supplied error. It does not inspect, decode or copy a response body.
`bad_input_or_server` shares the exact 400/code-2 versus otherwise/code-4
classification. `debug_status_error` additionally shares the exact message:

```text
Unexpected status code: {status}. Run with request.debug = 1 for more info
```

The missing terminal period is intentional. The gate accepts an error function,
not a protocol enum, configurable response builder or alternate parsing API.

All paths below are relative to `crates/auctioneer-bidder/src/bidders/`.
Codes are public bidder error codes, not HTTP statuses.

| Response Path | Gate Reuse | Error Contract / Reason For Nonreuse |
| --- | --- | --- |
| `appnexus/response.rs` | Opt out | Status >=400 is code 4, `status={status}`. Below 400, empty body is no-bid; a populated 204 is decoded. |
| `aps/response.rs` | Standard gate; local error closure | 400/code 2: `Unexpected status code: 400. Bad request from publisher.` Other errors/code 4: `Unexpected status code: {status}.` |
| `criteo/response.rs` | Standard gate + classification helper | Capitalized debug message **with** terminal period; 400/code 2, otherwise/code 4. |
| `generic/response.rs` | Opt out | Status >=400 is code 4, `status={status}`. Otherwise 204 or empty body is no-bid; populated other statuses are decoded. |
| `kargo/response.rs` | Standard gate; local error closure | Capitalized debug message **with** terminal period, always code 4, including 400. |
| `mocktioneer/response.rs` | Opt out | Status >=400 is code 4, `status={status}`. Below 400, empty body is no-bid; a populated 204 is decoded. |
| `msft/response.rs` | Standard gate + debug error helper | Capitalized debug message without terminal period; 400/code 2, otherwise/code 4. Existing `status_error` delegates to the shared helper. |
| `openx/response.rs` | Standard gate + classification helper | `unexpected openx response status`; 400/code 2, otherwise/code 4. |
| `pubmatic/response.rs` | Standard gate + classification helper | `status={status}`; 400/code 2, otherwise/code 4. |
| `rise/response.rs` | Standard gate + debug error helper | Exact no-terminal-period debug contract. |
| `rubicon/response.rs` | Standard gate + classification helper | `unexpected rubicon response status`; 400/code 2, otherwise/code 4. |
| `sharethrough/response.rs` | Standard gate + classification helper | `unexpected sharethrough response status`; 400/code 2, otherwise/code 4. |
| `sovrn/response.rs` | Standard gate + debug error helper | Exact no-terminal-period debug contract. |
| `thetradedesk/response.rs` | Standard gate + debug error helper | Exact no-terminal-period debug contract. |
| `triplelift/response.rs` | Standard gate; local error closure | Same no-terminal-period wording, but 400/code 2 and all other errors/code **999**. Do not use the debug error helper. |
| `unruly/response.rs` | Standard gate + classification helper | Lowercase `unexpected status code: {status}. Run with request.debug = 1 for more info`; 400/code 2, otherwise/code 4. |
| `yahoo_ads/response.rs` | Standard gate; local error closure | `unexpected yahoo ads response status`, always code 4, including 400. |

The punctuated Criteo/Kargo wording remains explicit because the classification
differs; Triplelift's matching wording does not justify changing its code 999.
APS's 400-specific publisher message remains local. The three body-sensitive
protocols deliberately do not use the standard gate.

## Construction Contracts

All 17 use the existing bounded sink. `json_request_meta` already provides the
exact POST, standard context endpoint, no-charset JSON/Accept headers and all
impression IDs needed by Appnexus, Generic and Kargo. Expanding its contract to
the other constructors would impose extra headers, wrong coverage or endpoint
semantics. The new helpers share only the two literal header pairs, in order:

- `json_headers`: `Content-Type: application/json`, `Accept: application/json`.
- `json_utf8_headers`: `Content-Type: application/json;charset=utf-8`, then the
  same Accept header.

| Request Path | Reuse | Owner-Local Responsibility Kept Intact |
| --- | --- | --- |
| `appnexus/request.rs` | Existing `json_request_meta`; existing `sink.write_json` convenience | Placement preflight and borrowed extension transform. Replaces only equivalent push/write closures. |
| `aps/request.rs` | `json_headers`; existing `sink.write_json` convenience | Enum-selected regional host, debug query, valid-impression coverage and account/SDK envelope. |
| `criteo/request.rs` | Existing sink and endpoint helper only | Intentionally no bidder headers; passthrough body and all-ID coverage. Adding either JSON pair would change the wire contract. |
| `generic/request.rs` | Existing `json_request_meta` unchanged | Passthrough body. No new builder needed. |
| `kargo/request.rs` | Existing `json_request_meta` unchanged | Passthrough body. No new builder needed. |
| `mocktioneer/request.rs` | Existing sink and endpoint helper only | Content-Type only, no Accept; preserve feature-gated loopback transport. |
| `msft/request.rs` | `json_utf8_headers` | Existing ten-survivor chunks, member route, SDK handling and `x-openrtb-version: 2.6`. |
| `openx/request.rs` | `json_utf8_headers` | Non-video aggregate and per-video groups, ordered per-ID media correlation and borrowed transforms. |
| `pubmatic/request.rs` | `json_utf8_headers` | Stateful impression walk, selected publisher and exact survivor coverage. No extra parsing/walk is introduced. |
| `rise/request.rs` | Existing sink and endpoint builder only | Publisher query, charset Content-Type only, no Accept and all-ID coverage. |
| `rubicon/request.rs` | `json_utf8_headers` | Regional host, tracker query, fixed User-Agent, authorization, media fanout and per-call correlation. |
| `sharethrough/request.rs` | `json_utf8_headers` | Impression/media fanout, floor conversion, prefix blocklists and sent media correlation. |
| `sovrn/request.rs` | Existing sink and endpoint helper only | Content-Type without Accept, scoped device/UID headers and filtered survivors. |
| `thetradedesk/request.rs` | `json_utf8_headers` | Typed SupplyId endpoint, parameter precedence and semantic route refusal. |
| `triplelift/request.rs` | `json_utf8_headers` | Borrowed filtered impression transform and survivor coverage. |
| `unruly/request.rs` | `json_utf8_headers` | Whole-request parameter refusal, empty-endpoint no-call rule and all-ID coverage. |
| `yahoo_ads/request.rs` | `json_utf8_headers` | Impression fanout, version 2.5, optional scoped User-Agent and banner-first media correlation. |

Header calls stay at their original metadata positions. The shared helpers do
not set endpoints, collect IDs, add headers implicitly or introduce a second
body decode. The Appnexus and APS convenience calls use the sink's existing
`push_request` implementation; retained output remains reservation-owned and
endpoint validation still precedes bounded serialization. No claim is made
that stack-local borrowed view construction has zero cost.

## Relocation Boundaries

These two owner splits follow the shared adoption above. Original function
bodies and all 58 unit bodies are preserved; imports, indentation and narrow
module visibility are adjusted. The counts below include named functions and
methods, including inline units and test helpers, but not closure expressions.
Two free delegation targets per owner account for the function-count increase.

| Owner | Root Functions / Tests | Request Functions / Tests | Response Functions / Tests | Total Functions Before / After | Units Before / After |
| --- | --- | --- | --- | --- | --- |
| `thetradedesk` | 8 / 0 | 30 / 14 | 16 / 9 | 52 / 54 | 23 / 23 |
| `yahoo_ads` | 9 / 1 | 33 / 21 | 19 / 13 | 59 / 61 | 35 / 35 |

- The Trade Desk request boundary: `make_requests`, `missing_supply`,
  `route_refused`, `invalid_parameters`, `Parameters`, `parameter_string`,
  `select_parameters`, and the complete `Request` through `PublisherView`
  borrowed serializer tree. The original first fourteen units, ending with
  `construction_limits_are_independent_and_refused_calls_refund_slots`, move
  inline here with their request-only `Policy`, `settings` and `body` helpers.
- The Trade Desk response boundary: `make_bids`, `unsupported_mtype`,
  `invalid_notification` and `replace_price_macros`. The final nine units, from
  `mtype_only_accepts_banner_video_native_and_preserves_valid_siblings` through
  `wrong_shaped_envelopes_are_shared_parse_errors_and_nodes_are_bounded`, move
  inline here with `response_fixture`, `decode` and `request_with_unsent_sibling`.
- Yahoo request boundary: `make_requests`, `Parameters`, `unique_field`,
  `parameters`, `banner_dimensions`, and the complete `Impression` / `Request`
  through `RegsExtension` serializer tree. Twenty-one construction units and
  `EndpointPolicy` move inline here.
- Yahoo response boundary: `make_bids` and `integer_error`. Thirteen units move
  inline here, including the two previously interleaved sent-media controls
  `actual_mixed_impression_response_uses_its_banner_first_annotation` and
  `opposite_call_media_hint_is_bid_local_even_when_original_imp_offers_both`.
  Response-only helpers are `video_request`, `sent`, `response` and `one_bid`.
- Both roots retain factory, trait delegation, debug schema and unchanged
  native-gated conformance declarations. Shared fixture helpers remain in the
  root inline test module with `pub(super)` only for actual test siblings.
  Yahoo's factory/debug unit stays at the root. All serving types remain
  private to request/response files. The Trade Desk's fixed parameter error
  function has a narrow parent-visible boundary and a native-test-only root
  import, preserving its existing conformance import without editing that file.

The shared extraction adds five production helpers and thirteen inline unit
tests: eight status controls and five metadata/header controls. The latter pin
both header pairs, ordered first/second header refusal, ordered unique coverage
and the existing explicit duplicate-coverage refusal. No ignored test is added.

## Integration Checks

MSFT and Rubicon use the same shared helpers after their independent splits:
MSFT's `response::status_error` delegates to `debug_status_error`; Rubicon uses
`bad_input_or_server(status, "unexpected rubicon response status")`. Both call
`json_utf8_headers` at the original request header position before their extra
protocol headers. Their endpoint, chunking, credential and correlation code
must remain owner-local. These integrations do not require a new dependency.

Use the existing native default/feature library, factory, status, metadata,
source/isolation, allocation, strict-lint and formatting lanes, followed by the
three existing guest CI lint scopes, against one stable source tree. The shared
helper tests do not replace bidder-specific status/error or resource controls.
No build or qualification result is asserted by this audit.

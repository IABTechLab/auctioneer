# Configure Mocktioneer endpoints through runtime policy

November 9 format update: the maintainer subsequently selected fields directly
under `.settings` for every bidder. The [flat settings design](2026-11-09-flat-bidder-settings-design.md)
supersedes the wrapper syntax recorded below; endpoint validation and authority
requirements continue to apply.

Status: operator contract approved in the November 9 conversation; implementation
and target verification belong to the accompanying PR. Related to #124.

## Problem and outcome

Mocktioneer currently always sends to the compiled
`http://127.0.0.1:8787/openrtb2/auction` endpoint. Its parameter schema mentions an
endpoint field, but the Rust request owner deliberately ignores request-level
endpoint overrides. Operators need to select their Mocktioneer deployment using
the existing typed config validate/diff/push flow without rebuilding for each
host. Keep the compiled loopback default and its qualification fixtures intact.

The approved configuration is:

```toml
schema_version = 1

[bidders.mocktioneer.settings]
implementation = "mocktioneer"

[bidders.mocktioneer.settings.values]
endpoint = "https://mock-bidder.example.test/openrtb2/auction"
```

Mocktioneer remains behind `insecure-bidders`, including when its selected
endpoint uses HTTPS. This is a development bidder feature, not a production
publication or qualification change. Existing deployments that omit settings
continue to use the exact compiled endpoint.

## Configuration and authority

Declare `endpoint = "optional_string"` in Mocktioneer's existing settings
manifest. Use the generated implementation-tagged settings types and their
fieldwise requested-identity/base fallback. Missing or null endpoint settings
inherit the base value, then the compiled default. An explicitly empty string
is invalid. Supplied invalid values must be rejected even when disabled or
shadowed by another identity's value. Unknown fields and mismatched
implementation tags retain their existing refusal behavior.

The operator's authenticated config-store write is the authority for this
Mocktioneer-only destination. Request bodies, bidder parameters, account names,
headers, aliases and response redirects cannot authorize another endpoint.
Other bidder implementations keep their compiled endpoint and host-allowlist
contracts. No global endpoint-override configuration is introduced.

## URL admission

The endpoint is a literal absolute HTTP(S) URL, at most 2048 UTF-8 bytes. Reject
empty input, repaired/missing authority syntax, whitespace, ASCII controls,
backslashes, credentials/userinfo (including an empty userinfo marker),
fragments, and any template placeholder. Preserve a supplied path, query and
explicit port; they are operator configuration, not request substitutions.

The exact borrowed spelling must also parse as the workspace's `http::Uri`,
which both outbound paths require before SDK normalization. Reject incompatible
spellings during configuration admission: use punycode for internationalized
host names and percent-encode path characters the HTTP parser cannot represent.
Literal Unicode paths that the HTTP parser accepts remain valid.

Reuse the shared URL-safety implementation. HTTPS public destinations are
accepted. HTTP is allowed only for the existing exact local literals
`127.0.0.1`, `[::1]` and `localhost` under `insecure-bidders`. Private, reserved
and alternate loopback literals and localhost-alias DNS services remain
rejected. This is URL admission, not a new DNS-resolution or rebinding defense;
provider/SDK transport guarantees retain their existing scope.

Check the raw authority as well as the parsed host for the local exception:
shortened/integer/hexadecimal IPv4, expanded IPv6 loopback and root-dotted
localhost spellings do not become approved literals through URL normalization.

All configuration and request-time refusal messages use fixed field/reason
categories. Never include a rejected URL, credentials, host or parser message.

## Request construction and enforcing sink

Add one defaulted, borrowed bidder endpoint-selection hook that accepts only
requested/base typed settings, with no request or header input. The default
returns no override. A private-field borrowed `ConfiguredEndpoint` value can
only be constructed through the shared literal-URL validator; the hook returns
that checked value rather than an unchecked string. Mocktioneer resolves its endpoint through
the same owner-local helper used by configuration validation. Direct callers
therefore cannot bypass URL admission by constructing settings programmatically.

After existing settings binding, the exchange asks the trusted bidder for its
configured endpoint. One borrowed selection supplies both
`BidderRequestInfo.endpoint_template` and the enforcing exchange validator.
A selected literal URL has no macros or host allowlist: the sink requires its
exact scheme, hostname and port, and still rejects unsafe addresses. The
validator must capture this policy before bidder construction; a bidder cannot
substitute another request-info object to widen it. Without a selection, use
the existing metadata endpoint/macros/allowlist without a behavior change.

The selection happens before credential lookup and request serialization.
Failures produce the existing fixed bidder error without a network call.
Retain reservation-before-body enforcement, bounded metadata/diagnostics, and
refund semantics. Mocktioneer continues to serialize the existing scoped
OpenRTB request and decode responses through the sealed shared decoder.

## Ownership and resource effects

Shared portable URL validation belongs to `auctioneer-config`; expose only the
narrow literal-endpoint validator needed by the bidder, without enabling native
build tooling in serving graphs. Mocktioneer owns its generated declaration,
semantic validator and endpoint resolution. The exchange owns binding the
trusted selection to construction and sink enforcement. Adapters do not parse
or reinterpret this policy.

Borrow endpoint strings from the already bounded coherent operator snapshot.
Do not clone bidder metadata, allocate a second config tree, or retain a new URL
cache. The self-review correction reuses the existing workspace `http`
dependency behind `config-model` to align admission with transport; it adds no
new dependency version or native tooling. URL parsing remains bounded by the
endpoint limit, and the URI compatibility probe is dropped before continuing.
Existing config byte/node/depth, metadata, call, body and deadline limits remain
unchanged. Adding a generated settings variant changes type/layout artifacts;
run the existing relevant layout/allocation owners and report any real change
instead of weakening frozen limits or substituting native evidence for guests.

## Acceptance

- Typed CLI validation and serving admission accept public HTTPS destinations
  and existing exact-loopback development URLs, and refuse the independent
  hostile URL boundaries described above with sanitized diagnostics.
- Alias/base fallback, omission/null, empty, disabled and shadowed settings are
  exercised independently. No setting retains the original loopback behavior.
- A real exchange/sink test with the shared capable fake proves the same
  selected policy reaches metadata construction and endpoint validation. A
  mismatching host, scheme or port refuses before body construction; forged
  request-info and request-level endpoint fields cannot override the policy.
- Mocktioneer-owned tests verify its generated settings, configured request URL,
  original request immutability, and unchanged response behavior.
- Preserve the manifest-fixture pin by updating only its digest for the reviewed
  settings declaration; retain all independent protocol requests and responses.
- Secure-default builds keep Mocktioneer absent. Feature-enabled metadata,
  registry, config validation, relevant allocation/layout controls and Fastly
  WASIp1 compilation exercise the new setting. Local Viceroy execution is
  reported separately from hosted execution and production resource fit.

## Delivery

Use the existing CLI commands. `config push --adapter fastly --local` updates
the Viceroy manifest and requires restart. A hosted push uses the existing
service/store binding. No deployment, config push, secret provisioning or live
bidder traffic is authorized by opening this PR. Tests use fictional domains
and synthetic bidders; the operator's actual endpoint stays in local policy.

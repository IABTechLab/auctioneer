# Flat bidder settings design

## Requested contract

The maintainer requested the following operator format for every bidder:

```toml
[bidders.mocktioneer]
enabled = true

[bidders.mocktioneer.settings]
endpoint = "https://mock-bidder.example.test/openrtb2/auction"
```

Remove the operator-facing `implementation` discriminator and `values` wrapper
consistently from TOML, JSON, typed CLI push/load, SDK extraction, documentation
and current test inputs. Common bidder controls remain beside `settings`.
Fields, types and semantic validators remain bidder-owned. The same generated
path supports Microsoft, The Trade Desk, Rubicon, Mocktioneer and newly discovered
settings declarations without a handwritten implementation list.

This extends #124 and the Mocktioneer runtime configuration work in PR #160.
Historical evidence and source captures retain their original bytes. This is a
pre-release authoring-format change within the current application schema;
existing tagged documents must be republished in the new form. Do not silently
maintain two accepted operator formats.

## Constraints established by the current implementation

`BidderOverrides<Settings>` already provides the `.settings` boundary. The
wrapper is imposed by `bidder_settings_codegen`, not by the provider adapters.
The current internal tag also pins settings to an implementation. Removing it
must preserve that protection: request aliases can override host mappings and
even shadow a compiled public name. The endpoint/credentials selected for one
implementation must never be reinterpreted as another implementation's settings.

Operator settings may name a future request-only alias. Its binding is unknown
at publication, so simply looking up the outer table name during deserialization
is insufficient. Empty settings, omission/null optional fields, independent
requested/base fallback and disabled/shadowed value validation must remain safe.

## Alternatives

1. **Flat generated scalar storage with borrowed typed views (recommended).**
   Decode a bounded, redacted object, expose owner-specific borrowed fields,
   resolve the implementation from the registry/host alias during admission,
   and pin the same operator binding in the request-owned projection. This
   supports shared field names across known implementations without guessing.
2. **Normalize flat input into the existing tagged enum.** This preserves owner
   APIs but needs contextual staging at every parser boundary and a deferred
   representation for request-only aliases. Avoid a second full config tree or
   an allocation of cloned strings merely to recover the removed wire tag.
3. **Infer an enum variant from fields alone.** Simple for today's disjoint field
   names, but ambiguous schemas and empty objects make this unsuitable as the
   general bidder configuration contract.

## Proposed data flow

Generate an opaque flat `BidderSettings` object from discovered declarations.
Its one owned representation is a private `Vec<FieldValue>` with a generated
closed key enum, string/i64/null scalar variants and an optional internal
`&'static str` implementation pin. Sort by generated key for stable output.
Serialization writes the fields directly as a map and omits the internal pin.
Only the existing closed scalar kinds are admitted: optional strings, signed
integers and secret references. Reject unknown keys, duplicate keys, arrays,
objects, booleans, floats, coercions and overflow, using fixed diagnostics.
Omitted and null optional values retain fieldwise absence; present empty strings
retain the owner's existing semantics. Keep the key of an explicit null during
schema checks and serialization: a null field must not become an absent field
and change which schemas match after a round trip. Empty means zero keys.
Redact all values in Debug/errors.

Generate owner-specific `Settings<'a>` as thin `Copy` wrappers borrowing the
opaque object, with typed getter methods (`endpoint()`, `hb_source()`, etc.).
`as_owner()` returns `Option<Settings<'_>>`; `resolve` consumes two optional
Copy views and returns the existing fieldwise fallback view. No temporary typed
reference, `PhantomData`, per-owner string copy or dependency is needed.
Shared admission and request `set_settings` must first perform fallible checks
of both supplied entries' pins, allowed fields, scalar types, secret references
and limits. An accessor mismatch must never be treated as absent settings.

For a compiled bidder or effective configured alias, admission resolves the
implementation through the existing independently compiled bindings. Validate
both supplied entries against that owner before invoking its semantic callback.
Independently resolve the operator owner of each supplied row, including a
fallback row. Host aliases do not chain: with `x -> alpha` and `alpha -> beta`,
the `alpha.settings` row is beta-owned and cannot supply alpha's settings to x,
even if both schemas declare the same fields. Finish the binding/schema pass
over the entire identity union before invoking any owner callback.
This permits two implementations to declare the same field name and still
select their own schema and rules.

For a future request-only identity, nonempty settings must identify exactly one
compatible linked settings-owning implementation. Reject no-match and ambiguous
cases before activation; declaring the alias in operator configuration supplies
the missing identity in an ambiguous case. Empty settings carry no values and
may remain unbound until the requested identity is resolved. A known owner with
no settings declaration rejects supplied settings, including `{}`. Never select
the first of several matching schemas or take an implementation from request data.

At request projection, independently recover the operator binding using the
effective host aliases and the host-supplied compiled bindings. Extend
`RuntimeBidderSettings::project` with those two trusted inputs; derive each row's
owner without consulting the request alias map (that map only selects which
rows to retain). Preserve an existing pin and refuse conflicting rebinding.
Store that
binding only in the existing request-owned settings object; omit it from wire
serialization. A request alias selecting a different implementation must fail
at settings binding before credentials, endpoint selection or body construction,
even when the two implementations share field names. Programmatic callers bind
an unbound object through their existing explicit trusted implementation input.

## Resource and compatibility effects

One bounded settings object owns scalar values; views borrow those values.
Projection makes only the copies already required by its existing lifetime
contract and does not copy unrelated settings or the whole policy. Charge and
measure any changed object/map layouts through their existing owners. Keep all
caps, ceilings, workloads and immutable evidence intact. The known construction
layout regression in PR #160 must not recur.

Settings-free bidders, provider storage/envelopes, bidder parameters carried by
auction requests, privacy, alias precedence and credential ownership retain
their existing contracts. Configuration examples store literal URLs, not
Markdown link syntax.

## Acceptance

- The maintainer's exact table layout parses, serializes without the removed
  wrappers, passes typed CLI validation/push dry-run and reaches serving code.
- Exercise all four existing owners with independent expected values; adding a
  synthetic owner requires only its manifest and ordinary owner-local code.
- Keep typed rejection, absence/null/zero/empty semantics, per-field alias/base
  fallback, credential redaction and validation of disabled/shadowed settings.
- Exercise compiled aliases, configured aliases, request-only aliases, empty
  objects and ambiguous/shared field declarations with capable synthetic owners.
- Exercise cross-shadowed alias fallback, mixed null/shared-field round trips,
  shared names with different scalar kinds and settings-free `{}` rejection.
- A request that retargets an operator-configured name cannot change its pinned
  implementation, including shared field names; refusal precedes credentials
  and body construction without mutating input or affecting valid siblings.
- Existing allocation/funding, exact native construction, CLI resource and guest
  checks pass without changed limits or rewritten frozen evidence.
- Update active readers, writers and examples together; preserve historical
  captures. Verify actual dispatched test counts and inspect CI independently.

## Review

An independent specification review identified the per-row fallback binding,
explicit-null identity, projection inputs and view-gating requirements above.
They are incorporated before implementation; no callback may observe an
unchecked row and no request alias may choose an operator settings pin.

# Bidder-Owned Endpoint Bindings

## Scope

AG approved bidder-owned endpoint declarations and continued with the proposed
typed-binding refinement on 2026-10-24. This is a shared M7 prerequisite, not
endpoint approval, factory promotion or completed catalog delivery.

Keep the existing endpoint builder, destination policy, runtime metadata shape
and memory limits. Do not introduce a template engine, runtime registry scan,
new crate or bidder-name switch in shared code.

## Declaration And Generation

Each adjacent `bidder.toml` may declare a nonempty mapping:

```toml
[endpoint.fields]
supply_id = "SupplyId"
```

Field names must be lowercase nonkeyword Rust identifiers. Placeholder names
must start with an ASCII letter and contain only ASCII letters, digits and
underscores, unique within the implementation. Reject
`Host` and sync/privacy placeholders: authority selection and sync rendering
retain their existing separate contracts. Empty declarations, unknown table
keys and nonstring values are build errors. Omit the table when unused.
Reserved sync names include `ExternalURL`, `SyncerKey`, `BidderName`, `SyncType`
and `UserMacro` as well as bidder-syncer/privacy placeholders.

The same discovered declaration generates borrowed typed bindings under
`auctioneer_config::bidder_endpoint::<implementation>::Values<'a>`. Each public
field is `&'a str`; constructing a binding requires naming its fields. A
generated `as_pairs()` returns a fixed stack array of literal placeholder names
and borrowed values for the existing `EndpointBuilder::from_request` API.
Do not derive a value-revealing `Debug`, allocate a map, decode runtime JSON or
introduce a second editable mapping. Use the existing `quote`/`syn` build tools.

Reference-only packages may declare types without gaining runtime eligibility.
The importer emits no endpoint declaration for unreviewed reference packages.

## Ownership And Runtime Behavior

Every public definition is checked against its owning implementation's declared
placeholder names, including inactive definitions. A definition cannot extend
that contract. Its generated `BidderInfo.endpoint_macros` still contains only
names actually present in its template, not all names supported by the family.
Aliases inherit the resolved implementation's contract. An undeclared name is
a build failure, even when a different implementation declares that name.

Existing implementations using the shared AccountID/PublisherID binding helper
declare those capabilities explicitly. There is no implicit global endpoint
placeholder allowlist. `Host` stays an authority-only special case with a baked
allowlist; declaring a path placeholder cannot relax scheme/host policy.

Bidder code owns precedence, defaults and required/nonempty value rules. TTD
selects its impression SupplyId before resolved host settings and refuses an
absent, empty or unsafe result with a fixed bidder-local diagnostic. It passes
the typed binding's stack pairs to the builder inside the funded construction
path. Binding types do not themselves prove that arbitrary input values are safe.
Endpoint fields describe computed builder inputs, not automatic host-setting
lookups; the same field spelling in both tables does not couple their sources.

The builder continues to validate template membership/completeness, bound input
and output, reject traversal, encode values and write into reserved metadata.
No placeholder value can supply an authority. Existing rollback and shared
destination validation remain unchanged. Existing implementations may retain
their shared context helper; protocol-specific callers use generated bindings.

## Evidence

Test arbitrary future implementation/field names rather than a TTD-only switch.
Vary declaration and template names independently, include a positive same-name
control, and prove undeclared, reserved, duplicate and malformed names refuse.
Check strict declaration parsing, inactive definitions and cross-owner leakage.
Generated-code controls check field types, literal mappings and no-declaration
output; a real consumer compiles and uses the generated TTD binding.

Exercise the existing bounded builder with hostile typed values, exact/one-over
metadata output, missing template membership and traversal. Retain existing
authority and sync separation controls. Verify source/retained integrity,
secure/default and feature lanes, allocation bounds and strict lint before
publication. No budget coefficient, workload ceiling or external capture changes
are part of this prerequisite.

# Bidder implementation and separate settings subconfig

## Requested contract

The maintainer selected this exact structure for every bidder:

```toml
[bidders.mocktioneer]
enabled = true
implementation = "mocktioneer"

[bidders.mocktioneer.settings]
endpoint = "https://mock-bidder.example.test/openrtb2/auction"
```

`implementation` sits on the bidder entry beside common controls. The separate
`.settings` subconfig contains only bidder-owned fields. Microsoft, The Trade
Desk, Rubicon, Mocktioneer and future discovered owners use this same structure.
Each owner retains its concrete settings type, allowed fields and validator.

The selector keeps its original schema-selection and binding role. It neither
registers nor activates an implementation. Omit both selector and settings for
an entry containing only common overrides. If either is present, require both;
`settings = {}` is valid only for an owner whose optional fields permit it.
Unknown selectors, conflicting operator identities, unknown fields and malformed
present settings must fail closed rather than silently becoming absent settings.

This extends #124 and the Mocktioneer runtime configuration work in PR #160.
It is a pre-release authoring-format change in the current application schema.
Operators must republish old wrapped documents after upgrading. Historical
evidence and source captures retain their original bytes.

## Implementation

Keep the original generated `BidderSettings(ImplementationSettings)` enum,
owner `Settings` structs, borrowed accessors, fieldwise fallback and semantic
validators. Its adjacent Serde tag becomes `implementation` with content named
`settings`, replacing `values`.

The shared generic `BidderOverrides<Settings>` serializes that typed envelope
as sibling fields at the bidder entry through `flatten`. Its Rust settings
field, ownership and validation APIs do not change. Deserialization uses a
private strict adapter instead of flattened Option decoding, which can suppress
errors. The adapter calls `deserialize_struct` with exactly `implementation`
and `settings`, peeks/replays one closed static key, and delegates the claimed
map to the existing typed enum. It returns None only when neither key exists;
all present-object errors propagate. The outer strict model rejects unclaimed
row fields. No JSON value tree, settings clone, inferred owner, new public API
or dependency is introduced.

The outer generated object guard and owner-local `deny_unknown_fields` remain.
Shared code enforces common bounds and implementation binding. Bidder owners
retain scalar types, credential-reference rules, requiredness and endpoint
policies. Existing admission validates both requested and fallback settings
before semantic callbacks; request aliases cannot reinterpret another owner's
typed settings. An owner field named `implementation` is in a different nested
namespace and does not require a new declaration restriction.

Update active readers, writers, tests and examples together. Reject the old
`.settings.implementation` / `.settings.values` operator shape. Historical
capture readers strictly check their recorded envelope/owner and convert only
at the fixture boundary; production does not accept a legacy format.

## Alternatives considered

- Required selector at the bidder entry with separate typed settings is the
  selected layout. It matches the maintainer's final example and preserves
  independent owner schemas and validation.
- Putting the selector inside `.settings` does not match that example.
- Optional field-based inference changes the discriminator's behavior and
  introduces ambiguity. A flattened Option without the strict adapter can
  swallow malformed supplied settings and is unsuitable.

## Verification and resource acceptance

Serde flattening buffers envelope fields before owner decoding. An unchanged
Rust layout does not establish equivalent decoding peaks. Run existing config
peak/lifecycle, SDK catalog growth, CLI resource, native construction and guest
controls without changing limits, workload sizes or immutable evidence.
Serialize heavy Cargo jobs and freeze source edits during identity checks.

Acceptance requires:

- Exact requested TOML and equivalent JSON round trips for all four owners,
  actual typed CLI validation/push dry-run and local Fastly/Viceroy execution.
- Omitted pair versus partial pair, null/wrong-type/unknown selector, malformed
  settings, duplicate selector/settings/common fields and unknown row fields.
- Selector/settings in either order, with common controls between them.
- Missing/unknown owner fields, scalar coercion, nonobject input, old wrapped
  format rejection, independent semantic validators and redacted diagnostics.
- Existing disabled/shadowed, alias, per-field fallback, request retargeting,
  credential and side-effect ordering contracts.
- Original resource ceilings and workloads, independent implementation review,
  format/import/raw-fixture gates and inspection of the new CI run separately.

## Review and authorization

The maintainer's final exact table places `implementation` beside `enabled`,
with bidder-owned fields under the separate `.settings` table. This supersedes
earlier selector-placement proposals. Independent review checked local Serde
behavior: `deserialize_with` bypasses Option error suppression,
`deserialize_struct` consumes only the two envelope keys, and the outer strict
model rejects leftovers. Verify these properties with executable boundary tests.

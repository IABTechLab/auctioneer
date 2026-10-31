# OpenRTB Field Metadata Derive

Status: approved by AG on 2026-10-27. Implementation follows the
[qualification-first plan](../plans/2026-10-27-openrtb-field-metadata-derive.md).
The qualification-first cutover is implemented; the source scanner is removed.
See the plan's [final execution record](../plans/2026-10-27-openrtb-field-metadata-derive.md#scanner-removal-and-final-acceptance-2026-10-27)
for guard migration and the native/guest acceptance scope.

## Goal And Boundaries

Replace Rust-source inspection in `auctioneer-config-schema::build_disclosure` with
metadata from the actual compiled request types. Keep privacy decisions in the
existing reviewed policy and independently pinned wire inventories. Metadata
describes storage; it must not infer consent rules or authorize disclosure.

No request-field, Serde behavior, scrub policy, bidder protocol, workload limit,
resource coefficient, external capture, toolchain/SDK pin or backward-compatibility change
is part of this work. File discovery, provenance and policy compilation remain
build-script responsibilities. The macro performs no filesystem or network I/O.
The shared source-import rules in `crates/AGENTS.md` remain mandatory.

## Approach

Use a derive plus a small typed metadata interface, rather than moving the
current source scanner into a procedural macro. A macro cannot resolve arbitrary
type aliases itself. Rust trait resolution must establish storage facts, including
aliases across modules and renamed imports.

An attribute-only storage allowlist was also considered. It would require a
second declaration of which fields contain raw JSON and could silently drift
from their actual types; do not use it.

### Ownership

- `auctioneer-macros`: `OpenRtbFieldMetadata`, implemented using workspace-managed
  `syn`, `quote` and `proc-macro2`; inline expansion/input-validation unit tests;
  compiler-level pass/fail fixtures in its own `tests/`.
- `auctioneer-openrtb`: the metadata vocabulary, typed storage traits, derived model
  descriptors and the request-root descriptor. It does not depend on config,
  exchange, privacy or bidder implementations.
- `auctioneer-config-schema`'s `build-support`: consume the compiled descriptors and
  retain membership, provenance, policy validation and Rust emission. Its
  `auctioneer-openrtb` dependency is optional and enabled by `build-support`, not
  by default or `config-model`.
- `auctioneer-config`: consume generated disclosure code and a compile-time model
  agreement assertion. Its ordinary dependency on `auctioneer-openrtb` is acyclic;
  the latter currently depends on none of these configuration crates.

Keep `auctioneer-config-schema` serde-only by default and do not enable build support
on a serving edge. The macro crate has no production dependency on a Auctioneer data
crate. Compiler tests may need a dev-edge back to the real metadata interface;
qualify that actual Cargo graph, not just the acyclic production edges.
Do not add another metadata/registry crate.

### Metadata Contract

The derive accepts owned, nongeneric named-field structs. It emits associated
constant descriptors, not request-time reflection or request-time allocations. Each
descriptor records the explicit wire-object name, a diagnostic Rust source label,
named wire fields, flattened fields, storage facts and links to child models.
Native collection compares compiler-provided `TypeId` values to distinguish
declarations; source labels can coincide inside macro expansions and are not a
uniqueness proof. Neither mechanism classifies a field by type spelling.

Use `#[openrtb_metadata(name = "...")]` for catalogue names differing from Rust
names, including `Eid`/`EID`, `EidUid`/`UID` and `BrandVersionFields`/`BrandVersion`.
Support an explicit metadata crate path for expansion in the owning crate and
compiler fixtures; do not require private-source imports or a new registry list.

Every field type must implement the storage contract. There is no blanket
"unknown means typed" implementation. Supply explicit leaves for existing
primitive/string/JSON types and typed propagation through `Option`, `Box`, `Vec`,
`JsonField` and the maps used by the request model. Compiler resolution, not
source-token matching, determines aliases and wrappers. In particular:

- `RawValue` remains raw through the supported wrappers, including unsized raw
  values behind `Box`; raw storage is never reported as traversable typed JSON.
- A named structured model remains typed. Raw storage in one child marks that
  child's slot, not every ancestor containing the model.
- A named `serde_json::Value` field remains non-raw, preserving today's policy
  validation; it does not invent child-model membership.
- Only a directly flattened `serde_json::Map<String, serde_json::Value>` proves
  arbitrary unnamed JSON members. A type alias to that exact map is equivalent.
  Typed flattening, other map owners/value types and wrapped maps must not acquire
  this capability. Refuse unsupported flattening with a specific diagnostic.
- A non-flattened map proves only its named field. Container wrappers do not
  propagate the direct-map catch-all capability.

A flattened field is not an additional named JSON key. Duplicate named wire
fields, multiple catch-alls, invalid metadata names and unknown macro helper
options are diagnostic-specific compile failures.

The derive records all potential serialized fields, including conditionally
omitted ones. Preserve source order in each descriptor; collectors may sort
lookup tables without changing serialization. Honor simple symmetric Serde
renames and raw Rust identifiers. Accept ordinary defaults and
`skip_serializing_if`. Reject unsupported wire-changing forms rather than ignore
them: directional renames/skips, `rename_all`, custom field codecs and container
serialization conversions require a separately tested extension.

`BrandVersion` needs an explicit, small forwarding implementation to its existing
`BrandVersionFields` wire model: its private `fields` storage is not a JSON key.
`DebugSignal` is an explicitly typed scalar leaf, not an invented object schema.
Keep these adapters beside their types and test them against actual Serde output.
Do not generalize this first derive into an enum/custom-codec framework.

### Complete Graph And Target Agreement

Start collection from the real `BidRequest` descriptor and follow child links.
There is no directory scan, global linker registry or handwritten model list.
Deduplicate repeated models and reject distinct Rust declarations sharing a wire
object name. Missing storage or child models still fail policy validation.
Recursive model graphs are outside this first contract: they must fail closed,
not produce a partial graph. Existing request types are finite owned models.

Keep descriptors available to compilation on every target, not only the build
host; there is no serving feature that can opt out of the agreement check.
Conditional compilation must describe the active fields without referencing
inactive types; the macro must not evaluate cfg expressions itself. Prove this
with compiler fixtures before relying on it.

The build compiler emits a structural expected-model signature with disclosure
code. A const assertion compares it against the target's compiled request graph,
including every reachable object/field, diagnostic source label, child link,
raw-storage flag and catch-all flag.
Compiler type identities are process-local, not a portable cross-target format.
The structural assertion does not distinguish a substitution with identical
labels and identical wire structure; the native collector still rejects distinct
declarations sharing a wire-object name, including ancestor/descendant conflicts.
Compare structures, not a lossy custom hash or type-name heuristic. Changes hidden
behind target cfg must fail this assertion even when the host policy check passed.
Do not silently accept host/target disagreement or replace it with a warning.
This is a disclosure-metadata consistency check, not a target ABI, memory or
privacy-policy oracle. It must not replace the independent raw-slot and policy
oracles below.

First qualify with the workspace-pinned toolchain, not a newer unpinned compiler.
Qualify the constant descriptor graph and structural assertion on native
and all three guest targets. If stable Rust cannot express this design as stated,
stop the cutover and revise the contract; retaining the scanner is preferable to
removing its protection without a replacement.

The serving use is const evaluation only, not a request-side metadata walk.
Native build/CLI tooling may collect descriptors. Do not claim zero retained
bytes solely because code uses constants; check generated artifacts and existing
resource-profile controls without changing allowances to obtain a pass.

## Verification And Cutover

1. Prove an acyclic real consumer build, compiler alias resolution, conditional
   fields and the host/target const agreement check before changing live policy
   compilation. The scaffold's zero tests are not evidence for these properties.
2. Add positive and negative compiler tests using the real metadata interface,
   not a copied stand-in. A native-only standard compiler conformance runner may be added as
   a workspace-managed dev-dependency; use diagnostic-specific failures and
   bounded CI execution. Ordinary `cargo test -p auctioneer-macros --all-targets`
   must run the compiler fixtures without an opt-in feature or ignored tests.
   No source-path imports are permitted.
3. Freeze the independently expected real raw-slot set: `App`, `Audio`, `Banner`,
   `BrandVersion`, `Dooh`, `Geo`, `Native`, `UID`, `UserAgent`, `Video`, each at
   `ext`. Preserve the existing hand-written disclosure descriptor oracle and
   exact policy/inventory coverage. Expected values must not be generated by
   the derive under test.
4. Cover alias/reexport/generic-alias resolution, shadowed type names, renamed
   fields, direct versus wrapped/non-flattened maps, missing type implementations,
   duplicate object/field names, unknown attributes, unsupported forms and custom
   wire adapters. Include paired raw/typed and opaque/child policy controls.
5. Map every existing source-scanner test to a compiled-metadata or compiler
   control before removing it. Preserve relocation/missing-model refusals, exact
   ten-slot coverage and the real new-field/exhaustive-scrub mutation harness.
   Compiler rejection may replace a parser rejection only with an explicit
   diagnostic-specific successor. Do not use zero-match filters or delete a guard
   because the old scanner helper disappeared.
6. Compare the complete generated disclosure descriptors and privacy outcomes
   with the existing independent oracles. Run secure and development privacy
   suites, composition/differential/allocation controls, model mutation harness,
   guest checks, strict Clippy and source-isolation gates. Keep historical captures
   byte-identical. Then remove source-walking/alias-spelling classification from
   production build support, retaining no alternate permissive fallback.

Implementation and promotion require the written contract's review. This work
does not resolve the separately tracked M7 schema/resource qualification or
hosted-readiness blockers.

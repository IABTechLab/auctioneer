# Bidder Reference Inventory

The offline inventory records source facts, not runtime registration,
endpoint approval, migration completion or live certification. This directory
contains documentation and review reports. Shared catalog witnesses live under
[static bidder references](../../crates/auctioneer-bidder/tests/conformance/fixtures/reference/catalog/); schemas,
protocol captures and local conformance cases live beside their bidder.
Runtime builds do not read the shared reference captures or this directory.
AG approved one bidder-owned folder on 2026-10-23; the
[migration plan](../superpowers/plans/2026-10-23-bidder-owned-assets.md)
tracks moving metadata, schemas, captures and TOML provenance together while
preserving the distinction between reference-only and promoted inputs.
Relocation is not promotion.

The source project is [Prebid Server](https://github.com/prebid/prebid-server),
maintained by Prebid.org and its contributors. The source revision is
`d7f8515b86258688304b0d9b6668c6a0e258bc9e`.
Attribution covers its Apache-2.0 licensed
[metadata](https://github.com/prebid/prebid-server/tree/d7f8515b86258688304b0d9b6668c6a0e258bc9e/static/bidder-info),
[parameter schemas](https://github.com/prebid/prebid-server/tree/d7f8515b86258688304b0d9b6668c6a0e258bc9e/static/bidder-params)
and [implementation tree](https://github.com/prebid/prebid-server/tree/d7f8515b86258688304b0d9b6668c6a0e258bc9e/adapters).
See the [project acknowledgements](../../README.md#acknowledgements) and
[NOTICE](../../NOTICE) for shared credits and retained copyright notices.

Use a local checkout containing that revision:

```sh
cargo run --locked -p auctioneer-cli -- bidders inventory \
  --source /path/to/reference-checkout \
  --output /path/to/new-inventory.toml
AUCTIONEER__BIDDER__SOURCE_DIR=/path/to/reference-checkout \
  cargo test --locked -p auctioneer-cli --test bidders_source --test bidders_import
cargo run --locked -p auctioneer-cli -- bidders check \
  --source /path/to/reference-checkout
```

Missing source prerequisites fail the tests; they are not skipped. CI uses
its own `vendor/bidder-reference` checkout, separate from other fixture pins.

The command reads pinned Git tree/blob objects, not working files or `HEAD`.
Dirty files and replacement refs cannot change the result. Each Git child
has a 10-second deadline, 4-MiB combined output ceiling and 64-KiB stderr
ceiling; a source inspection has a 120-second deadline and 64-MiB aggregate
read ceiling. Refusals emit fixed diagnostic codes, not source contents.
Output is published from a synchronized temporary file without replacing
an existing artifact. Invalid input, source refusal and output failure exit 2.

`bidders check` compares the complete parsed inventory with fresh pinned
objects. `--inventory PATH` selects a different artifact; by default it uses
`crates/auctioneer-bidder/tests/conformance/fixtures/reference/catalog/source-inventory.toml`.
Unchanged reference facts exit 0; drift
exits 1; malformed, oversized, linked or unreadable inputs exit 2. Diagnostics
never echo parser text or input bytes. Object key order does not matter, but
unknown fields, omitted or duplicate identities, changed provenance and
forged migration fields fail checking.

Its JSON report explicitly has `scope = reference_inventory_only` and
`migrated = 0`. `--require-complete` currently exits 1 even for an intact
inventory: source integrity alone cannot supply protocol review or current
producer receipts. Later ledger/evidence work must replace that refusal with
evidence-derived migration accounting, not trust a hand-written status.

The version 2 inventory contains 358 identities (261 bases, 97 aliases), 259
implementation-directory keys and 261 schema witnesses. The schema source
bytes total 164,518, with a 3,778-byte maximum. Metadata field names remain
visible for the later semantic mapping audit. Directory keys do not imply a
public-identity-to-factory binding; that mapping requires its separate audit.
The committed [source-inventory.toml](../../crates/auctioneer-bidder/tests/conformance/fixtures/reference/catalog/source-inventory.toml) is compared
byte-for-byte with fresh pinned-object output by the CLI integration test.

## Reference Import

Schema-engine preparation is recorded separately in
[schema-qualification.md](schema-qualification.md). The inventory and native
compiler controls do not certify the engine or enable validation in auctions.

`bidders import` produces bidder-owned reference packages, not callable definitions:

```sh
cargo run --locked -p auctioneer-cli -- bidders import \
  --source /path/to/reference-checkout --staging /path/to/new-reference
cargo run --locked -p auctioneer-cli -- bidders import \
  --source /path/to/reference-checkout \
  --staging crates/auctioneer-bidder/src/bidders --check
```

The original catalog has 259 implementation folders with 358 source metadata
witnesses and 261 byte-identical source schemas. Each `bidder.toml` holds
the immutable source witnesses; metadata YAML is read from the pinned external
Git checkout, not duplicated locally. There is no second JSON provenance manifest.
A fresh import contains 521 files, with `factory = false` and no effective
definitions. Primary schemas are `parameters.json`; independently sourced bases
sharing an implementation have explicit `schemas/<public>.schema.json` paths.
Metadata notes preserve relevant source interpretations without inventing TOML nulls.
The shared [source license](../../crates/auctioneer-bidder/src/bidders/LICENSE.reference) retains its
copyright notice. The license blob is separately pinned to
`b7fa401fa4b79b208b1dbc4679b09b8f4691adbf` at the same source revision.
Git attributes preserve every captured artifact and witness file byte-for-byte
across checkout newline settings. Raw schemas also retain their original
whitespace without normalization.

Historical source-ignored spellings are identified by source witnesses and named
in the TOML artifact's notes: `usersync`, top-level `enabled`, sync `default`,
`maintainer.gvlVendorID`, and `openrtb.gpp_supported`. They are not corrected
into effective sync, vendor or GPP settings. Every other unknown field fails
conversion, including nested fields. This is source preservation, not a
replacement for runtime parameter validation or reviewed inheritance.

Import refuses every existing destination, including an empty directory.
`--check` writes nothing and compares only the pinned source projection; it
preserves reviewed code, definitions and supplemental captures rather than
equating a serving package with a newly generated non-callable package.

Before the first YAML tree is constructed, the native-only `yaml-rust2
0.11.0` event parser rejects anchors, aliases and tagged data, and enforces
64-KiB input, 8,192 nodes (including keys), and 32 containers. Serde YAML
0.10.0 then converts the bounded tree. Duplicate/merge/non-string keys and
multiple documents are refused. Quoted indicators, comments and block text
remain ordinary data. Native staging is bounded to 16 MiB total and 1 MiB
per emitted file; individual raw schemas remain limited to 16 KiB. These
are CLI admission limits, not measured runtime/isolate memory charges.

Normal import publishes a fresh directory atomically without replacing even
an empty destination. Failed construction leaves no partially published
tree. Publication currently requires the Linux/macOS no-replace rename
primitive; other native platforms refuse rather than use an overwrite-prone
fallback. `--check` creates or repairs nothing: a matching source projection
exits 0, missing/changed/unowned content exits 1, and unsafe inputs or I/O refusals exit 2. Both modes retain the existing bounded, offline Git-object reader.
Check mode walks real directory handles without following links, including
directory components hidden by trailing slashes or `/.`. Supply an actual
non-symlink path; for example, use `/private/tmp` rather than macOS's `/tmp`
alias when checking a snapshot there. Parent-directory components are refused;
use a confined relative path or its actual absolute path.

Every imported source-only package still awaits runtime promotion. Static HTTPS,
source activation and implementation sharing grant no routing approval;
white-label bases are alias-only, HTTP endpoints unavailable, and other
endpoints await reviewed policy. Promotion, opt-in cohorts, source-default
reclassification and code generation remain separate pending plan steps.

## Sync Families

The source defines 196 sync identities in 187 families; 162 identities have
no defined contract. The 196 comprise 188 own declarations and eight
inherited keys. Of the 162 without a contract, 38 carry support hints and
124 have no effective declaration. These counts precede source activation,
white-label, endpoint safety, host configuration and privacy checks.

Only the exact source `userSync` field participates. A nonempty key,
iframe/redirect object, external URL, format override or skip condition
defines a source contract; support hints and the enabled flag alone do not.
An alias without its own block inherits only the parent's family key, not
the parent's endpoint/settings object. An explicit empty block suppresses
that inheritance. Each public identity retains its own vendor ID, defaulting
to zero when absent; vendor identity never follows family inheritance.

`sync_family` records exact case-sensitive keys, members and owners of
iframe/redirect objects. An external URL alone is not an endpoint provider.
This inventory neither selects a provider nor approves a template, grants
sync permission or changes the shipped cookie behavior. Semantic import
must still validate templates, whole-contract compatibility and runtime
eligibility. The pinned rules come from
[metadata resolution](https://github.com/prebid/prebid-server/blob/d7f8515b86258688304b0d9b6668c6a0e258bc9e/config/bidderinfo.go)
and [family construction](https://github.com/prebid/prebid-server/blob/d7f8515b86258688304b0d9b6668c6a0e258bc9e/usersync/syncersbuilder.go).

## Factory Capture

[source-bindings.toml](../../crates/auctioneer-bidder/tests/conformance/fixtures/reference/catalog/source-bindings.toml) captures all 261 base bindings
to 259 source directories. It records both the factory-table blob and the
public-name definition blob, including the two shared implementations:
`mediafuse` uses `adapters/appnexus`; `emx_digital` uses
`adapters/cadent_aperture_mx`. This is reference evidence, not a production
constructor list. Runtime builds do not consume it; automatic production
discovery remains a later implementation step.

The capture is explicitly **pending human review**. The capture date is not
an approval date, and no reviewer has been invented. Set/mapping regression
tests cannot substitute for that review. Import/promotion must not treat this
pending file as approved factory evidence.

The planned runtime contract keeps all bidder implementations in the existing
`auctioneer-bidder` crate, under `src/bidders/<bidder_key>/mod.rs` with adjacent
`bidder.toml` declarations and `create_bidder() -> Box<dyn Bidder>` factories.
Public concrete types use `FooBidder`, without compatibility aliases. Internal
keys use the design's deterministic source-directory mapping, with `bidder_`
for digit-leading results and `_bidder` for Rust keywords. Public names remain
exact and distinct from those keys: public `yahooAds` binds through
`implementation = "yahoo_ads"`; in general each baked base declares
`implementation = "<bidder_key>"`, and multiple bases may share a key.
This is a pending discovery/promotion contract, not shipped registration.
Platform crates `auctioneer-adapter-{axum,cloudflare,fastly,spin}`, provider wording,
exact source paths and provenance, wire identifiers such as `baseadaptersonly`,
and imported-schema terminology remain unchanged.

Exactly 78 bases declare top-level HTTP endpoints: 77 source-enabled and
one source-disabled. Their URLs are provenance, never permission to send a
request. Endpoint approval, effective routing inheritance, factory-binding
review and the evidence-derived migration ledger remain
work in the [implementation plan](../superpowers/plans/2026-10-16-m7-bidder-catalog.md).

## Bidder Layout

The canonical [implementation rules](../../crates/auctioneer-bidder/src/bidders/README.md)
live beside the bidders. Each implementation owns `bidder.toml` for registration,
effective public metadata, aliases and provenance, its JSON parameter schemas,
inline unit tests and local fixtures. Only conformance tests use separate files.
Shared test machinery and cross-bidder inventory remain shared, not another
editable catalog. The approved asset migration is pending; current readers must
be switched together before its old paths are removed. Registration and protocol
work use the existing shared boundaries; adding a factory does not promote metadata.

## Discovery Preparation

The build-only [discovery generator](../../crates/auctioneer-config-schema/src/bidder_discovery.rs)
validates adjacent declarations under a fixed `src/bidders` root, records
rebuild inputs and renders direct module includes plus sorted declaration and
feature-filtered factory tables. It has no central list of bidder names and
does not execute constructors while discovering or rendering.

```sh
cargo test --locked -p auctioneer-bidder --test discovery_codegen
```

The native controls compile and execute synthetic factories, including nested
and sibling modules from another working directory and feature off/on/off.
This is preparation, not automatic registration in the deployed server: the
production build script, funded catalog join and incremental Cargo rebuild
proof remain pending. The existing live registry and catalog are unchanged.

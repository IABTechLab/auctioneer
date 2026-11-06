# Bidder Implementation Rules

Keep all implementations in this crate, using the same `Bidder` contract and
shared infrastructure. Different protocols need different transformations,
not different registration, transport, budgeting or test frameworks.

The [shared crate rules](../../../AGENTS.md) apply here: handwritten `#[path]`
and source-file `include!` imports are forbidden, including within this crate.
Only generated-code includes are allowed.

## Configuration Validation

An executable owner with a manifest `settings` declaration supplies ordinary
`mod config;` wiring and an owner-level `validate_config` in `mod.rs`.
Discovery derives this requirement from settings plus factory ownership; it
emits a compiler-checked reference with the factory's existing eligibility.
Settings-free owners need no config module or callback, and reference-only
definitions do not become executable validation providers.

`validate_config` is only a thin adapter to the bidder-local borrowed view's
`validator::Validate`. Generated settings stay in `auctioneer-config`; do not
add a settings DTO, implement an external trait on those external types, or
introduce validation constraints in TOML. The one `BidderConfigInput` borrows
requested settings, base settings and optional resolved metadata, together
with caller-resolved enablement. Validate both supplied entries even when
disabled or overridden. Check intrinsic requiredness only for an enabled,
resolvable identity, using existing fieldwise fallback. Preserve null, zero,
empty, optional defaults and valid impression/request-supplied values.

Callbacks must not construct bidders, decode requests, build outbound calls,
perform I/O, resolve secrets, inspect environment or install mutable globals.
Reuse actual pure protocol predicates when needed. Return only reviewed static
`BidderConfigError` field/reason identifiers; do not format or retain validator
value parameters. An owner with no additional semantic rule records that fact
in its local view instead of inventing required deployment fields or ranges.

Validated definitions and implementation lookup retain callbacks in the existing
catalog indexes, not a second registry. Shared admission/orchestration belongs
to core; these callbacks are not activated by the old serving policy reader.
Request-local aliases retain their request-time binding and effective checks.

Task 4 must account for the declaration's derived settings flag, the optional
function pointer added to each registration and validated definition, and the
optional pointer stored alongside each constructor in the existing factory
index. The temporary feature-index value also gains the derived settings flag.
These layouts grow when registry validation builds its indexes even before
callbacks are activated; the metadata census does not automatically fund them.
Alignment and B-tree node layout remain target-dependent. Local
`ValidationErrors` can allocate on failure and must also be bounded/funded
before serving activation. The config-owned `MetadataStorageDescriptor` census
does not cover these bidder-owned tables or registry-plan B-trees. Catalog cost
aggregation consumes separately qualified owner contributions; it does not
automatically inspect these layouts. The registry owner's contribution and
startup peak therefore need Task 4 accounting even before callbacks are invoked.
This contract is not resource-readiness evidence.

## Source Layout

```text
src/bidders/<implementation_key>/
  bidder.toml       # registration, public definitions, aliases and provenance
  parameters.json   # canonical parameter schema, when the bidder has one
  mod.rs            # factory, Bidder trait wiring and conformance declarations
  request.rs        # request construction, borrowed transforms and inline units
  response.rs       # sealed decoding, normalization and inline units
  uid_policy.rs    # optional family-specific sync UID policy, independently discovered
  tests/
    conformance.rs  # optional source/protocol conformance, same-crate test module
    allocation_conformance.rs # optional native allocation conformance, discovered
    response_conformance.rs # optional sealed-response conformance, discovered
    schema/         # optional source/compiler controls, discovered
  fixtures/         # bidder-specific protocol/parameter test captures
    factory.json    # locally authored discovered-factory conformance cases
    privacy.json    # authored protocol facts when linked privacy-wire tests use this bidder
```

This is the authoritative target layout approved on 2026-10-23, with metadata
YAML removal approved on 2026-10-24 and the uniform request/response module
layout approved on 2026-10-30. The
[asset migration plan](../../../../docs/superpowers/plans/2026-10-23-bidder-owned-assets.md)
tracks the coordinated cutover. Config, factory discovery, import and schema
qualification now consume the bidder-owned manifests, schemas and captures.
Do not treat metadata relocation as completed catalog delivery or qualification.

- Every executable bidder uses ordinary `mod request;` and `mod response;`
  declarations. Keep `mod.rs` compact: factory, `Bidder` trait wiring and
  owner-local conformance declarations. Keep construction and borrowed wire
  transforms in `request.rs`, and sealed decoding and normalization in
  `response.rs`, including for small bidders. Reference-only packages do not
  acquire executable modules by copying this layout.
- Add parameters, native or projection modules only for a genuine cohesive
  responsibility, not to meet a file-count target. Use narrow `pub(super)` only
  where sibling modules actually need access; do not expose private serving
  internals for tests.
- Keep unit tests in inline `#[cfg(test)] mod tests { ... }` beside the code they
  test in its owning file, never a separate `tests.rs`. A small root test context
  helper may support owner-local conformance without moving ordinary units into
  it. Do not add empty helper files, fixture directories or invented schemas.
- Only conformance tests belong in separate test files under `tests/`. Wire
  private-code controls through ordinary child declarations such as
  `mod conformance;` inside the existing inline `#[cfg(test)] mod tests` block;
  retain native and factory-feature gating. Do not expose private serving code
  or compile the serving module again to make a test reachable. Cross-crate,
  transport and allocator/process conformance stay with their actual boundary
  owners; a global counting allocator must not affect the whole unit suite.
  Do not rename ordinary units to conformance just to move them out.
- Cargo test targets stay within their owning crate. Never point core or
  exchange at a bidder's source file, recompile private serving modules, or hide
  that coupling behind generated includes. Shared HTTP and normalized-response
  conformance uses the capable fake; provider decoders and transforms retain
  their own bidder-owned proof under `tests/`. Do not compile another crate's
  production or build source into a test; use independent test-only readers.
  Crate-level conformance entrypoints live directly under `tests/`, with immutable
  multi-provider captures in `tests/conformance/fixtures/`. Generic consumers must
  not select by real names.
- Shared tests under `tests/` use the single `auctioneer_bidder::test_support::fake_bidder`
  capability implementation, synthetic schemas and synthetic metadata. Do not
  import a production bidder constructor, branch on a production identity, or
  use a production protocol as a convenient fixture. Extend the fake when a
  shared test needs a supported capability; do not add another fake bidder
  implementation to that test. The fake must exercise the real sink, endpoint,
  metadata, finalization and sealed parsing APIs, not bypass them with canned
  results. Its controls cover aggregate/split/chunked/filtering behavior, all
  four media, errors, browser configurations and observable factory execution.
  The [capability inventory](../../tests/support/fake_bidder/README.md) records
  the shared contract covered by that fixture.
- The shared fake uses the same compact root plus ordinary request/response
  modules. Its optional controls module owns synthetic metadata and capability
  settings. Keep it under feature-gated `test_support`, outside serving
  discovery, with inline helper units and public-boundary conformance under
  this crate's `tests/`. Do not add a bidder manifest or register it as a
  production implementation merely to enforce its source layout.
- Apply that rule across the workspace, including inline unit tests. Upper
  layers reuse this same fake through the bidder crate's `test-support` feature and explicit
  local registries. Lower layers use synthetic names and metadata without
  depending upward on bidder implementations. Do not register the fake in a
  serving catalog, expose a new runtime injection hook, or add another fixture
  crate or sharing mechanism. Keep captured-source, shipped-catalog and real-binary
  conformance explicit: those tests must continue to exercise actual inputs,
  not acquire fake expected answers. The
  [workspace isolation design](../../../../docs/superpowers/specs/2026-10-25-workspace-test-bidder-isolation-design.md)
  defines that boundary.
- All synthetic bidder identities and aliases outside their real bidder owner
  start with `fake_`: for example `fake_bidder`, `fake_consented_bidder` and
  `fake_alias`. Apply this to JSON/TOML, scratch registries and generated test
  cases too. Do not invent unprefixed fixture brands or put `fake_` in the
  middle of a name. Preserve literal reserved/invalid inputs only where the
  test specifically exercises their protocol refusal, and preserve immutable
  external source captures as evidence rather than fabricated fake captures.
- A real App or linked-binary test selects required roles from published
  metadata, not a handwritten bidder roster. Use the single
  `auctioneer_bidder::test_support::serving_catalog` interface for bidder-owned valid
  parameters and captured request expectations. Do not duplicate protocol
  parameters in core/exchange or replace real serving coverage with a fake.
  Frozen external reference inputs and required typed extension names remain
  evidence, not arbitrary fixture identities to rename.
- Keep production-specific allocation evidence in bidder-local
  `tests/allocation_conformance.rs`. Existing build discovery automatically includes
  it in the isolated native counting-allocator runner with the factory's feature
  ownership. No central module list or per-bidder Cargo target is needed. Shared
  infrastructure tests use the fake; automatically discovered local conformance
  still exercises the actual factory. Never replace a production resource proof
  with a passing synthetic example or drop its independent positive controls.
- Keep bidder-specific sealed-parser controls in bidder-local
  `tests/response_conformance.rs`. Discovery uses the same factory feature ownership
  as production and includes them in the response conformance target. Keep
  reviewed source/compiler cases in bidder-local `tests/schema/`; the source
  conformance target must still exercise the complete real catalog, separately
  from the fake's small compiler controls.
- Keep all bidder-specific metadata, schemas, provenance and fixtures in this
  folder, not separate metadata/schema collections. `bidder.toml` is the single
  editable owner for registration, reviewed public definitions, aliases and
  provenance; do not maintain a second JSON provenance manifest. Do not retain
  duplicate metadata YAML or a retained-path/digest witness for metadata.
  JSON schemas and protocol/parameter captures remain byte-identical files,
  not another effective configuration.
- Multiple independent public schemas sharing an implementation live under
  `schemas/<public_name>.schema.json`, with explicit local associations in
  `bidder.toml`; aliases may reference their canonical schema without copying it.
  Shared cross-bidder inventories, legal notices and test infrastructure remain
  shared, but must not become a second editable bidder catalog.
- Use `parameters.json` for a canonical schema and `fixtures/parameters.json`
  for captured parameter test cases. These are distinct document shapes, not
  two names for the same input. Do not add empty captures or an invented schema
  just to make directory listings match.
- Keep source attribution and shared license notices in the
  [root acknowledgements](../../../../README.md#acknowledgements), not a new
  license file in each bidder folder. Existing byte-pinned license captures
  move with their integrity readers during the coordinated asset cutover.
  Credit Prebid Server and its contributors by name when adapting their
  implementations, metadata, schemas or captures; keep exact provenance and
  intentional port differences in the owning `bidder.toml`.
- Use the shared test harness and `protocol_fixtures!` macro in
  `src/bidders/mod.rs`. Add reusable test machinery there, not a copied macro
  or sink/response framework in every bidder. Keep protocol-specific assertions
  and reviewed source differences local. Shared factory-contract support lives
  in `src/test_support/factory_contract.rs`, loaded only by the same crate's native
  test module. From bidder-local test sources, use `../fixtures/` and
  `../bidder.toml` paths; the captures and manifests stay in their owning folder.

## Registration And Configuration

- Declare the implementation key, explicit factory presence and any required
  Cargo feature in adjacent `bidder.toml`. The directory name must match the key.
  A reference-only package has no executable factory or promoted definitions;
  folder presence and imported source enablement grant no runtime eligibility.
- Implement `create_bidder() -> Box<dyn Bidder>` and use discovery. Do not add
  bidder-name matches or constructor lists to a central production registry.
  Factories are synchronous, infallible and stateless; no I/O or unfunded cache
  initialization belongs in construction.
- Keep public bidder identity distinct from implementation identity. Aliases
  may share an implementation without sharing vendor identity or permissions.
- Record source repository, immutable revision, source paths/blob IDs, lengths,
  SHA-256 digests and reviewed differences in TOML provenance, including
  source-only metadata witnesses. Keep supplemental source pins distinct and
  mark locally authored definitions honestly. Import and source checks read
  metadata YAML from the pinned external Git checkout but publish neither YAML
  nor retained witnesses for metadata. Record relevant source interpretations,
  including missing/null distinctions and ignored spellings, in TOML notes
  rather than inventing a TOML null or retaining duplicate YAML.
- HTTPS is the default transport. The importer records an HTTP source template
  in metadata-artifact `https_default.source_url`, its scheme-only selection in
  `selected_url`, and `inherited_from` only for an inherited alias endpoint.
  These typed records are checked against immutable source data; they are not
  callable definitions, URL-safety validation or proof of provider connectivity.
  Preserve explicit ports and template/path/query bytes. No separate vendor TLS
  review is required merely to select HTTPS. HTTP needs a scoped exception;
  keep the existing development-only loopback exception unchanged and never
  downgrade automatically after an HTTPS failure.
- Use `source_path`, `source_bytes` and `source_sha256` consistently for artifact
  witnesses and their nested `retained` records. At artifact scope these describe
  the upstream file; inside `retained` they describe the local copy relative to
  the bidder folder. The local copy may differ only where transformations are
  permitted and documented. The old `path`/`bytes`/`sha256` field names are rejected.
- Wrap `bidder.toml` prose, especially `port_differences`, within 100 columns.
  Put long arrays on multiple lines and wrap notes with TOML multiline basic
  strings and line-ending backslashes, preserving the decoded value and spaces.
  Keep URLs and source/file paths intact on a single line, even beyond 100 columns.
- Cover the external implementation and shared helpers, JSON schemas, protocol
  and parameter captures, metadata/aliases and attribution separately. Associate
  the Rust port with its source behaviors and intentional differences, not a
  false verbatim-copy claim. Generated evidence binds actual Rust/generated
  inputs and their dependencies to the tested build; a changed digest alone
  does not renew that evidence or prove a completed port.
- Configuration generation reads only the committed data in these folders,
  never bidder implementation code or constructors. Build outputs belong in
  `OUT_DIR`; no central registration list, runtime directory scan or additional
  workspace crate is needed. The workspace-only sibling data input must also
  be present in isolated build/qualification workspaces.
- Read resolved parameters, settings and endpoint policy through the shared
  request context. Do not invent bidder-local config readers, secret clients
  or hardcoded runtime enablement. Adding code is not metadata promotion.
- Declare non-secret host fields in `[settings.fields]` using the supported
  optional kinds. Config generates strict typed settings and borrowed views
  from this same declaration. Consume `info.settings()` / `base_settings()`
  only after exchange binding checks; apply protocol defaults after independent
  per-field alias precedence. Do not coerce explicit zero/empty into absence
  at the generator layer or log serialized settings.
- Declare a host secret reference with `optional_secret_reference`, not a
  password/token field or an impression parameter. Missing/null is absent;
  present references are 1..=128 UTF-8 bytes without ASCII controls. A bidder's
  pure `credential_key` preflight must reject deterministically unusable work
  before returning the borrowed reference. It cannot perform I/O. The exchange
  checks privacy/parameter admission, prepaid construction scratch, available
  call capacity and the original deadline before polling the request-bound
  credential source. Consume only the supplied coherent `info.credentials`
  bundle; never cache it in a factory, copy it into diagnostics or fetch its
  fields separately. One sequential scratch bank covers construction, with
  retained outbound headers covered by the existing metadata allowance.
- Use borrowed `info.floor_currency` for required floor conversion. Rates come
  from the validated operator snapshot; no bidder-local feed, map copy, fallback
  multiplier or response-currency relabeling. USD identity needs no snapshot;
  foreign conversion requires a valid unexpired rate. Preserve each protocol's
  request-fatal versus impression-local failure policy and test both with valid
  siblings. Full response-currency conversion remains separate work.
- Keep parameter schemas as the constraint source. Use `validator::Validate`
  for applicable typed settings instead of duplicating checks or writing
  another schema engine; protocol-specific normalization is a separate step.
- Declare supported endpoint placeholders in `[endpoint.fields]`, mapping
  lowercase Rust field names to protocol names, for example
  `supply_id = "SupplyId"`. Do not add a central protocol-name allowlist.
  Discovery generates borrowed `bidder_endpoint::<implementation>::Values`
  and its fixed-array `as_pairs()` for the existing endpoint builder. Aliases
  inherit their resolved implementation's contract; definitions cannot extend
  it. Common context placeholders also require an explicit owner declaration.
  Keep value precedence and required/nonempty checks in bidder code, and URL
  encoding, traversal checks and memory caps in the shared builder. A binding
  declaration does not approve a destination or enable a reference package.

## Protocol Implementation

- Use OpenRTB field types and semantics as the baseline, plus each bidder's
  reviewed parameter schema and required protocol extensions. Reference source
  is evidence for those contracts, not a requirement to emulate every legacy
  parser or host-language quirk. Do not add compatibility machinery solely for
  nonstandard numeric spellings or invalid text. Record intentional differences
  in `bidder.toml`; keep malformed-input safety, privacy, bounded work and
  failure-isolation tests even when compatibility with that input is out of scope.
- Use `OutboundRequestSink` for construction. Reserve before retaining bodies,
  metadata or diagnostics; stream borrowed transforms into its capped writer.
  Do not clone the whole request or build an uncapped serialized body first.
- Use the shared endpoint builder and validator. Never interpolate unchecked
  request data into an authority, copy inbound transport headers wholesale,
  or assume changing an endpoint's scheme proves equivalence.
- Preserve caller input and fields not intentionally transformed. Never restore
  data removed by shared privacy enforcement. Review new typed fields when
  changing an exhaustively destructured serializer.
- Decode only through the sealed response API. Use shared media classification,
  immutable sent-call correlation and bounded diagnostics. Preserve valid
  siblings when a failure is candidate-local; keep whole-response failures
  distinct. Status, currency and media rules must follow the reviewed bidder
  contract; intentional source differences must be explicit.
- Preserve `SeatBid.seat` in `TypedBid.source_seat` before flattening parsed seats. Share one immutable owner across the bids from each nonempty seat; missing or empty seats stay absent. Shared response classification does this automatically. Custom decoders must preserve the same rule, never substituting a requested alias, implementation, marketplace label or host provider ID. Exchange-owned wire attribution remains separate.
- Move optional browser-auction configurations from the sealed parsed extension
  into `BidderResponse.auction_configs`. Do not copy their JSON, parse the body
  again, or supply authoritative bidder/adapter labels. The exchange checks
  scoped and exact sent-call impression coverage, supplies identities and funds
  wrapper expansion independently of ordinary bids and currency. Missing or
  empty configuration output stays omitted from the final response.
- Use fixed/redacted errors. Do not echo raw parameters, credentials, request
  bodies, validator text or provider response bodies into diagnostics.
- Do not add a provider HTTP client, execution engine, dependency, crate,
  workload limit or budget coefficient as a bidder-local workaround.

## Test Evidence

- Use the real bounded sink and sealed response wrapper through shared helpers,
  not recording doubles that bypass their ordering and limits.
- Construct ordinary JSON with `serde_json::json!`. Raw JSON literals are for
  lexical/malformed-byte controls or byte-sensitive measurements, with the
  required explanation and exact fixture-budget entry.
- Preserve external capture bytes, source revision, digests and attribution.
  Verify the supplied capture against its pinned witness before protocol
  dispatch, including filtered test runs. Never generate expected answers
  by running the implementation being tested.
- Checkout-dependent primary-source controls use the `source_objects_` test-name
  prefix and `AUCTIONEER__BIDDER__SOURCE_DIR`. The shared CI invocation runs that prefix
  explicitly with `--ignored`; do not add a bidder-specific workflow command.
  Supplemental sources need their own declared checkout and verification owner.
- Vary independent permissions and limits independently. Cover mixed valid and
  invalid candidates, positive results, exact error attribution, immutability
  and boundary refusal. Assert nonzero expected fixture/test counts so a filter
  or operation rename cannot turn coverage into an empty loop.
- Name tests for behavior, not a capability. Run secure defaults and applicable
  feature builds separately. Source conformance, local tests and compilation
  do not establish complete resource qualification or live provider acceptance.

The source-only `registry_source_conformance` target checks inline unit ownership
and ordinary request/response modules against every discovered declaration,
including feature-disabled bidders. The
[shared-contract audit](../../../../docs/testing/bidder-shared-contract-audit.md)
records which construction and status behaviors are shared and which must stay
protocol-specific.

## Conversion Workflow

An ordinary implementation adds or changes only its own folder. Do not extend a
central bidder-name list, schema-pin table, provenance-count map or protocol
switch in a shared test. Shared infrastructure changes are warranted only for a
genuinely new common capability.

Start new work from the deliberately inert
[bidder template](../../../../docs/templates/bidder/README.md), outside discovery.
It has explicit placeholders, no public definitions and `factory = false`.
Copying a template or adding executable code never promotes reference-only
metadata; source, endpoint and public-definition reviews remain separate.

Every factory owns `fixtures/factory.json`. Existing discovery automatically
generates its native `bidders::factory_contract_tests::<key>` test, gated by the same
required feature as the factory. The fixture has a strict versioned shape,
implementation key, frozen manifest SHA-256 and nonempty named cases containing
request/context, exact outbound body/URI/coverage, header expectations and sealed
response expectations. Multiple calls and response outcomes are supported.
Successful response expectations may include `auction_configs` with exact
`impid` and `config` values; omission means the factory must return none.
These are locally authored expectations, not externally captured reference data;
do not list them as immutable source artifacts in `bidder.toml`.
For a protocol field that carries the built package version, declare its exact
JSON pointer in `package_version_paths` and use `$CARGO_PKG_VERSION` there in the
expected body. Only those declared fields are substituted from build metadata;
ordinary protocol expectations must remain literal and independent.

Keep expectations independent of bidder output. When intentionally changing
metadata, review the change and update its local frozen manifest digest; a
digest refresh alone does not establish source or protocol correctness. The
migrated baseline cases retain their original inputs, including unrelated
parameter fields. They are snapshots, not templates requiring future bidders to
copy those fields. New cases should use the bidder's own valid parameters and
explicit preservation sentinels.

Published implementations used by the full-privacy-context wire suite also own
`fixtures/privacy.json`. These narrow, authored Auctioneer contracts declare publisher
substitution, scoped impression-extension replacement, and root-extension output
for absent/present caller input, plus additive protocol metadata for a standard
banner response without a provider extension. They are not upstream captures or snapshots of
the implementation under test. Verify them against the protocol source, including
FPD-to-targeting conversion; do not extrapolate from an unrelated factory example.
The shared test keeps identifier/geo denial assertions and input-presence guards
unconditional. Facts cannot rewrite user, device or source privacy expectations.
Canonical ingress defaults, such as the app channel, remain shared expectations.
Response facts cannot overwrite common type, targeting or metadata assertions.

### Development Lane

Use the existing Cargo target directory, locked dependencies and two jobs. For
example, while editing MSFT:

```sh
export CARGO_INCREMENTAL=0
cargo test --offline --locked -j2 -p auctioneer-bidder --lib 'bidders::msft::'
cargo test --offline --locked -j2 -p auctioneer-bidder --lib bidders::factory_contract_tests::msft -- --exact
cargo fmt --all -- --check
```

Run the owning shared regression target when editing a helper or boundary;
shared-code edits expand the affected test set immediately. Discovered bidder
modules also need direct `rustfmt --check` because workspace formatting does not
follow every generated module path. Use focused Clippy during iteration. A data
fixture edit does not require rerunning the full catalog importer after each
assertion change, but it still belongs in the publication checks below.

### Publication Lane

Freeze a coherent batch, then run the complete required source/catalog checks,
secure-default and applicable feature lanes, changed construction/response
allocation controls, strict lint, formatting and all three guest library checks
once against that exact tree. Existing CI gates remain unchanged. Include the
explicit source-checkout controls with their existing environment requirements;
an ignored source check is not passed evidence.

Do not reuse a passing result across changes to its inputs. A failure sends the
affected checks back through repair and verification before publication. Batch
publication changes scheduling, not resource, privacy, protocol or readiness
requirements. Keep one shared-capability owner and two disjoint bidder owners
active where dependencies permit; serialize Cargo instead of creating duplicate
target directories. Reuse completed source audits and keep compatibility work
within the approved OpenRTB-first scope.

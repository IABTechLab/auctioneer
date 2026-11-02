# Workspace Test Bidder Isolation

## Approved Boundary

Shared behavior tests must not depend on a shipping bidder's name, parameters,
implementation or capabilities. Reuse the existing capable fake through
test-only source modules; do not create a workspace crate, Cargo feature,
production fake registration or runtime registry override.

Tests below the bidder dependency boundary use synthetic names and metadata,
not an executable bidder or an upward dependency. Different synthetic names
must still exercise ordering, case folding, aliasing and independent permissions.
All synthetic bidder names and aliases outside their real bidder owner must
start with `fake_`, including JSON/TOML labels, registry controls and generated
test names. Use `fake_bidder`, `fake_consented_bidder` and `fake_alias` rather
than unprefixed role labels or partially embedded prefixes. This names fixtures,
not new serving implementations. Keep deliberate reserved/invalid protocol
inputs literal when changing them would stop testing that refusal; document
that purpose rather than presenting them as ordinary fixture bidders.
AG approved one other explicit exception on 2026-10-26: the fixed-ingress
maximum-cardinality allocation proof retains generated shortest legal wire
aliases. These are adversarial punctuation keys, never real bidder names;
adding a naming prefix would reduce the population the proof exercises.
Use existing explicit-registry entry points where available. Core handler unit
tests can exercise the private registry-parameterized implementation; do not
add a serving-build injection API solely to migrate integration tests. The
subsequent 2026-10-26 decision permits a narrow explicit handler entry point
under the existing `conformance-suite` feature for shared HTTP
conformance. Compile those tests within their owning crate; do
not include implementation files across crates to reach private handlers.

## Strict Component Boundary

The 2026-10-26 clarification supersedes source-bound exceptions in earlier
rounds: crates other than auctioneer-bidder must not contain provider-specific
names, constructors, field accessors, case selectors, fixture payloads or
configuration. GVL JSON is the explicit data exception. The bidder component
owns immutable multi-provider reference captures and catalog/wave witnesses
under conformance/fixtures; consumers treat these as opaque, independently
pinned data and must not branch on a provider's identity.

Cargo test targets stay within their own crate. Do not point a target at a
provider file in another crate or conceal that dependency with generated
includes. Shared handler and response projection tests use the canonical fake.
Owner-local decoders retain the literal protocol output and allocation proof;
exchange independently checks equivalent normalized shapes, signed metadata,
opaque-field retention and assembly funding. This is component composition,
not a claim that a fake proves a provider's decoding contract.

The same boundary applies inside shared bidder helpers, not only integration
tests. Scan complete shared Rust files outside manifest-owned bidder folders;
do not infer ownership from test attributes or miss separately declared helper
modules. Imports, grouped/renamed imports, factory paths, constructors and
embedded fixtures all count as dependencies. Test pure shared helpers directly;
retain actual caller behavior and factory-fixture negative controls inline with
their implementation. The public framework exposes discovered registrations,
not concrete implementation types. Owner allocation tests resolve their real
factory before entering the measurement window; discovery must not alter the
existing measured inputs, charge coefficients or independent bounds.

### Existing Sync UID Policy

Preserve existing family-specific sentinel rejection without naming the family
in the shared cookie codec. Discovery may compile an optional owner-local
uid_policy.rs independently of whether that owner has a serving factory. Its
FAMILY constant and pure accepts_uid function form the contract. Generate a
generic accepts_sync_uid dispatch using constant match arms; duplicate families
must fail compilation. No heap registry, I/O, new configuration override,
serving factory promotion or resource-limit change is needed. Shared cookie
read/write/decode paths call this generic policy, retaining generic empty-UID
and opt-out handling. Policy units stay inline with the owner; source-capture
drivers retain the independent rejection expectation without naming its family
in consumer code. An owner without a policy leaves nonempty opaque UIDs alone.
Importer checks preserve this exact owner-local policy filename even when
factory is false, without accepting arbitrary nested source or unknown owners.

## Evidence Ownership

Preserve real bidder protocol, transformation, allocation and source-capture
evidence. Relocate provider-specific evidence beside its implementation where
the existing discovery mechanism supports it. Provider-specific conformance
stays in the bidder component. Other crates may consume its captured data
generically but may not encode provider-specific knowledge in the driver.
Shared tests must not substitute a fake for provider protocol evidence. Captured
artifacts, source revisions, budgets and independent expected results stay
unchanged. Immutable capture text is not a rename candidate.

The real-priority delivery contract, import inventory, operational qualification
harness and shipped-default checks remain real-data tests. Synthetic negative
controls for their generic parsers and generators do not need real identities.

## Scope And Non-Goals

Audit inline unit tests and integration tests in exchange, core, configuration,
privacy, OpenRTB, runtime, CLI and target adapters. Convert generic test inputs;
separate and describe real-data conformance where conversion would lose proof.
The follow-up approval includes discovery-based serving wiring for the existing
published definitions only. Generate constructor dispatch from the already
validated metadata-to-implementation declarations. Core must not name bidder
implementations or silently substitute a generic factory. Only published
bindings appear in this generated path; do not traverse the all-candidate
factory table or install the allocating full-catalog validation plan. Borrow
the original metadata, retain existing feature predicates and runtime enable /
disable behavior, and allocate only the existing registry and bidder instances.
Missing factories and mismatched feature ownership remain build errors. A
runtime mismatch between generated metadata and dispatch is an internal build
invariant violation, not an excuse to guess a factory.

Expose the existing borrowed binding view as a const accessor. At target compile
time, compare the complete active public-name/implementation sequence with the
generator's feature-filtered expectation. Host build-dependency features alone
cannot prove target feature alignment. This check must retain no runtime table
or index, and must reject extra rows and independently changed names or keys.
Group public names sharing one factory without constructing that factory during
discovery. Empty and entirely feature-disabled rosters must compile cleanly.

The initial shared-surface guard was incomplete: its manually selected files
missed inline handler tests and baked-data callers. Extend the ownership audit
to those sources. Actual App tests select metadata by their required role;
they still exercise the serving registry, not a test-only replacement. External
captures retain their literal identities and bytes.
Locally authored GDPR handler snapshots use `fake_` host aliases bound to
property-selected serving metadata and bidder-owned parameters. Keep expected
JSON literal and independent: do not rewrite expected bidder names, warning
suffixes, targeting keys or seat ordering at assertion time.
The source guard checks decoded query values, ASCII case variants and parsed
TOML keys as well as constructor paths and exact literals. Required protocol
extension names and frozen reference assertions are not arbitrary fixtures.
Do not claim catalog completion, schema qualification or hosted readiness.

## Validation

First demonstrate a guard detects the existing shared-test coupling, then
migrate without deleting semantic assertions. Preserve independent values and
vary capabilities independently. Run affected default and feature tests, source
conformance, strict lint, formatting and fixture gates. Review the final diff for
weakened coverage, new deployed test hooks and accidental source-data edits.

## Disposable Model Coverage Checks

The existing scrub-model mutation must still reach the exchange's exact E0027
diagnostic. Other crates can independently guard the same model exhaustively
and fail first. In the copied workspace only, record those exact missing-field
diagnostics and apply rustc's machine-applicable explicit named-field ignore
suggestion. Never apply a wildcard, touch the exchange guard, accept unrelated
errors, or write outside copied sources. Retry with a fixed upper bound, restore
all temporary edits, and require the unchanged exchange diagnostic and a clean
restored baseline for every model. No bidder-name exemptions are involved.

# Local Development And Test Ownership

Status: first phase approved by AG, 2026-10-30.

## Scope

Shorten the normal edit loop and give each test one clear owner without
changing serving behavior, resource accounting, profiles or CI acceptance.
Use Cargo directly; do not introduce another crate, runner or test registry.

## Development Loops

Use an affected package's library check and owner-local unit tests while
editing. Preserve normal Cargo incremental state; do not routinely clean the
target directory or force `CARGO_INCREMENTAL=0`. A package checkpoint adds
the affected public integration/conformance targets and feature controls.
Full CI, guest execution and qualification remain separate acceptance gates.
A focused pass must never be reported as full capability acceptance.

CLI maintenance commands may use its existing `--no-default-features` mode
when the command supports it; application/resource commands retain the real
application graph. Do not weaken automatic discovery watches or artifact
source identity to reduce invalidation. Directory watches currently protect
new and deleted bidder detection.

## Ownership

- Pure units live inline beside their implementation, including helper units.
- Integration/conformance lives under its owning crate's `tests/`, using
  ordinary modules. Group related cases in a domain target when compatible;
  do not create a binary per pure unit behavior.
- Keep separate binaries for incompatible process-global boot gates, logging,
  counting allocators, sockets/process lifecycle and cold-state measurements.
- Real bidder protocols/fixtures belong to that bidder; shared contracts use
  the capable fake. Retain real published-catalog integration controls.
- Share mechanical setup within the owning fixture family, not expected
  values, resource rows, failure injections or independent oracles. Preserve
  immutable external captures and historical evidence bytes.
- No private-source imports, handwritten `#[path]`/`include!`, catch-all test
  utility crate or new production API just to reach private tests.

## First Slice

Relocate all 36 cases from bidder `endpoint_builder` (10), core
`account_resolution` (18), `usersync_alias_limit` (6) and `outbound_limits` (2)
into their implementation owners. Preserve assertions and compare the case
inventory before/after. Existing public endpoint and handler integration
controls remain. Cargo integration targets decrease from 243 to 239; this
is a structural reduction, not a promised wall-clock speedup.

Share fixed config-store, integrity envelope and registry binding setup in
core's existing opt-in `test_support`, with helper units compiled once there.
The existing `tests/bounded_store/` shim re-exports its bounded-read fallback
for custom stores. The four fixed-store consumers keep their own configs,
limits, assertions and process isolation. Delayed/error stores retain their
custom behavior; no helper installs global state.

## Acceptance

Record baseline dispatch and re-run all relocated cases, all four setup
consumers, relevant library suites, source-isolation controls, strict affected
target lint and formatting. Test the shared store at/over both byte caps and
at an expired deadline. Compare focused repeated commands only under the
same package/profile/cache conditions; distinguish rebuild time from warm
time. Leave full CI commands unchanged. bidder catalog delivery/readiness blockers are
not closed by this cleanup.

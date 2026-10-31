# Config Extraction Funding Boundary

Date: 2026-10-18
SDK: `bc300ace5ca1ffe7060594a7bdb01c1da80de305`
Implementation based on `5344376b` in the existing M7 branch.

AG explicitly approved refusing unfunded extraction while retaining bounded
liveness. This changes the initialization contract, not normal workload caps.

## Enforced Contract

The single `resolve_runtime_config` owner now requires installed `BootGates`
and captured SDK backend-byte, blob-byte and value-node caps no larger than
the installed normalized caps. The check precedes `AppConfig::from_store`, so
refused contexts perform no config-store read. Missing bindings retain the
existing missing-config policy because they perform no extraction.

The closed `unfunded_extraction` diagnostic carries no input values. Resolver
refusals render fixed nonretryable HTTP 500 bytes with `Cache-Control: no-store`.
Earlier admission refusals keep their own statuses. `/status` instead remains
live with bounded degraded output. Platform clock/timer/RNG initialization is
still required; no fallback primitives hide an uninitialized adapter.

An oversized App snapshot remains refused after boot; a newly configured App
recovers. Equality and narrower snapshots are supported, and each of the three
over-cap dimensions is tested independently. The single-test integration
binary exercises actual App dispatch, zero-read refusals, repeated liveness,
new-App recovery, cancellation and subsequent recovery. Raw SDK extraction
lifecycle controls remain unchanged.

## Fixture Migration And Evidence

The complete core baseline had 1,135 passing tests. The guard's initial sweep
reported 1,053 passing and 86 failing tests before fixture migration; this was
not presented as a green run. The red integration specifically observed one
forbidden store read where zero was required.

Standalone SDK-default test contexts now install their actual funding through
one test helper. Existing malformed/config/deadline tests preserve specific
error classifications and positive bounded-read evidence; merely returning
500 is insufficient. Two intentionally pre-budget tests now require refusal
instead of the obsolete unfunded extraction behavior. The Axum socket fixture
uses public configured-App dispatch under unchanged shipped caps.

| Verification                                                                             | Result                                                                                 |
| ---------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| Core all-targets final run                                                               | 1,139 passed; 0 failed; 1 existing ignored; 96 targets                                 |
| Four native adapter all-targets final run                                                | 323 passed; 0 failed; 6 existing ignored; 37 targets                                   |
| Feature-gated Mocktioneer control                                                        | 2 passed; both executed                                                                |
| Parent rerun: funding, fail-closed, deadline, store-failure and privacy-outbound targets | 36 passed; 0 failed                                                                    |
| Core/four-adapter all-targets Clippy with `-D warnings`                                  | Passed                                                                                 |
| Native Axum and three actual WASM adapter `cargo check --locked` builds                  | Passed                                                                                 |
| Raw fixture gate                                                                         | 168 fixtures across 50 files; passed after removing the obsolete two-fixture allowance |
| Formatting and diff checks                                                               | Passed                                                                                 |

Local records: `/private/tmp/r0-config-funding-{core-verified,adapters-green,
insecure-verified,check-axum,check-cloudflare,check-fastly,check-spin}.log` and
`/private/tmp/r0-parent-funding-checks.log`. Initial socket failures and failed
intermediate compile/test runs remain in separate logs; escalated final runs
resolved local-network restrictions. No new ignore or suppression was added.

The adapter sweep also repaired the earlier config-value correction's native
test golden: required receipt **58,580,992**, unchanged declared slot
**134,217,728**, explicit margin **75,636,736**. The accounted native total
remains **2,533,631,672**. No deployment limit was tuned to the lower receipt.

## Not Established

This closes the unfunded extraction entry path, not all config/merge allocation
bounds. It does not prove response backing capacity, guest allocation peaks,
Fastly's separate stack requirement, missing provider facts, absolute WASM fit
or hosted qualification. Full CLI execution and deployed WASM execution were
not part of this scoped run. Existing ignored vendor probes retain their
explicit CI owners; historical captures are not recertified.

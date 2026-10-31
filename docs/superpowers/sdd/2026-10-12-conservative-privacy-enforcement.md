# Conservative Privacy Enforcement Verification

## Scope And Approval

On 2026-10-12 the owner approved both stricter Auctioneer policies: remove all four
named user-FPD branches on COPPA requests, and apply enabled CCPA ID/geo scrubbing
to malformed selected nonempty USP. These are product decisions, not an
IAB-prescribed deletion list, malformed-input mandate, or legal approval.

Implemented on `m4-privacy`, based on `36e87e1`, under the
[implementation plan](../plans/2026-10-12-conservative-privacy-enforcement.md).
Main, M5 and M6 are not changed. Earlier pending-policy evidence is superseded;
its historical verification results are not presented as new checks.

## Implementation

- Classified composition now sets `deny_user_fpd = !transmit_ufpd || coppa`.
  COPPA removes `user.data`, `user.keywords`, `user.kwarray` and `user.ext.data`
  with activity permission and no-sale exceptions present. Contextual site/app/
  impression data and independently gated transaction IDs remain unchanged.
- Normalization retains the selected `Result<Option<Usp>, UspParseError>`.
  `usp()` remains validated-only; `usp_is_malformed()` carries no raw value.
  Enabled CCPA applies to valid sale opt-out or selected malformed nonempty USP,
  subject to exact per-bidder no-sale exceptions.
- Absent/empty USP, valid non-opt-out, inactive malformed payloads, and invalid
  unselected legacy USP remain distinct. Source precedence, syntax validation,
  warning order/count/text, outer-envelope fallback, and legacy projection are
  unchanged. Invalid USP is never fabricated into a valid consent value.
- Independent typed/flattened/shadow 256-mask wire oracles include COPPA FPD
  denial. Frozen previous-policy and pinned reference outputs are unchanged;
  stricter COPPA results are explicitly named differentials.

## Red And Green Evidence

The new selected-USP enforcement matrix executed one test and failed on
`malformed-legacy`: the old resolver returned `ccpa=false` instead of `true`.
After the selected-result/resolver change, all 24 normalization/source tests
passed, followed by the complete privacy all-target suite.

Before the COPPA composition change, the exchange library run passed 213 tests
and failed 11 updated regressions, including the composition owner test, wire
matrices, shadow matrices, FPD audit and pinned-reference differential. Live
exchange capture also failed on prohibited `/user/data` surviving serialization.
The one-line composition amendment resolves those failures.

Verification then caught two auxiliary COPPA oracle expectations still retaining
FPD, one absolute-path strict-lint failure in the new test, and a new test's
boolean TOML value where account CCPA requires a tri-state string. Those fixtures
and the import were repaired; subsequent complete runs passed without changing
production parsing, limits, budgets, factors, or warning counts.

## Executed Checks

Cargo commands use Rust 1.95.0, `--locked --offline -j 2`.

| Check                                                                                | Result                                                                                                                                                                    |
| ------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `cargo test -p auctioneer-privacy --all-targets`                                         | 343 passed; benchmark executed separately from performance qualification                                                                                                  |
| `cargo test -p auctioneer-exchange --all-targets`                                        | 483 passed across 30 binaries; one compiler-mutation test intentionally ignored                                                                                           |
| `cargo test -p auctioneer-core --all-targets --features conformance-suite`               | 636 passed across 69 binaries, including all four outbound capture tests and terminal privacy/error-order tests; one baked-config-only parity probe intentionally ignored |
| Core default-feature `privacy_outbound` and `privacy_wire_signal_400` targets        | 12 passed; same shipped-adapter and error-order controls without the conformance feature                                                                                  |
| `cargo clippy --workspace --all-targets --features conformance-suite -- -D warnings` | Passed                                                                                                                                                                    |
| Shared privacy/exchange and Fastly all-target strict Clippy, WASIp1                  | Passed with adapter conformance feature                                                                                                                                   |
| Shared privacy/exchange and Cloudflare all-target strict Clippy, unknown-unknown     | Passed with adapter conformance feature                                                                                                                                   |
| Shared privacy/exchange and Spin all-target strict Clippy, WASIp2                    | Passed with adapter conformance feature                                                                                                                                   |
| Raw JSON fixture budgets and all negative fixtures                                   | Passed: unchanged 92 fixtures in 36 files                                                                                                                                 |
| Roster gate and all rejection fixtures                                               | Passed: unchanged 139 uniquely owned members                                                                                                                              |
| Batch rebase gate, formatting and whitespace                                         | Passed                                                                                                                                                                    |

The shipped-adapter captures exercise Generic, Appnexus and Pubmatic through the
HTTP auction handler and runtime config loader, with transport recorded rather
than network sends. COPPA-alone controls cover site/app inventory and no-sale;
malformed selected legacy/GPP USP covers enabled/disabled account settings and
each exact bidder exception, including preserved FPD/TIDs and restricted IPs.

WASM lint compiles but does not execute WASM tests or certify provider deployment.
This round does not repeat the isolated model-mutation/baked-config child drivers,
full workspace test matrix, vendor packaging, hosted CI, or physical-isolate
measurements. Their earlier evidence remains separate. The debug GVL cold-parse
benchmark reports p95 about 42 ms; it is not a release performance acceptance.

## Review And Publication

The plan/spec review and independent scoped USP implementation review found no
concrete issues. Final integrated review found no code/test defects, but identified
one stale master-spec sentence retaining user FPD unless activity denies it.
The sentence now includes unconditional COPPA removal, matching the composition
formula and scrub table. The scoped re-review confirmed the correction and found
no remaining findings; reviews were read-only and did not independently rerun
the parent-executed verification commands above.

Committed and pushed code, tests, specs, coverage and plan/evidence documents as
`9e5df6d20a33ec44c8776c7ddbf3030b9398015f`. `git ls-remote origin
refs/heads/m4-privacy` returned that exact SHA after the successful push. This
publication-record update is a subsequent documentation-only commit.

# User Sync Review Repairs

**Goal:** Close the confirmed user-sync review findings on the final merged
privacy baseline and the current EdgeZero PR #275 revision.

**Architecture:** Preserve the generated handler path, bounded cookie codec,
and one-snapshot request processing. Reuse the shared publisher restriction
and URL safety helpers rather than introducing parallel policy implementations.

## Execution

- [x] Reconcile final merged privacy changes from `origin/main`; retain
      implemented sync routes, renamed master design and downstream roadmap.
- [x] Pin all EdgeZero dependencies and lock entries to PR #275 head
      `28038cce26a499499fd4e050387d4d02e35fa7c7`.
- [x] Add red tests and repair cookie duplicate/null/opt-out semantics,
      validate overwritten timestamps, and normalize timestamp serialization.
- [x] Add red tests and make publisher denial/conflicts precede every
      Purpose 1 bypass; independently cover disabled purpose/vendor controls.
- [x] Add red tests and require exactly one callback bidder binding,
      canonical callback base URLs, and the shared public-authority classifier.
- [x] Add red tests for body-read timeout HTTP 408 and preserve its category
      through the route finalization path.
- [x] Add red tests for catalog spelling of cooperative additions while
      preserving explicitly requested bidder spelling.
- [x] Colocate pure handler response tests with implementation; retain
      routed and cross-module integration tests outside source.
- [x] Run workspace tests, secure-default config tests, child-build config
      tests, strict Clippy, formatting, provider WASM checks, fixture guards,
      and controlled browser acceptance.
- [x] Review the combined diff and record exact verification results.
- [x] Commit and push the repair round.

For each behavior repair, run the new regression before production edits and
confirm it fails for the reported reason. Use behavioral test names rather
than lifecycle labels. Do not treat compilation as provider runtime evidence.

## Evidence

- Chooser: the new cooperative-spelling regression failed with `APPNEXUS`
  rather than `appnexus` for an added candidate with debug off; after repair,
  all six colocated chooser tests passed.
- CI: the inherited secure-default target-enumeration guard failed because
  `usersync_config` and `usersync_callback_validation` were omitted. Both
  targets are now included in the command and its mutation guards; all 46
  workflow tests passed.
- Cookie codec: seven unit and two integration regressions failed before
  repair. After repair, 20 cookie units, 20 cookie integration cases, 18
  reference cases and four cross-module flow cases passed. Null semantics
  were checked against the pinned reference codec, not assumed from a
  different JSON implementation.
- Permissions: six new cases failed before repair, then all 12 colocated
  permission cases and the 399-test privacy suite passed.
- Timeout: the reader returned 400 and the expired route returned 503 before
  repair; the regressions now return bounded HTTP 408. Thirty-six existing
  response-only tests moved inline without dropping their assertions.
- Baseline integration: config-node fixtures now fund the complete auxiliary
  work profile, keeping an independently meaningful 4-GiB ceiling sweep.
  Production caps are unchanged. The node-budget and real extraction tests
  passed after this fixture repair.
- Test feature isolation: the default Axum test graph includes its insecure
  dev-dependency. A separately built production binary now has an explicit
  CI socket check for loopback rejection and absence of the test bidder;
  the secure check passed, while the normal test graph retains its allowed
  loopback control.
- Independent review caught a remaining scheme-delimiter bypass:
  `https:sync.example.test://unused` parsed differently against a browser
  base. Seven new config/rendering regressions failed before the anchored
  prefix repair and passed afterward. The shared CIDR implementation remains
  unchanged; callback validation now reuses it in build and runtime contexts.

## Final Verification

Verified on October 13 with the complete repair tree and EdgeZero
`28038cce26a499499fd4e050387d4d02e35fa7c7`. Counts below overlap; they
are separate feature and boundary runs, not one summed total.

| Command Group                                                                         | Result                                                                                  |
| ------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| `cargo test --locked --offline -j2 --workspace --all-targets --exclude auctioneer-config` | 2,528 passed, zero failed, nine ignored, 184 targets                                    |
| Secure-default bidder/exchange/core `--all-targets`                                   | 1,521 passed, zero failed, two ignored, 123 targets                                     |
| All non-harness secure-default config targets                                         | 322 passed, zero failed, 11 targets                                                     |
| Full `build_failure_harness`, serial                                                  | 77 passed, zero failed                                                                  |
| Four explicit `fanout_conformance` targets                                            | 77 passed, zero failed                                                                  |
| Separately built secure Axum binary, native socket                                    | Loopback configuration rejected; test bidder absent                                     |
| Strict workspace Clippy and provider WASM Clippy                                      | Passed with `-D warnings`; only expected GVL build diagnostics                          |
| Provider WASM compilation                                                             | Cloudflare, Fastly and Spin passed                                                      |
| Fastly local guest suites                                                             | 14 logging and nine health cases executed successfully                                  |
| Fastly packaged release and Viceroy smoke                                             | Passed; package 8,803,757 bytes                                                         |
| Chromium acceptance                                                                   | CORS, preflight, redirect callback, cookie acceptance/readback and denied origin passed |
| Browser-harness shutdown                                                              | Two cases passed                                                                        |
| Formatting, whitespace, raw fixture/negative gates, roster and batch-doc guards       | Passed                                                                                  |

The controlled browser test is cross-origin, same-site loopback. No real
bidder, live hosted deployment, cross-site third-party-cookie behavior or
other browser is qualified by these results. Fastly deployment resource
links and genuinely unavailable named logging endpoints remain unverified;
reserved-name rejection and stderr fallback were exercised locally.

## Publication

The verified code and merged privacy baseline were committed and pushed as
`de3778d5c0c5d86526edb6a3163c3426e4a04b07` to `origin/m5-usersync`,
updating [PR #18](https://github.com/IABTechLab/auctioneer/pull/18), which targets
`main`. Local and remote branch hashes matched after the push. No force push,
deployment, downstream milestone mutation or remote-CI completion is claimed.

# Response Classification Review Repairs

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

**Goal:** Close the verified October 10 review findings on bounded fanout without importing
the follow-on privacy enforcement scrub redesign. Commit and push each verified repair round,
then propagate through privacy enforcement, user sync, and diagnostic qualification.

**Architecture:** Keep the existing bounded response parser and consume parsed
bids by value. Classify each bid independently through one shared classifier;
successful responses can carry bid-local errors into the exchange. Whole-body
parse failures remain fatal. Adapter capabilities remain adapter-specific.

**Verification:** Use regression-first tests and the retained offline Linux
Docker cache, not the host Cargo target occupied by another session. Run native
tests, strict Clippy, formatting, fixture/roster gates, and affected downstream
tests. Hosted CI remains an external account issue, not evidence of a passing
build.

## Round 1: Wire Output And Documentation

- [x] Add multi-bid/multi-seat fixtures in `auctioneer-bidder/src/bidders/mod.rs`.
- [x] Reproduce good-bid loss and Generic/Pubmatic `mtype` divergence before
      changing production code.
- [x] Add a bid-local error channel to `BidderResponse`; partition classifications
      without cloning bids; retain all-error responses as errors.
- [x] Route all four adapters through the shared classifier; preserve reviewed
      legacy hints and AppNexus's audio exclusion; reject conflicting hints.
- [x] Test the classifier directly and exercise mixed responses through the real
      exchange, checking winning bids, error entries, and media targeting.
- [x] Classify against post-FPD, privacy-scrubbed request context; test format
      additions/removals and retain sequential bounded materialization in bounded fanout.
- [x] Cross-link the conservative bounded fanout Geo catch-all behavior to the privacy enforcement disclosure
      design in `scrub.rs` and the master specification. Name lost context explicitly.
- [x] Verify and commit/push this round.

## Round 2: Load-Bearing Limits And Guards

- [x] Preserve the verified bridge `Other` code 999 / status 100 contract; add a
      separate bounded-wave `BadServerResponse` code 4 / status 100 wire assertion.
- [x] Run pre-expired-deadline assertions against bridge and bounded wave with
      an injected counting timer; refuse spent work before composition/submission.
- [x] Enforce the funded impression cap in the real auction handler's prepare
      and outbound sink limits; test below/at/above a smaller installed limit.
- [x] Sweep 1, 8, and 100 impressions with and without supplements in the sparse
      request allocation guard, charging actual bidder metadata rather than a
      duplicated independent literal.
- [x] Make warning-code uniqueness exhaustive at compile time, rather than a
      manually enumerated subset of variants.
- [x] Report cumulative and retained parse-allocation ratios for all fixtures,
      including structured UA; record observed results without inflating bounds.
- [x] Verify and commit/push this round.

## Integration

- [x] Independently review the implementation and evidence, fixing confirmed
      remaining issues before claiming closure.
- [x] Merge and verify bounded fanout into privacy enforcement, preserving privacy enforcement-owned privacy composition.
- [x] Merge and verify privacy enforcement into user sync, preserving usersync and the renamed master spec.
- [x] Merge and verify user sync into diagnostic qualification, preserving diagnostic qualification's planned hardening contracts.
- [x] Run affected downstream tests and push each branch. Report precise heads,
      verification limitations, and the final merge recommendation.

## bounded fanout Conformance Baseline

- The Round 2 transport-name premise was rejected after inspection:
  `transport_error_yields_other_code_999_statuscode_100` accurately asserts the
  bridges' `Other` contract and remains bridge-only. The separate bounded row
  `transport_error_yields_bad_server_response_code_4_statuscode_100_on_bounded_wave`
  asserts typed `Transport` as `BadServerResponse`, including wire diagnostics.
- `pre_expired_deadline_at_send_all_entry_short_circuits(target)` and
  `deadline_equal_to_send_all_entry_short_circuits(target)` share one compressed
  fixture and injected timer witness for bridge and bounded wave. Creation,
  first poll, and cancellation are counted separately. Both spent and equal
  cutoffs must finish before gzip finalization or timer/submission side effects.
- Core, Cloudflare, Spin, and Fastly register those rows under `entry_deadline`
  and `transport_contract`. The adapter registrations reuse the core algorithm
  harness, not provider runtime timing/cancellation. Wave unit regressions also
  live under `entry_deadline`. Historical roster counts are not revised here.

## privacy enforcement Integration Adaptation

- Retain the composed, privacy-scrubbed `PerBidderRequest` already passed to
  response decoding; do not introduce bounded fanout's sequential reconstruction helper.
- Retain sealed outbound requests and exchange-owned encoding finalization.
  Fanout entry tests receive already-finalized gzip bodies and assert no timer
  creation or submission; the wave composer cannot finalize encoding at all.
- Retain neutral `TransportErrorKind::Initial` and its fixed message on privacy enforcement's
  bounded path. The explicit row is named
  `transport_error_yields_initial_code_4_statuscode_100_on_bounded_wave` and pins
  code 4 / statuscode 100 without restoring bounded fanout's `BadServerResponse` category.
- Retain boot-aware preparation and scope-sized metadata on privacy enforcement. The new
  real-ingress test verifies the installed eight-impression cap end to end.
- Adapt the sparse allocation sweep to grouping and survivor projection while
  preserving privacy enforcement's prepaid projection and response reservations.
- Give the response-charge fixture matching impression context before its
  measurement window. Require the negative node-count guard to report the
  actual node-budget error, not an unrelated classification failure.

## privacy enforcement Verification Evidence (2026-10-10)

- Final native workspace run with the same features and config exclusion:
  1,976 passed, zero failed, six ignored. Strict workspace Clippy passed with
  `--all-targets --keep-going` and `-D warnings`.
- Raw-fixture, roster and batch-API gates passed; the fixture and roster
  mutation suites also passed. No funding constants were raised.
- A read-only review of the merged response path found no introduced
  correctness issue. Provider packaging and the config subprocess matrix were
  not rerun on this merge.

## user sync Verification Evidence (2026-10-10)

- Rebuild project artifacts and both compile-probe caches against the user sync
  worktree; do not reuse stale generated configuration from another capability.
- Final native workspace run: 2,287 passed, zero failed, six ignored. Ordinary
  config library/integration targets, including usersync configuration and
  callback validation: 195 passed, zero failed. The subprocess build-failure
  matrix and provider packaging were not rerun.
- Strict workspace Clippy, formatting, diff checks, raw-fixture, roster and
  batch-API gates passed. The impression-limit fixture derives user sync's additional
  auxiliary allowance from the real profile and baked-config charge; production
  limits and parse/allocation factors are unchanged.

## bounded fanout Verification Evidence (2026-10-10)

- Regression-first runs reproduced four adapter failures, mixed-response bid
  loss, the ignored funded impression cap, nine spent-deadline failures, and
  classification against pre-FPD rather than bidder-visible formats.
- Final offline Linux aarch64 workspace run: 1,662 passed, zero failed, five
  ignored, using `--all-targets --features conformance-suite,insecure-bidders`
  with `auctioneer-config` excluded from that invocation. Its library and ordinary
  integration targets passed separately: 164 passed, zero failed. The unchanged
  subprocess build-failure matrix and provider packaging were not rerun here.
- Strict native workspace Clippy passed with those features, `--all-targets`,
  and `-D warnings`. Formatting, diff checks, raw-fixture and roster gates,
  their mutation fixtures, and the batch-API gate passed.
- The sparse guard exercises all six impression/supplement combinations. Parse
  observations are recorded in `auctioneer-openrtb/tests/parse_factors.rs`; the
  original 29.405x binding fixture and all funding constants remain unchanged.
- Two scoped, read-only closure reviews found no remaining correctness issue
  in the response/classification or deadline/handler repairs.
- GitHub's current check annotation says jobs were not started because account
  payments failed or the spending limit needs increasing. Hosted CI is not a
  green result; this round does not certify provider runtime behavior.

## diagnostic qualification Integration And Verification (2026-10-10)

- The merged production code, tests, workflows, scripts and Cargo manifests
  match the verified user sync tree. Existing diagnostic qualification hardening documents remain intact;
  its debug spec and Task 6 now cover both fatal error vectors and bid-local
  errors in successful responses, preserving valid bids and exactly-once
  diagnostics. This does not claim diagnostic qualification debug is implemented.
- Fresh native workspace verification: 2,287 passed, zero failed, six ignored.
  Ordinary config library/integration verification: 195 passed, zero failed.
  Strict native workspace Clippy passed with all targets and `-D warnings`.
  The config subprocess build-failure matrix and provider packaging were not
  rerun. Formatting, diff checks, fixture/roster gates and their mutation suites,
  and the batch-API gate passed.
- The retained logs show successful verification, not an OOM-killed container.
  Hosted checks still fail before starting due to the repository account's
  billing/spending-limit condition; this remains separate from local evidence.
- Verified and pushed integration ancestry is bounded fanout -> privacy enforcement -> user sync -> diagnostic qualification; main was
  not modified. The confirmed findings from this review are closed. This is
  not a hosted-green or provider-packaging sign-off; those checks remain
  explicit verification limitations rather than claims of success.

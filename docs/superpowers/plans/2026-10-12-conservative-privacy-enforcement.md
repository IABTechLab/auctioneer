# Conservative Privacy Enforcement Implementation Plan

> **For agentic workers:** Use superpowers:subagent-driven-development or superpowers:executing-plans. Preserve red/green regression evidence before production edits.

**Goal:** Implement the owner's approved COPPA user-FPD removal and malformed selected nonempty USP enforcement on `privacy-enforcement`, based on `36e87e1`.

**Architecture:** Keep source selection and syntax separate from enablement and per-bidder exceptions. Retain the selected USP parse result inside normalization; expose malformed state without fabricating a valid USP. Compose COPPA into the existing classified FPD denial at the mutation boundary. These are explicit Auctioneer policies, not IAB-prescribed deletion rules.

**Tech Stack:** Rust, Cargo, existing typed/classified scrub and integration harnesses.

## Task 1: COPPA FPD Denial

Files: exchange `src/scrub/{decision.rs,composition_tests.rs,classified_tests.rs}` and `tests/{privacy_policy_composition.rs,disclosure_policy_diff.rs}`.

- [x] Change the composition test expectation to `coppa.deny_user_fpd == true` and verify red with the focused `resolved_removeall_fpd_tid_and_allow_controls_keep_separate_ownership` test or the full exchange suite containing it.
- [x] Set `deny_user_fpd: !transmit_ufpd || coppa`. Keep CCPA/LMT-only FPD behavior unchanged and preserve site/app/imp context, ordinary request/impression IDs, independently gated TIDs, and absent objects.
- [x] Update both independent 256-mask oracles to remove FPD when bit 4 or 128 is set. Update COPPA-specific expectations and named reference deviations without regenerating reference JSON.
- [x] Run all exchange library and privacy-composition tests; retain typed, flattened, shadows, idempotence, audit and real-adapter-boundary coverage.

## Task 2: Preserve Malformed Selected USP State

Files: privacy `src/{sources.rs,resolve.rs}`, `tests/{privacy_sources.rs,privacy_normalization.rs}`.

- [x] Add a normalization enforcement matrix for malformed legacy and active GPP values, whitespace, absent/empty, valid opt-out/non-opt-out, invalid unselected legacy, inactive/missing section, malformed envelope fallback, and disabled/account override/no-sale controls. Verify red before changing production code.
- [x] Store the selected `Result<Option<Usp>, UspParseError>` instead of collapsing errors. Preserve `usp()`'s validated-value API and add a raw-free malformed-state accessor.
- [x] Resolve CCPA as enabled AND (valid sale opt-out OR selected malformed nonempty). Per-bidder no-sale remains an independent exception. Never project an invalid value or retry another source to get a different result; warnings/order/raw-value redaction remain unchanged.
- [x] Run all privacy targets. Parser/reference syntax results remain unchanged; only policy consumers gain the conservative distinction.

## Task 3: Wire Coverage And Documentation

Files: exchange `tests/privacy_policy_composition.rs`, core `tests/privacy_outbound.rs`, privacy fixture `tests/data/privacy/README.md`, master/privacy enforcement/classified specs, repair checklist and new SDD record.

- [x] Add outbound ID/geo assertions for malformed selected USP with enabled/disabled/no-sale controls, a valid unselected legacy control, unchanged raw GPP/projection rules, and original/sibling isolation. Keep invalid-no-sale and terminal GPP limit precedence.
- [x] Confirm COPPA strips all four user-FPD branches with activity allowed, no-sale and GDPR bypass controls; use shipped-adapter captures where available.
- [x] Reconcile approved policy formulas, source tables, scrub tables and named reference deviations. Mark earlier pending policy evidence superseded, not historically re-executed.

## Task 4: Review And Publish

- [x] Run affected native/all-target suites, strict workspace Clippy and shared privacy/exchange/adapter all-target Clippy for all three WASM targets, formatting, raw-fixture and documentation gates. Do not raise budgets, factors or warning counts to pass.
- [x] Obtain an independent repair review, resolve concrete findings, record exact verification limits, commit and push to `privacy-enforcement`, and verify the remote SHA. Code/documentation commit `9e5df6d20a33ec44c8776c7ddbf3030b9398015f` is published and independently read back with `git ls-remote`. Main/user sync/diagnostic qualification remain untouched.

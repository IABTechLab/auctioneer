# Privacy Review Repairs Implementation Plan

> **For agentic workers:** Use superpowers:subagent-driven-development or superpowers:executing-plans. Run regression tests before and after behavioral repairs.

**Goal:** Resolve the self-review and peer-review findings on `m4-privacy` at `70c9042` without silently changing approved privacy policy.

**Architecture:** Preserve the classified scrub/compiler boundary and existing runtime admission contract. Tighten test harnesses and use behavioral test names; policy changes require an explicit owner decision.

**Tech Stack:** Rust, Cargo, TOML, GitHub Actions.

## Tasks

- [x] Make configured round-trip fixture roots and all directory traversal errors fatal. Prove a missing configured corpus cannot pass with the vendored subset. Keep local unconfigured runs supported.
- [x] Reject zero `max_bidders_per_auction` with `LimitBelowMinimum { minimum: 1 }`; run a failing colocated regression before the guard.
- [x] Drive actual `/openrtb2/auction` ingress before boot, asserting bounded refusal without auction/body work; point direct-handler documentation at that regression.
- [x] Rename milestone-based test modules, integration binaries, helper names and fixture directories by behavior. Update CI, harness and documentation references in the same change.
- [x] Require scrub-oracle before-state presence and exact malformed audit counts; document the compiler-guaranteed unreachable delegate fallback without introducing a panic.
- [x] Derive adapter proof scope deltas from the installed construction descriptor rather than duplicating layout constants.
- [x] Reconcile the M4 status and Geo contract with classified scrubbing and the closed local resource acceptance, while retaining provider/physical-isolate qualifications.
- [x] Independently inspect all disclosure-policy rows against the pinned source and supported extension inventory; record results, not invented human approval.
- [x] Evaluate an upstream digest monitor separately from automatic catalogue ingestion; never update reviewed membership or policy on a schedule.
- [x] Resolve COPPA user-FPD and malformed selected USP policy with the owner. Both stricter Auctioneer policies were approved on 2026-10-12; implementation and verification are tracked in `2026-10-12-conservative-privacy-enforcement.md`.
- [x] Run affected suites, actual compiler-mutation and baked/runtime parity drivers, strict workspace Clippy, formatting, and available WASM lint checks. Record exact verification limits.
- [x] Review the integrated diff, commit, push `m4-privacy`, and verify the remote head. Do not modify `main` or downstream milestones without a propagation request.

## Verification

Use `cargo test --locked --offline -j 2` with actual behavioral binary/test names after renaming. Check that filtered regression commands execute at least one test. Run `cargo clippy --workspace --all-targets --features conformance-suite --locked --offline -j 2 -- -D warnings` and `cargo fmt --all -- --check`.

Record red/green evidence and final command outcomes in `docs/superpowers/sdd/2026-10-12-privacy-review-repairs.md`. Treat hosted CI and vendor deployment evidence as distinct from local checks.

## Policy Decision Resolved

The owner requested the IAB basis for the two proposed changes. OpenRTB defines
`regs.coppa` as an applicability signal, not an exact user-FPD deletion list.
The US-Privacy String specification defines the signal grammar, not the
exchange's response to malformed input. Neither source establishes the proposed
transform as an IAB requirement. The owner approved both stricter Auctioneer policies
on 2026-10-12. The earlier repair retained the previous behavior; the subsequent
conservative-enforcement plan implements the approved change.
This is not legal approval or a considered-and-declined legal assessment.

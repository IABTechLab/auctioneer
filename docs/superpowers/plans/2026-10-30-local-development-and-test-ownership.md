# Local Development And Test Ownership Plan

> **For agentic workers:** use superpowers:subagent-driven-development or
> superpowers:executing-plans for this approved slice.

**Goal:** focused development loops and explicit test ownership, without
changing acceptance or production behavior.

**Design:** [approved first phase](../specs/2026-10-30-local-development-and-test-ownership-design.md).

1. Capture the four old targets' 36 passing cases and workspace target count.
   Record warm library-check timing separately from rebuilding selected tests.
2. Move those cases inline into bidder endpoint construction and core config
   resolution/validation. Delete only their four redundant integration roots.
   Keep external public API/conformance coverage and all assertions.
3. Consolidate the four fixed-store consumers through core's existing opt-in
   `test_support`, retaining the `tests/bounded_store/` fallback shim. Test binding and fallback
   caps/deadline behavior; keep configs and independent expectations local.
4. Publish edit/package/acceptance commands in `docs/testing/development.md`,
   link from README, and clarify crate-wide ownership/isolation rules.
   Do not create command aliases/another runner or change CI/profiles.
5. Verify case inventory, library suites, affected integration consumers,
   `test_isolation`, strict Clippy and formatting. Record evidence and honest
   timing scope, inspect diff, commit and push the M7 branch.

Isolation-sensitive binaries, build watches, resource identity, immutable
captures and CI feature/guest matrices are intentionally unchanged. Further
consolidation requires an owner and isolation review, not target-count goals.

## Completion

Implementation and checks are complete. The 36 original cases remain,
four unnecessary binaries are removed, and the fixture implementation has
one test-support owner. See the [executed checks and timing boundary](../../testing/evidence/local-development-test-ownership-20261030.md)
for the 1,929 passing scoped tests, strict lint, layout controls and unchanged
full-acceptance requirements. This does not complete M7.

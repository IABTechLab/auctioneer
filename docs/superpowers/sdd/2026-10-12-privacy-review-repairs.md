# Privacy Review Repairs Verification

Date: 2026-10-12
Branch: `m4-privacy`, worktree `.claude/worktrees/m4-privacy`
Repair base: `70c90424b253faf1d649f222ef808f3ca9ed7e4c`
Implementation commit: `83b672a15639db8ecf01d0d47f74f005dc103766`, pushed to
`origin/m4-privacy` and verified against the remote head.
Status: technical repairs verified and published; two policy choices pending.

This record covers the [repair plan](../plans/2026-10-12-privacy-review-repairs.md)
and the self-review and peer findings against the repair base. The prior
[classified-disclosure record](2026-10-11-gdpr-scrub-classification.md) remains
historical evidence, not a claim that every prior check was repeated today.

## Repairs

- Explicitly configured fixture roots, empty roots and traversal errors fail
  the round-trip gate rather than falling back to a smaller vendored corpus.
- Boot validation rejects a zero bidder population before multiplication can
  erase construction charges. One bidder remains accepted.
- A real configured auction route with absent boot gates aborts before body
  polling or dispatch. Its isolated child process excludes sibling OnceLocks.
- Scrub wire-oracle removals require before-state presence; the malformed count
  is exactly six. The compiler-guaranteed delegate fallback remains fail-closed.
- Adapter proof deltas use the owning construction descriptor; independent
  layout and installed-versus-engine assertions remain intact.
- Test binaries, modules, helpers and fixture directories use behavioral names.
  CI and child Cargo invocations refer to the renamed binaries. The pinned
  reference JSON, reference generator and grouped snapshot have identical Git
  blob IDs before and after moving; reference expectations were not regenerated.
- The older M4 design now points at classified disclosure and describes the
  reviewed Geo/context behavior and closed local resource acceptance correctly.
- A read-only weekly/manual upstream digest check detects source drift without
  updating reviewed pins, catalogue membership, policy or human review metadata.
  The schedule starts only on the default branch; mutation tests reject missing
  triggers, skipped commands, disabled/soft-failing execution and write access.
- Disclosed-vendor-only changes are compared under identical core/GVL inputs
  with every bidder-decision field checked. A separate 128-case property jointly
  varies source, applicability and independent enforcement/activity controls.
- WASM all-target lint now includes shared exchange/privacy test code. Unused
  Proptest fork/timeout features are disabled; only browser-target development
  builds opt into the existing `getrandom` Web Crypto backend. Production
  privacy dependencies do not acquire that backend or Proptest.

## Independent Classification Audit

A read-only technical review checked all 86 field rows and 11 schema rows
against the pinned standard and supported extension artifacts. Counts are Device
33, User 12, Geo 14, UserAgent 8, BrandVersion 3, EID 6, UID 3, DeviceExt 2,
DeviceExtPrebid 1, Interstitial 2 and UserExt 2. No identifier-to-context
misclassification was found. EID/UID provenance leaves cannot survive removal
of their enclosing identifier collections. Deprecated hashes, customdata and
both EID representations remain identifier-gated; child shapes and domains were
included in the review.

This is technical review, not new human approval or a claim to detect identifiers
concealed in valid contextual strings/numbers. Existing reviewer metadata remains
unchanged. A fresh upstream download also passed the real catalogue `check`:
the current upstream document still matches the reviewed digest and inventory.

## Regression Evidence

- The missing-configured-corpus and unreadable-root tests each failed against
  the original collector; the repaired binary passes all three tests.
- Zero bidder capacity was accepted before the scalar guard. The regression
  now requires the exact `LimitBelowMinimum` variant and accepts a one-bidder
  positive control.
- Temporarily replacing missing-gate admission with admit-all made the new
  served-ingress regression fail its abort assertion. The mutation was removed.
- The oracle presence regression failed because the original helper silently
  ignored absent keys; it now passes by requiring the expected panic.
- The DV regression detects a test-only change to core authorization inputs;
  the restored implementation passes all eight paired DV comparisons.
- Earlier WASM trials failed on unused Proptest fork dependencies and a missing
  browser entropy backend. Test-only dependency repairs resolved both without
  changing production RNG behavior. Subsequent strict lint caught a property
  assertion style/ownership error, repaired without weakening its checks.
- Integrated review found that the new joint property lacked the fixture's
  granting alias/vendor mapping. An explicit grant expectation failed with
  granting legacy consent, GDPR enabled and no bypass or activity denial.
  Mapping `ax` to fixture vendor 8 resolves it; both grant/deny outcomes and
  fetch-denial precedence are now asserted. The fresh complete privacy suite
  passes 341 tests, and native/all-three-WASM strict lint passes again.

## Executed Checks

Commands use the installed Rust 1.95.0 toolchain, locked/offline Cargo and two
build jobs on the native host unless a deployment target is named.

| Check                                                                                                | Result                                                                                   |
| ---------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| Shared runtime/privacy/exchange/OpenRTB/bidder/catalogue all targets                                 | Passed; one intentionally ignored compiler mutation separately driven below              |
| Final privacy all-target rerun after the granting-control repair                                     | 341 passed; benchmark also executed, not a performance qualification                     |
| Config library, codegen, simple-fold and disclosure-codegen targets                                  | Passed                                                                                   |
| Core workflow, ingress, missing-boot, config-parity, outbound, privacy-wire and sync-absence targets | 66 passed; one baked-config-only probe intentionally ignored and separately driven below |
| Mandatory configured fixture corpus and round-trip parity                                            | 3 passed                                                                                 |
| Real ignored model-field compiler-mutation driver                                                    | 1 passed; actual disposable-workspace mutations executed                                 |
| Renamed baked/runtime config parity parent driver                                                    | 1 passed; executes and verifies the isolated otherwise-ignored core probe                |
| Four adapter/core library suites, conformance feature                                                | Passed                                                                                   |
| Strict workspace all-target Clippy, conformance feature                                              | Passed                                                                                   |
| Shared exchange/privacy plus Fastly all-target strict Clippy, WASIp1                                 | Passed                                                                                   |
| Shared exchange/privacy plus Cloudflare all-target strict Clippy, unknown-unknown                    | Passed                                                                                   |
| Shared exchange/privacy plus Spin all-target strict Clippy, WASIp2                                   | Passed                                                                                   |
| Real freshly fetched upstream digest/catalogue check                                                 | Passed                                                                                   |
| Raw JSON budgets and rejection fixtures                                                              | Passed: 92 raw fixtures in 36 files                                                      |
| Roster, rejection fixtures and batch-rebase gate                                                     | Passed: 139 uniquely owned roster members                                                |
| Formatting and whitespace checks                                                                     | Passed                                                                                   |

WASM lint compiles deployment/test code; it does not execute WASM tests or certify
a provider deployment. This round does not repeat vendor packaging, WASI engine
execution, all 61 isolated config-build cases, hosted CI or physical-isolate
measurements. Earlier evidence for those checks is separate. Main and downstream
milestones are not modified by this repair.

## Policy Decisions Still Pending

COPPA still removes identifiers and all Geo but retains user FPD unless
`transmitUfpd` is denied. Malformed selected nonempty USP still warns without
activating CCPA sale-opt-out enforcement. These are the existing approved
behaviors, not accidental changes in this repair.

The owner asked what IAB prescribes. [OpenRTB's Regs definition](https://github.com/InteractiveAdvertisingBureau/openrtb2.x/blob/main/2.6.md#objectregs)
defines applicability; it does not provide an exact COPPA user-FPD deletion list.
The [US-Privacy String specification](https://github.com/InteractiveAdvertisingBureau/USPrivacy/blob/master/CCPA/US%20Privacy%20String.md)
defines format, not the exchange's malformed-input enforcement policy. Approval
to tighten both has been requested but not received. Neither policy is marked
fixed, legally approved, or considered-and-declined.

### Subsequent Owner Decision

On 2026-10-12 the owner approved both stricter Auctioneer policies. This supersedes
the pending status above, not the historical verification results. The follow-up
implementation is tracked in
`../plans/2026-10-12-conservative-privacy-enforcement.md`: COPPA also removes the
four named user-FPD branches, and malformed selected nonempty USP activates
enabled CCPA ID/geo scrubbing subject to the existing no-sale exceptions.
These are Auctioneer product decisions, not IAB-prescribed transforms or legal approval.

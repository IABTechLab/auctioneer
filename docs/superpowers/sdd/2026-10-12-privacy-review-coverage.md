# Privacy Review Coverage Repairs

Follow-up: [privacy wire and config review](2026-10-12-privacy-wire-config-review.md)
repairs the node-cap clamp and replaces the four-owner raw-slot guard described
in this historical round. Its fresh verification is recorded separately.

Date: 2026-10-12
Worktree: `.claude/worktrees/m4-privacy`
Starting revision: `8fe8d9e`; peer feedback originally targeted `a6a869b`.

## Scope

This round verifies the eleven peer findings against the current code and
approved tables, and includes the owner-approved Basic-policy tightening for
findings 1 and 2. Permission rules, Auctioneer expectations, and the approved
tables change together; frozen reference outputs remain unchanged and
serve as comparators, not authorization policy. Dependencies,
field inventory, and budget ceilings remain unchanged. Unit fixtures remain
colocated; the new real-extraction boundary test belongs under `auctioneer-core/tests`.

## Owner Approval

On 2026-10-12, after the initial review characterized findings 1 and 2 as
existing Basic-table behavior pending a policy choice, the owner approved
"Tighten Basic's privacy safeguards". Basic must deny matching `NotAllowed`
or conflicting restrictions before its bypasses, and its LI-transparency
override requires a nonzero resolved vendor ID. This is explicit Auctioneer
policy tightening, not a normative IAB mandate or a legacy compatibility
requirement. Other Basic allow rows remain valid, including vendor ID 0
where permitted; GVL declarations and single-restriction basis remapping
remain Full-only. Master §7.3 / Appendix A.3 and M4 §5.3 now specify the
approved rules. Both algorithms use the same matching/conflict resolver;
Basic alone adds the nonzero-ID guard to its LI override. Unit tests first
failed on the old implementation, then passed after the policy change.

## Disposition

| Finding                                                      | Disposition                                                                                                                                                                                                                                                                                                                                                            |
| ------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1: Basic ignores publisher restrictions                      | Implemented the owner-approved tightening (2026-10-12): matching `NotAllowed` or conflicting `RequireConsent` / `RequireLegitInterest` deny before any Basic bypass. Single-restriction basis remapping remains Full-only. Unit and resolver tests include disabled flags, exceptions, core/alias list matching, both record orders, and duplicate/unrelated controls. |
| 2: Basic LI-transparency grants vendor 0                     | Implemented the owner-approved tightening (2026-10-12): the LI-transparency override requires a nonzero resolved vendor ID, without adding that requirement to other Basic allow rows. Unit and resolver tests distinguish vendor 0 from vendor 8, including a vendorless generic bidder and host/account algorithm replacement.                                       |
| 3: Outer finalization checkpoint lacks a guard               | Added an empty-accepted sibling whose budget lookup produces a warning while live. Expiry must suppress that lookup. Removing only the outer check now fails the regression.                                                                                                                                                                                           |
| 4: Verification omits config integration and core unit tests | The committed workflow already executes disclosure integration tests and the full workspace, all targets. Its guard now also rejects narrowing the workspace step to config's library, excluding core, or disabling/ignoring the step. Fresh verification must include those actual targets.                                                                           |
| 5: Gzip fixture never exhausts call budget                   | Added a one-call-cap, three-attempt row: exactly one accepted and two rejected. Both live and expired controls assert diagnostics, funded buffer lifetimes, and the original absolute deadline.                                                                                                                                                                        |
| 6: Test environment naming                                   | Isolated unit-test selectors now use the `AUCTIONEER__TEST__CONFIG_NODE_BUDGET__...` namespace. They are not production config overrides.                                                                                                                                                                                                                                  |
| 7: Source includes integration support                       | Gate unit fixtures are colocated with their implementation; integration support remains under `tests/support`.                                                                                                                                                                                                                                                         |
| 8: Broad account-Basic assertion                             | Exact permission equality replaces the permissive variant match. The test characterizes an existing rule, but changing resolution to ignore the account algorithm makes it fail.                                                                                                                                                                                       |
| 9: Node-cap normalization called a no-op                     | The SDK extractor still uses its fixed 8,192-node cap. Normalization changes the retained accounting row and charge, not SDK behavior. Added a differing-declaration comparison and real inclusive extraction boundaries at 8,191/8,192/8,193.                                                                                                                         |
| 10: Raw-extension guard covers four owners                   | Complete for the pinned seven-object membership scope, not automatically extensible. Added rejection cases for App, Dooh, Banner, Video, Native, and Audio. All six remain outside the reviewed inventory.                                                                                                                                                             |
| 11: Weak descriptor/reference checks                         | Assert exact action, child schema, and domain descriptors for accepted traversal policies, and the exact frozen `user.keywords = "interest"` values.                                                                                                                                                                                                                   |

## Extension Boundary

The standard catalogue describes wire membership and shapes, not Rust storage
or visitor capabilities. It cannot derive whether an object field is stored
as `RawValue`, traversable JSON, or a typed child. The four-owner guard is an
explicit adapter capability check, not a standard-field derivation.

Before expanding the reviewed inventory to another raw-extension owner, the
same change must add its typed disclosure adapter, explicit traversal capability
validation, exhaustive model coverage, and negative/positive generator tests.
Do not infer child traversal from an OpenRTB `object` shape alone. This round
does not introduce a source parser or pretend those six owners are supported.

## Regression Evidence

- The new Basic unit tests failed twice on the old code: matching publisher
  denial granted, and the LI-only override granted vendor ID 0. After the
  policy change, all 273 privacy unit tests passed.
- The resolver matrix covers 1,056 cases through legacy TCF and selected GPP,
  including a granting conflicting legacy signal. Permissions and fetch denials
  are exact; warning code, scope, message, count and draining remain pinned.
  Both record orders, duplicate consent and duplicate LI records, matching
  core versus alias names, and account-over-host algorithm selection are controls.
  LI-only cases also pin Basic's unchanged single-restriction behavior.
- Removing only the outer finalization checkpoint leaves the earlier eight
  tests passing but fails the new empty-sibling regression. Its live control
  proves the budget lookup is observable rather than merely assuming it ran.
- Changing account algorithm resolution to ignore the account fails the exact
  resolver unit expectation. Removing node-cap normalization fails all twenty
  nondefault declaration cases; four default-declaration controls still pass.
- Changing generated descriptor expectations to the wrong action fails all
  three strengthened positive checks. The workspace-step mutation test first
  failed before the new gates, then passed after adding them.
- Independent review found no actionable correctness issue in the approved
  Basic-policy change. Its two suggested controls, duplicate LI and LI-only
  consent restriction, were added before the final privacy run.

## Verification

Parent Cargo commands used `--locked --offline -j2`. The isolated config-build
harness controls its own child Cargo arguments. Both complete workspace runs
executed after the production policy change; the final privacy rerun also
includes the later test-helper lint repairs and expanded single/duplicate
restriction controls.

| Executed Check                                                                                                               | Result                                                                                                                                                                             |
| ---------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `cargo test --workspace --all-targets`                                                                                       | 2,395 passed, 0 failed, 7 ignored, 174 test-result summaries; includes all 61 isolated config-build cases, 40 disclosure-codegen tests, 284 core unit tests and 37 workflow guards |
| Workspace all targets, `insecure-bidders conformance-suite`, excluding `auctioneer-config`                                       | 2,222 passed, 0 failed, 7 ignored, 169 test-result summaries                                                                                                                       |
| Config library, codegen, disclosure codegen, archive, smoke and URL-safety targets, `insecure-bidders test-harness-override` | 207 passed, 0 failed; the hermetic build harness is covered by the full default run                                                                                                |
| Final privacy all targets, including the expanded resolver matrix                                                            | 356 passed, 0 failed, 12 test-result summaries; 1,056 resolver cases are subcases within 17 integration tests, not additional test-count inflation                                 |
| Workspace all-target strict Clippy, `insecure-bidders conformance-suite`                                                     | Passed after borrowing the helper's expected permissions and removing name shadowing; no lint suppression added                                                                    |
| Cloudflare / Fastly / Spin adapter, exchange and privacy all-target strict Clippy, `conformance-suite`                       | Passed on `wasm32-unknown-unknown` / `wasm32-wasip1` / `wasm32-wasip2`                                                                                                             |
| Independent catalogue importer check                                                                                         | Passed; source and reviewed inventory unchanged                                                                                                                                    |
| Raw JSON fixture gate and its rejection fixtures                                                                             | Passed: 92 fixtures across 36 files; all negative cases reject                                                                                                                     |
| Roster gate and its rejection fixtures                                                                                       | Passed: 139 expanded members; all negative cases reject                                                                                                                            |
| Batch-API documentation gate, formatting and whitespace checks                                                               | Passed                                                                                                                                                                             |

The first workspace run, before the Basic-policy change, passed 2,390 tests;
it is baseline evidence rather than proof of the new policy. The seven ignored
rows are not counted as executed by these parent summaries. Their explicit
drivers and preceding vendor qualification remain separately recorded in the
[classified-scrub execution record](2026-10-11-gdpr-scrub-classification.md).
The debug GVL benchmark is informational and does not qualify its 10 ms release
latency target under concurrent verification load. This round does not rerun
vendor packaging, hosted handlers or physical-isolate measurements, and does
not make an unexecuted GitHub check green. No human merge approval is implied.

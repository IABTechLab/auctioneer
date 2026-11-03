# Adapter-Owned Resource Audit

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

Date: 2026-10-18. Same bidder catalog branch and PR #21.

The [canonical typed audit](../../../crates/auctioneer-core/tests/fixtures/resource_envelopes/audit.toml)
records the exact source baseline, SDK, lockfile, compiler, features and four
profile digests. [Generated human tables](../adapter-resource-envelopes.md) come
from that record, not a second calculator. This is a source audit, **not** a
frozen remediation package, AG approval, physical measurement or resource qualification completion.

## Ownership Boundary

Each adapter publishes a core `ResourceProfile` alongside its existing envelope.
Bootstrap borrows that profile, and the CLI aggregates its raw inputs, scope,
headroom class, intended ABI and actual SDK metadata. Configurable Fastly/Spin
dispatch uses the same metadata field; fixed SDK wrappers retain their published
metadata. Core still normalizes config caps, and runtime still owns the checked
equation and receipts. No workload cap, accounting coefficient or boot refusal
policy changed. The native schema-2 wire shape and historical captures are
unchanged.

Dispatch alignment now compares metadata, not provider names. A control feeds
a Fastly profile to a differently labeled report row and separately varies
Spin metadata while keeping its labels fixed. Matching metadata cannot clear
other blockers or turn host-native layouts into guest allocation evidence.
Frozen record validation uses recorded accounting scope, not later declarations.

AG selected Akamai Functions hosted Spin and approved adapter-owned profiles
through explicit user replies on October 18. Current Spin dispatch remains
generic and its mismatch remains reported; selecting the intended deployment
does not prove or approve its later dispatch/memory remediation.

## Adapter Selection

The resource command now follows the existing build/deploy/serve selector:
`bidders resources --check --adapter fastly`. Each adapter supplies its own
compiler ABI and resource profile. Operators do not select a Rust target or map
one to an adapter. `--all-adapters` explicitly selects the complete matrix;
canonical audit and review modes require it. JSON reports declare their scope
and use `adapters`/`adapter` identities. A single-adapter fit result is not global
resource qualification acceptance. The unmerged source-audit format is cut over without aliases;
historical qualification captures and their schema are unchanged.

## Executed Verification

| Check                                                      | Result                                                                                     |
| ---------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| Complete CLI unit target at clean `9656a3bc`               | 599 passed, 0 failed, 2 existing child-probe entrypoints driven by parents; 188.75 seconds |
| Resource unit controls after the Markdown repair           | 62 passed, 0 failed; included in the full target above, not additional distinct tests      |
| CLI resource/help/qualification command targets            | 9 / 4 / 15 passed                                                                          |
| Core unit target                                           | 474 passed, 0 failed                                                                       |
| Fresh-process boot target                                  | One parent and all ten counted installing children passed                                  |
| Non-installing catalog profile target                      | 2 passed, 0 failed                                                                         |
| Four adapter library and boot-envelope targets             | Eight binaries passed: 22/25, 19/1, 13/1, 19/1                                             |
| Native schema-2 resource-profile target                    | 36 passed, 0 failed with required local listener/process permissions                       |
| Cloudflare/Fastly/Spin guest compile checks                | Passed on wasm32-unknown-unknown / wasm32-wasip1 / wasm32-wasip2                           |
| Six-package all-target Clippy; final CLI all-target Clippy | Passed with `-D warnings`                                                                  |
| Maintenance-only help/build controls                       | 4 / 2 passed, including corrupt-archive repair                                             |
| Formatting, whitespace, raw-JSON, batch and roster checks  | Passed; 169 budgeted raw fixtures, 139 roster members                                      |

The full CLI run precedes a two-line rendering/test-only correction: the actual
compiler identity has no trailing newline, so its Markdown fence previously
closed on the compiler's last line. A new assertion failed before the fix;
all 62 resource controls and strict CLI lint passed afterward. The final audit
is recaptured against the committed repair, rather than reusing a dirty-source
or superseded capture. No CLI build or source mutation ran during the complete
unit run, whose provider tests hash their executable.

An earlier native resource-profile run had 35 passes and one sandbox-denied
listener (`EPERM`). Its unrestricted rerun passed all 36 without assertion or
timeout changes. An earlier resource command run overlapped source edits and
correctly refused stale provenance; final command checks use frozen source.
The focused unit filter also selected zero integration tests once; the actual
integration binaries were then run separately and their counts are shown above.

### Adapter Selector Follow-Up

The selector implementation is committed at
`ac6d54112638e0576cf57cc6dde2333188e60530`. The final focused binary run
(`cargo test --locked -p auctioneer-cli --bin auctioneer-cli bidders::resources`)
passed all 73 resource/platform controls. The actual command binaries were then
rerun from that clean source: resource commands 12/0, help 4/0 and qualification
15/0. The clean resource run exercises successful capture/render as well as
refusals; it does not take only the dirty-source branch. All four adapter names
are exercised with independently asserted derived ABIs and strict-fit exits.
Missing selectors, compiler triples, partial canonical operations and mixed
single-adapter/review modes are rejected by the real argument parser.

All-target CLI Clippy passed under `-D warnings`, formatting/whitespace checks
passed, and the raw-JSON gate retained 169 budgeted fixtures across 50 files.
No new lint suppression was added. The full CLI unit target was not repeated
after this selector correction; its earlier result above remains bound to
`9656a3bc`, not relabeled as a full run of this later revision.

The canonical audit and generated tables were recaptured with the new
`--all-adapters` syntax at the full code commit above. Each of the four recorded
identities has `dirty = false` and `source_tree_matches_commit = true`.
Rendering revalidated every source profile digest and completed with exit zero.
The artifact records that immutable code baseline, not the later documentation
commit that carries it. It is not the remediation approval checkpoint D.

## Remaining Acceptance Work

The audit captures source-known ownership and keeps every artifact/ABI proof
pending. Its complete component ledger identifies nested entries as explanatory,
not new summands. Every target remains nonpassing under required fit. Known
configured-cap overflow remains a local demand failure even where SDK facts are
unpublished.

The new boot target proves the real installing consumer, SDK strict accounting,
independent zero caps and baked-byte retention in fresh processes. It does not
execute the actual CLI against each synthetic source profile; that cross-consumer
parity is still unfinished. Matched catalog allowances, a concrete justified
remediation package, immutable review approval/CI binding, owner changes and
artifact-bound guest/host/stack proofs remain pending. The audit snapshot commit
is not the plan's review checkpoint D and authorizes no implementation window.

The [auxiliary investigation](resource-envelope-audit-20261018.md#auxiliary-handoff-feasibility-follow-up)
also records the preboot/App-across-boot config-bound gap before any proposed
handoff credit. Its ownership is Auctioneer's, not an SDK publication issue.
Missing host ingress/framing facts, Axum ceiling/population, Cloudflare inbound
population and Fastly stack evidence retain their separate owners/actions.
No live deployment was contacted or mutated; hosted qualification remains publisher pilot.

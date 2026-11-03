# Resource Ownership Controls

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

Date: 2026-10-18. Starting code baseline: `e9293a366199` on the existing bidder catalog PR.
This is progress on the resource qualification audit/proof work, not allocation remediation,
permission to reduce a charge, target qualification or resource qualification completion.

Later source changes are recorded separately: the
[config-value correction](config-value-accounting-20261018.md) replaces the
byte-derived auxiliary node population described below under AG's subsequent
authorization. This baseline record is not a current-accounting recapture.

## Survivor Request Ownership Follow-Up

An October 18 read-only audit at `5344376b` verified that the projection's bidder
population cannot be replaced by one or by stream width. The real
`auctioneer-exchange/src/split/mod.rs::materialize_per_bidder` pushes a complete
sanitized typed request into its output for every surviving bidder. Earlier
requests remain owned while later requests are materialized.
`run_auction` retains that output as `capped`, iterates it by mutable reference
during request construction, and later supplies those same requests to
`make_bids` in `process_results`. Neither failed request construction nor gzip
refusal removes the corresponding tree. The vector releases on return, fatal
error or cancellation.

| Phase                            | Simultaneously retained request state                                                             |
| -------------------------------- | ------------------------------------------------------------------------------------------------- |
| Materializing survivor `i`       | Shared grouping, `i-1` completed typed trees, current scoped/FPD/decode temporaries               |
| Raw/gzip construction and fanout | All survivor trees; independently funded outbound bodies and bounded metadata                     |
| Response handling                | All survivor trees supplied to `make_bids`, plus independently call-count-funded response results |

The existing projection allocator guard measures live peak, not cumulative
allocation. Its factor also covers sequential projection temporaries, so
splitting retained storage from those transients remains a possible **unproved**
correction. The source comment suggesting the projection allowance implies no
per-bidder retained tree does not describe current ownership and must not be
used as evidence for a population reduction.

No population-correction savings were established. A valid follow-up must
measure actual `run_auction` at suspended fanout while independently varying
survivors and container nodes, retain exact `make_bids` scope semantics, cover
amplified supplements/errors/cancellation, and detect an extra retained-tree
clone as a positive control. The sparse scope test that drops each scope before
the next cannot substitute for that production path. Keep the existing aggregate
work-refusal ceiling and six prepaid receipts unchanged until a separately
justified work-versus-memory model preserves both contracts. No byte credit or
new bound follows from this read-only audit.

## Actual Boot And CLI Parity

`auctioneer-cli/tests/resource_boot_parity.rs` builds the real CLI and the existing
single-test core boot probe in an isolated source copy. Only that copy receives
synthetic adapter profiles and two explicit corruption controls. Production
has no environment-based profile override. The helper checks the source IDs
and feature sets of six resource-owner libraries across both builds.

Ten fresh boot children each install once: eight independently vary the zero
backend-byte/blob-byte/node caps, and two retain boot's warn-and-install policy
against a one-byte configured ceiling. Each is compared with both ordinary and
strict actual CLI commands. The comparison covers raw/effective caps, the full
compiled baked host/account tuple's byte count and digest, positive catalog
C/I/V/K terms, construction descriptors, all eleven isolate terms, six auction
receipts and the complete derivation. The native synthetic platform supplies
known facts and a 512-GiB ceiling so these tests isolate consumer plumbing.
That ceiling is not a proposed deployment or a substituted provider fact.

Eight strict commands exit zero; two over-configured-ceiling commands exit
one. Ordinary commands preserve those same accounting rows with exit zero;
only boot warns/installs. Scratch-only bypasses of config normalization and
baked-byte counting each refuse with exit two before exporting a report.
The parent installs nothing. This is not a second budget equation, guest
measurement, production fit or approval of reduced charges.

Verification:

```sh
cargo test --locked --offline --jobs 2 -p auctioneer-cli --test resource_boot_parity -- --nocapture
```

One test passed in 166.85 seconds: ten boot children, twenty CLI comparisons
and two corruption refusals. The worker also ran the parent target with
`auctioneer-core/conformance-suite`, `auctioneer-config/test-harness-override` and
`auctioneer-adapter-axum/insecure-bidders` enabled; it passed in 164.24 seconds.
Those parent features do not supply an oracle from a different compiled baked
tuple: the isolated CLI and boot owner libraries must still match.

## Completed Response Ownership

`auctioneer-core/tests/response_retention.rs` drives the actual completion-based
fanout scheduler with a thread-local counting allocator. The fake driver
creates each payload only when its slot is polled: no queue of preallocated
response bodies can satisfy the measured retention assertions.

The six executed rows use stream-width/call-count/body-byte triples
`(1, 5, 16381)`, `(2, 9, 32749)` and `(3, 10, 65521)`, each through normal
completion and cancellation during the second wave. At every held EOF, the
actual live allocation delta must include all response payloads observed so
far, including earlier completed waves. The completed result retains every
original payload after all stream permits have returned. Cancellation and
dropping the final result each release all payload storage and stream permits.

This establishes a lower bound on retained native requested allocations, not
an upper bound, WASM observation or replacement coefficient. Stream width
cannot replace call multiplicity for completed results under this ownership
model. The isolate equation and every production allowance remain unchanged.

Verification: `cargo test --locked -p auctioneer-core --test response_retention`
passed one test covering all six rows. Targeted Clippy passed with `-D warnings`.

## Config Node Units

Read-only inspection of SDK `f6c708cc` and the real core extraction path found
that a direct substitution of node count for byte count would mix units:

| Quantity                                  | Unit and coverage                                                                                                                                       |
| ----------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| SDK `max_json_nodes`                      | JSON values, including containers and envelope metadata; object keys do not count. The structural scan precedes whole-envelope `Value` materialization. |
| Runtime `NODE_RETAINED_BYTES = 384`       | Syntactic values plus object keys. It is not a general typed-config allocation bound.                                                                   |
| Config `CONFIG_NODE_RETAINED_BYTES = 623` | The config-specific value-count allowance; separate byte and baked-policy terms still apply.                                                            |

For a scan admitting `V` values, syntactic nodes are at most `2*V - 1`.
Secret replacement changes existing string leaves rather than creating new
container structure, but secret payload bytes are not node-bounded. The
current Auctioneer app config declares no secret paths. Ordinary typed conversion,
defaults, validation scratch and merged policy copies still need their own
funding; they are not bounded merely by the original input node count.

The existing auxiliary config-node allowance is `384 * blob_byte_cap`, and
`AuxWorkCaps.config_nodes` is derived by dividing it by 384. Core has no
production `ensure_nodes(Config)` call that would independently enforce that
derived cap. Actual config structure enforcement comes from the SDK. Ledger
reservation is not allocated storage or an allocator interceptor.

At a 1,048,576-byte blob cap and 8,192 SDK values, the current config-node
allowance is 402,653,184 bytes. The separate config-value candidate is
`623 * 8192 = 5,103,616` bytes. This is a comparison of terms, **not** approved
savings. With `aux_route_peak_bytes` unchanged, reducing a derived auxiliary
receipt saves zero bytes in the isolate equation.

The actual-context lifecycle is now exercised as described below; its
allocation upper bound remains unproved. App-held extraction limits are not
retroactively changed by later boot installation. At SDK defaults of 16 MiB
backend and 8 MiB blob, the existing byte-config terms alone are 3,238,205,440
bytes with 1,584 baked bytes. Node tightening does not repair that coverage
gap. Probe config also has its independent count gate, not the auxiliary byte
receipt.

The next config allocation proof must distinguish structural rejection,
typed-conversion/merge work, live allocation peaks and gate reservations.
Vary backend bytes, blob bytes, baked bytes and SDK value count
independently; do not add a post-conversion node check that rejects typed
defaults. No concrete accounting change or smaller accepted workload is
approved by this investigation.

## App Snapshot Lifecycle

`auctioneer-core/tests/config_node_cap.rs` adds a counted, bounded fresh child that
extracts a real SDK config through the actual App dispatch path three times:
before boot, on that same App after boot, and on a newly configured App after
boot. The first two use the default 16-MiB/8-MiB/8192-value caps. The new App
uses independently declared 128-KiB/64-KiB/4096-value caps.

The shallow envelope has 6144 JSON values, with 6138 array elements. Both
default-cap extractions return all elements; only the newly configured App
rejects with `StoreExtractionReason::StructuralLimit`. Three bounded-store
read argument pairs, child PID, one installation, exact caps and outcomes
are checked in a structured receipt. Missing/duplicate receipts and wrong
read/install counts refuse. The parent never installs.

Review found that libtest's serial progress text could hide the first receipt
on a single-CPU runner. Forcing serial execution reproduced the failure. A
leading newline fixes it, and the child now always runs serially to pin that
case. A separate worker negative control temporarily changed only its active
test to reuse the old App for the third read; the receipt check failed. It was
restored before final verification; that mutation was not a separate scratch
capture.

Final combined command:

```sh
cargo test --locked --offline --jobs 2 -p auctioneer-core \
  --test config_node_cap --test response_retention --test boot_strict_parity
```

Passed: three config tests, one response-retention test (six rows) and one
boot-probe test (ten children). The core library's full unit suite also passed:
474 tests, zero failures. Scoped Clippy for the three core targets and real
CLI parity target passed with `-D warnings`. These runs are not a full
workspace sweep. The extraction test proves lifecycle, not allocator peaks,
all-context funding or production entrypoint ordering.

The existing pure `catalog_resource_profile` target passed both tests. The
actual CLI `bidders_resources` target passed all twelve tests, including
adapter-name selection and all four unchanged rows refusing strict fit.
Formatting/diff checks and both raw-fixture gate scripts passed. The one new
allowlist entry is a Rust source-format template, not a JSON fixture; `syn`
validates that scratch source before compilation.

## External SDK State

The latest [EdgeZero PR #275](https://github.com/IABTechLab/edgezero/pull/275)
head was queried on this date and remains
`f6c708ccb5be19ba9fa92575db3c08c0f17e0d72`. No newer dependency revision is
available from that PR to supply the currently missing facts. Exact contract,
enforcement and negative acceptance requirements are now recorded separately
for EdgeZero and Auctioneer in the resource qualification specification's section 5.4.

No live deployment was contacted or mutated. Hosted qualification remains
publisher pilot. The existing canonical source audit retains its recorded baseline;
neither this evidence nor the selector changes constitute review checkpoint D.

## Execution Authorization Update

After progress commit `7b5601c2`, AG explicitly selected on 2026-10-18:
"Proceed with unchanged-workload improvements; ask before limit/equation
changes." This is the actual user reply to the execution-policy question,
not a self-issued approval or an invented frozen review SHA.

The spec/plan now apply that authority to verified implementation-only changes
while retaining specific approval before limit/model changes. Existing
freeze/scope tooling remains available for those packages. This authorization
does not approve a workload reduction, smaller coefficient, changed charged
population, different equation, fitted deployment or fabricated SDK fact.

## Hosted Spin Dispatch Alignment

The actual entrypoint already bootstraps and dispatches from the same exported
adapter profile. Its dispatch constant now selects `AKAMAI_FUNCTIONS_PLATFORM`
and its label is `spin_hosted`, matching AG's deployment selection. No numeric
limits, scope, coefficients, receipts or equation changed. The CLI negative
control now deliberately substitutes generic metadata; it still refuses that
mismatch rather than relying on a defect in the shipped profile.

Before production edits, three Spin assertions and two CLI assertions failed
on the original metadata/label. After the fix the Spin featureful owner suite
passed 54 tests with one existing vendor-loader test ignored; the CLI current
record tests passed three and its real-command resource target passed twelve.
The `wasm32-wasip2` compile check passed, exercising the gated entrypoint. This
was not a release/component-size or hosted memory measurement. Exact commands,
red/green output and entrypoint trace were retained locally under
`/private/tmp/auctioneer-hosted-spin-dispatch.vJFaqL/verification.txt` and its sibling
logs; the commands use `--locked --offline` and two build jobs.

The parent independently reran all fifteen Spin envelope tests and strict
Clippy for native library/tests and the `wasm32-wasip2` library; all passed.
Separate read-only spec and quality reviews found no in-scope issues. The one
existing ignored vendor-loader case was not executed in this local round.

The current real CLI row must show equal dispatch/platform facts but still
include `unknown_host_ingress_memory_accounting`, `required_fit = false` and
strict refusal. Historical audit/capture bytes retain their older source
identity and mismatch; they were not silently rewritten or requalified.

## Auction Overlay Release

Baseline production source: `8783e325`. `auction_inner` now drops `app_cfg`
after `resolve_auction_plan` returns owned state and before entering the
exchange. Host configuration, selected account, alias/timeout plan, features
and selected activity-rule `Arc` handles remain independently owned. The full
sanitized bidder request remains available to `make_bids`; there is no config
reread, cache, validation reordering, reduced cap, coefficient or equation edit.

`auction_config_retention` uses the actual generated App and mutable store,
one warmed App, a fake clock/RNG and thread-local requested-allocation counts.
The measured requests vary an unselected account's vendor list, not the active
account or auction work. The original implementation failed the held-memory
assertion before the production drop was added. All six rows now pass:

| Unselected entries | Prior held growth over empty, bytes | Final native held bytes | Config-phase peak bytes |
| -----------------: | ----------------------------------: | ----------------------: | ----------------------: |
|                  0 |                                   0 |                 312,576 |                  46,467 |
|                256 |                               7,936 |                 312,576 |                  61,600 |
|               4096 |                             126,976 |                 312,576 |                 303,520 |

The same rows execute through successful completion and suspended cancellation.
Each checks a single config read, unchanged selected privacy/alias/cap behavior,
active reservations at suspension, and exact zero dispatch-owned allocation/
gate/receipt residue after cleanup. Completed rows additionally compare exact
response JSON, including selected targeting and debug behavior.
Both byte caps and a stricter keys-plus-values node count admit every fixture.
The phase-specific peak positive controls require config work to grow even
when larger auction work would otherwise mask it in the whole-request peak.

The first fixture attempt encountered a 64-byte cold-App retention difference.
Reusing the same warmed App isolates per-request work without adding a cleanup
tolerance. Some test-only instrumentation/future-layout cleanup followed the
red run; therefore **absolute old-versus-new base totals are not a saving
claim**. The independently varied vendor-list deltas above are the evidence;
the earlier 131,578-byte subtraction from different fixture builds is not used.
Nor are the final absolute native numbers portable limits or guest observations.

Focused worker verification passed seven tests across
`auction_config_retention`, `auction_config_lifecycle`, `privacy_config_sharing`,
`auction_budget_concurrency` and `response_retention`, plus scoped strict Clippy.
The parent independently passed 474 core unit tests, all three existing
config-lifecycle tests and the six-row retention test, and compiled the shared
change through Cloudflare `wasm32-unknown-unknown`, Fastly `wasm32-wasip1` and
Spin `wasm32-wasip2`. Commands use `--locked --offline --jobs 2`. These are
compilation/native ownership checks, not release size, guest heap or hosted
qualification proofs. The extraction peak's general bound, preboot auxiliary
coverage, unchanged budget's large overflow and external facts remain open.

The broader CLI unit command first ended with 553 passes, 48 failures and two
existing ignored child probes in the restricted environment. A representative
loopback test failed with `port_collision` there and passed unchanged with
local socket access. Using that same compiled test binary outside the sandbox,
the fixture/meter/HTTP/native groups passed all 220 tests in 9.88 seconds. A
name-set check verified that all 48 original failures were among those passes;
none was silently filtered out. The original failed log is retained, not
described as a green full-suite run. Both existing ignored probes are invoked
by their passing parent tests; no ignored test was added.

The real CLI resource integration target separately passed all twelve tests
in 37.68 seconds. Final formatting, raw-fixture gate and whitespace checks
passed, and separate read-only spec and quality reviews found no in-scope
issues. This is still not a full workspace or live deployment qualification.

Local CLI verification logs:

- `/private/tmp/runtime-overlay-cli-final.log`: original complete unit run,
  stopped before integration targets after the sandbox failures.
- `/private/tmp/runtime-overlay-cli-local-access.log`: same binary, filters
  `qualification::harness::fixture::`, `qualification::harness::meter::`,
  `qualification::harness::http::`, `qualification::harness::native::`, with
  `--test-threads=2` and local socket/process access; 220 passed.
- `/private/tmp/runtime-overlay-cli-report-final.log`: separate
  `cargo test --locked --offline --jobs 2 -p auctioneer-cli --test bidders_resources`;
  twelve passed.

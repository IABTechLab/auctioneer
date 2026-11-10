# Native Resource Qualification

See [current resource proof ownership](resource-proof-ownership.md) for the source/SDK-bound closure, existing ownership controls and remaining target facts. This document's retained captures keep their original source, profile and acceptance scope.

The Rust harness in the existing `auctioneer-cli` measures the actual
manifest-generated Axum application through bounded external processes. The
harness itself is not acceptance evidence; each measured run needs its own
committed profile and retained evaluation. The
[historical JS capture](evidence/native-axum-e0a540f-local-pass/README.md) remains
unchanged and is not Rust evidence. The
[repaired Rust capture](evidence/native-axum-rust-d68ba17-local-pass/README.md)
has its own committed source/profile predeclaration: all 45 local rows and the
separate frozen-artifact control passed. Overall qualification remains
unsupported, all six SDK attestations pending and publisher readiness false.
The first Rust attempt and its failed separate control are retained, not
relabeled. No hosted provider was contacted; selected deployment execution
remains publisher pilot work.

## Active Resource Protocol

Native resource export, qualification profiles, observations and reports now use
schema version 2. Every `resources` object requires an object-valued `catalog_costs`
with explicit `resident_bytes`, `initialization_bytes`,
`schema_validation_per_auction_bytes` and `context_per_auction_bytes` integers.
There are no omitted-field defaults or positional-array representations. Zero
means no incremental catalog cost beyond the matched baseline, not free catalog
residency. The native harness checks the exported resource digest and rederives
the total and four catalog terms before binding a run.

This is a hard cutoff: active qualification commands reject version-one profiles
and observations rather than reinterpret them. The retained captures below remain
unchanged historical evidence for their recorded source and CLI. Reproducing their
evaluation or frozen-artifact probe requires that pinned historical CLI; a current
test fixture reconstructed from an old shape is not a new capture. Workload
candidate/catalog, fixture, provider-control and runner metadata protocols keep
their independent version-one formats.

## Commit Before Measurement

Review and commit `fixtures/resource-qualification/native-axum.json`,
`fixtures/resource-qualification/catalog.json`, this harness, the finalized CLI
schema and the intended compiled source before execution. The candidate currently
requires a real 4 GiB local budget, 16 calls, two auctions, 32 KiB requests, 2048
JSON nodes, eight impressions and 16/32 KiB encoded/decoded responses. It preserves
the actual four-entry GVL cache. SDK ingress/store settings are explicitly
narrowed within the unchanged funded boot ceilings: 256 connections, a 16 KiB
head/100-header cap and five-second head timeout; 16 store entries, 1 MiB file,
256-byte keys, 256 KiB values, 2 MiB resident and 8 MiB startup limits. They are
validated by the pinned SDK parsers and bound to each inspected resource export;
they are not the SDK defaults or a reduction of the funded parser/store reserves.
The full exported equation, not just those caps, must fit with 256 MiB headroom.
The earlier 512,000,000-byte / 64-call / 64-auction compiled row is refused before
load; a warning at application boot is not qualification.

Confirm the final compiled row matches the candidate and that the real host supports
the declared budget. Review the debug reference's `fixture_shape` limiting factor,
16 captured rows, 4--64 KiB projected body content and 8--64 KiB serialized debug
output. These are predeclared candidate-specific bounds, not measured maxima or
an assertion that every independent global debug limit can bind together.
The dense debug fixture projects at most eight banner formats and retains the
remaining raw JSON nodes in opaque fixture context. It still sends 2048 nodes,
32 KiB and sixteen bidder aliases; ordinary maximum shape and FPD expansion are
separate cases. The format bound is included in the debug fixture, review and
aggregate fixture identities. FPD uses one
impression so its per-bidder merge patches fit the same 2048-node input bound.
The earlier 512-node attempt and its bound
`native-axum-local-7106af6.json` profile remain failed historical evidence. The
2048-node maximum is stronger raw-node work for a new source/candidate identity;
it does not relax latency, peak, rejection or repetition criteria. Neither the
bid-status setting nor runtime budget guards are disabled to make a row pass.

Do not loosen criteria or switch to a non-simultaneous allowance after seeing
results. A failed attempt remains failed. Changes require a new reviewed candidate
commit, new digest-bound preparation and a new evidence directory.

The current 3 GiB peak criterion is intentionally a broad envelope ceiling,
not a tight regression budget. The historical JS capture's maximum service
lifetime high-water is 38,748,160 bytes, so this check permits roughly 83 times that
measurement. It does not establish a minimum safe memory size or tight growth
regression protection. Independent checked accounting, declared headroom,
workload/dispatch witnesses, retained-growth and latency limits still apply.
Smaller deployment candidates or tighter regression budgets require separately
reviewed predeclarations and fresh runs; do not retrofit this capture's criteria.

## Invocation

From the shared worktree, after the candidate/source commit:

```sh
auctioneer-cli qualify native --profile docs/testing/fixtures/resource-qualification/native-axum.json --output /private/tmp/auctioneer-q2-run
```

The normal invocation runs one serial, locked, offline release build of
`auctioneer-adapter-axum` and `auctioneer-cli` with
`--features auctioneer-adapter-axum/insecure-bidders`. Cargo metadata supplies the
target directory; the CLI executable is `auctioneer-cli`, not its Clap display name
`auctioneer`. Do not run competing feature/default builds in the same Cargo target
directory. Both executables are copied to private, digest-verified run paths
before inspection and serving; later target-directory overwrites cannot change
the measured executable. Copies are checked again before each variant and after
the run.

For already verified release artifacts, add `--no-build`. To materialize
the exact CLI profile and checked accounting without starting a listener or load,
add `--prepare`. Preparation still freezes/inspects the release copies and uses
the real typed CLI push in isolated temporary stores. For example:

```sh
auctioneer-cli qualify native --profile docs/testing/fixtures/resource-qualification/native-axum.json --output /private/tmp/auctioneer-q2-prepared --no-build --prepare
auctioneer-cli qualify native --profile /private/tmp/auctioneer-q2-prepared/profile.json --output /private/tmp/auctioneer-q2-measured --no-build
```

The second command must reproduce the first command's identity and criteria.
`--source-commit <40 lowercase hex>` can name an earlier source commit when only
profile/docs were committed later. Tracked or untracked executable-source changes
relative to that commit are refused. `--no-build` requires the operator to establish
the artifact/source association; a digest is not a supply-chain attestation.
Output directories must not already exist. There are no arbitrary command,
bidder-URL, body, worker-count or duration overrides.

## Evidence and Limits

`profile.json` is the authoritative CLI-schema profile, fixed before traffic.
`preflight/report.json` is the real evaluator's empty-evidence accounting check,
not a passing measurement. `observations.json` and `report.json` contain only
allowlisted identities, numerical aggregates and enum values. `runner.json`
contains bounded fixture counters, sampling summaries, process IDs, snapshot
probe results, digest-bound admission proofs, the exact profile/observations/report
file SHA-256 digests and fixed failure categories. `artifacts/` retains the exact
two release executables. Config sources, SDK store files, meter text and temporary
fixture stores are deleted; raw logs, debug bodies, consent and auction/bidder
payloads are never retained in the evidence directory or echoed.
`runner.failures` adds allowlisted stage names, such as
`admission_first_parked`, `snapshot_observe_b` and `settings_dispatch`, with fixed
cause categories/stages where available. Underlying exception messages are never
retained. A later harness correction does not repair or requalify a failed report;
new execution requires new harness/profile identities and a fresh evidence directory.
The 2048-node attempt bound to source `4bcc666` also remains failed: its unbounded
projected formats correctly caused debug collector truncation, while its corrupt
snapshot oracle incorrectly required a config-status header on the auction response.
The corrected geometry and refusal oracle require a new reviewed identity and
execution; neither changes the capture minima, caps or acceptance criteria.

The harness binds executable SHA-256, source revision, linked build features, SDK
pin, harness/catalog/candidate/consent-seed identities, baked config and reference
digests, complete resource export, hardware and meter identity. The resource
export is obtained with the exact sanitized SDK environment used by serving
processes. No raw environment is hashed. OS/meter versions identify the kernel
interface; native runtime version identifies the Axum package. New identities
also bind the Rust harness source inventory, executable and fixture protocol 1.

The bidder is a separate `auctioneer-cli qualify fixture` Rust process on the compiled
Mocktioneer endpoint `http://127.0.0.1:8787/openrtb2/auction`. Auctioneer uses a different port.
The private child reads bounded newline-delimited JSON on stdin, starting with
closed `Options` tagged `kind: "native"`, with `port`, `body_cap`, `max_requests`
and `max_sockets`, and announces
`{"ready":true,"protocol":1,"port":PORT}` on stdout. Controls and counters use
that private NDJSON channel, never public HTTP control routes or a JS wrapper.
Sixteen runtime aliases reach sixteen real adapter calls; no bidder catalog bidder or private
auction engine is substituted. Fixture counters observe received call IDs,
expanded FPD fields and emitted encoded/decoded sizes. Merely sending FPD does
not count as expansion. Encoded gzip padding uses a legal FEXTRA member.
Decode work requires both an emitted compressed response and an actual returned
bid. Intentional stall, mixed-outcome and cancellation cases also contain
independent healthy work so their byte/call rows cannot be populated from intent.

All 24 CLI subcases are scheduled. Ordinary cases have two repetitions; cold
startup/config variant/restoration use fresh process identities with one row each
(45 scenario rows). Each variant has separate one-second cold/warm windows;
scenario windows are at least two seconds with a 250 ms warmup. Workers are capped
at 32, ordinary requests at 128 per phase, process output at 32 KiB, fixture IPC
at 4 KiB, response bodies at 1 MiB and RSS samples at 128 per window. There are no
unbounded sample or latency queues. Four fixed noncumulative histogram buckets
(500,000; 1,000,000; 2,000,000; 120,000,000 microseconds) include every offered
terminal outcome. The reported p99 is a coarse bucket upper bound, not an exact
quantile. `max_histogram_buckets=4` is predeclared in each profile row.
HTTP status takes precedence over auction-body inspection on non-2xx responses:
429/503 remain rejections even when their bounded bodies are plain text, and other
HTTP errors remain errors. Successful responses still require valid auction JSON;
their bidder error/timeout semantics refine the terminal category. The diagnostic
overload-body hash check still runs, without storing its body.

Achieved concurrency uses the predeclared `parked_two_auctions_v1` application
admission oracle, not the requested worker count or simultaneous bidder auctions.
The fixture parks the first ordinary auction's initial stream wave, then a second
ordinary auction is sent. While neither response has started, status must remain
healthy with the exact expected banner/config state, and a third valid auction
must return HTTP 503 with the exact `overloaded\n` body. With 256 SDK connections,
these three requests cannot exhaust SDK connection admission. The source-bound
application/egress admission invariant identifies two occupied application slots.

The fixture then releases exactly one two-leg wave and must observe the second
auction's bidder work before either primary response starts. A second healthy
status/503 challenge confirms both application slots remain occupied. Both held
primaries must subsequently complete successfully with all sixteen actual calls.
The proof records the probes, body digest, returned call counts, exact resource
digest and review-input digest; its canonical digest accompanies each scenario's
attestation. Always-503, only-one-pending, unhealthy/wrong status, early completion
and missing-503 subprocess controls must all reject this oracle.

The indivisible shared stream-wave lease may still allow only one bidder auction
at a time. Fixture bidder-overlap counters are diagnostics, not the candidate's
application-concurrency criterion. The two held primary auctions are included in
the bounded 128-request population and terminal histogram; diagnostic overload
challenges are counted separately. No non-simultaneous allowance is selected.
Queued-call evidence is the observed later-wave call population beyond the
compiled stream width, not an SDK scheduler counter.
Successful ordinary request IDs are independently reconciled against the
fixture's exact expected call count. Empty or fabricated bid responses without
dispatch fail. Ordinary byte/node/call geometry comes from ordinary counters,
not the earlier admission witnesses; those witnesses cannot hide an idle load.
The stall/timeout fixture lets earlier waves return gzip and holds the final
funded wave, deriving its placement from declared calls and the exported active
stream width. Stalling the first wave cannot prove sixteen dispatched calls when
only two streams are available. This retains the declared call minimum rather
than crediting queued intent or restoring witness maxima.

## Debug and GVL Oracles

Debug references use compact recursively sorted-key JSON SHA-256, matching CLI
`Digest::canonical`. They bind the exact fixture and resource export, effective
debug limits and the candidate bounds. `review_sha256` identifies the numeric
reference submitted for review; it does not claim that review happened.
Returned DTOs supply per-auction row/content/output bounds and aggregate capture
counts without persisting their bodies. Public numeric request-ID ranges identify
debug auctions at the fixture. Debug cases use the same parked/wave-handoff
oracle: the second auction must reach actual bidder work while the first response
has not started, both slots must still reject the third auction, and both returned
responses must contain sixteen captured call rows. The pinned exchange invariant
constructs and commits each auction's capture slots before fanout can expose its
first bidder request (`Capture::commit_request`). This proves coexistence of two
constructed capture populations, not simultaneous bidder waves or SDK delivery
lifetime. Simultaneous finalized-call evidence is bounded by the two returned row
minima and that source invariant, never worker intent. Capture release requests
debug for the two held oracle primaries, then runs ordinary work and a healthy
recovery wave. Changed source or resource identity invalidates the proof.

GVL scenarios first drain the independent two-admission oracle, then park a fresh
GDPR=0 ordinary auction with actual bidder calls. Before and after every sequential
GVL lookup, its response must remain unstarted and the fixture must still observe
active held bidder work. Missing or serialized overlap fails. The held auction is
released and drained on every exit; successful completion requires the expected
fixture call count. Its terminal result joins the primary latency population and
uses one of the bounded request slots. After this witnessed phase, the ordinary
load uses one worker. No phase offers two ordinary workers plus a third GVL
request. `worker_budget` describes the maximum scheduling allocation, not inferred
activity. GVL requests remain a separate effect population; rejection bounds are
unchanged.
The probes
use the pinned valid TCF fixture with fixed bit-field updates for a registered CMP,
compiled archive version/policy and legal policy-4+ LI bits. Updates preserve all
trailing seed segments, including the Disclosed Vendors segment required by the
pinned parser. The native 2.6 wire field remains `user.consent`, not
`user.ext.consent`. Typed runtime snapshots explicitly configure existing host
Purpose2 policy with `enforce_algo="full"`, `enforce_purpose="enabled"` and
`enforce_vendors="enabled"`; this uses the existing resolver, not a new gate.
GDPR0 ordinary auctions still dispatch, while consented Mocktioneer vendor0 is
denied and is not claimed as dispatched work. This policy is installed before
startup and bound into config/settings digests. A first-use entry counts only when all of these
conditions hold: the version is present in the clean source archive associated
with the frozen artifact, the generated consent probe returns HTTP 200, a typed
10017 privacy-denial warning proves the enforced path, and no 10001/10018/10019
malformed/unavailable/stale warning is present. Missing or malformed TCF, expired
GCL, absent entries, ordinary GDPR=0 responses and reused versions count zero.

This is a source-bound path proof, not an invented parse counter. In pinned
`crates/auctioneer-privacy/src/resolve.rs` around line 275, successful CMP authentication
and `(spec,list)` selection call `GvlStore::get` before bidder/vendor permission
checks. `gvl.rs` constructs an empty lazy four-entry cache. Fresh processes and
disjoint, previously untouched archive entries therefore force the real cold
parse path. Two cold probes and separate six-entry miss groups avoid crediting
already warm entries; each miss group exceeds the actual LRU capacity while
ordinary auctions overlap. Changed source/reference identity invalidates this
proof. A failed oracle leaves zero evidence and cannot pass local acceptance.

## Configuration and Meter

Every A/B/restored snapshot is installed by typed `config push --local --yes
--no-env --no-diff` before the inspected binary starts. Status banner/config
health, node refusal and actual alias/debug dispatch verify effective settings
before load. Same binary, source store digest and resolved SDK settings are bound
to each process. A dedicated isolated probe pushes B while A runs and verifies A
still serves; only restarting makes B active. Another probe keeps the outer SDK
store JSON valid but corrupts its consumed envelope. Fresh degraded status must
precede an HTTP 500 auction refusal with `Cache-Control: no-store` and the exact
source-pinned fixed refusal-body hash. Auction errors do not carry the status
route's config-status header. No response body is retained. The probe then
restores through typed push and a fresh healthy process.
Measured stores remain immutable. Native live reload is not claimed.

macOS uses BSD `/usr/bin/time -l` maximum resident set size in bytes; Linux uses
GNU `/usr/bin/time -v` maximum RSS in KiB converted to bytes. The service PID is
discovered as the time wrapper's unique child. `ps` RSS or `/proc/<pid>/status`
samples observe that PID only, excluding the generator and bidder. The hard
measurement is the process-lifetime high-water returned by `wait4`, not sampled
RSS or an average. Each separate window retains a bounded sample summary/growth;
its final RSS sample starts only after both work completion and the declared
minimum window. That final read consumes the same 128-sample allowance, its
elapsed time is included, and cancellation or read failure refuses the window.
its hard peak is conservatively the whole variant process high-water, explicitly
`process_lifetime_high_water`, including other phases. Unsupported/failed meters
leave missing evidence. The evaluator rejects decreasing lifetime peaks within a
process's cold/warm and declared scenario schedule; it does not impose that rule
on window-scoped meters. Signals, failures and successful wrapper exit retain
bounded process-group INT/TERM/KILL cleanup ownership until descendants are gone;
cleanup/restoration failures cannot produce a passing evaluation.
The candidate's `max_run_ms` deadline is armed before build/preparation work and
latched in the run's abort context. Expiry aborts active work, reaps managed
processes with bounded escalation and forbids every later child launch, including
snapshot restoration or another variant. Catching an error cannot clear the
latch or return success. An aborted attempt has no completed qualification;
restoration requires a separate fresh run after cleanup.

## Acceptance and Recheck

The real `auctioneer-cli qualify evaluate` decides acceptance. Exit 0 means qualified;
1 means a completed nonqualified/pending/unsupported evaluation, candidate
preflight refusal or run deadline; 2 means malformed input/tool failure. Signals exit 130/143.
A completed local matrix can yield `local_acceptance=pass` while
`profile_qualification=unsupported` because the linked `insecure_bidders` feature
is development-only, and required SDK guarantees have no verified evidence here.
Pending SDK rows are not converted to verified. `publisher_pilot_ready` stays
false. Missing/failed rows, underload, short windows or absent repetitions cannot
pass local acceptance either. Preparation and synthetic schema controls are not
measurements.

This fail-closed property belongs to the runner **and** evaluator. The runner
records observations, not acceptance. The evaluator walks the predeclared
scenario/repetition schedule, not just the rows supplied by the runner: every
missing mandatory row becomes `pending`, which successful sibling rows cannot
promote to `pass`; failed rows and unsupported capabilities can make the result
stricter. Cleanup/cancellation fences prevent publication of an aborted run,
but do not replace this declared-schedule check. Preserve both boundaries when
changing evidence emission. New CLI and harness JSON files use one terminating
newline; historical evidence bytes and their digests are unchanged.

```sh
cargo fetch --locked
cargo test --locked -p auctioneer-cli --all-targets
node --test scripts/tests/usersync-browser-shutdown.test.mjs
```

Rust process/TCP tests require loopback bind permission. The all-target command
includes colocated harness unit tests and the actual `qualification_harness`
CLI/process integration target, as well as the existing `qualification_cli`
schema controls. Install the [pinned SDK verifier prerequisites](provider-handlers.md#offline-checks)
before either workspace `--all-targets` or standalone CLI tests. Our Rust TOML
handling requires neither Node nor yq; the SDK archive verifier still requires
jq, yq 4.53.3 and tar. Node 24 remains for the actual browser shutdown control
shown above and Playwright browser checks, not either qualification harness.
Cargo metadata supplies target paths and honors `CARGO_TARGET_DIR`; tests do not
assume a repository-local `target/` directory.
CLI controls are tracked under `scripts/tests/fixtures/resource-qualification/`;
their README marks every row as synthetic, not measured evidence. Exact guards
require all 24 schema subcases and 24 one-repetition observations. Missing CLI or
control files fail the suite with a prerequisite message, never a passing skip.
Run Cargo validation serially, without competing native builds. Final release
execution follows the reviewed candidate commit, not a test-suite run.

The retained opt-in frozen-artifact microprobe must park one ordinary GDPR0 auction
at its actual bidder wave, then run fourteen compiled GVL versions with that strict
Purpose2 snapshot. Each probe must finish privacy-denied while the ordinary
response has not started, without adding any bidder call. Missing, malformed and
missing-segment consent cannot earn parse evidence. After release, both the held
ordinary auction and a fresh recovery auction must return sixteen bids each.
This is the required proof of actual ordinary/GVL admission coexistence without
worker intent. It must verify both frozen artifact hashes and leave the original
profile, runner, observations and report files unchanged.

The private Rust control is invoked explicitly:

```sh
cargo run --locked -p auctioneer-cli -- qualify frozen-probe --evidence <evidence-dir>
```

The directory must contain retained `artifacts/`, `profile.json`, `runner.json`,
`observations.json` and `report.json` from the same attempt. Ordinary Cargo tests
do not silently skip this execution based on an environment variable; the
command is a separate opt-in check. It installs only temporary isolated typed
config, performs no build or full workload, and creates no passing profile/report.
Until the Task 9 execution record records its result, its actual frozen-artifact
execution remains pending; offline unit controls do not replace that run. The stricter
runtime policy is declared before the probe, not weakened in response to results.

For a bounded rollout, retain the prior known-good artifact/config identities,
start with the declared local envelope and stop on any identity mismatch, failed
local row, cleanup failure, ceiling/headroom violation or unexpected outcome.
Rollback means typed-push the prior config, restart the prior artifact with its
recorded SDK settings, verify status/effective gates and rerun healthy recovery.
A file rewrite alone is not rollback. A secure production artifact, actual SDK
guarantees and deployment-specific rollout/monitoring/rollback remain separate
publisher pilot acceptance; this loopback artifact is never promoted as production-qualified.

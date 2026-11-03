# Rust Qualification Harnesses

## Decision and Scope

The user approved replacing both diagnostic qualification JavaScript harnesses with Rust commands in
the existing `auctioneer-cli`, keeping JavaScript only for Playwright browser checks.
This changes implementation language and operator commands, not qualification
criteria, evidence meaning or the publisher pilot live-provider boundary. The
[diagnostic qualification design](2026-10-05-diagnostics-and-qualification-design.md), current runbooks and existing
negative controls remain the behavioral requirements.

The chosen approach is modules in the existing CLI. A separate Rust tool crate
would isolate dependencies but add another workspace ownership/build boundary.
Separate unregistered Rust binaries would keep the old script-shaped invocation
but split argument/error handling. Neither is needed for this migration.

No compatibility wrappers, Node subprocesses, copied JavaScript implementations,
new workspace crate, live-provider execution or new signing/config loader are
introduced. Historical measured captures and their executed command records
remain immutable; their source revisions retain the old harnesses.

## Commands and Ownership

Existing `qualify validate` and `qualify evaluate` preserve their behavior.

```text
auctioneer-cli qualify native --profile PATH --output DIRECTORY
  [--source-commit REVISION] [--no-build] [--prepare]
auctioneer-cli qualify providers offline --input PATH
auctioneer-cli qualify providers prepare --input PATH
auctioneer-cli qualify providers capture-build --input PATH
auctioneer-cli qualify providers stage-spin --input PATH
auctioneer-cli qualify providers stage-fastly --input PATH
auctioneer-cli qualify providers verify-fastly-archive --input PATH
```

Build/handoff commands consume bounded typed JSON requests/receipts. They do not
execute arbitrary caller-supplied commands. The existing SDK still verifies and
deploys Fastly releases. Provider preparation changes only isolated copied bidder
metadata; production metadata is never rewritten. No command accepts a live
handler URL or provider credentials. Offline success always leaves deployed
handler acceptance pending, resource qualification not evaluated and publisher
readiness false.

Private fixture subcommands start the separate controlled bidder process.
Control messages use bounded newline-delimited JSON on stdin/stdout instead of
Node IPC. They bind only loopback, expose no HTTP control endpoint and accept
only the closed fixture operations/options. Public HTTP routes carry no scenario
or control selector. The provider fixture retains optional operator-supplied TLS
certificate/key files; neither contents nor paths enter diagnostics/evidence.

The hidden `auctioneer-cli qualify frozen-probe --evidence DIRECTORY` command is an
explicit local opt-in control, not a Cargo test silently skipped by an environment
variable. It reuses retained frozen native executables, verifies their declared
digests, installs only isolated temporary config and parks ordinary bidder work
across fourteen real compiled GVL lookups. Missing, malformed and truncated
consent earns no credit; released ordinary work and a fresh recovery request
must each return sixteen bids. Both executables and all four original evidence
files must remain unchanged. It never builds, deploys, mutates provider state or
writes a qualification report, and it retains bounded cleanup on every exit.

Modules live under `crates/auctioneer-cli/src/qualification/harness/`. Common modules
own bounded IO/digests, child supervision, loopback HTTP, fixture protocol and
metering. Native modules own candidate/catalog binding, geometry, orchestration
and evidence. Provider modules own source/build inventory, artifact handoff,
offline case evaluation and recovery controls. Existing qualification profile,
report and evaluator types are reused rather than reimplemented.

## Execution and Resource Contracts

- All untrusted files, JSON/IPC records, subprocess output, response bodies,
  requests, tasks, socket populations, latency buckets and RSS samples retain
  explicit caps. Structured Rust serde/TOML APIs replace JavaScript and `yq`.
- Public file-based JSON inputs reject duplicate decoded object keys recursively
  before Value conversion, including ignored fields. This retains the byte/node/
  depth limits and prevents last-key-wins authorization or identity evidence.
- Fastly archive preflight uses the CLI-only Rust tar parser before SDK lookup
  or extraction. Cap raw entries at 128, regular files at exactly five, each
  CLI/package at 128 MiB and each release/manifest at 64 KiB. Cap regular payload
  at 268,632,064 bytes and the complete expanded gzip stream at 277,096,448 bytes,
  including bounded GNU/PAX records, headers and padding. Validate GNU long
  names and restrict PAX to ownership/timestamps/comments; reject structural
  path/size/link/sparse overrides before any parser can reinterpret record
  boundaries. Require canonical complete single-line PAX records: multiline
  values and blank/incomplete records are intentionally unsupported rather than
  partially consumed by the library's line-based iterator. Refuse links/sparse/
  special files and retain SDK verification.
- Child environments are explicit allowlists. Provider errors, raw output,
  configuration, debug/auction/consent bodies and credentials are never echoed.
  Diagnostics contain only fixed categories and allowlisted stage identifiers.
- Managed Unix process groups remain owned after a successful wrapper exit.
  Timeout, overflow, signal and ordinary exit paths drain/reap children and
  verify bounded group termination. Permission/termination failures stay failures.
  Native process metering remains Linux/macOS only; unsupported platforms refuse.
- Run interruption and the absolute watchdog cancel work, then perform cleanup
  with a separate bounded deadline before deleting temporary state. A timer race
  alone is not a cancellation or side-effect completion fence.
- Use Rust libraries for HTTP, TLS, gzip, hashing, TOML and process signals. Do
  not hand-roll HTTP framing, compression or JSON parsing. Framework-owned Cargo,
  Git, OS metering, archive and SDK verifier tools remain subprocess boundaries.
- Keep the checked-in native candidate/catalog, all 45 scheduled rows, windows,
  repetitions, outcome/latency/peak/growth thresholds and four-entry GVL funding.
  Preserve actual export/environment binding, isolated typed config push and
  A/B/corrupt/restored process observations.
- Preserve separate ordinary dispatch/decode geometry, exact per-successful-ID
  fixture coverage, two-admission witnesses, held-work GVL overlap, final-wave
  stalls, all-offered latency populations and exactly the existing debug bounds.
- Preserve process-lifetime high-water semantics and limitations; sampled RSS
  is not silently promoted to a window peak. The real Rust evaluator continues
  to enforce nonzero readings, independent ceilings, headroom and refusal verdicts.
  Read the final RSS endpoint after both work completion and the minimum window;
  count it against the unchanged sample cap and include its elapsed time. A
  failed/cancelled final read refuses evidence rather than reusing a stale value.
- Source inventory includes untracked/ignored executable/build inputs and
  precisely excludes Cargo-owned generated target directories, not `src/target`.
  Inputs, feature sets, emitted artifacts, release members and staged files are
  revalidated before and after each handoff. Symlink/hardlink/path escapes fail.
- Offline corruption controls retain authorization/isolation, known-good and
  armed-recovery prerequisites, cancellation, explicit termination and completion
  fences. Restore only after injection is quiescent, with a fresh cleanup deadline;
  an unquiescent writer prevents a false restoration claim.

## Evidence and Exit Semantics

Native output retains `profile.json`, `observations.json`, `report.json`,
`runner.json`, frozen executables and the empty-evidence preflight. Existing
profile/observations/report schemas stay authoritative. Serialized bytes and
harness identity may differ, so bind the Rust harness source inventory/binary
and new fixture protocol version into new predeclarations. Canonical digests use
the existing sorted serde representation; never compare JSON property order.
Historical reports are not rewritten or attributed to Rust.

Acceptance is an end-to-end runner/evaluator contract. The runner records only
what it observed; the evaluator enumerates the declared scenario/repetition
schedule. Missing mandatory rows are pending and cannot become a pass through
successful siblings. Failure/unsupported status may dominate pending, never
promote it. Publication cleanup/cancellation fences complement, not replace,
this schedule check. New JSON artifacts have a single trailing newline.

Measured native completion returns 0 only for overall qualified, 1 for completed
nonqualification (including local pass/overall unsupported), and 2 for invalid
input/internal failure. Successful `--prepare` returns 0 with `category: prepared`,
not a qualification claim. Preflight envelope/candidate/hardware refusals and the
absolute run deadline retain exit 1; handled SIGINT/SIGTERM retain exits 130/143
after bounded cleanup. Provider offline control returns 0 only for successful
offline checks, otherwise 2 with a fixed safe diagnostic. It never grants hosted
or publisher acceptance. Existing `qualify` invocation redaction covers all new
subcommands.

## Tests and Cutover

Before deletion, map every old top-level and generated negative-control case to
a Rust owner. Preserve test intent rather than promising identical test counts.
Unit tests are colocated; actual CLI/process/socket tests live in CLI integration
targets. Tests must assert executed row/operation counts and independent expected
values; avoid empty-loop, unvaried-parameter and self-referential guards.

Required families include descendant cleanup after successful exit, ignored
signals, timeout/output/IPC limits, early exit, malformed data without canary
leakage, safe environments, source/fixture/artifact mutation, exact release member
sets and SDK verification, malformed/duplicate identities, connection refusal,
real bounded bidder HTTP/TLS behavior, incomplete recovery fences, serialized GVL
overlap, ordinary underload and dispatch without decode. Retain current real CLI
and opt-in frozen-native controls. Explicit invalid SDK roots must fail, not skip.

CI runs the Rust harness controls through Cargo with exact guarded commands and
feature/environment scopes. Rust harness code and its own TOML controls need
neither Node nor `yq`. Node remains for browser checks and external platform
CLIs. The pinned SDK's external archive verifier retains its own `jq`, `yq` and
tar prerequisites; this migration does not replace or modify that verifier. Once Rust
coverage and commands pass, remove both harness scripts, their JavaScript tests
and the provider fixture script. Update current runbooks/spec/plan commands and
CI mutation guards; dated execution records remain historically accurate.

A fresh release build, committed Rust-source profile predeclaration and actual
45-row native run are required after cutover. Preserve failed runs, do not tune
thresholds after measurement, and independently review the new capture. No live
Cloudflare/Fastly/Spin mutation is authorized; those remain publisher pilot.

# Hosted Handler Preparation

## Scope

This is **offline preparation**, not hosted acceptance. M6 supplies the harness,
isolated artifact handoff, refusal tests, bounded bidder process and this runbook.
All credentialed Cloudflare/Fastly/Spin execution, corrupt-config writes,
selected-deployment Q2-D and publisher rollout remain **unexecuted in M7.6**,
after M7.5 and the selected bidder migration wave/host setup. Full bidder catalog
closure and video parity are not additional prerequisites.

No command in the harness deploys, contacts a hosted service or writes a provider
store. There is no `--live` flag or JS wrapper. Injected observations and recovery
operations are process-control tests only. The HTTPS fixture artifact is not a production
artifact, genuine bidder certification, resource qualification or pilot approval.
The [Rust native resource harness](resource-qualification.md) is independent;
do not use its Mocktioneer HTTP endpoint for hosted requests.

## Offline Checks

From the repository root, with the workspace Rust toolchain, Git, jq, tar and
Mike Farah yq 4.53.3 available:

```sh
cargo fetch --locked
cargo test --locked -p auctioneer-cli --all-targets
git diff --check
```

Socket tests need permission to bind **ephemeral loopback** ports. This is not
permission to access providers. A sandbox `listen EPERM` is a failed local check,
not a hosted failure/pass. Both harnesses run inside the existing `auctioneer-cli`,
not a new crate. Rust serde/TOML APIs parse and serialize our bounded requests
and manifests without Node or yq. The **pinned SDK archive verifier** still uses
jq, yq and tar; keep its yq 4.53.3 prerequisite. Never substitute `sed`, a TOML
regular expression, a handwritten serializer or SDK edits.

The SDK archive tests are ordinary Rust unit tests, including in workspace
`--all-targets` and standalone CLI runs, not optional JS tests. They use an
explicit `EDGEZERO_SDK_ROOT`, or discover the dependency checkout through locked,
offline Cargo metadata. A missing or invalid root fails rather than skips.
The checkout must come from [IABTechLab/edgezero PR #275](https://github.com/IABTechLab/edgezero/pull/275)
at exactly `775366fdac923135e8430b1e3dffebedc648ef7c`. Follow the
[fresh-checkout bootstrap](development.md#fresh-checkout) before offline runs;
an empty cache cannot fetch a Git revision in offline mode.
Historical evidence retains the SDK revision it actually exercised; this
dependency update does not requalify those captures for a new executable.
CI restores the Cargo cache and toolchain, fetches with `--locked`, installs
pinned yq, and uses structured metadata JSON plus jq to select exactly one
pinned `edgezero-cli` dependency. `/usr/bin/git --no-replace-objects` verifies
the checkout revision before a newline-checked private path is appended to
`GITHUB_ENV`. No YAML environment declaration may shadow that SDK selection.
All setup precedes every Rust test command that executes the archive controls.
The verifier checks the SDK script/common helper against Git with replacement
refs disabled before executing either. It runs
only the SDK's offline `prepare-release.sh`, never deploy or provider CLIs.
Node 24 remains for browser controls; external provider CLI runtimes remain
provider-owned. Neither is an implementation dependency of these two harnesses.

## Evidence Boundary

`auctioneer-cli qualify providers offline --input PRIVATE_CONTROL.json`
accepts only `offline_handler_control`, the exact schema constructed in the
tests, all 14 distinct cases, a declared isolated environment, and matching
expected/observed identities. Exit 0 means **offline controls passed**; exit 2
means refused. It never upgrades supplied assertions to live observations.
The checked-in case template is intentionally not an observation/report and is
rejected by this command.

The safe vocabulary shares Q2's digest/revision, build-feature, target,
runtime-version, harness/fixture/reference/baked-config identity categories and
verified/failed/pending/unsupported meanings. It uses the Rust harness's shared
bounded IO and digest helpers without replacing the existing Q2 CLI/schema. Handler
coverage is a distinct evidence kind, not a memory-qualified scenario.

Reports allow only bounded enums, counters, numeric case IDs, versions and
lowercase digests. They always retain `deployed_handler_acceptance: pending`,
`resource_qualification: not_evaluated`, `publisher_pilot_ready: false`.
Identity also includes `source_sha256` and `fixture_metadata_sha256`. Provider
stderr, URLs, bodies, configs, user IDs, headers, tokens, TLS keys, debug objects
and exception messages/stacks are not report fields. Unknown fields are refused,
not copied. Keep build plans, Cargo JSON, manifests, config snapshots, operator
authorization and raw diagnostics outside the public evidence directory.

The offline control matrix's cancellation row is synthetic process control,
not a claim that a disconnected caller can receive HTTP 200. For live results,
record only actually observable cancellation/delivery lifecycle facts. Q1/Q3/Q4
guarantees remain SDK/provider-owned, pending or unsupported where unobservable.

## Isolated Build

`auctioneer-cli qualify providers prepare --input PRIVATE_PREPARATION.json` reads
the shape in
[`preparation.example.json`](fixtures/provider-handlers/preparation.example.json).
Supply an existing **empty directory beneath the OS temporary directory**,
outside the repository, the selected provider, and the same independently
approved credential-free HTTPS origin in `origin`/`approved_origin`. The example
has null safety gates and cannot be run as-is. Use structured JSON to populate it.

```sh
auctioneer-cli qualify providers prepare --input "$PRIVATE/preparation.json"
```

All public provider requests are closed snake_case JSON DTOs: unknown,
duplicate or old camelCase fields are rejected. Duplicate rejection precedes
Value conversion and covers nested/ignored fields, arrays and equivalent escaped
key spellings; the last occurrence cannot override an authorization or identity
refusal. Existing byte, node and depth limits apply before semantic evaluation.
`prepare` accepts `workspace`,
`provider`, `origin`, `approved_origin` and `build_features`; optional `repo_root`
defaults to this source repository. The private `WORK/build-plan.json` contains
the complete typed preparation. The safe preparation summary on stdout is not
a replacement for that plan in later requests.

| Command After `auctioneer-cli qualify providers` | Request Fields                                                                        |
| ------------------------------------------------ | ------------------------------------------------------------------------------------- |
| `capture-build --input PATH`                     | `preparation` (full plan object), `cargo_json_lines` (string), `started_ms` (integer) |
| `stage-spin --input PATH`                        | `preparation` (full plan object), `build` (full build-receipt object)                 |
| `stage-fastly --input PATH`                      | `preparation`, `build`, `bundle_root`, `attestation`                                  |
| `verify-fastly-archive --input PATH`             | `sdk_root`, `archive`, `archive_sha256`, `source_revision`, `destination`             |

`preparation` and `build` are embedded objects, not filenames. Use structured
JSON APIs to compose them from the retained plan/receipt. Build receipts contain
`artifact`, `artifact_sha256`, `source_sha256`, `fixture_metadata_sha256`,
`started_ms`, `target`, `profile` and `build_features`. Keep these requests and
path-bearing handoff outputs private, separate from allowlisted evidence reports.

Preparation compares every bidder-owned production file against committed HEAD
and copies the full tree into `WORK/bidders`, retaining every original byte hash.
It discovers exactly one source-owned HTTPS banner passthrough: a factory-enabled
manifest, a fixed HTTPS endpoint without macros, host overrides or implementation
settings, and a first owner factory case whose sole outbound body equals its request.
Missing or ambiguous eligibility refuses preparation. The private plan carries
the selected relative manifest path and public key as `fixture_bidder`; no name is
assumed. Only that definition's existing endpoint becomes `ORIGIN/openrtb2/auction`.
The fixed authority feeds the existing compiled endpoint/SSRF check.
No new bidder, host macro, URL path switch, runtime endpoint field or TLS bypass
is added. All other production files and all reference/default data remain unchanged.
Rust preparation checks verify the original bytes again before handoff and
revalidate the full source inventory, harness/fixture inputs and **all** isolated
bidder metadata. The fixture metadata
digest covers the entire copied directory, including its member set, not just
the selected manifest. Preparation also writes private `WORK/banner-request.json`
by binding the template's synthetic `fake_provider` parameter slot to the selected
public key through structured JSON. Frozen-input checks bind this request too.

The private `WORK/build-plan.json` records exact source-file digests, source
revision, metadata digest, arguments and the two required build environment
variables. It is **not** a public evidence report. Source identity inventories the
actual Cargo/application/config/crate filesystem inputs, including tracked edits,
untracked Rust modules/build inputs and ignored files under those input roots.
It does not use Git's tracked-file list as the build-input inventory. Traversal
excludes only generated `target` directories at actual Cargo package/workspace
roots, identified from structured Cargo manifests; this includes the nested
`crates/auctioneer-config/target/build_failure_harness` cache. Workspace-root build
output is outside the declared source roots. This is not a blanket exclusion of
all directories named `target`: `src/target.rs`, `src/target/mod.rs` and ignored
Rust source files remain inputs. Cache creation, changes and removal must not
alter the source digest. The isolated source test fixtures copy only the declared
source-inventory file list, never recursively copy package build caches.
Traversal
refuses symlinks and is bounded to 10,000 entries, depth 32 and 512 MiB. The
revision alone is not an assertion of a clean shared worktree. Freeze those
inputs while building; additions, deletions or changed bytes invalidate the
preparation. Revalidation occurs at receipt capture and before/after Spin or
Fastly handoff. Restart preparation after any source/config/fixture change.

The generated build command is, with the selected provider/target substituted:

```sh
cargo build --offline --locked --no-default-features --message-format=json \
  --target wasm32-wasip2 --profile release \
  -p auctioneer-adapter-spin -p auctioneer-config \
  --features auctioneer-config/test-harness-override \
  --features auctioneer-adapter-spin/spin
```

Run from the source root in a credential-free build environment with
`CARGO_TARGET_DIR=WORK/cargo-target` and
`AUCTIONEER__BIDDER_INFO__DIR=WORK/bidder-info`. Other Auctioneer overrides must be absent,
especially defaults, insecure bidder-info and disclosure directories. Do not
inherit `RUSTFLAGS`, alternate Cargo profiles/configs or runtime endpoint overlays.
Use only the approved toolchain/cache environment. Do not enable `insecure-bidders`.

| Provider   | Actual WASM Target       | Profile   | Required Features                                       |
| ---------- | ------------------------ | --------- | ------------------------------------------------------- |
| Cloudflare | `wasm32-unknown-unknown` | `release` | `cloudflare`, `auctioneer-config/test-harness-override` |
| Fastly     | `wasm32-wasip1`          | `release` | `fastly`, `auctioneer-config/test-harness-override`     |
| Spin       | `wasm32-wasip2`          | `release` | `spin`, `auctioneer-config/test-harness-override`       |

Capture the start epoch in milliseconds **before** the command and private Cargo
JSON stdout separately from stderr. Compose the closed capture request with the
full `preparation`, raw bounded `cargo_json_lines` string and `started_ms`.
Only after a zero build exit, run:

```sh
auctioneer-cli qualify providers capture-build --input "$PRIVATE/capture-build.json"
```

It requires a successful Cargo completion,
actual `auctioneer_config` feature observation, a release-profile adapter artifact,
and the emitted WASM filename inside the isolated target/profile. It computes the
artifact digest, checks freshness and rejects repository-target fallback.
The receipt also binds `fixture_metadata_sha256`; a receipt for another isolated
fixture cannot be reused merely because its source revision/features match.
Synthetic eight-byte WASMs in unit tests are control inputs, not compiled Apps.

The existing `crates/auctioneer-config/build.rs` reads the override only when
`CARGO_FEATURE_TEST_HARNESS_OVERRIDE` is enabled. Production/default builds ignore
it. This harness does not change that gate, add it to any production manifest,
or rebuild a production artifact. Production builds must omit the override
feature and all fixture environment variables.

### Spin Handoff

The application `edgezero.toml` selects **release**, but committed `spin.toml`
points to `../../target/wasm32-wasip2/wasm-release/...`. Setting
`CARGO_TARGET_DIR` does not repair that path/profile mismatch.

```sh
auctioneer-cli qualify providers stage-spin --input "$PRIVATE/stage-spin.json"
```

The typed `preparation`/`build` request aligns the fixture to the **release**
profile actually emitted above, copies that exact component to `WORK/spin-handoff`, and uses
structured TOML to set only the `auctioneer-adapter-spin` component's source to
`./auctioneer_adapter_spin.wasm`. It removes that isolated component's automatic build
command so deployment cannot rebuild from an unrelated target/source. It retains
variables/triggers and verifies the copied digest/features. Missing, stale,
wrong-profile, wrong-feature/source and digest-mismatched artifacts fail.

Before M7.6 deployment, use structured TOML operations to prepare the isolated
application manifest selecting that exact staged Spin manifest/component and the
dedicated app name/KV label. Keep its digest in the private handoff. Provision
the actual KV linkage and health/UID variable providers for that app; the
committed Spin manifest is not evidence that a dedicated hosted KV binding exists.
Never change the committed manifest or silently use its repository-local WASM.

### Fastly Managed Release

A standalone WASM, `pkg/` file or successful `auctioneer-cli build` is insufficient.
Use the pinned SDK's **existing** build-app-cli and
`package-application-release-fastly` actions/scripts, with `auctioneer-cli` as both
application CLI package and binary. Their release packager probes the CLI's
lifecycle help and constructs `release.json`; do not handwrite that metadata,
sign/promote a new format, or change the SDK. The exact wrapper entry point is:

```sh
"$SDK/.github/actions/package-application-release-fastly/scripts/package-release.sh"
```

Its inputs are `GITHUB_WORKSPACE` (isolated input confinement root), `RUNNER_TEMP`,
`EDGEZERO__RELEASE__APP_CLI_ARCHIVE`, `EDGEZERO__RELEASE__FASTLY_PACKAGE`,
`EDGEZERO__RELEASE__APPLICATION_MANIFEST`, `EDGEZERO__RELEASE__ADAPTER_MANIFEST`,
`EDGEZERO__RELEASE__SOURCE_REVISION`, and optional
`EDGEZERO__RELEASE__ARTIFACT_NAME`. The wrapper sets Fastly's lifecycle policy.
Use a CLI built from the same recorded Auctioneer source/SDK pin, not an older
cached binary. An older inspected cache lacked `--application-release` and
described obsolete staging-key behavior; do not use that help or binary.

The complete bundle contains exactly the CLI archive, Fastly package, application
manifest, referenced adapter manifest and `release.json`. At this pin the latter
has `format: 1`, `lifecycle_protocol: 1`, `adapter: fastly`, full
`source_revision`, and `{path,sha256}` records for `app_cli`, `package`, and both
`manifests.edgezero`/`manifests.adapter`. Feature/source/artifact identity is kept
in a separate private build receipt, not new unknown release metadata fields.

Package the **emitted receipt artifact**, not a rebuilt repository component.
Installed Fastly CLI help exposes `fastly compute pack --wasm-binary PATH` for
precompiled WASM packaging; run only in the isolated adapter directory with a
private CLI home and no provider credentials. Inspect the resulting package
before handoff. With the full plan/receipt, extracted `bundle_root` and
`attestation` embedded in the private request, run:

```sh
auctioneer-cli qualify providers stage-fastly --input "$PRIVATE/stage-fastly.json"
```

The command checks all member digests, exact file set/path confinement, selected adapter reference,
source/lifecycle/feature identity, and the package's `bin/main.wasm` and
`fastly.toml` bytes against the receipt/adapter digest. The attestation fields are
`source_sha256`, `artifact_sha256`, `build_features`, `package_sha256`,
`app_cli_sha256` (CLI **archive**), and `release_sha256`. They must come from the
frozen build/release pipeline, not be regenerated to bless changed files.

```sh
auctioneer-cli qualify providers verify-fastly-archive --input "$PRIVATE/verify-fastly-archive.json"
```

The closed `sdk_root`/`archive`/`archive_sha256`/`source_revision`/`destination`
request invokes the pinned SDK's offline
`.github/actions/release-core/scripts/prepare-release.sh`. Use a new isolated
temporary extraction destination and a separately recorded archive digest. It
checks exact members, confinement, metadata, source identity and digests before
any provider operation. Before SDK lookup/execution, Auctioneer uses streaming Rust
gzip/tar inspection without extraction: at most 128 raw entries, exactly five
regular release files, 128 MiB per CLI/package member and 64 KiB per release or
manifest JSON/TOML member. Aggregate regular payload is at most 268,632,064 bytes;
the complete expanded stream is capped at 277,096,448 bytes including bounded
extension records, headers and padding. Raw GNU/PAX records are bounded before
the parser processes them. GNU long names are validated; PAX is limited to
ownership, timestamps and comments, never path/size/link/sparse overrides whose
interpretation differs between tar parsers. Links, sparse/special files,
PAX records with multiline values or incomplete/blank framing,
unsupported extension semantics, duplicate/unsafe paths and nonzero hidden
trailing archives fail before invoking the SDK. A subprocess timeout does not
substitute for these byte limits. SDK verifier success is internal consistency, not source
provenance, signature verification, deployed execution or qualification. The
unit-test CLI member is deliberately synthetic and never executed or certified.
When creating archives on macOS, set `COPYFILE_DISABLE=1` and use tar's
`--no-xattrs --no-acls` options; AppleDouble `._*` companions and xattr records
are not declared release members. Do not relax the member checks to accommodate
platform-specific hidden files.

`stage-fastly` reports `sdk_verification: required_at_deploy`; its local preflight
does not replace the SDK verifier. Actual M7.6 deployment must use the matching
application CLI and `--application-release` so the Rust SDK verifier runs again
on the exact loaded manifests/package. Never upload a package directly to evade
managed release/resource-link checks. Do not use `--package` passthrough.

## Controlled Bidder

The private `auctioneer-cli qualify fixture` command starts a separate bounded Rust
process, not a JS fixture or wrapper. Its first stdin record is closed `Options`
tagged `kind: "provider"`, with `port`, `scenario`, `max_requests`, `key_file`
and `cert_file` (null for HTTP). It announces
`{"ready":true,"protocol":1,"port":PORT}` on stdout. Both directions use
newline-delimited JSON with a 4 KiB per-record bound; partial, malformed,
unknown-field and duplicate-field records fail closed. Native children use
`kind: "native"` and their own bounded options, not provider scenario flags.
Scenarios are fixed at startup: `bid`, `failure` (503), `stall` (bounded), and
`canary` (valid JSON with a synthetic private **top-level response** extension).
The canary is outside `seatbid[].bid[]`: the passthrough adapter discards upstream
root extensions while normal client response construction forwards complete bid
values, including ordinary bid extensions/creative. This placement supports the
fixture-specific no-canary client oracle without claiming general redaction of
normal bid data. There is no HTTP
scenario/stats/control route and no outbound-URL scenario switch. Metrics are
available only over the parent's private stdin/stdout NDJSON channel: dispatch
count, last numeric case ID and rejected count. Run one case at a time, compare snapshots,
and require the exact matching request ID, not merely a healthy fixture process.

Limits: 16 KiB request/response budget, 32 requests, eight connections, 32 headers,
two-second stall, three-second socket/read ceiling and 30-second process life.
Only `handler-case-1` through `handler-case-14`, a single `fixture-imp`, and the
fixed auction path are accepted. Tests use ephemeral **loopback only**;
the loopback HTTP client cannot target a hosted origin. The Rust controls cover
local HTTP/TLS behavior, connection refusal, timeout and sanitized transport
classification, not deployed dispatch. The fixture returns one fixed safe banner
bid; no real bidder traffic is used.

In M7.6 expose a supervised fixture at the approved HTTPS origin through an
authorized HTTPS reverse proxy/tunnel, or pass the test-only key/cert paths to
the fixture's TLS listener. It still binds loopback; the approved proxy supplies
external reachability. Provision valid trust chains, do not disable certificate
verification. Keys, exposure tokens, provider credentials and private metrics
channels stay outside repository/evidence. Keep the same approved public origin
across separately started scenarios; never encode operator controls in endpoint
paths, auction params or runtime config. A 30-second fixture restart changes its
counter baseline and must be observed explicitly.

## Deferred Commands

**M7.6 only, after explicit authorization. Do not execute in M6.** Arguments
below come from pinned SDK `crates/edgezero-cli/src/args.rs`, adapter CLI sources
and committed Auctioneer manifests, not guessed provider flags. Build the actual
`auctioneer-cli` binary from the recorded source/pin in an isolated target directory.
Audit its `build`, `config validate`, `config push`, `deploy`, `healthcheck` and
`rollback` help before supplying any credentials; refuse unsupported flags.
Operational targets must be dedicated development/staging applications, never
production. Resolve `EDGEZERO_MANIFEST` to the isolated manifest for build/deploy.
All provider-native output is private, capped and never copied into reports.

```sh
auctioneer-cli build --adapter spin
auctioneer-cli config validate --manifest "$ISOLATED/edgezero.toml" --app-config "$PRIVATE/known-good.toml" --no-env
auctioneer-cli config push --adapter spin --manifest "$ISOLATED/edgezero.toml" --app-config "$PRIVATE/known-good.toml" --no-env --no-diff --yes
auctioneer-cli deploy --adapter spin
auctioneer-cli deploy --adapter cloudflare
auctioneer-cli deploy --adapter fastly --application-release "$VERIFIED_RELEASE" --service-id "$DEDICATED_SERVICE_ID" --staging
auctioneer-cli config push --adapter fastly --manifest "$VERIFIED_RELEASE/edgezero.toml" --app-config "$PRIVATE/known-good.toml" --staging --no-env --no-diff --yes
auctioneer-cli healthcheck --adapter fastly --service-id "$DEDICATED_SERVICE_ID" --version "$STAGED_VERSION" --domain "$APPROVED_DOMAIN" --path /status --staging --retry 3 --retry-delay 1 --timeout 5
auctioneer-cli rollback --adapter fastly --service-id "$DEDICATED_SERVICE_ID" --version "$STAGED_VERSION" --staging
```

`build --adapter` exposes no profile/target/manifest option; fixture compilation
uses the explicit Cargo plan above, not an invented CLI flag. The current SDK
Spin build itself defaults to release and copies into `pkg/`; it does not rewrite
Spin's static manifest source. Cloudflare's build chain also needs its Worker
shim/package tooling in the isolated adapter tree; a Rust WASM check alone is
not a deployable Worker. Its committed commands are `wrangler build/deploy --cwd
crates/auctioneer-adapter-cloudflare`; Spin's are `spin deploy --from
crates/auctioneer-adapter-spin`; Fastly's root manifest has `fastly compute build/deploy`
from the workspace root. These paths must resolve to the isolated
staging tree through the selected manifest, never to committed runtime endpoints
or a stale target. Managed SDK dispatch, not direct Fastly upload, owns deployment.

`--staging` deployment/healthcheck/rollback is Fastly-only. Do not append it to
Cloudflare/Spin commands or reuse Fastly lifecycle semantics there. The existing
healthcheck command is a Fastly lifecycle probe, not a universal adapter health
command. For other providers, use a bounded HTTPS client against `/status` and
the real auction route and require observed outcomes. The shipped `/status`
body defaults empty (204); choose a typed staging-only nonempty banner if the
particular lifecycle health probe requires 200, and record that config digest.
Authenticated `/_auctioneer/health/gvl` remains separately restricted and fail-closed;
never make health public to make a test pass.

## Corruption Authorization And Addressing

Valid installation/recovery always uses the **typed** Auctioneer config push. It
rejects malformed input and regenerates a correct `BlobEnvelope.sha256`.
Rejected push, parse failure before installation or a well-formed re-enveloped
push is **not deployed-handler corruption evidence**.

Before any M7.6 write, obtain explicit operator authorization naming the approved
provider account/application/service/version, physical binding/store ID and
exact consumed root/key. Names/IDs belong in the private authorization record;
evidence contains their digests, never credentials. Prove the binding is dedicated
and unshared, inspect runtime addressing and arm independent bounded recovery.
An approval for deployment is not approval for corrupt-config mutation.

- Cloudflare: inspect the staged Worker KV binding/namespace ID, its declared
  `EDGEZERO__STORES__CONFIG__AUCTIONEER_RUNTIME__NAME` and `__KEY`, and the deployed
  environment. The committed file has no provisioned namespace proof. The SDK
  writes with `wrangler kv bulk put FILE --namespace-id=ID --remote` from the
  selected wrangler manifest's directory. Native fixture payload is a structured
  JSON array of `{key,value}` with the bounded corrupt string as `value`; never
  write the whole runtime tree or use default/local namespace selection.
- Spin: inspect the selected component's KV label/link and actual runtime backend.
  KV configuration is not an arbitrary component variable. For Fermyon Cloud the
  pinned writer uses `spin cloud key-value set --app APP --label LABEL KEY=VALUE`.
  Other backends need their approved native API; unsupported remote visibility
  stays pending. `--local` writes SQLite, not the hosted binding. Do not mistake
  that success for a cloud observation. No cloud readback command is invented here.
- Fastly: inspect the selected deployment environment, linked Config Store ID,
  logical alias and **actual selected key**. Do not guess a `_staging` suffix:
  this pin uses environment-selected physical stores/keys. The root may be a
  direct envelope or a `fastly_config_chunks` pointer. Entries over 8,000
  characters use 7,000-character content-addressed chunks; runtime per-entry
  reads cap at 32,000 bytes. Chunk keys have `.__edgezero_chunks.` infix.
  Replace **only the authorized consumed root** with a small direct corrupt value;
  never overwrite/delete shared content-addressed chunks. Readback uses
  `fastly config-store-entry describe --store-id=ID --key=KEY --json`; bounded
  private output includes `item_value`, which must not enter evidence. Native
  mutation uses `fastly config-store-entry update --store-id=ID --key=KEY --upsert
--stdin`, with the fixture on stdin, exactly as the SDK writer does.

The safe `malformed-config.txt` is intentional malformed bytes; the separate
`digest-mismatch.json` is structured valid v1 envelope JSON with an incorrect
data digest. Rust corruption-fixture controls construct equivalent values through
JSON APIs except the intentional malformed bytes. Both are under 1 KiB, below
Fastly's direct-entry limit; verify the actual compiled config-value/node/deadline
limits and each provider's current limit before authorization. Never enlarge a
loader cap, weaken digest validation or introduce a permissive push mode.

## Visibility And Recovery

1. Install a reproducible known-good **safe staging** config through typed push.
   Preserve its exact input/digest, application release/version and restoration
   command privately. Observe a healthy auction with matching fixture dispatch
   before arming mutation. A generic status/health 200 is insufficient.
2. Arm a separate operator/watchdog recovery deadline and private incident record
   **before** injection. Set one bounded visibility window, per-request timeout,
   finite retry count and maximum mutation duration. Do not restart the window
   after each inconclusive response. Rust recovery unit controls test the analogous
   interruption/refusal decisions with injected operations offline; this is not a
   provider executor or real safety watchdog. Each of `inject`, `observe`,
   `restore` and `recover` supplies an owned operation with a cancellation-aware
   run future and termination future. Completion must fence **all** side effects;
   fire-and-forget work and unfenced callbacks are not supported. Termination must
   stop/drain the operation (for a managed process, kill and await close) and
   acknowledge true. Timeout/interruption aborts the operation, then requires
   both acknowledgement **and** completion settlement within the separate bounded
   termination deadline before restoration may begin. Abort acknowledgement
   alone is not proof of quiescence. Missing/invalid completion or inability to
   stop yields `operation_not_quiescent`, skips restore/recover and requires the
   independent operator procedure. Never claim recovery while a writer can still
   complete late; a timer race alone does not cancel side effects.
3. Write only the authorized consumed root. Independently establish readback or
   equivalent native visibility at that exact address, then observe the real
   generated App return its specific fail-closed config response (currently 500,
   `Malformed host configuration`) with **zero additional fixture dispatches**.
   Compare counters and case ID against a recorded baseline. A write error,
   stale valid response, missing-chunk propagation 503, backend read failure,
   HTTP connection refusal or timeout does not pass the integrity row.
4. After confirming the writer has stopped/drained, restore through the normal
   typed push in `finally` and on handled SIGINT/SIGTERM.
   Use a new bounded cleanup deadline independent of the aborted auction/run.
   Require observed healthy auction **and** matching recovered fixture dispatch;
   record replacement/corruption/recovery visibility windows and any required
   restart/cache behavior per backend. Do not infer instant/global propagation
   or live reload from an in-memory one-read test. Native restart-only semantics
   and unavailable hosted visibility must remain explicit.
5. Failed or timed-out restoration fails the whole run and opens a safety incident.
   Stop further mutation/traffic, keep the isolated deployment quarantined, and
   hand the exact authorized IDs/version and known-good input to the operator.
   Never publish the config or an exception body to explain the incident.

An unquiescent injection is also a safety incident, not an observed restoration
failure followed by a speculative retry. Keep the dedicated target quarantined;
the independent operator must first stop/confirm cessation of the writer and
only then restore/observe recovery. The offline helper cannot terminate arbitrary
unowned in-process work. Its operation contract must honestly account for all work;
use separately supervised processes for work requiring forced termination.

SIGKILL, host loss and terminal/network loss cannot be repaired by Rust
signal handlers. Independent recovery must survive the harness: operator reapplies
the recorded typed push to the verified dedicated binding, restarts/redeploys the
known-good **matching** artifact if required, observes dispatch/auction recovery,
then deactivates/removes the test deployment only under separate deletion approval.
Do not mark an interrupted run complete merely because a cleanup callback ran.
If isolation, mutation authorization or visibility is missing, leave the live row
pending and do not write anything.

## Live Matrix And Handback

Every row in [`cases.json`](fixtures/provider-handlers/cases.json) remains pending
for **each** Cloudflare/Fastly/Spin deployment. Execute only selected authorized
targets in M7.6; unrelated providers do not gate that publisher's pilot.
Use the prepared `WORK/banner-request.json` with sequential numeric case IDs and
the selected passthrough factory. Derive staging account/debug permissions through typed config;
never turn privacy enforcement off or put live identifiers/consent into fixtures.
Use the existing `runtime.accounts.handler_fixture_allowed.debug_allow = true`
and `runtime.accounts.handler_fixture_denied.debug_allow = false` fields in the
private typed config for the corresponding synthetic publisher IDs. Debug cases
set `ext.prebid.debug` through structured JSON; retain the normal consent checks.

Require returned banner ID/imp/price/currency plus dispatch; for bidder failure
and timeout require the exact returned safe status/error category, not a network
error contacting the App. Add malformed request, both config corruptions and
recovery, denied debug, fully redacted debug/canaries, input/output boundaries,
response-header fidelity, valid config replacement and observable lifecycle rows.
For debug, test allowed/denied accounts and assert no synthetic private canary,
credential, URL or raw config reaches the **debug projection, logs or report**.
For this fixture the top-level canary must also be absent from the ordinary
client bids: upstream root extensions are not forwarded. The offline regression
models that normal whole-bid forwarding plus a positive captured response body
with numeric bid shape retained and root extension masked as `[REDACTED]`; it is
not a deployed capture observation. Arbitrary normal bid/creative extensions
are forwarded, not privacy-redacted by the debug projector. Do not apply a
whole-client-absence assertion to canaries placed in ordinary creative data.
Cloudflare's original
response Content-Type provenance is Unknown: body omission is expected, not
positive capture. For output caps test both sides of actual compiled thresholds;
the fixed bidder provides bounded content, not a fabricated large response.
Assert exact application headers; provider-added framing remains outside that
application-only claim. Unobservable cancellation/raw-parser/egress facts stay
pending/unsupported with their SDK owner reference, never synthesized.

Bound every forward build/deploy/probe/rollback command with a supervised process
deadline and capped private stdout/stderr. The Rust supervisor drops inherited
credentials and handles process-group INT/TERM/KILL termination, escalating after
100 ms with a 1.2-second signal-escalation budget plus bounded reader/monitor joins.
It is for offline tooling only in M6. Do not use its
default scrubbed environment as an improvised credentialed deploy driver. Live
execution needs a reviewed least-privilege operator environment and independent
provider recovery; killing a local CLI does not undo an accepted remote deploy.
Fastly staging cleanup uses the exact rollback/deactivate command above. No
provider-generic delete flag exists in Auctioneer. Cloudflare/Spin deletion requires
approved native CLI/API help and exact test IDs; never guess a delete command or
touch a shared binding/chunk. Do not run config GC as fixture cleanup.

Hand back the measuring Auctioneer revision **and actual source digest**, SDK pin,
provider/target/runtime version, deployment/version identity digests, actual build
features, source/artifact/CLI/package/manifest/release/fixture/config/harness
digests, per-case safe dispatch/outcome/visibility observations and cleanup status.
Keep restoration material and authorization private. Invalidate evidence after
any changed source, feature, fixture, config, package, manifest, SDK or deployment.
Task15 owns the Q2-E/publisher runbook. Selected actual deployment Q2-D, resource
guarantees, selected bidder consent/identity/targeting/creative acceptance and
bounded pilot/rollback remain separate M7.6 obligations. Do not mark live
acceptance, real bidder support or production qualification executed from this
offline handoff.

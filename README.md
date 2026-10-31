# Auctioneer

[IAB Tech Lab repository](https://github.com/IABTechLab/auctioneer).
This is a fresh-history import. Historical plans and evidence are retained;
see [repository import](docs/repository-import.md) for their status.

An OpenRTB 2.6 auction engine written in Rust, usable as a standalone server
or a library for in-binary integration. Built on the
[EdgeZero](https://github.com/IABTechLab/edgezero) framework, its standalone
service runs across Axum (native), Cloudflare Workers, Fastly Compute@Edge,
and Spin (Fermyon). Direct Trusted Server integration is a product goal;
see the [vision](vision.md) for the distinction from the existing library.

**Product vision**: [`vision.md`](vision.md)
**Architecture overview**: [`docs/architecture.md`](docs/architecture.md)
**Design spec**: [Master design](docs/superpowers/specs/2026-05-21-auctioneer-openrtb-server-design.md)
**Approved roadmap**: [`docs/roadmap.md`](docs/roadmap.md)
**M1 plan**: [`docs/superpowers/plans/2026-05-25-m1-foundations.md`](docs/superpowers/plans/2026-05-25-m1-foundations.md)
**Upstream credit**: [Prebid and other acknowledgements](#acknowledgements)

## Workspace

| Crate                                                                   | Purpose                                                                                                                                                                    |
| ----------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [`crates/auctioneer-core`](crates/auctioneer-core/)                             | EdgeZero app entry point; routes and handlers land in M2                                                                                                                   |
| [`crates/auctioneer-runtime`](crates/auctioneer-runtime/)                       | Host-environment seams: `Clock` + `Rng` traits with deterministic test impls (`FakeClock`, `SeededRng`), Fisher-Yates `shuffle`, Lemire `gen_range`                        |
| [`crates/auctioneer-openrtb`](crates/auctioneer-openrtb/)                       | OpenRTB 2.6 typed model + `prepare(req, headers)` — the sole construction gate for a `NormalizedBidRequest`                                                                |
| [`crates/auctioneer-config`](crates/auctioneer-config/)                         | Host / account / bidder configuration. Build-time codegen reads bidder-owned `bidder.toml` manifests, `auctioneer.toml`, and shared reference data, then emits typed accessors |
| [`crates/auctioneer-adapter-axum`](crates/auctioneer-adapter-axum/)             | Axum (native) entry point                                                                                                                                                  |
| [`crates/auctioneer-adapter-cloudflare`](crates/auctioneer-adapter-cloudflare/) | Cloudflare Workers (wasm32-unknown-unknown) entry point                                                                                                                    |
| [`crates/auctioneer-adapter-fastly`](crates/auctioneer-adapter-fastly/)         | Fastly Compute@Edge (wasm32-wasip1) entry point                                                                                                                            |
| [`crates/auctioneer-adapter-spin`](crates/auctioneer-adapter-spin/)             | Spin / Fermyon (wasm32-wasip2) entry point                                                                                                                                 |

## Status

- **M1 (foundations)** — complete. `auctioneer-runtime`, `auctioneer-openrtb`, `auctioneer-config` with their unit and integration tests.
- **M2 (auction core, no privacy logic)** — complete. `/openrtb2/auction` end-to-end plus `/status` and `/info/*` per the design spec.
- **M3a (GDPR/TCF privacy + axum fanout)**: complete on the `m3-gdpr` branch, including TCF v2 parsing and legal-basis enforcement, GVL/GCL authenticity and freshness, GDPR scrubs, config overlays (per-bidder kill-switch, `sample_rate`, host aliases, per-account), and golden fixtures. Subsequent M3b/M4 work hardens configuration loading and resource ownership. Production deploys use [EdgeZero's production-deploy tooling](https://github.com/IABTechLab/edgezero/pull/381); commands in `edgezero.toml` are dev/staging-only.
- **M4 implemented and locally verified**: the `m4-privacy` worktree includes the latest M3b fanout changes, GPP source normalization, independent CCPA/LMT/COPPA and activity decisions, bidder-local scrubs/projection, bounded outbound construction, and measured per-auction reservations. Native default/alternate-feature tests, conformance, strict Clippy, and all three WASM check/conformance builds passed. These are logical accounting limits, not a physical-OOM guarantee; see the [verification record](docs/superpowers/plans/2026-09-28-m4-verification.md). Merge M3b first, then M4 into `main`; hosted CI is separate.
- **M5 user sync implemented in this worktree**: `/cookie_sync`, `/setuid`, `/getuids`, and the preflight use the generated App. Issued UID state has a mandatory MAC envelope. Native and Chromium loopback checks are recorded in the [verification record](docs/superpowers/plans/2026-09-30-m5-verification.md); cross-site browser policy and hosted-provider execution remain separate qualification.

## Roadmap

M6 retains its existing parity-hardening scope. Under the approved
[2026-10-26 scope correction](docs/roadmap.md#2026-10-26-scope-correction),
M7 owns foundations and full selected-flow acceptance for all 14 priority
bidders, including Sharethrough, Rubicon and APS; M20 owns remaining
full-catalog migration and closure, not a prerequisite for M7 completion.
Direct Trusted Server embedding remains M7.5, followed by stored
requests/accounts (M8), currency/multibid/auction identity (M9), creative caching (M10), and full
video support (M11). Remaining shared parity work is assigned to M12-M19.
These are planned capabilities, not implementation or production-readiness
claims. See the [roadmap](docs/roadmap.md) for scope, dependencies, and
completion criteria.

A banner/display publisher with its required bidders implemented can target
a controlled pilot at M6 without waiting for the full catalog or video.
Publisher-specific dependencies can move that checkpoint; see the
[pilot requirements](docs/roadmap.md#6-publisher-pilot-checkpoint).
For the equivalent in-binary Trusted Server pilot, the integration checkpoint
is [M7.5](docs/roadmap.md#7-trusted-server-embedding-milestone); selected bidders
can make that integration possible before the full catalog is complete.

## Build

Install the versions declared in [`.tool-versions`](.tool-versions) using your
tool manager. The file pins Rust, Node.js, Fastly CLI, Spin, Viceroy, and
Wasmtime to the reviewed EdgeZero toolset. A version declaration does not
install a missing tool or its version-manager plugin.

Rust builds every target; Node.js serves the browser and Worker tooling.
Fastly CLI and Spin package their adapters, and Viceroy runs the local Fastly
guest controls. The pinned Wasmtime is required for selected-source WASIp1/P2
schema execution tests, including the workspace test run. CI installs its
checksum-pinned release before those consumers. Compile/package-only WASM jobs
still do not establish runtime execution. Relevant CI setup reads `.tool-versions` instead of
maintaining separate Rust, Node.js, Fastly, Spin, or Viceroy version literals.

For the normal edit loop, start with the affected package rather than the
workspace. See [development loops and test ownership](docs/testing/development.md)
for focused commands, required-feature checks and isolation rules. Retain
Cargo's incremental cache; do not routinely clean targets between edits.

```sh
cargo check --locked -p auctioneer-bidder --lib
cargo test --locked -p auctioneer-bidder --lib endpoint_builder::tests::
```

Broad native checks (not the complete CI/guest/qualification matrix):

```sh
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

The `insecure-bidders` cargo feature on `auctioneer-config` enables
deterministic loopback test bidders:

```
cargo test --workspace --features auctioneer-config/insecure-bidders
```

Axum's default `cargo test -p auctioneer-adapter-axum --all-targets` also runs the
handler-to-socket privacy test. Its test-only dependency enables the loopback
bidder; normal production builds retain the secure defaults. The test sweeps
CCPA and all three disclosure activities independently.

## Auction privacy configuration

The executable examples in [`auctioneer.toml`](auctioneer.toml) cover baked defaults
and pushed runtime overrides. CCPA and LMT enforcement default off; activity
controls default to allow. GDPR enablement does not bypass independent
CCPA, LMT, COPPA, or activity decisions. COPPA has no disable override.
Activity configuration accepts at most 128 rules per activity, 128 combined
name/type entries per rule, and 128 UTF-8 bytes per component name. Runtime
rule arrays replace inherited arrays; `rules = []` clears them.

The internal GPP envelope parser supports TCF section 2 and USP section 6
through separate policy modules. Other sections remain opaque; this does
not implement US-state policy enforcement. Parser ceilings are:

| Input or work                                    | Ceiling   |
| ------------------------------------------------ | --------- |
| Raw GPP UTF-8 value                              | 256 KiB   |
| Encoded header                                   | 4 KiB     |
| Total sections, including unknown/empty sections | 64        |
| Numeric section ID                               | 1..=65535 |
| Charged decoding operations                      | 524288    |

Limit violations and invalid `ext.prebid.nosale` names return `400` before
bidder construction, even when enforcement is disabled. Within-limit format
errors instead use fixed warnings and the documented source-selection rules;
an empty or invalid active TCF section never retries granting legacy consent.
See the [M4 design](docs/superpowers/specs/2026-09-26-m4-privacy-design.md)
for precedence, scrubs, and the remaining acceptance requirements.

`[baked.host.resource_limits]` defines independent `max_outbound_raw_bytes`
and `max_outbound_request_bytes` ceilings for raw and transport-encoded bidder
bodies. Set the same fields under `[runtime.auction]` for pushed overrides.
Both default to 256 KiB, accept 1 KiB through 16 MiB, and are clamped
to the installed boot envelope. An oversized bidder request is a bidder-local
code-5 refusal; exceeding an auction's prepaid resource allowance instead
returns fixed `503 overloaded\n`. The latter never changes consent selection.

## User sync operator setup (M5)

Provision `usersync_cookie_keyring` in the default EdgeZero secret store before
serving UID routes. See [key provisioning and rotation](docs/deployment/usersync-cookie-keys.md).
Unsigned/tampered cookies are rejected; unavailable keys return 503 without
cookie writes. A valid signature protects issued state, not ownership of the
public callback's supplied UID or an authorization/consent grant.

The example [`auctioneer.toml`](auctioneer.toml) uses
`https://sync.example.test` only as a demonstration callback base. Set
`baked.host.user_sync.external_url` (or its `runtime.user_sync` override)
to the public HTTPS origin and optional base path that actually reaches
this deployment, for example `https://sync.publisher.example/auctioneer`.
The default redirect template then builds callbacks under that base path.
Do not derive the callback from request `Host`, `Origin`, or forwarded
headers. Baked changes require a rebuild; runtime changes require a config
push and take effect on a subsequent request. Invalid effective sync config
is rejected rather than replaced by permissive defaults.

`host_cookie.cookie_name` and `family` identify a separate host cookie to
import; Auctioneer's writable UID cookie is always named `uids`. Configure an
optional cookie `domain`, positive `ttl_days` (default 90), and optional
opt-out cookie `name` and `value` together. `max_cookie_size_bytes = 0`
disables only the logical eviction threshold, not the hard Set-Cookie cap.
Account `cookie_sync` settings select default/cooperative limits and can
replace host priority groups, including with an empty list. A missing or
empty account selects baked/default policy; a supplied account ID selects
its named override when present. `runtime.bidders.<name>.usersync_enabled`
can disable sync independently of auction participation, but cannot create
a syncer absent from the baked bidder metadata.

Cross-origin JavaScript access is off by default. Add exact
scheme/host/port origins to `user_sync.allowed_origins`, without paths or
wildcards, and send browser requests with credentials when cookies are
needed. Allowed application responses use credentialed CORS and
`Vary: Origin`; `OPTIONS /cookie_sync` permits POST with `Content-Type`.
Pre-config/admission/framework failures cannot promise CORS headers.
Auctioneer-owned sync responses use `Cache-Control: no-store`. The `uids`
cookie uses `Path=/` and an expiry; `Domain` is optional. The current
Chrome/CriOS 67+ user-agent rule adds `Secure` and `SameSite=None`.
Browser acceptance, especially in third-party contexts, is not guaranteed
by a successful server response and needs real-browser validation.

| Sync surface                  | Hard limit in this worktree                                                                                                                                    |
| ----------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `POST /cookie_sync`           | 32 KiB body and 4,096 JSON nodes; compressed bodies rejected                                                                                                   |
| `GET /setuid`, `GET /getuids` | 32 KiB encoded query, at most 32 parameters; adapter request-target profiles allow 33 KiB, subject to provider limits                                          |
| Cookie decode                 | Current profiles admit 16 KiB of request headers; codec ceilings are 64 header fields, 32 KiB header/encoded value, 24 KiB decoded value, and 4,096 JSON nodes |
| Selection/config              | 256 requested or catalog names, 128 UTF-8 bytes per name, 32 priority groups, 256 priority entries                                                             |
| Output                        | 32 KiB per sync URL, 256 KiB response body, 4 KiB complete signed Set-Cookie field value                                                                       |

These are resource ceilings, not a promise that every legal browser or
provider request is admitted. M5 does not migrate the full upstream bidder
catalog and does not feed stored `buyeruid` values into auction bidder
requests; `/getuids` exposes the stored map to clients only. See the
[M5 design](docs/superpowers/specs/2026-09-30-m5-usersync-design.md)
for endpoint ordering and intentional reference differences.

## Per-adapter dev commands

Routes are wired in M2; the commands below are the cargo / framework
entry points the adapter shells use when handlers exist.

| Adapter             | Local dev                                              |
| ------------------- | ------------------------------------------------------ |
| Axum                | `cargo run -p auctioneer-adapter-axum`                     |
| Cloudflare Workers  | `wrangler dev --cwd crates/auctioneer-adapter-cloudflare`  |
| Fastly Compute@Edge | `fastly compute serve -C crates/auctioneer-adapter-fastly` |
| Spin                | `spin up --from crates/auctioneer-adapter-spin`            |

## Operational endpoints (`/_auctioneer/*`)

`GET /_auctioneer/health/gvl` reports GVL archive health (archive age,
bundled vendor-list versions, and the rolling unavailable-lookup
counter). The handler is fail-closed: it answers 401 unless the adapter
boot installed an explicit access policy — either a bearer token
(`AUCTIONEER__RUNTIME__HEALTH_GVL_TOKEN` on Axum / Cloudflare; the platform
secret store on Fastly / Spin) or the machine-checkable
`allow_unauthenticated_health = true` opt-in.

Deployments that opt in to serving `/_auctioneer/*` without a token MUST
restrict the `/_auctioneer/*` path prefix at the ingress layer. That
restriction is an L7 (HTTP-path) rule — an L3/L4 network policy such as
Kubernetes `NetworkPolicy` cannot match an HTTP path and does NOT
satisfy it. Per-platform recipes (Cloudflare Access, Fastly VCL,
Kubernetes ingress `HTTPRoute` + `AuthorizationPolicy`, nginx
`auth_request` for standalone Axum) live in
[`docs/deployment/restrict-operational-endpoints.md`](docs/deployment/restrict-operational-endpoints.md).

## License

Copyright 2026 IAB Technology Laboratory, Inc.

Auctioneer is licensed under the [Apache License 2.0](LICENSE), matching
[IAB Tech Lab Trusted Server's license](https://github.com/IABTechLab/trusted-server/blob/main/LICENSE).
Third-party copyrights and attribution remain in [NOTICE](NOTICE) and the
source-specific notices; the project copyright does not replace them.

## Acknowledgements

Auctioneer builds on [EdgeZero](https://github.com/IABTechLab/edgezero) and the
[IAB Tech Lab OpenRTB specification](https://github.com/InteractiveAdvertisingBureau/openrtb2.x).

Auctioneer credits [Prebid.org](https://prebid.org/) and the
[Prebid Server](https://github.com/prebid/prebid-server) contributors for the
selected bidder implementations ported and adapted to Rust, bidder metadata,
parameter schemas, auction/privacy/user-sync reference behavior, and captured
test fixtures. Additional bidder reference material comes from the
[Stackpop fork of Prebid Server](https://github.com/stackpop/prebid-server).
GPP reference fixtures also use
[Prebid's go-gpp](https://github.com/prebid/go-gpp/tree/v0.2.0);
this fixture reference is not a Go runtime dependency.

Auctioneer's Rust adaptations, policy decisions and extensions differ from these
upstream projects. Attribution does not imply complete behavioral parity,
certification, affiliation or endorsement.

The [NOTICE](NOTICE) records upstream credit and the retained Prebid Server
copyright notice. The unchanged
[shared reference license](crates/auctioneer-bidder/src/bidders/LICENSE.reference)
remains with the bidder source witnesses. The patched schema compiler retains
its [MIT license](vendor/jsonschema-macros-core/LICENSE). The patched
`regex-automata` retains its [MIT](vendor/regex-automata/LICENSE-MIT) or
[Apache-2.0](vendor/regex-automata/LICENSE-APACHE) license alternatives.
Other third-party materials retain their own licenses and notices.

Reference-derived bidders' `bidder.toml` files record their immutable source
revisions and intentional differences. Attribution and shared license notices
belong here, not in a new license file in every bidder folder. Existing
byte-pinned reference captures are consolidated with their integrity readers under the
[asset migration plan](docs/superpowers/plans/2026-10-23-bidder-owned-assets.md);
their duplication is not the convention for new bidders.

# Auctioneer

> [!IMPORTANT]
> Auctioneer is under heavy development and is not yet stable. We are not accepting outside contributions or pull requests at this time. This notice will be updated when the project is ready for external contributions.

[IAB Tech Lab repository](https://github.com/IABTechLab/auctioneer).
This is a fresh-history import. Historical plans and evidence are retained;
see [repository import](docs/repository-import.md) for their status.

An OpenRTB 2.6 auction engine written in Rust, usable as a standalone server
or a library for in-binary integration. Built on the
[EdgeZero](https://github.com/IABTechLab/edgezero) framework, its standalone
adapters target Axum (native), Cloudflare Workers, Fastly Compute@Edge,
and Spin (Fermyon). Target support is not deployment qualification.
Direct Trusted Server integration is a product goal;
see the [vision](vision.md) for the distinction from the existing library.

**Product vision**: [`vision.md`](vision.md)
**Architecture overview**: [`docs/architecture.md`](docs/architecture.md)
**Design spec**: [Master design](docs/superpowers/specs/2026-05-21-auctioneer-openrtb-server-design.md)
**Approved roadmap**: [`docs/roadmap.md`](docs/roadmap.md)
**Foundations plan**: [`docs/superpowers/plans/2026-05-25-foundations.md`](docs/superpowers/plans/2026-05-25-foundations.md)
**Upstream credit**: [Prebid and other acknowledgements](#acknowledgements)

## Workspace

| Crate                                                                           | Purpose                                                                                                                                                                        |
| ------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| [`crates/auctioneer-core`](crates/auctioneer-core/)                             | EdgeZero app, HTTP routes and handlers, host integration, and response serialization                                                                                         |
| [`crates/auctioneer-runtime`](crates/auctioneer-runtime/)                       | Host-environment seams: `Clock` + `Rng` traits with deterministic test impls (`FakeClock`, `SeededRng`), Fisher-Yates `shuffle`, Lemire `gen_range`                            |
| [`crates/auctioneer-openrtb`](crates/auctioneer-openrtb/)                       | OpenRTB 2.6 typed model + `prepare(req, headers)` — the sole construction gate for a `NormalizedBidRequest`                                                                    |
| [`crates/auctioneer-config`](crates/auctioneer-config/) | Shared configuration models, pure validation and optional native build support; no compiled deployment data |
| [`crates/auctioneer-config-data`](crates/auctioneer-config-data/) | Compiled product defaults, bidder metadata, disclosure tables and reference archives; build-time generation reuses the shared models |
| [`crates/auctioneer-adapter-axum`](crates/auctioneer-adapter-axum/)             | Axum (native) entry point                                                                                                                                                      |
| [`crates/auctioneer-adapter-cloudflare`](crates/auctioneer-adapter-cloudflare/) | Cloudflare Workers (wasm32-unknown-unknown) entry point                                                                                                                        |
| [`crates/auctioneer-adapter-fastly`](crates/auctioneer-adapter-fastly/)         | Fastly Compute@Edge (wasm32-wasip1) entry point                                                                                                                                |
| [`crates/auctioneer-adapter-spin`](crates/auctioneer-adapter-spin/)             | Spin / Fermyon (wasm32-wasip2) entry point                                                                                                                                     |

## Status

- **Foundations and auction core**: runtime primitives, typed OpenRTB preparation, generated configuration, and the auction, status, and bidder-info routes are implemented with unit and integration tests.
- **GDPR fanout**: TCF v2 parsing and legal-basis enforcement, GVL/GCL authenticity and freshness, GDPR scrubs, config overlays (bidder kill switches, `sample_rate`, aliases, and accounts), and golden fixtures are implemented.
- **Bounded fanout and privacy enforcement**: strict config resolution, GPP source normalization, independent CCPA/LMT/COPPA and activity decisions, bidder-local projection/scrubs, bounded outbound construction, and measured per-auction reservations are integrated. The [verification record](docs/superpowers/plans/2026-09-28-privacy-enforcement-verification.md) records native tests, conformance, linting, and WASM compilation checks, not a physical-OOM guarantee or hosted qualification.
- **Signed user sync**: `/cookie_sync`, `/setuid`, `/getuids`, and the preflight use the generated App; issued UID state requires a MAC envelope. [Recorded native and Chromium loopback checks](docs/superpowers/plans/2026-09-30-usersync-verification.md) do not establish third-party-cookie acceptance or hosted-provider readiness.
- **Bounded diagnostics**: permission-gated capture and sanitized projection into auction `ext.debug` are implemented. [Local diagnostic and qualification evidence](docs/testing/evidence/diagnostics-qualification-verification-20261015.md) retains explicit provider, resource, and publisher-readiness limitations.
- **Controlled-native bidder delivery**: source registration is generated, and all 14 priority selected flows, including Sharethrough, Rubicon, and APS, are implemented under the [approved native contract](docs/superpowers/specs/2026-10-30-controlled-native-bidders-completion-design.md). [Recorded local acceptance](docs/testing/evidence/controlled-native-bidders-20261030.md) is not full-catalog completion or complete worst-case resource certification. New priority publications are native-only and inactive by default; existing guest publications remain unchanged.

Complete resource certification is outstanding; affected guest production
readiness remains blocked. Hosted execution, Trusted Server embedding, and
live publisher acceptance remain separate work.
Production deployment uses [EdgeZero's production-deploy tooling](https://github.com/IABTechLab/edgezero/pull/381);
commands in `edgezero.toml` are dev/staging-only.

## Roadmap

The [approved roadmap](docs/roadmap.md) separates delivered diagnostic hardening
and controlled-native priority flows from planned integration and qualification.
The next priorities are direct Trusted Server embedding and a selected-bidder
publisher pilot, then demand-led bidder expansion. Stored requests/accounts,
currency/multibid/auction identity, creative caching, and full video delivery
follow. Other shared parity work covers first-party data, floors/adjustments,
AMP/native, privacy/transparency, compressed ingress, hooks/modules,
metrics/analytics, and browser privacy compatibility. Residual full-catalog
migration remains distinct from priority-flow delivery and bidder expansion.
These plans do not claim implementation or production readiness.

Diagnostic qualification is a local/staging checkpoint, not a live publisher
pilot. [Trusted Server embedding](docs/roadmap.md#7-trusted-server-embedding-capability)
owns in-binary integration; [publisher-pilot acceptance](docs/roadmap.md#8-limited-bidder-publisher-pilot-and-hosted-acceptance)
owns hosted execution and live limited-bidder rollout. Selected banner/display
flows need not wait for the full catalog or video, but publisher-specific
dependencies and deployment checks still apply; see the
[pilot requirements](docs/roadmap.md#6-publisher-pilot-checkpoint).

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

The `insecure-bidders` Cargo feature permits development HTTP/loopback
transport. Mocktioneer itself is available in normal builds:

```
cargo test --workspace --features insecure-bidders
```

Axum's default `cargo test -p auctioneer-adapter-axum --all-targets` also runs the
handler-to-socket privacy test. Its test-only dependency permits loopback
transport; the fixture explicitly enables and configures its bidder. The test sweeps
CCPA and all three disclosure activities independently.

## Auction privacy configuration

[`auctioneer.toml`](auctioneer.toml) is the direct runtime policy: root
`schema_version = 1`, `host`, `account_defaults`, `accounts`, `bidders`,
`features` and `floor_currency`. Existing EdgeZero typed validate/diff/push
commands consume that payload without `baked`, `startup` or `runtime` wrappers.
Neutral compiled defaults are product data, never operator input to a build.
CCPA and LMT enforcement default off; activity
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
See the [privacy enforcement design](docs/superpowers/specs/2026-09-26-privacy-enforcement-design.md)
for precedence, scrubs, and the remaining acceptance requirements.

`[host.resource_limits]` requests independent `max_outbound_raw_bytes`
and `max_outbound_request_bytes` caps for raw and transport-encoded bidder
bodies. Physical facts, bindings and installed capacity remain host-owned.
Both default to 256 KiB, accept 1 KiB through 16 MiB, and are clamped
to the installed boot envelope. An oversized bidder request is a bidder-local
code-5 refusal; exceeding an auction's prepaid resource allowance instead
returns fixed `503 overloaded\n`. The latter never changes consent selection.

## User sync operator setup

Provision `usersync_cookie_keyring` in the default EdgeZero secret store before
serving UID routes. See [key provisioning and rotation](docs/deployment/usersync-cookie-keys.md).
Unsigned/tampered cookies are rejected; unavailable keys return 503 without
cookie writes. A valid signature protects issued state, not ownership of the
public callback's supplied UID or an authorization/consent grant.

Neutral product defaults have no configured sync service. In
[`auctioneer.toml`](auctioneer.toml), set `host.user_sync.external_url`
to the public HTTPS origin and optional base path that actually reaches
this deployment, for example `https://sync.publisher.example/auctioneer`.
The default redirect template then builds callbacks under that base path.
Do not derive the callback from request `Host`, `Origin`, or forwarded
headers. Policy changes require a config push and the backend's existing
activation lifecycle; the native startup snapshot requires restart. Invalid effective sync config
is rejected rather than replaced by permissive defaults.
Unconfigured sync routes refuse before reading signing keys; auctions and info
do not require a sync service.

`host.host_cookie.cookie_name` and `family` identify a separate host cookie to
import; Auctioneer's writable UID cookie is always named `uids`. Configure an
optional cookie `domain`, positive `ttl_days` (default 90), and optional
opt-out cookie `name` and `value` together. `max_cookie_size_bytes = 0`
disables only the logical eviction threshold, not the hard Set-Cookie cap.
Account `cookie_sync` settings select default/cooperative limits and can
replace host priority groups, including with an empty list. A missing or
empty account selects product policy plus `account_defaults`; a supplied account
ID selects its named overlay when present. `bidders.<name>.usersync_enabled`
can disable sync independently of auction participation, but cannot create
a syncer absent from the baked bidder metadata.

Cross-origin JavaScript access is off by default. Add exact
scheme/host/port origins to `host.user_sync.allowed_origins`, without paths or
wildcards, and send browser requests with credentials when cookies are
needed. Allowed application responses use credentialed CORS and
`Vary: Origin`; `OPTIONS /cookie_sync` permits POST with `Content-Type`.
Pre-config/admission/framework failures cannot promise CORS headers.
Auctioneer-owned sync responses use `Cache-Control: no-store`. The `uids`
cookie uses `Path=/` and an expiry; `Domain` is optional. The current
Chrome/CriOS 67+ user-agent rule adds `Secure` and `SameSite=None`.
Browser acceptance, especially in third-party contexts, is not guaranteed
by a successful server response and needs real-browser validation.

| Sync surface                  | Current hard limit                                                                                                                                             |
| ----------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `POST /cookie_sync`           | 32 KiB body and 4,096 JSON nodes; compressed bodies rejected                                                                                                   |
| `GET /setuid`, `GET /getuids` | 32 KiB encoded query, at most 32 parameters; adapter request-target profiles allow 33 KiB, subject to provider limits                                          |
| Cookie decode                 | Current profiles admit 16 KiB of request headers; codec ceilings are 64 header fields, 32 KiB header/encoded value, 24 KiB decoded value, and 4,096 JSON nodes |
| Selection/config              | 256 requested or catalog names, 128 UTF-8 bytes per name, 32 priority groups, 256 priority entries                                                             |
| Output                        | 32 KiB per sync URL, 256 KiB response body, 4 KiB complete signed Set-Cookie field value                                                                       |

These are resource ceilings, not a promise that every legal browser or
provider request is admitted. User sync does not migrate the full upstream bidder
catalog and does not feed stored `buyeruid` values into auction bidder
requests; `/getuids` exposes the stored map to clients only. See the
[user sync design](docs/superpowers/specs/2026-09-30-usersync-design.md)
for endpoint ordering and intentional reference differences.

## Per-adapter dev commands

Routes are wired in `auctioneer-core`; the commands below are the Cargo/framework
entry points for the adapter shells.
Run them from the workspace root.

| Adapter             | Local dev                                                 |
| ------------------- | --------------------------------------------------------- |
| Axum                | `cargo run -p auctioneer-adapter-axum`                    |
| Cloudflare Workers  | `wrangler dev --cwd crates/auctioneer-adapter-cloudflare` |
| Fastly Compute@Edge | `fastly compute serve`                                    |
| Spin                | `spin up --from crates/auctioneer-adapter-spin`           |

### Configure a Mocktioneer destination

Mocktioneer is available in normal builds and is disabled by default. Enable
it and configure its HTTPS destination in your runtime policy:

```toml
[bidders.mocktioneer]
enabled = true
implementation = "mocktioneer"

[bidders.mocktioneer.settings]
endpoint = "https://mock-bidder.example.test/openrtb2/auction"
```

When settings are supplied, the bidder row requires `implementation`; the
separate `.settings` subconfig contains only that owner's fields. The key must
match the resolved bidder owner. Omit both fields to use inherited or compiled
defaults. Republish older wrapped configuration in this form after updating
the binary.

Use a literal absolute URL of at most 2048 UTF-8 bytes. Public destinations
require HTTPS; the development feature also allows HTTP to exact loopback
literals. Private/reserved addresses, localhost-alias services, credentials,
fragments and templates are rejected. Use ASCII host names (punycode for
internationalized names) and percent-encode characters the HTTP transport
cannot represent. Omitted/null endpoint values inherit the base bidder setting.
An enabled Mocktioneer requires a resolved endpoint; it has no compiled default.
An empty endpoint is invalid.
Impression parameters cannot override the selected destination.

Validate and push the complete policy to local Fastly/Viceroy from the workspace
root, then restart Viceroy to read the updated local config store:

```sh
auctioneer-cli config validate --app-config auctioneer.fastly-local.toml --no-env
auctioneer-cli config push --adapter fastly --local --app-config auctioneer.fastly-local.toml --no-env
```

HTTPS needs no extra Cargo features in either the CLI or the serving guest.
For an explicitly configured local endpoint such as
`http://127.0.0.1:8787/openrtb2/auction`, build the CLI with
`--features auctioneer-core/insecure-bidders` and the serving adapter with
`--features insecure-bidders`. The feature never supplies a destination.
If another configured implementation is missing from the build, CLI errors and runtime
operator logs identify its compiled name and required Cargo feature. These
diagnostics omit configured alias names, endpoint values and credentials.
Endpoint changes take effect through the existing config publication lifecycle;
they require no endpoint-specific rebuild. The other bidders' compiled endpoint
and host-allowlist rules remain unchanged.

## Operational endpoints (`/_auctioneer/*`)

`GET /_auctioneer/health/gvl` reports GVL archive health (archive age,
bundled vendor-list versions, and the rolling unavailable-lookup
counter). Configure access in the pushed operator policy:

```toml
[gvl.authenticated_health]
enabled = true
token_secret = "health_gvl_token"
```

The handler is fail-closed. `token_secret` names an entry in the declared
default Secret Store. Omit the section to keep health closed. To permit anonymous access, set `enabled = false`
and omit `token_secret`. The shared handler resolves config and the selected
secret on each health request; updates take effect with the provider's existing
config/secret visibility lifecycle. Axum's file-backed config keeps its startup
snapshot until restart. Missing or invalid policy, unavailable/empty secrets,
and secrets larger than 4096 bytes return 401 with a Bearer challenge. `/status`
remains independent. Ordinary routes do not fetch health credentials.

Provision `health_gvl_token` through the existing SDK secret binding:

- Axum: `EDGEZERO__SECRETS__HEALTH_GVL_TOKEN`; entry names accept ASCII letters,
  digits and underscores and are uppercased for this environment lookup.
- Cloudflare: `wrangler secret put health_gvl_token` uses the entry name literally.
- Fastly: the `health_gvl_token` item in the linked default secret-store alias.
  The local manifest's `AUCTIONEER__RUNTIME__HEALTH_GVL_TOKEN` is only a Viceroy
  provisioning source; the runtime does not read that environment variable.
- Spin: a secret application variable declared and mapped into the component.
  The SDK lowercases entry names; a different `token_secret` needs its own
  matching declaration and component mapping.

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

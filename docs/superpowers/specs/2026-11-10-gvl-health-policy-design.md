# GVL health policy in operator configuration

The operator owns GVL health authentication in `auctioneer.toml`:

```toml
[gvl.authenticated_health]
enabled = true
token_secret = "health_gvl_token"
```

`enabled` is required when this section is present. Authentication enabled
requires a nonempty secret entry name, at most 128 UTF-8 bytes, without ASCII
controls or leading/trailing whitespace. It is a reference in the declared
default Secret Store, never token bytes. Unknown fields and explicit nulls
are rejected. An omitted section stays closed. `enabled = false` explicitly
permits anonymous access and must omit `token_secret` to avoid conflicting
intent. No adapter-specific flag or environment alias remains authoritative.

The shared health handler uses the existing funded, bounded runtime-config
resolver. It drops the complete policy before fetching the selected secret
through the request's default secret binding. It accepts at most 4096 bytes,
within the existing auxiliary signing allowance, with the fixed config-fetch
deadline and a post-read deadline/cap check. Provider APIs may materialize the
whole value before enforcing this accepted-size limit; synchronous provider
deadline checks are cooperative. Empty, missing, excessive or
unavailable secrets deny access. Secret bytes are hashed for constant-time
comparison and never retained globally or exposed through diagnostics.
Missing/malformed config also denies access; `/status` remains the independent
liveness and config-fault diagnostic. HTTP denials retain 401 and the Bearer
challenge. Health replies carry `Cache-Control: no-store` across authorization
and freshness outcomes. Existing header parsing and health payload semantics remain.

Adapters install only their immutable counter scope: process, request, isolate
or instance. Each health request resolves policy and token through the provider's
existing visibility lifecycle (Axum's file config remains a startup snapshot);
ordinary auctions, status and bidder-info reads do not fetch health secrets.
The obsolete global boot health policy and provider flag readers are removed.
Local manifests retain only provider secret wiring, with no inline credentials;
Fastly's `auctioneer_config` entry stays empty until CLI push.

Axum's SDK maps `health_gvl_token` to `EDGEZERO__SECRETS__HEALTH_GVL_TOKEN`
and accepts ASCII alphanumeric/underscore entry names. Cloudflare uses the
entry name literally as a Worker secret. Spin lowercases the entry name and
requires a declared, mapped component variable. Fastly uses the entry name
in the captured default Secret Store. Provider naming restrictions still apply.

Validation covers strict TOML/JSON round trips, required/contradictory fields,
all authorization-header controls, runtime config updates and secret rotation,
renamed default bindings, missing/malformed config and secret faults, bounded
and cancelled reads, and independent status/auction availability. Adapter
contracts and a real Fastly/Viceroy request verify shared SDK binding behavior.
Existing resource ceilings and historical evidence are unchanged.

The pinned native build measures owner storage at 19,256 bytes per auction,
up from the separately retained 19,160-byte baseline. The 96-byte increase is
in the compiler-owned auction execution frame holding the expanded policy;
prepared state, failure and ledger layouts are unchanged. Active native
accounting controls add this measured delta to demand and subtract it from
headroom. They retain every workload, ceiling and historical capture. Guest
execution is checked separately; this native measurement is not a guest-size
or memory-fit qualification.

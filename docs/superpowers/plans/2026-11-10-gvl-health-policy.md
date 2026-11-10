# GVL health policy implementation plan

> **For agentic workers:** Use superpowers:executing-plans or superpowers:subagent-driven-development for these steps. Preserve the existing author worktree and serialize Cargo jobs.

**Goal:** Configure GVL health authentication and its secret reference through the shared runtime policy on every adapter.

**Architecture:** Preserve the existing `[host]` and account scopes. Add strict optional `host.gvl` configuration with flat `authenticate_health` and `authenticate_health_token_secret` leaves; accept no root GVL or nested authenticated-health alias. An omitted section or mode stays closed. True requires a nonempty valid reference; false permits an absent, empty or valid named reference and never reads it. Validate every nonempty reference, including with false; a supplied reference without a mode is invalid. Resolve and authorize only in the shared health handler through bounded SDK stores; adapters install counter scope only.

**Tech stack:** Rust, Serde, validator, EdgeZero typed extraction and bounded stores; existing native and guest test owners.

## 1. Shared model and handler

- [x] Add failing inline schema tests to `crates/auctioneer-config/src/policy.rs` for `[host.gvl]` with the two flat fields, strict null/unknown/type/mode rules and bounded secret references. Run `cargo test --locked --offline -j2 -p auctioneer-config --features config-model --lib policy::tests::gvl` and observe the absent-field failure.
- [x] Add optional GVL policy under `HostPolicy`, with optional `authenticate_health` and `authenticate_health_token_secret` leaves and nested host validation. Preserve omission in serialization and existing neutral config fixtures; reject root GVL aliases, true with empty references and references without a mode.
- [x] Replace boot authorization in `crates/auctioneer-core/src/handlers/health_gvl.rs` with existing runtime resolution and bounded default-secret reads. Retain 401/challenge and SP-only Bearer parsing. Keep secret loading out of SDK eager field metadata.
- [x] Replace global health-policy state in `crates/auctioneer-core/src/host_primitives/registry.rs` with `install_health_counter_scope`; retain constant-time `BearerToken` verification. Update exports and existing test owners.
- [x] Migrate the real HTTP authorization fixtures to `host.gvl`; preserve config/secret selection, dynamic rotation, denied faults and all authorization-header controls. Assert false with absent, empty and valid named references succeeds without secret reads. Adapt payload/observer/allocation setup without changing independent resource oracles.

## 2. Adapter migration

- [x] Replace boot policy readers in Fastly, Cloudflare, Spin and Axum (including baseline probe) with counter-scope installation. Remove obsolete provider flags and update their owning health/manifest tests to verify no provider policy authority and shared runtime behavior.
- [x] Migrate adapter fixtures to `host.gvl` while preserving all eleven Fastly guest scenarios, caller fixture controls, Axum environment child contracts and Cloudflare/Spin status-response assertions. Preserve SDK dispatch/logger/envelope order and store wiring. `fastly.toml` keeps `auctioneer_config = ''`; secrets remain env-backed. Cloudflare secret names and Spin variable mapping must match the selected default store entry after the SDK's lowercase conversion.
- [x] Run affected adapter tests and strict test Clippy, serialized with core jobs. Compile all changed guest targets and execute Fastly guest controls when the requisite artifacts are built.

## 3. Operator documentation and verification

- [x] Document `[host.gvl]` and its exact flat fields in `auctioneer.toml`, `README.md`, deployment guidance, architecture, active config inventory and docs tests. State false with absent/empty/valid named references never reads a secret, true with an empty reference is invalid, and omitted mode stays closed. Leave historical records intact.
- [x] Run config/core health suites, runtime-config failure/funding contracts, CLI config contracts, adapters, formatting and diff checks. Execute real Fastly/Viceroy health requests with synthetic typed config and a custom secret name; record unavailable checks honestly.
- [x] Review final spec compliance and implementation correctness independently, fix findings, and repeat only impacted checks.
- [x] Present the corrected wire shape and serialized verification evidence for review. Obtain any required authorization before remote writes, deployment or merge.

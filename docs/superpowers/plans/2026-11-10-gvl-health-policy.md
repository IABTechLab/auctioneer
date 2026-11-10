# GVL health policy implementation plan

> **For agentic workers:** Use superpowers:executing-plans or superpowers:subagent-driven-development for these steps. Preserve the existing author worktree and serialize Cargo jobs.

**Goal:** Configure GVL health authentication and its secret reference through the shared runtime policy on every adapter.

**Architecture:** Add strict optional `gvl.authenticated_health` configuration. Resolve and authorize only in the shared health handler through bounded SDK stores; adapters install counter scope only.

**Tech stack:** Rust, Serde, validator, EdgeZero typed extraction and bounded stores; existing native and guest test owners.

## 1. Shared model and handler

- [x] Add failing inline schema tests to `crates/auctioneer-config/src/policy.rs` for the documented nested shape, strict null/unknown/type/required rules and bounded secret references. Run `cargo test --locked --offline -j2 -p auctioneer-config --features config-model --lib policy::tests::gvl` and observe the absent-field failure.
- [x] Add the optional root GVL policy, with separately validated authenticated-health settings. Preserve omission in serialization and existing neutral config fixtures.
- [x] Replace boot authorization in `crates/auctioneer-core/src/handlers/health_gvl.rs` with existing runtime resolution and bounded default-secret reads. Retain 401/challenge and SP-only Bearer parsing. Keep secret loading out of SDK eager field metadata.
- [x] Replace global health-policy state in `crates/auctioneer-core/src/host_primitives/registry.rs` with `install_health_counter_scope`; retain constant-time `BearerToken` verification. Update exports and existing test owners.
- [x] Extend `crates/auctioneer-core/tests/health_gvl_auth.rs` for config/secret selection, dynamic rotation, denied faults, and unchanged header controls. Adapt payload/observer/allocation test setup to real typed config and funded boot.

## 2. Adapter migration

- [x] Replace boot policy readers in Fastly, Cloudflare, Spin and Axum (including baseline probe) with counter-scope installation. Remove obsolete provider flags and update their owning health/manifest tests to verify no provider policy authority and shared runtime behavior.
- [x] Preserve SDK dispatch/logger/envelope order and store wiring. `fastly.toml` keeps `auctioneer_config = ''`; secrets remain env-backed. Cloudflare secret names and Spin variable mapping must match the selected default store entry after the SDK's lowercase conversion.
- [x] Run affected adapter tests and strict test Clippy, serialized with core jobs. Compile all changed guest targets and execute Fastly guest controls when the requisite artifacts are built.

## 3. Operator documentation and verification

- [x] Add the secure reference section to `auctioneer.toml`; update `README.md`, `docs/deployment/restrict-operational-endpoints.md`, active config inventory and docs tests. Leave historical records intact.
- [x] Run config/core health suites, runtime-config failure/funding contracts, CLI config contracts, adapters, formatting and diff checks. Execute real Fastly/Viceroy health requests with synthetic typed config and a custom secret name; record unavailable checks honestly.
- [x] Review final spec compliance and implementation correctness independently, fix findings, and repeat only impacted checks.
- [x] Commit the focused change and push the already-authorized PR branch; update PR 160 description with accurate scope and evidence. Do not deploy or merge.

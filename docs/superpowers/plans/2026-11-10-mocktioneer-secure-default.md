# Mocktioneer secure defaults implementation plan

> **For agentic workers:** Execute the approved change inline, task by task,
> using test-driven development and verification-before-completion. Obtain an
> independent review before publishing the updated PR.

**Goal:** Use Mocktioneer with explicit HTTPS configuration in normal builds.

**Architecture:** Decouple adapter publication from the development transport
exception. Model optional compiled endpoints explicitly, enforce required runtime
settings in the owner, and preserve the existing enforcing sink.

**Tech stack:** Existing Rust workspace, generated TOML settings, CLI and Fastly.

**Contract:** [Approved design](../specs/2026-11-10-mocktioneer-secure-default-design.md).

## Task 1: Prove default publication and required configuration

- [ ] Add a metadata smoke regression asserting Mocktioneer exists without a
  feature, has no compiled endpoint, and is disabled by default. Run
  `cargo test --locked --offline -j2 -p auctioneer-config-data --test smoke`
  and observe failure against current metadata.
- [ ] Add owner controls for missing endpoints in enabled/disabled identities,
  requested/base inheritance and secure/development URL admission. Preserve
  independent invalid/shadowed controls.

## Task 2: Admit and enforce missing compiled endpoints

- [ ] Change raw/generated `BidderInfo.endpoint` to `Option<String>` in
  `auctioneer-config/src/{types.rs,codegen/mod.rs}`. Render existing values
  unchanged; derive HTTPS/macros only from present defaults.
- [ ] In `auctioneer-config-data/build.rs`, require a default-disabled factory
  with typed endpoint settings for omitted defaults; validate every present URL
  and reject unrelated omissions. Add build-harness positive/negative controls.
- [ ] Update metadata consumers and synthetic fixture constructors mechanically.
  Make `EndpointBuilder::new` refuse a missing default and keep exchange
  selection before credentials/body work. Do not add metadata clones or a new
  allocation across the credential await.

## Task 3: Publish Mocktioneer securely

- [ ] Remove its `required_feature` and compiled endpoint; set
  `defaultEnabled = false` in its manifest. Refresh only its manifest digest in
  the protocol fixture index.
- [ ] Validate that enabled identities resolve an endpoint in owner config,
  and reject missing endpoints in direct selection. Preserve URL checks on all
  supplied values, including disabled/shadowed values.
- [ ] Add a third, configured-endpoint serving cohort to the publication census
  and same-App acceptance (default off, explicit disable, missing refusal, HTTPS
  dispatch), preserving the frozen 14 priority flows and all resource limits.
- [ ] Exercise the shared missing-default guard with a capable fake, both with
  and without a configured endpoint, before credential/body/construction work.
- [ ] Update default/development tests and native qualification configuration to
  explicitly activate and configure the selected bidder. Strip those new fields
  from historical-binary documents and assert the original row independently.
  Keep fixture payloads
  and independent error/allocator expectations unchanged.

## Task 4: Verify and publish

- [ ] Update README/config examples and supersession notes in the prior design.
- [ ] Run owner/config/core/CLI/endpoint contracts in both feature lanes, then
  compiler resource tests and strict affected lint. Use retained Cargo caches
  and serialize heavy jobs. Run formatting and fixture/source-isolation gates.
- [ ] Build default and development Fastly guests; use synthetic policy and
  isolated Viceroy configuration to verify valid HTTPS admission, missing
  endpoint failure and development loopback behavior. Do not deploy.
- [ ] Request independent review, fix findings, and commit only intended files.
  Update existing PR #160 and inspect actual CI separately from local checks.

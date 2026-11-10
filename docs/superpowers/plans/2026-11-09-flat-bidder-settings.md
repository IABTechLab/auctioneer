# Flat bidder settings implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Accept and emit bidder-owned fields directly under `.settings` for all implementations while retaining operator binding, validation and resource contracts.

**Architecture:** One generated opaque settings object owns a bounded vector of closed scalar fields. Generated typed views borrow it; admission resolves the operator owner, and request projection pins that same owner independently of request aliases. The user-approved flat layout replaces the pre-release wrapped authoring format.

**Tech Stack:** Existing Rust workspace, Serde, quote/syn code generation, existing config/owner/exchange test owners; no new dependencies.

**Design:** `docs/superpowers/specs/2026-11-09-flat-bidder-settings-design.md`.

## Task 1: Generate flat storage and borrowed owner views

**Files:**
- Modify `crates/auctioneer-config/src/bidder_settings_codegen.rs`.
- Modify `crates/auctioneer-bidder/src/bidder_settings.rs` only if reusable scalar decoding belongs outside emitted code.
- Adapt settings consumers/tests under `crates/auctioneer-bidder/src/bidders/{msft,thetradedesk,rubicon,mocktioneer}/`.
- Adapt `crates/auctioneer-bidder/tests/bidder_settings_compile/{mod.rs,bidder_settings_semantics.rs}` and relevant compiler fixtures through quote/syn.

- [ ] Add failing tests for flat JSON/TOML objects, canonical flat output, explicit null preservation, duplicate/unknown fields and independent scalar type/refusal boundaries. Confirm the old decoder rejects the requested flat shape.
- [ ] Generate a private owned field vector, closed key enum and scalar string/i64/null variants. Decode only objects, reject duplicates/coercions and retain explicit null keys. Stable output follows generated key order. Keep both scalar strings and keys bounded by existing settings/config admission.
- [ ] Generate these shared APIs (fixed diagnostics on failure):

```rust
impl BidderSettings {
    pub fn matches_implementation(&self, key: &str) -> bool;
    pub fn matching_implementations(&self) -> impl Iterator<Item = &'static str> + '_;
    pub fn implementation_key(&self) -> Option<&'static str>; // pinned owner only
    pub fn is_empty(&self) -> bool; // zero keys, not all-null
    pub fn is_unbound_empty(&self) -> bool;
    pub fn bind_implementation(&mut self, key: &'static str) -> Result<(), &'static str>;
    pub fn as_mocktioneer(&self) -> Option<mocktioneer::Settings<'_>>;
    // Other owner accessors are discovered, not handwritten.
}
```

`matches_implementation` requires a declared schema, compatible supplied keys,
exact scalar kinds/secret reference rules and a compatible internal pin. A failed
rebind must leave an explicitly invalid object whose matches/accessors all refuse
and whose serialization fails; it must not silently clear or replace the old pin.
`is_unbound_empty` excludes this invalid state. Internal pins never serialize.

- [ ] Generate each owner `Settings<'a>` as a `Clone + Copy` thin borrow with typed getters; string getters return `Option<&'a str>`, integer getters `Option<i64>`. Generate `resolve(requested: Option<Settings<'a>>, base: Option<Settings<'a>>) -> SettingsRef<'a>` with unchanged per-field fallback. No temporary references or string copies.
- [ ] Update owner-local consumers to call getters and store views by value. Preserve semantic validators and configured endpoint checks; update inline fixtures to flat fields without changing protocol captures or manifest provenance.
- [ ] Verify generator units and owner units/compile controls. Cover empty catalogs, integer-only/string owners, shared names with different scalar kinds, explicit null, and a newly declared synthetic owner. Keep fake ownership and exact dispatch assertions.

## Task 2: Preserve admission and request-time implementation binding

**Files:**
- Modify `crates/auctioneer-core/src/config/validation.rs`.
- Modify `crates/auctioneer-core/src/config/bidder_settings.rs`.
- Modify `crates/auctioneer-core/src/auction/prepare.rs`.
- Modify `crates/auctioneer-bidder/src/request_info.rs`.
- Extend `crates/auctioneer-bidder/tests/support/generated_settings_contract.rs` and its existing compiler harness in `tests/generated_settings.rs`.
- Adapt other existing fixtures only where the owner API/wire format changed.

- [ ] Write synthetic regression cases for two owners sharing `token`, host aliases `x -> alpha` and `alpha -> beta`, settings on alpha, and request aliases retargeting a configured name. Confirm binding failures precede all callbacks, credentials and body closures, without failing valid siblings or mutating source input.
- [ ] Keep the first admission pass over the complete identity union. For each requested and fallback settings row, independently derive its operator owner from effective host aliases, compiled definitions, or a unique compatible linked settings-owning implementation for an unknown request-only identity. Compare that row owner against the context's expected owner before callbacks.
- [ ] Empty unknown request-only objects may remain unbound. Known settings-free owners reject any supplied settings, including `{}`. Unknown nonempty no-match or ambiguous schema candidates fail with the existing fixed binding category. Explicit null keys participate in matching and survive round trips.
- [ ] After the entire binding/schema pass succeeds, invoke existing semantic callbacks with borrowed typed views. Preserve intrinsic versus effective requiredness and disabled/shadowed validation.
- [ ] Extend projection with trusted operator identity inputs:

```rust
pub fn project(
    policy: &AuctioneerAppConfig,
    requested: &[BidderName],
    aliases: &AliasMap,
    operator_aliases: &BTreeMap<String, String>,
    bindings: &[BidderImplementationBinding],
) -> Self;
```

The request alias map selects retained rows only. For each selected row derive
its pin from its independent operator context, never from a request alias target.
Resolve the effective host map before this call and pass `services.bidder_bindings`.
Only copies already required by existing projection ownership are permitted.
Retain a failed bind as a refusing/invalid settings object, never omit it.

- [ ] Update `BidderRequestInfo::set_settings` to fallibly check both settings objects' owner pins, schema, kinds, references and limits before exposing either view. Clear previous bindings before any failure as currently required. Direct trusted callers can bind an unbound compatible object through the explicit implementation argument; an existing pin is immutable.
- [ ] Update generated contract fixtures through their ordinary owner modules and quote/syn wiring. Maintain independent expected values and workload sizes. Record actual executed dispatch counts; adjust reconciliation only for intentionally added test names, never to accept zero dispatch.

## Task 3: Update the operator contract and verify the full affected boundary

**Files:**
- Modify active examples in `README.md`, `auctioneer.toml`, `docs/bidders/controlled-native.md` and current deployment guidance if it contains the removed wrapper.
- Modify active CLI/config/integration fixtures that use tagged settings.
- Update the Mocktioneer design/plan with a dated superseding-format note and link; do not rewrite historical approvals, evidence or external captures.
- Preserve root-local operator configuration; update only the task-created local Mocktioneer config to the requested flat form when validation supports it.

- [ ] Exercise the exact requested TOML shape and equivalent flat JSON for each existing implementation. Parse/push with the actual CLI; emitted config contains neither removed wrapper field. Check hostile Mocktioneer URLs and invalid shadowed/base values still refuse with redacted errors.
- [ ] Run focused generator and owner tests, both bidder/exchange feature lanes and public endpoint/settings contracts. Run the explicit generated-settings compiler owner, its allocation variants and config peak/lifecycle/SDK extraction controls from `docs/testing/development.md`.
- [ ] Run both Axum envelope lanes, both CLI `bidders_resources` lanes and exact native construction charge owners. No ceilings, caps, workload reductions, frozen total edits or unreviewed evidence recaptures.
- [ ] Run strict affected library/test Clippy, Fastly WASIp1 guest build/lint and actual guest settings/publication controls as applicable. Serialize heavy Cargo work and retain the existing target cache.
- [ ] Run format, source-import isolation, raw JSON fixture gate and diff checks. Read independent spec/implementation review findings and fix concrete issues.
- [ ] Commit and update the existing requested PR with an imperative title and description describing the final config behavior, reviewed design and actual verification. No deployment, merge or public review submission. Inspect the new CI run separately and report pending/blocked checks honestly.

## Review and authorization

The maintainer explicitly selected the flat `.settings` contract for every
bidder in the current task. Implementation choices preserve the existing
operator/request trust boundaries. The independent spec review's findings are
incorporated above; implementation and final review must check the concrete
alias, null, type and memory cases, not merely the example syntax.

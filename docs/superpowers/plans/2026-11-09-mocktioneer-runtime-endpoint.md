# Mocktioneer runtime endpoint implementation plan

November 9 format update: the [flat settings plan](2026-11-09-flat-bidder-settings.md)
replaces the earlier operator wrapper syntax consistently for every bidder.
This plan retains the original endpoint behavior and verification record.

> Follow test-driven development and the repository's verification ownership.
> The user approved the runtime endpoint contract in the November 9 conversation.

**Goal:** Select Mocktioneer's destination through typed runtime settings while
retaining its compiled default and enforcing the selected authority in the sink.

**Architecture:** Add a narrow portable literal-URL validator, bidder-owned typed
settings/resolution and a defaulted borrowed endpoint hook. The exchange binds
one trusted selection to both request construction and outbound validation.

**Contract:** [Design](../specs/2026-11-09-mocktioneer-runtime-endpoint-design.md).
Related to #124; no change to another bidder's endpoint authority.

## 1. Portable URL validation

Files: `crates/auctioneer-config/src/url_safety.rs` and `src/lib.rs`.

Write inline failing controls for absolute spelling, public HTTPS with independent
path/query/port values, exact loopback under both feature-policy inputs, private
and alternate loopback literals, DNS aliases, userinfo, fragments, templates,
whitespace, controls, backslashes and the 2048-byte boundary. Reuse existing
private-address and literal-loopback helpers through a narrow exported validator.
Keep fixed error categories and serving portability.

Run the owner module's inline tests and strict selected-target lint. Confirm
the new boundary tests execute and that the production API uses the tested helper.

## 2. Bind trusted endpoint policy in the exchange

Files: `crates/auctioneer-bidder/src/bidder.rs`,
`crates/auctioneer-exchange/src/run/mod.rs` and `src/validate_endpoint.rs`.

Add failing inline controls for the checked endpoint value and hook's no-override
default, and a shared-fake integration in the existing `run_auction` test target.
Prove exact selected authority
acceptance and rejection of another hostname, scheme and port before body writes.
Prove a forged bidder request-info object cannot change the captured authority.

Implement a defaulted borrowed hook receiving only requested/base settings. Keep
existing settings binding before selection, fixed failure mapping and the default
metadata policy. Bind selected literals to construction without macros/allowlists
and capture the same selection in the enforcing validator. Avoid metadata clones
and preserve existing public endpoint validation semantics.

Run affected bidder units, exchange endpoint units/public contract tests and the
owner's request-construction allocation tests. Keep shared controls synthetic.

## 3. Mocktioneer settings and request selection

Files: `crates/auctioneer-bidder/src/bidders/mocktioneer/bidder.toml`,
`mod.rs`, new `config.rs`, existing inline `request.rs` tests, and
`fixtures/factory.json` digest metadata only.

Add failing owner-local controls for generated endpoint settings, requested/base
fallback, omitted/null/empty settings, independent invalid supplied values under
disabled/shadowed combinations, sanitized refusals and request-parameter override
attempts. Add the optional-string declaration and pure validator callback using
existing conventions. Override the borrowed endpoint hook for Mocktioneer and
retain the exact compiled loopback endpoint and feature gate.

Update the fixture's manifest digest solely to bind the reviewed declaration.
Keep protocol fixture payloads unchanged. Run Mocktioneer unit/conformance tests,
manifest pins, feature-enabled source/metadata smoke checks and secure-default
publication controls.

## 4. CLI and Fastly verification and documentation

Files: `auctioneer.toml` (commented bidder-owned example), `README.md` (brief usage
and development feature scope), and this design/plan.

Verify real typed CLI validate and local push dry-run with a fictional HTTPS
endpoint and the Mocktioneer-enabled feature graph. Exercise the same request
through local Viceroy when its runner/backend prerequisites are available.
Compile the actual Fastly WASIp1 guest with `insecure-bidders`; report compilation,
guest execution and hosted qualification separately. Do not contact a hosted
bidder or push live configuration as a verification substitute.

Run formatting, affected native/guest lint and relevant source/layout/allocation
owners using serialized heavy Cargo jobs and retained caches. Record exact
commands, counts, failures and unavailable prerequisites in the PR description.

## 5. Review and publication

Request independent spec/implementation review, resolve material findings, then
commit only this branch's intended files. Open the user-requested focused PR
against `IABTechLab/auctioneer:main` with the repository template and actual
verification evidence. Preserve the original worktree's untracked local policy
and planning files. Do not merge or change issue/project status.

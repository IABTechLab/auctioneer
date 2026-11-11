# Controlled Native Priority Bidders

This delivery uses the [approved native boundary](../superpowers/specs/2026-10-30-controlled-native-bidders-completion-design.md):
tested reservations and enforced schema/input/work limits, not complete
worst-case resource certification. It does not qualify guest deployments or
claim live bidder authorization. Complete proofs belong to resource qualification and hosted
publisher acceptance to publisher pilot.

## Enablement

New priority definitions are compiled into the native application but inactive
by default. Change runtime configuration and push it through the existing
configuration path; no application rebuild or registry reconstruction is needed.
The existing published priority definition keeps its previous default behavior.
Guest publication excludes the new native-only definitions.

For example, the direct `auctioneer.toml` payload contains a required root
version and bidder sections, not a `runtime` wrapper:

```toml
schema_version = 1

[bidders.msft]
enabled = true
implementation = "msft"

[bidders.msft.settings]
hb_source = 5
hb_source_video = 6

[bidders.thetradedesk]
enabled = true
implementation = "thetradedesk"

[bidders.thetradedesk.settings]
supply_id = "operator-issued-supply-id"

[bidders.ttd]
enabled = true

[bidders.rubicon]
enabled = true
implementation = "rubicon"

[bidders.rubicon.settings]
region = "us-east"
credentials_secret = "operator-managed-credential-bundle"
```

`ttd` retains compiled ancestry to `thetradedesk`; both must be enabled.
Disabling the canonical base also disables its compiled aliases. When settings
are supplied, the bidder row requires `implementation`; the separate `.settings`
subconfig contains only that owner's fields. The key must match the resolved
bidder owner. Omit both fields to use inherited or compiled defaults. Rubicon
requires one of `us-east`, `us-west`, `eu`, or `apac` and an atomic secret-store
credential bundle; missing settings or credentials refuse that bidder, not valid
siblings.
Never place credential values in runtime configuration. See the existing
[publisher pilot handoff](../deployment/publisher-pilot.md) and
[cookie-key provisioning](../deployment/usersync-cookie-keys.md).

The frozen flow set is `msft`, `ttd`, `rise`, `sovrn`, `yahooAds`, `unruly`,
`sharethrough`, `openx`, `kargo`, `rubicon`, `pubmatic`, `triplelift`, `criteo`,
and `aps`. Endpoint, capabilities, parameter schema, sync and provenance remain
inside each implementation's bidder directory. Source references are not live
authorization or a guarantee that an operator has a corresponding account.

## User-Sync Availability

Sync eligibility comes from each published definition's declared templates,
not membership in the 14-flow set. APS has no `userSync` declaration in its
pinned source metadata, so Auctioneer publishes no APS sync URL and
`/cookie_sync` emits no APS sync operation. Enabling the bidder or its runtime
sync switch does not invent a template. Adding one requires a reviewed source
contract, not a fallback URL.

Generated-App acceptance round-trips every applicable declared template through
the signed-cookie path and asserts an empty selection for definitions without
templates. Fourteen auction flows therefore does not mean fourteen sync-capable
definitions.

## Admission And Verification

Final privacy-surviving parameter objects are validated before secret reads or
outbound construction. Invalid impressions are removed locally. Shared work
tokens are charged before evaluation, never refunded or renewed for another
alias, impression or bidder. Missing installed schema funding, initialization
failure, expired deadlines and exhausted work refuse explicitly. The original
auction clock remains authoritative.

Native boot reserves 16 MiB residency, 8 MiB initialization and 512 KiB sequential
schema scratch per admitted auction using the shared additive equation. Existing
context charges remain separate. Activation also requires the installed demand
and the shared native headroom to fit the configured ceiling. Legacy warn-and-boot
does not grant native schema admission to an over-ceiling context.

Focused acceptance commands, from the active worktree:

```sh
cargo test --locked -p auctioneer-bidder --test native_schema_resources
cargo test --locked -p auctioneer-core --features conformance-suite --test native_bidder_onboarding
cargo test --locked -p auctioneer-core --features conformance-suite,insecure-bidders --test native_bidder_onboarding
cargo test --locked -p auctioneer-adapter-axum --lib envelope::tests::
```

The resource target owns cold initialization and allocator state. The onboarding
target owns its generated App and boot state and uses portable test reservations;
the Axum profile target independently checks the actual shipped native envelope.
Source/protocol, secure-default, feature and guest nonactivation/build checks
remain required. Passing this focused set alone is not a full CI or provider
readiness claim.

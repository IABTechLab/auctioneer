# Disclosure Inventory Sources

These artifacts support the proposed classified scrub. They are build-time
evidence, not a second runtime privacy implementation.

The standard catalogue is generated from the exact IAB source bytes in
`../openrtb-2.6-source.md`; the source manifest carries its explicit reviewer,
revision and digest. Regeneration uses the host-only importer:

```sh
cargo run --locked -p auctioneer-cli --no-default-features -- openrtb catalogue check --manifest crates/auctioneer-config/data/openrtb-2.6-source.toml --source crates/auctioneer-config/data/openrtb-2.6-source.md --catalogue crates/auctioneer-config/data/openrtb-2.6-catalogue.toml
```

`user-extension.source` and `device-extension.source` retain exact, unmodified
extension declarations from the attributed immutable references recorded in
`../supported-extension-catalogue.toml`. Their Apache-2.0 license is retained in
`LICENSE-extensions`. Only the seven explicitly catalogued extension fields are
supported here, not every field in either source artifact. Extension EIDs are
not standard User membership.

`ownership-classification.md` is an exact line-range excerpt from the pinned
Auctioneer scrub design. It records Auctioneer's user-FPD delegation and exact TID
ownership, not an IAB standard or an externally prescribed policy. In particular,
`UserExt.data` has JSON shape because it is the explicitly opaque FPD branch,
not an object-valued context exemption. FPD denial removes the complete branch.

AG supplied the human reviewer name on 2026-10-11. Review metadata is committed
explicitly; neither importer generation nor clock changes renew it. The generated
standard membership has no independent copy of review or classification data.

The policy has one disposition for each of 79 standard members and seven
supported extension members, across 11 protected schemas. Child and element
shapes are explicit, including device controls, sparse structured UA, Geo context,
and interstitial constraints. Domains are intentionally absent from policy TOML:
the compiler derives them from actions, transforms, delegates, children and the
protected unknown-field rule.

The live cutover remains governed by the scrub implementation plan. Adding these
inputs alone does not relax the current conservative scrub or qualify resource,
audit, depth or provider guarantees.

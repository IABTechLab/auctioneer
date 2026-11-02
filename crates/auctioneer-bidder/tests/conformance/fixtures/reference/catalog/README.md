# Catalog Source Reference

This unpromoted reference capture is shared by CLI inventory/import checks and
bidder schema qualification. [Bidder-owned captures](../../../../../src/bidders/)
retain the original bytes and [source license](../../../../../src/bidders/LICENSE.reference);
`source-inventory.toml` and `source-bindings.toml` retain their source witnesses.
`supplemental-schemas.toml` independently freezes the supplemental schema pins
outside that original inventory. Qualification verifies its checksum before
accepting an owner's local exclusion; coordinated local provenance edits cannot
authorize a different source. It is reference evidence, not a second effective
runtime catalog.

These fixtures do not establish implemented bidders or a qualified schema
candidate. Production metadata and schema ownership belongs beside each implementation in
`crates/auctioneer-bidder/src/bidders/`, not here.
[Bidder documentation](../../../../../../../docs/bidders/) contains the review and qualification reports.

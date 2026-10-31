# GVL archive provenance

`gvl_archive/` is a snapshot of the IAB TCF Global Vendor List corpus
plus the Global CMP List, maintained by
`cargo run --locked -p auctioneer-cli --no-default-features -- gvl refresh`
and validated on every build by `auctioneer-config/build.rs` against
`gvl_archive/manifest.json` (SHA-256 pins, schema validation, freshness
gates).

## Sources

- Spec-3 current list: `https://vendor-list.consensu.org/v3/vendor-list.json`
- Spec-3 archives: `https://vendor-list.consensu.org/v3/archives/vendor-list-v<N>.json`
- Spec-2 frozen list (final TCF v2.2 GVL, the only spec-2 version IAB
  will ever publish): `https://vendor-list.consensu.org/v2/vendor-list.json`
- Global CMP List: `https://cmplist.consensu.org/v2/cmp-list.json`

## Snapshot

- Snapshot date: see `manifest.json` `generated_at` — the manifest is
  the authority; this README does not repeat it (a hardcoded copy here
  went stale within a week of the weekly GCL refresh cadence).
- Contents: spec-2 v224 (`lastUpdated` 2023-11-16T16:05:29Z, its exact
  upstream metadata) plus the spec-3 window the manifest lists,
  retained newest-first under
  the 24 MiB raw cap minus a 4 MiB growth margin, at most 180 days back;
  `gcl.json` with its upstream `lastUpdated` and the RFC 9111
  Age/Date-corrected cache lifetime recorded in the manifest.
- Checksums: per-file SHA-256 pins live in `manifest.json` and are
  re-verified by `build.rs` on every build; an auditor reproduces the
  snapshot by re-fetching the recorded versions and comparing digests.

## License / provenance

The IAB Tech Lab publishes the Global Vendor List and Global CMP List
under the Tech Lab's terms. auctioneer ships them as a snapshot at the
recorded `lastUpdated` instants. Downstream deployers are responsible
for re-snapshotting
(`cargo run --locked -p auctioneer-cli --no-default-features -- gvl refresh`) when they adopt
more current policy; the build's manifest-age and GCL-freshness gates
enforce that a deployment cannot silently serve a stale snapshot.

# Repaired Rust Native Capture

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

Source: `d68ba170f0b25b40fe4890d9d56bdd9d5048251c`. The exact profile was
committed and pushed as `7a1b246` before traffic. Its SHA-256 is
`6bb2a0a158f097da31957b6122218b43aa868962a7c95a743369ddc11abd243c`.
The actual release application and repaired Rust CLI ran on October 15, 2026:

```sh
target/release/auctioneer-cli qualify native \
  --profile docs/testing/fixtures/resource-qualification/native-axum-rust-d68ba17.json \
  --output /private/tmp/auctioneer-rust-native-d68ba17-run \
  --source-commit d68ba170f0b25b40fe4890d9d56bdd9d5048251c --no-build
target/release/auctioneer-cli qualify frozen-probe \
  --evidence /private/tmp/auctioneer-rust-native-d68ba17-run
```

## Observed Results

All 45 declared rows passed local acceptance. All 45 runner attestations are
present, failures and report reasons are empty, and three configuration variants
have four actual A/B/corrupt/restored process observations. Ordinary dispatch,
encoded/decoded response work, debug and fourteen GVL parse credits are retained
separately from admission witnesses. Maximum service process-lifetime high-water
was 37,765,120 bytes; maximum sampled retained growth was 15,826,944 bytes.
Accounting remains 2,533,631,672 bytes against 4 GiB, with 256 MiB required
headroom. No criterion, population, SDK setting or debug bound changed from
the first Rust profile; only source/hardware/hash provenance was excluded in
the exact structural comparison.

The separate frozen control exited 0: fourteen distinct archived versions and
parses, three invalid-consent controls with zero credit, sixteen ordinary and
sixteen recovery bids, 32 fixture calls, and unchanged captured evidence.
`frozen-probe.json` records this control, not an additional resource measurement.
Missing consent requires warning-free denial; malformed/core-only consent
requires refusal. Parse credit is bound to the exact supplied proof.

## Limits

All six SDK capability attestations remain pending. Overall qualification is
unsupported and publisher readiness is false; the measured command's exit 1
is the specified completed-nonqualification outcome. No production qualification,
minimum safe memory size, hot config reload, per-window high-water or tight
growth-regression guarantee is established. The existing 3-GiB peak ceiling is
a broad envelope, roughly 85 times this observed high-water.

This is a shared local host, not a dedicated benchmark machine. An unrelated
project's Docker integration test was running near the start; no competing
compiler was observed at preflight or the later process check. No Auctioneer build
or source edit ran during measured or frozen-control traffic. The exact release
copies and serving snapshots stayed isolated and digest-bound.

`SHA256SUMS` covers the retained safe JSON files. Executables remain only in
the private local evidence directory; their digests are retained in metadata.
Raw HTTP/config/consent bodies, logs, credentials and temporary stores are not
committed. The [first Rust capture and failed separate control](../native-axum-rust-aefcf47-local-pass/README.md)
and all earlier historical attempts remain unchanged. No live provider test
was run; selected-target hosted acceptance and rollout remain publisher pilot work.

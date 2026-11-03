# Corrected Native Local Acceptance

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

Executed 2026-10-15 against the committed
[predeclaration](../../fixtures/resource-qualification/native-axum-local-e0a540f.json),
published before measured traffic in `ed27119`. All 45 local rows pass with zero
runner failures. This is not production qualification, hosted-provider evidence
or publisher readiness.

- Serving/harness source: e0a540fa75bed6329480c6afda2021087efcbe90.
- Frozen release artifact: 8c06dd1bd249c533961dab62b5bc046b8e492d114b0287be7596dab6508801b3.
- SDK: 65312643f4fd969bb1651e3f7dbdabd1479b2e74.
- Profile SHA-256: 81274a2310c1c19182c164039438f54a493bfcfec04197279654da9a4d72e4fe.
- Six families, 24 subcases, 45 observations, three configuration variants and
  four A/B/corrupt/restored process observations.
- Accounted envelope: 2,533,631,672 bytes plus 268,435,456 bytes required
  headroom within the unchanged 4 GiB budget.
- Maximum service process-lifetime high-water: 38,748,160 bytes; maximum
  retained growth: 14,647,296 bytes. These are service-only observations, not
  window-specific peaks or provider-managed memory bounds.
- Local acceptance is `pass`; overall qualification is `unsupported`, publisher
  readiness is false, and the harness exits 1. The local artifact links
  `insecure_bidders`; all six required SDK capability attestations remain pending.

The strict harness reconciles ordinary successful request IDs with actual fixture
dispatch and decoded bids. Earlier admission witnesses cannot supply ordinary
call/byte/node/decode geometry. Both repetitions of `stalled_io` and `timeout`
reach the declared sixteen-call ordinary maximum before final-wave stalling;
successful ordinary auctions independently require all sixteen calls. Timed-out
or cancelled auctions are not claimed to complete every queued call. GVL cases
separately park a real ordinary auction across each
credited lookup, with one ordinary worker plus one lookup worker. All fourteen
archived-version lookups are observed; no third auction is offered in that phase.

```sh
node scripts/qualify-resources.mjs --profile docs/testing/fixtures/resource-qualification/native-axum-local-e0a540f.json --output /private/tmp/auctioneer-diagnostic-qualification-q2-measured-e0a540f --source-commit e0a540fa75bed6329480c6afda2021087efcbe90 --no-build
```

This run used no competing build or test workload. Resources, hardware, variants,
all 45 scheduled rows and every acceptance criterion are identical to the failed
35ef524 declaration; only source/harness identity changed. Its failed capture and
all earlier captures remain unchanged. The final-wave fixture correction is
described in the [runbook](../../resource-qualification.md).

Safe files below are byte-identical to the private capture. They contain bounded
aggregates, enums and digests, not traffic/debug/configuration bodies, credentials
or raw process logs. Frozen executables remain outside the repository. publisher pilot owns
actual deployment Q2-D, credentialed provider execution and limited publisher
rollout/rollback after Trusted Server embedding and selected bidder/host setup.

| File | SHA-256 |
| --- | --- |
| report.json | 096072cfe18bc65f9e36290bc11eb88dda97b19ce771ad517aefc9145833d63f |
| observations.json | b754baec7a062289a16db2b5bce71777c10e95854cd4bf21e991dd4d06de1607 |
| runner.json | 85c8df73631029489c69e720127413b3421062de1d6fd51b9f64b9f29a1265be |

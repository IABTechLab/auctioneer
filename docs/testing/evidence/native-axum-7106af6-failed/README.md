# Failed Native Qualification Attempt

Executed on 2026-10-15 against the committed
[predeclared profile](../../fixtures/resource-qualification/native-axum-local-7106af6.json).
This attempt is failed evidence, not acceptance or a publisher-readiness claim.

- Serving source: 7106af605bd0cb439737373526da996a6622a585.
- Release artifact: da097938ca5e605ffc756f4c145f764b964c4d71923ba0ccc6e4224eb4482e82.
- SDK: 65312643f4fd969bb1651e3f7dbdabd1479b2e74.
- Profile SHA-256: b6026e8f74ba274912d6ff1ae945aba4e6b7f27c73539e23bc3a0cd0ac805283.
- Accounting: 2,355,369,656 bytes, plus 268,435,456 required headroom, fits 4 GiB.
- Result: local acceptance fail; profile nonqualified; publisher pilot not ready.
- Only 30 of 45 planned observation rows were retained. Maximum/FPD/debug
  witnesses and the second configuration variant failed. GVL evidence was absent;
  configuration recovery and overload outcomes also failed their criteria.
- Maximum observed process-lifetime high-water among retained rows: 22,315,008
  bytes. This does not establish a peak for missing work or complete the matrix.

Invocation:

```sh
node scripts/qualify-resources.mjs --profile docs/testing/fixtures/resource-qualification/native-axum-local-7106af6.json --output /private/tmp/auctioneer-m6-q2-measured-7106af6 --source-commit 7106af605bd0cb439737373526da996a6622a585 --no-build
```

Exit status was 1. Thresholds were not adjusted after execution. Diagnosis is
pending; any corrected harness/candidate needs a new committed identity and run.
These files contain only bounded safe metrics, enums and digests, not traffic,
debug bodies, configuration snapshots, credentials or child logs. Release copies
remain outside the repository in the private execution directory.

| File | SHA-256 |
| --- | --- |
| report.json | cf06ece2b8fd0dd1cf2662f3302adf5c8d0041331d4ec1aad4ccc10eeb51b196 |
| observations.json | 37da5aedde679d39eea6b34675353a7e62c20156ae4637555b53403ad5f29859 |
| runner.json | c782b6fe8dd3a7de6d8e35dfc37f2bd17775fc05c2923f6201750fdb24322390 |

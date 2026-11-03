# Second Failed Native Qualification Attempt

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

Executed on 2026-10-15 against the committed
[predeclared profile](../../fixtures/resource-qualification/native-axum-local-4bcc666.json).
This attempt remains failed evidence, not acceptance or publisher readiness.

- Serving source: 4bcc6663f746e4cf1a21241dfe1c40ebf713c446.
- Release artifact: d6e399f9801a2648fb50c0220baa92f3a7270a916010592e9d7ac8999b94aeed.
- SDK: 65312643f4fd969bb1651e3f7dbdabd1479b2e74.
- Profile SHA-256: cfff4e9c96d3ee1ee507fd9e7220efe2210efa1dc8793f4623568c93c9a9500d.
- Accounted total: 2,533,631,672 bytes plus 268,435,456 required headroom
  within the same 4 GiB budget. No acceptance threshold changed.
- Result: local fail/nonqualified; publisher pilot not ready; exit status 1.
- 39 of 45 observation rows: 35 pass, four config-recovery failures, six debug
  rows pending after their admission completion witness failed.
- Maximum observed process-lifetime high-water: 51,494,912 bytes. This is
  not a bound for missing debug work or complete matrix acceptance.
- Maximum-shape, FPD and GVL rows progressed past the first attempt's failures.
  Corrupt-config observation and debug completion require independent diagnosis.

```sh
node scripts/qualify-resources.mjs --profile docs/testing/fixtures/resource-qualification/native-axum-local-4bcc666.json --output /private/tmp/auctioneer-diagnostic-qualification-q2-measured-4bcc666 --source-commit 4bcc6663f746e4cf1a21241dfe1c40ebf713c446 --no-build
```

Safe report/observations/runner files are retained byte-for-byte. They contain
bounded aggregates, enums and digests, not traffic, debug bodies, configuration,
credentials or raw logs. Frozen release copies remain outside the repository.
Any repair requires a new harness/profile identity and full rerun; these files
are never rewritten to pass after execution.

| File | SHA-256 |
| --- | --- |
| report.json | d482727a79e0754f573879e140820c09380ac052eb8a35c3560811dd8dcdd934 |
| observations.json | 5bb6211ef2c60d0ab5b693cdffd801db857c5a01383d9174b95c0f89c0cb66a9 |
| runner.json | b1d0a1e19b52eaef8b221e30454f67664ba9d80429959c96db5b1aca6685283a |

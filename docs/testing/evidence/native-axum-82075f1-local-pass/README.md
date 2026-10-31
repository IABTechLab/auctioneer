# Native Local Acceptance

Executed on 2026-10-15 against the committed
[predeclared profile](../../fixtures/resource-qualification/native-axum-local-82075f1.json).
All 45 local acceptance rows pass. This is not production qualification,
hosted-provider evidence or publisher readiness.

- Serving source: 82075f11abded7265aa4245e01cb0191523c85bc.
- Frozen release artifact: d6e399f9801a2648fb50c0220baa92f3a7270a916010592e9d7ac8999b94aeed.
- SDK: 65312643f4fd969bb1651e3f7dbdabd1479b2e74.
- Profile SHA-256: f610add0ec58aa48ccfff1a5286304d914b67dad83158b9cc93d3de02e40d67f.
- 24 subcases, six workload families, 45 scheduled observations, three
  configuration variants, four A/B/corrupt/restored process observations.
- No runner failures. Accounting is 2,533,631,672 bytes within the declared
  4 GiB budget, plus the predeclared 268,435,456-byte required headroom.
- Maximum observed process-lifetime high-water: 50,905,088 bytes; maximum
  observed retained growth: 9,699,328 bytes, within the predeclared criteria.
  Lifetime high-water is not a per-window peak or a provider-managed bound.
- Overall qualification remains `unsupported`, publisher readiness is false,
  and the harness exits 1: this local fixture uses `insecure_bidders`, and the
  six required SDK capability attestations remain pending. A local workload
  pass cannot grant these capabilities or qualify the selected live deployment.

```sh
node scripts/qualify-resources.mjs --profile docs/testing/fixtures/resource-qualification/native-axum-local-82075f1.json --output /private/tmp/auctioneer-m6-q2-measured-82075f1 --source-commit 82075f11abded7265aa4245e01cb0191523c85bc --no-build
```

The report distinguishes `local_acceptance: pass` from
`profile_qualification: unsupported`. Neither the exit status nor pending SDK
rows were changed to manufacture overall success. Both failed attempts remain
retained. Acceptance thresholds were unchanged across the three executions.
The final fixture's explicitly reviewed, digest-bound projected geometry is
described in the [harness runbook](../../resource-qualification.md).

Safe report/observations/runner files are retained byte-for-byte. They contain
bounded aggregates, enums and digests, not traffic, debug bodies, configuration,
credentials or raw logs. Frozen executable copies remain outside the repository.
M7.6 owns actual deployment Q2-D, credentialed provider execution and the limited
publisher rollout/rollback after M7.5 and the selected bidder wave/host setup.

| File | SHA-256 |
| --- | --- |
| report.json | ba9174b8a049a7ec1b27cea602a3b31cbe5d2e9f8ae867a38e95ec6cf2c23d3d |
| observations.json | e795300e6be74ce9d9e20dcc41384bb26eddbdb1115f67695214d6b6550c62ef |
| runner.json | c501330309199c87caad3f83681e79bf080aae5772933ab9c9f140d209edc01c |

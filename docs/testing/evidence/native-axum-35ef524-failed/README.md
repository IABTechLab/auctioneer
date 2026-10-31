# Corrected-Harness Attempt: Underload

Executed 2026-10-15 against the committed
[predeclaration](../../fixtures/resource-qualification/native-axum-local-35ef524.json).
All 45 rows were measured: 41 passed and four failed `underload`. Local acceptance
is fail, overall qualification is nonqualified, publisher readiness is false,
and the harness exited 1. Runner failures were zero. This is not a passing capture.

- Source: 35ef5241b77ad34d4b09ecc1c6cbc1673fa867b0.
- Artifact: 8c06dd1bd249c533961dab62b5bc046b8e492d114b0287be7596dab6508801b3.
- SDK: 65312643f4fd969bb1651e3f7dbdabd1479b2e74.
- Profile SHA-256: c86bbec1ae69187cd2108f8c65861b76f7ad4d95d424e36456794ece097d739f.
- Accounted envelope: 2,533,631,672 bytes; required headroom: 268,435,456 bytes.
- Maximum service process-lifetime high-water: 37,863,424 bytes; maximum retained
  growth: 10,780,672 bytes. These measurements do not qualify underloaded rows.

Both repetitions of `stalled_io` and `timeout` reached only two calls per ordinary
auction against a declared sixteen-call load. Stalling the first stream wave
prevented later queued calls from reaching the fixture before their deadlines.
The old merged witness maxima hid this; the corrected ordinary-only accounting
properly failed it. No acceptance threshold, resource limit or timeout bound was
relaxed. The follow-up fixture allows earlier waves to complete and stalls the
final funded wave, requiring all declared calls to arrive before induced timeout.
A new committed harness/profile and fresh measured run are required.

Safe files below are byte-identical to the private capture. No traffic bodies,
debug, configuration, credentials or raw process logs are retained. Historical
captures are unchanged; hosted execution and deployment qualification remain M7.6.

| File | SHA-256 |
| --- | --- |
| report.json | 090de434094b66903764fb0d18ea1fdb094b6f171372689e83894b1532bc842d |
| observations.json | 9744071f6ce0c132cc58a08e18fd1f3ccbaa68f60cde74966986c125eb346335 |
| runner.json | 29d8e0c23d3953d8c79bf1fc7898b4405722eb6f2f0346b247ab47674e4ba82e |

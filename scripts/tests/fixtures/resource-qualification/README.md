# Synthetic Qualification Controls

These files are invented schema/evaluator controls, not native measurements,
production acceptance evidence, a platform budget, or a runnable Q2 candidate.
Do not copy their observations or verified capability rows into measured evidence.

The profile and observations are based on the offline CLI examples and the
synthetic Rust integration fixture in
`crates/auctioneer-cli/tests/qualification_cli.rs`. The authoritative contracts are
`crates/auctioneer-cli/src/qualification/{profile,report,evaluate}.rs`. Explicit debug
workload references and histogram capacity fields match those contracts; tests
do not silently add missing schema fields after loading.

The schedule is exactly 24 required subcases, one repetition and one observation
per subcase. This minimal synthetic schedule is distinct from the native
candidate's 45-row schedule. Identities and memory/latency/effect observations are
dummy values. Every string was checked against schema enums or lowercase hex
digest/revision forms before inclusion; every number is a nonnegative safe
integer. There are no URLs, credentials, raw logs, consent strings or payloads.

The observation profile digest binds the compact JSON plus newline emitted by
the test evaluator helper, not this pretty-printed fixture's whitespace.
Debug references use the CLI's sorted-key canonical JSON convention.

Build the debug CLI before running the Node suite:

```sh
cargo build -p auctioneer-cli --locked --offline
node --test scripts/tests/resource-qualification.test.mjs
```

Cargo metadata determines the target directory, including `CARGO_TARGET_DIR`.
Missing CLI or control files fail the suite instead of skipping assertions.
Only the separately documented `AUCTIONEER_RESOURCE_PROBE_DIR` native microprobe is
optional. Schema changes require deliberate fixture review and regeneration,
followed by the real CLI validation/evaluation tests; do not treat a synthetic
passing evaluation as local qualification.

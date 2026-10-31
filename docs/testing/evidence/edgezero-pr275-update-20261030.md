# EdgeZero PR #275 Update

## Source

On October 30, 2026, the authenticated GitHub PR API reported
[PR #275](https://github.com/IABTechLab/edgezero/pull/275) at
`9fe6c56c648f0a601b1c6c051ce71c09d3733ea9`, 14 commits ahead of the
previous pin `0a04ddb083147a1161eb46610bdd9644b3c92cbe`.
The workspace dependencies, eight lockfile source entries, current resource
identity, CLI provider checks and CI pin expectations advance together.
No unrelated dependency version changed. The SDK and Auctioneer tool declarations
match, including Rust 1.98.1; the existing Spin SDK fork remains unchanged.

## Contract Audit

The source comparison retains explicit cache bypass, no implicit redirects,
original absolute deadlines, immutable indexed receipts, EOF/completion rules,
and independent encoded, decoded, final-buffer, header, chunk and decoder
limits. The core dispatch, batch, head, scope and payload implementations do
not change in this increment. Axum's outbound runtime changes are tests only.
Recognized encoding surviving SDK processing is still refused before parsing.

The new connect-timeout capability is Native on Fastly and BestEffort on
Axum, Cloudflare and Spin. Fastly also advertises Native flexible phase budgets.
These declarations do not change Auctioneer's required admission/cache-bypass
capabilities or upgrade cancellation/header fidelity guarantees.

Fastly now records request-scoped registration counts and durations.
Its recorder and distinct-configuration set introduce retained SDK owners;
the observation handle retains numeric statistics after callback exit, not
configuration keys. These allocations are not evidence of a complete funded
outbound bound. Complete opaque-host resource accounting remains unsupported,
and the existing M7-R0 correspondence/readiness blockers remain open.
Spin extracts its unchanged visible-response-head normalization into a helper;
its header fidelity remains BestEffort.

The SDK relocates integration fixtures into its test-harness crate. Auctioneer has
no active references to those old fixture paths. Its offline release verifier's
release/deploy helper script paths remain unchanged.

This update changes no workload limit, ceiling, budget equation, schema
reservation or vendor patch. Frozen qualification profiles and captures retain
their original revision and bytes. They are not rerun or recertified merely
by changing the current integration pin. The PR-branch source-availability
dependency described in the original adoption record remains.

## Verification

The first all-adapter run exposed a stale Axum boot-test oracle: it omitted
the already-installed 512-KiB validation scratch and the once-per-isolate
16-MiB/8-MiB catalog reservations. The library oracle and production equation
already included them. The integration oracle now pins those approved inputs
independently and includes all three in its worked sum; production charges
are unchanged.

Completed local runs against the new revision:

| Command Scope | Observed Result |
| --- | --- |
| Core library, exact workflow gates, codec/source pins | 607 + 124 + 2 passed |
| Secure generated-App onboarding | 8 passed; all 14 priority flows and 300 privacy dispatches |
| Development generated-App onboarding and shared handler/fanout conformance | 8 + 8 + 35 passed; another 300 privacy dispatches |
| All targets in all four adapter crates, explicit conformance feature | 246 passed, 6 ignored across 37 binaries |
| CLI library, binary units and qualification harness integration | 36 + 743 + 8 passed; 2 subprocess helper tests marked ignored and driven by their parent tests |
| Affected core/CLI/four-adapter all-target Clippy | Passed with `-D warnings` |
| Cloudflare and Fastly guest libraries and entrypoints | Compile checks passed |
| Spin guest library | Compile check passed |
| Formatting and whitespace checks | Passed |

Commands used `--locked --offline -j2`, with the two bidder reference source
directories and `EDGEZERO_SDK_ROOT` set to their verified local checkouts.
The CLI test run used command-line-only SHA-256 optimization overrides, not a
repository profile change:

```sh
cargo test --locked --offline -j2 -p auctioneer-core --features conformance-suite \
  --lib --test codec_pins --test ci_workflows
cargo test --locked --offline -j2 -p auctioneer-core --features conformance-suite \
  --test native_bidder_onboarding
cargo test --locked --offline -j2 -p auctioneer-core \
  --features conformance-suite,insecure-bidders --test native_bidder_onboarding \
  --test fanout_conformance --test bidder_handler_conformance
cargo test --locked --offline -j2 -p auctioneer-adapter-axum \
  -p auctioneer-adapter-cloudflare -p auctioneer-adapter-fastly -p auctioneer-adapter-spin \
  --all-targets --features conformance-suite
cargo test --locked --offline -j2 \
  --config 'profile.dev.package.sha2.opt-level=3' \
  --config 'profile.test.package.sha2.opt-level=3' \
  -p auctioneer-cli --bin auctioneer-cli --lib --test qualification_harness
cargo clippy --locked --offline -j2 -p auctioneer-core -p auctioneer-cli \
  -p auctioneer-adapter-axum -p auctioneer-adapter-cloudflare \
  -p auctioneer-adapter-fastly -p auctioneer-adapter-spin \
  --all-targets --features conformance-suite -- -D warnings
cargo check --locked --offline -j2 -p auctioneer-adapter-cloudflare \
  --target wasm32-unknown-unknown --lib --bins
cargo check --locked --offline -j2 -p auctioneer-adapter-fastly \
  --target wasm32-wasip1 --lib --bins
cargo check --locked --offline -j2 -p auctioneer-adapter-spin \
  --target wasm32-wasip2 --lib
```

The adapter ignores require a separately built secure production binary,
prebuilt Viceroy guest tests, or the explicit Spin manifest-loader CI runner.
Those specialized runners, package-size gates, live deployments, the complete
workspace matrix and the longer bidder CLI evidence-integration target were
not rerun in this SDK increment. This is scoped compatibility verification,
not a new qualification of historical artifacts. Cargo also reports a
`proc-macro-error2 2.0.1` dependency future-incompatibility warning.

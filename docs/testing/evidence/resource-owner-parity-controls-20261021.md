# Resource Owner And CLI Parity Controls

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

Date: 2026-10-21
Starting commit: `ced48cd4`
Scope: native accounting and test-harness verification, not catalog qualification.

## Independent Owners

The existing `auctioneer-core/tests/catalog_resource_profile.rs` remains pure and
non-installing. Its original eight tests are retained. Two new tests use
field-complete projections of all 11 isolate terms and six auction receipts;
adding a field requires an explicit test-oracle decision.

Twelve counted owner controls independently vary baseline residency, GVL
cache population, fixed construction, auction egress, auxiliary work, terminal
egress, transmit state, parser population, body streams, catalog residency and
initialization, and the H1-dominant parser branch. The transmit-state case
expects unequal changes in E/A/T/P; the auxiliary-permit case also accounts
for its separate GVL population. Every unaffected term and receipt must stay
unchanged. Expected deltas are literal values, not the runtime constants or
checked-sum implementation under test.

Seventy-two further counted controls cross N=1/3/7 auctions and 0/1/5 auxiliary
permits with eight independent changes: baked bytes, backend cap, blob cap,
node cap, C, I, V and K. Baked/blob changes cost 128 bytes per input byte per
auction, the backend envelope costs 129, and a config node costs 623. C/I
remain once-per-isolate, while V/K enter the projection receipt per auction.
The complete receipt vector distinguishes a correct total in the wrong owner.
Synthetic baked-byte changes go only to the pure runtime call; core's cached
complete host/account tuple allowance remains unchanged and no gates install.

## Mutation Evidence

All following probes compiled and failed their selected new runtime assertion,
not a compiler or infrastructure check:

| Probe                                                                    | Count | Owning Control                                                      |
| ------------------------------------------------------------------------ | ----: | ------------------------------------------------------------------- |
| Omit one of B/G/Q/E/A/T/P/H/S/C/I from the checked sum                   |    11 | Complete owner-input vectors and independently expected total delta |
| Move V or K individually from projection to config, preserving the total |     2 | Complete auction receipt vectors                                    |
| Charge C 128x, per auction or per auxiliary permit; charge I per auction |     4 | Independent catalog/config population controls                      |
| Change the node charge from 623 bytes to one                             |     1 | Independently fixed node-cap slope                                  |

Each of the 18 invocations reported zero passed, one failed and nine filtered
tests. The production source was restored after every probe. The before/after
SHA-256 of `auctioneer-runtime/src/budget.rs` is
`05ca7a50cdc7fe691270c3e95dd4c1932665bc62ef6ad0f0207d1ad7bcecabe7`,
and its Git diff is empty. Logs use `/private/tmp/bidder-catalog-resource-owner-omit-*.log`
and the `validation_to_config`, `context_to_config`, `resident_128x`,
`resident_per_auction`, `resident_per_auxiliary`, `initialization_per_auction`
and `node_unit_one` suffixes under that same prefix.

These are local mutation observations. No mutation hook or alternate equation
is committed to the runtime.

## Real CLI Boundary

The existing `resource_boot_parity` target still builds one bounded isolated
copy of the real CLI and the existing sole core `boot_strict_parity` probe.
Only that copy gets synthetic adapter profiles. The parent never installs
gates, and each installing row uses a fresh child rather than resetting any
process-global lock.

The CLI now uses `--check --all-adapters`. Ten cap/default cases produce four
rows each, matched against 40 counted fresh boot installations. Each adapter
has a distinct synthetic baseline with an independently written literal
expectation, so reusing Axum for another selector cannot pass. The comparison
includes all 11 retained input/accounting fields, full baked tuple size/digest,
all terms/receipts and the expected strict-versus-inspection exit behavior.
The synthetic native profiles and large test ceilings are not provider
declarations, guest execution, physical-memory evidence or readiness results.

Four scratch-only corruptions exercise global raw-zero and zero-baked inputs,
a Cloudflare-only raw-zero row and a Spin-only zero-baked row. The last two
leave preceding rows valid. Each must return exit 2 and no JSON. Fixed traces
prove that the intended adapter executed the corruption before refusal;
unknown-hook refusal alone cannot pass. Construction resolves all rows and
validation resolves them again, so global traces must be Axum/Cloudflare/
Fastly/Spin/Axum, while the targeted controls repeat their own adapter twice.
Only reviewed literal hook names are traced, never an unchecked env value.

An initial run exposed an existing infrastructure error: Cargo emitted the
same resource owner more than once, while the collector demanded exactly one
artifact. A focused positive control reproduced the failure. The collector
now accepts identical package/source and normalized-feature pairs across
compilation units, while exact-message negative controls reject differing
source, differing features and duplicates replacing a missing owner. It still
requires all six resource owners and compares the actual CLI/boot builds.

After that repair, the expanded test failed as intended with four passed and
one failed because only Axum had a synthetic profile. Extending injection to
all four adapters produced five passed, none failed/ignored and 40 fresh
install receipts before the trace improvement. Review then identified the
trace replay count; final trace-enabled results are recorded below.

## Verification

Final selected invocations have zero failures and zero ignored tests. Counts
overlap across feature modes and are not a summed workspace total:

| Invocation                                                                   | Observed Result                                                   |
| ---------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| Secure core profile and boot targets                                         | 10 pure tests; one boot parent with ten fresh installing children |
| Same core targets with insecure-bidder/conformance features                  | Same 10 plus one parent, independently executed in that build     |
| Complete real CLI parity target after trace-string lint cleanup              | 5 passed; 40 fresh boot comparisons and four traced refusals      |
| Core `ci_workflows`                                                          | 103 passed                                                        |
| Strict workspace all-target Clippy with insecure-bidder/conformance features | Exit 0                                                            |
| Formatting, diff and four repository guard scripts                           | Exit 0; 168 raw fixtures across 50 files and 139 roster members   |

The final CLI run reports 171.99 seconds for the test target, including its
isolated builds and subprocesses. This is harness duration, not application
latency evidence. The trace assertion was verified again after replacing
format/collect with direct `writeln!` accumulation. Assertion-helper additions
also use checked arithmetic; no expected value or criterion changed and no
lint suppression was added.

Reproduction commands use Rust 1.98.1, locked offline dependencies and two jobs:

```sh
CARGO_INCREMENTAL=0 cargo test --offline --locked -j2 -p auctioneer-core \
  --test catalog_resource_profile --test boot_strict_parity -- --nocapture
CARGO_INCREMENTAL=0 cargo test --offline --locked -j2 -p auctioneer-core \
  --test catalog_resource_profile --test boot_strict_parity \
  --features insecure-bidders,conformance-suite -- --nocapture
CARGO_INCREMENTAL=0 cargo test --offline --locked -j2 -p auctioneer-cli \
  --test resource_boot_parity -- --nocapture
CARGO_INCREMENTAL=0 cargo clippy --offline --locked -j2 --workspace \
  --all-targets --features insecure-bidders,conformance-suite -- -D warnings
```

Final logs are `/private/tmp/bidder-catalog-resource-owner-core-final-secure.log`,
`/private/tmp/bidder-catalog-resource-owner-core-final-features.log`,
`/private/tmp/bidder-catalog-resource-owner-parity-final.log`,
`/private/tmp/bidder-catalog-resource-owner-clippy-ready.log` and
`/private/tmp/bidder-catalog-resource-owner-ci-workflows.log`. The CI guard invocation is
`CARGO_INCREMENTAL=0 cargo test --offline --locked -j2 -p auctioneer-core --test
ci_workflows`. Existing secure core and workspace/standalone CLI all-target
workflow commands already execute these targets; no new target or lane was
added. Local CI-guard success is not a hosted workflow result.

Collector and dispatch RED logs are
`/private/tmp/bidder-catalog-resource-owner-cargo-units-red.log` and
`/private/tmp/bidder-catalog-resource-owner-all-adapters-dispatch-red.log`. The compiling
focused collector RED is zero passed/one failed; its repair is four passed.
After that, the all-adapter dispatch RED is four passed/one failed. Both are
runtime assertion failures, not zero-match or compile failures. Separate
read-only spec and quality reviews approved this scoped increment; the spec
review's trace-count finding was corrected and re-reviewed before final
verification. Reviewers did not run Cargo or supply execution evidence.

Existing GVL deleted-vendor diagnostics and Cargo's third-party
`proc-macro-error2` future-incompatibility notice remain. Neither is presented
as resolved by the successful lint exit.

The existing child harness keeps its bounded debug-free scratch profile and
locked dependency resolution. No production profile, SDK/toolchain pin,
reference capture, workload limit, budget coefficient or equation changes.
No hosted CI, guest build/execution or live provider test is claimed.

This does not qualify metadata/validator/cache storage or enable a serving
catalog. Full Task 4 still requires its remaining strict/aggregate boundaries,
qualified descriptors and live consumers. Schema-engine qualification, full
bidder catalog migration and resource qualification readiness remain open. The separate roadmap edit is
outside this increment.

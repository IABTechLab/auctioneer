# EdgeZero PR #275 Update

## Source

On November 2, 2026, the authenticated GitHub API reported
[PR #275](https://github.com/IABTechLab/edgezero/pull/275) at
`775366fdac923135e8430b1e3dffebedc648ef7c`, 22 commits ahead of the
previous pin `9fe6c56c648f0a601b1c6c051ce71c09d3733ea9`.
A second query during verification returned the same head.

The six workspace declarations and eight lockfile source entries advance
together with current resource identity, provider identity checks, fixtures,
the CI SDK locator and current bootstrap instructions. No dependency version,
decoder tuple or tool version changes. The shared tool declarations still
match, including Rust 1.98.1; Auctioneer retains its worker-build declaration.
The SDK remains pinned to an immutable commit on the open PR, not a branch
selector or a commit claimed to be merged into main.

## Contract Audit

Prepared outbound dispatch, required admission/cache bypass, no implicit
redirects, paired clocks, original absolute deadlines, immutable indexed
receipts and independent payload/header/decoder caps remain. The production
batch, payload and limit implementations do not change in this increment.
No Auctioneer workload, coefficient, ceiling, budget equation or schema
reservation changes.

Spin now preserves a certified empty ingress body instead of constructing an
external stream: one valid zero Content-Length with no Transfer-Encoding stays
buffered. Its admitted path still performs framing/admission checks before
deciding whether to construct the source. Nonempty and unknown bodies retain
the existing bounded/deadline reader path.

The new SDK body-read, ownership and Fastly wait observations are gated by
`test-utils`. Locked metadata for Auctioneer's workspace conformance graph
reports no enabled features on `edgezero-core`, so these optional observers
are not selected here. Upstream adds acceptance and native-runtime fixtures;
that is not an execution or qualification claim for Auctioneer's deployments.
The release/deploy verification scripts used by the offline archive checker
are byte-identical across these SDK revisions.

Complete opaque-host allocation correspondence and the existing resource/hosted
readiness blockers remain open. Neither new upstream observations nor this pin
change closes them. Historical profiles, captures and the October 30 update
record retain their original bytes and SDK revision; they are not recertified.

## Verification

The existing codec/source test rejected the old lockfile after its expected
revision was advanced, then passed with the new lockfile. No new pin-guard
framework or test target was added.

Completed tests used `--locked --offline -j2`, the existing target cache and
serialized heavy Cargo work:

| Scope | Result |
| --- | --- |
| Core library, codec/source pins, SDK fanout, conformance, handler wave and serialization allocation bounds | 609 + 2 + 2 + 35 + 1 + 7 passed |
| All targets in all four adapters, explicit conformance feature | 232 passed, six documented ignores across 37 binaries |
| CLI provider/archive/preparation/artifact units | 90 passed; real pinned SDK archive verification included |
| CLI public-command publication/cleanup units | Eight passed |
| CLI library, resource reports and retained evidence integrity | 36 + 13 + 5 passed |
| CLI qualification integration | Eight passed with temporary localhost fixture permission |
| Affected core/CLI/four-adapter native all-target Clippy | Passed with conformance enabled and `-D warnings` |
| Existing CI adapter/exchange/privacy guest Clippy selections | All three deployment triples passed with `-D warnings`; compile/lint, not guest execution |
| Actual CI SDK locator, ShellCheck and formatting | Passed |

Total executed compatibility tests: 1,048 passed, with the six pre-existing
adapter ignores reported separately.

The first CLI integration run had two fixture-start failures. A direct socket
probe reproduced sandbox `PermissionError`; the unchanged eight-case target
passed with local listener permission. No retry loop, assertion relaxation or
production repair was introduced for that environmental failure.

The initial guest lint command incorrectly selected core's native test graph
as a WASM root. `cargo tree` traced Tokio/Mio to core's existing native Axum
dev-dependency; Mio rejects `wasm32-unknown-unknown`. The corrected commands
mirror CI's adapter/exchange/privacy roots. Core's portable library still
compiles transitively, and its native tests already passed. No dependency,
platform rule or CI coverage was changed to address that selection error.

The six adapter ignores require the secure production binary, prebuilt Viceroy
guest tests or the explicit Spin manifest-loader runner. Their CI owners remain.
The complete workspace matrix, full producer/compiler suites, specialized
vendor runners and live deployments are not local acceptance claims here.
Cargo's existing proc-macro-error2 future-incompatibility warning remains.

# Auxiliary Config Value Accounting

> Editorial normalization (2026-11-02): delivery labels and displayed local names are descriptive. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

Date: 2026-10-18
SDK: `bc300ace5ca1ffe7060594a7bdb01c1da80de305`

AG approved proof-backed accounting corrections with unchanged workload limits.
This correction changes one auxiliary receipt component, not the isolate total.

## Bound And Units

The SDK checks whole-envelope JSON structure before materializing its
`serde_json::Value`. Its node counter counts values, including containers and
envelope metadata, but not object keys. Core installs the normalized
`max_config_json_nodes` as the SDK's `max_json_nodes` for normally booted Apps.

Previously, auxiliary work derived config storage as `blob_byte_cap * 384`,
then divided by 384 to report a config node count. That was a byte-derived
syntactic-node allowance, not the population the extractor enforces. The
runtime now takes `config_json_values: u32` explicitly, charges each at the
existing config-specific 623-byte allowance, and retains that exact value cap.
Body and cookie parser allowances remain independent at 384 bytes per
syntactic node, including keys. Envelope/blob/baked parse and conversion
charges are unchanged.

At the shipped 1-MiB blob and 8,192-value caps:

| Component            |      Before |     After | Receipt Reduction |
| -------------------- | ----------: | --------: | ----------------: |
| Config value storage | 402,653,184 | 5,103,616 |       397,549,568 |

The declared auxiliary slots and runtime-owned additive isolate equation do not
change. **Isolate-total savings: zero.** This is not permission to reduce a slot
or claim fit from the lower receipt.

## Verification

Tests failed before implementation: nine runtime assertions and one real-App
resource assertion. They distinguish config values, envelope/blob/baked bytes,
body and cookie storage; cover zero, exact and over-cap checks, full `u32`
counts and checked-sum overflow; and use literal 623/384 expectations rather
than the production constants as their oracle.

| Command                                                                                                                                | Result               |
| -------------------------------------------------------------------------------------------------------------------------------------- | -------------------- |
| `cargo test --locked --offline -j 2 -p auctioneer-runtime --all-targets`                                                                   | 191 passed, 0 failed |
| `cargo test --locked --offline -j 2 -p auctioneer-core --test usersync_resources --test usersync_handler_alloc --test config_parse_factor` | 27 passed, 0 failed  |
| Runtime all-targets Clippy under `-D warnings`                                                                                         | Passed               |
| Formatting and diff checks                                                                                                             | Passed               |

The second command includes 18 unchanged real-SDK extraction/allocation
controls, seven actual handler allocation tests and two resource integration
tests. Parent reruns independently reproduced both test totals. Logs:
`/private/tmp/resource-qualification-config-node-*.log` and `/private/tmp/resource-qualification-parent-config-*.log`.

## Remaining Boundaries

The inherited 623-byte allowance plus byte/conversion charges is not a new
independent per-phase guest allocation proof. Preboot/stale-App extraction
must separately be refused or funded; AG has approved the refusal contract,
but this correction alone does not implement it. The planned resolver change
must preserve bounded liveness and prove refusal before any store read.
The subsequent [funding-boundary verification](config-funding-boundary-20261018.md)
records that separate implementation and its fixture migration; it does not
retroactively turn this earlier unit correction into a complete allocation proof.

No all-route response-handoff bound, separate Fastly stack proof, missing SDK
memory fact, WASM absolute fit or hosted qualification is discharged here.
Historical audits and captures retain their original source identities.

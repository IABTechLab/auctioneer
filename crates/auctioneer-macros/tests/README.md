# Compiler Conformance Tests

Place each exported macro's compiler-level tests and pass/fail fixtures here.
Expansion-helper unit tests stay inline with their implementations in `src/`.

Cargo discovers `conformance/main.rs`; its ordinary `mod field_metadata`
declaration owns the field-metadata contract checks. Supported Rust inputs live
under `conformance/pass/`, and rejected inputs and diagnostic snapshots under
`conformance/fail/`. The runner checks fixture counts before compiling them.

Run with `cargo test --locked -p auctioneer-macros --test conformance`.
Do not import private implementation source or shadow the real metadata API.

# Auctioneer Macros

Workspace home for procedural macros that transform Rust declarations.
`OpenRtbFieldMetadata` describes the fields of owned OpenRTB request structs.
The compiler resolves storage capabilities through `auctioneer-openrtb`'s explicit
trait implementations; the macro does not guess capabilities from type names.
Disclosure policy remains in the reviewed policy tables, not in this derive.

The [shared crate rules](../AGENTS.md) apply to this crate and its compiler
fixtures. Do not import handwritten source through `#[path]` or `include!`;
generated-code includes are the only exception.

## Boundaries

- Use workspace-managed `syn`, `quote` and `proc-macro2` dependencies; do not
  hand-compose generated Rust strings.
- Transform explicit Rust input. Keep bidder-directory discovery, file-based
  configuration and schema compilation, provenance, archives and build identity
  in their existing tested generators and build-script entrypoints.
- Do not introduce provider-specific knowledge into shared macros.
- The [field-metadata design](../../docs/superpowers/specs/2026-10-27-openrtb-field-metadata-derive-design.md)
  defines the first migration. The policy compiler reads compiled request
  metadata; the source scanner and its type-spelling heuristics are removed.
  Compiler conformance cases and independent policy/raw-slot oracles retain
  the old controls' intent without a second model implementation.

## Tests

- Colocate unit tests for token expansion and input validation with each macro's
  implementation under `src/`.
- Keep compiler-level integration tests and their pass/fail fixtures under this
  crate's `tests/` directory. Exercise the exported macro, not a private source
  file imported with `#[path]` or `include!`.
- Check generated behavior as well as expansion syntax. Negative cases must fail
  for the intended diagnostic, not an unrelated missing import or type error.
- Native-only `trybuild` compiler conformance cases use the real metadata interface. They compile
  supported inputs and snapshot the intended refusal diagnostics; fixture counts
  prevent missing directories from producing a zero-case success.

```sh
cargo test --locked -p auctioneer-macros --all-targets
```

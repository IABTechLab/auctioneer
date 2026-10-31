# Rules For Every Crate

These rules apply to every current and future crate under `crates/`, including
production code, build scripts, unit tests, integration tests and conformance
tests. A crate-local convention must not weaken them.

## Source Imports

- Use ordinary Rust module declarations and crate dependencies. Share code
  through its owning crate's explicit API, including opt-in test-support APIs.
- Do not use handwritten `#[path = "..."]` attributes, including attributes
  nested in `cfg_attr`, to import source files. This prohibition applies to
  both same-crate and cross-crate source imports.
- Do not use handwritten `include!` to compile existing implementation, build,
  helper or test source files. Do not recompile another crate's private source
  or point a Cargo test target outside its owning crate.
- The only permitted `include!` use is to compile generated Rust from an
  explicit build-output directory. Use `OUT_DIR`, or the existing
  `SETTINGS_BASE`/`SETTINGS_EXPANDED` directories in compiler probes, with a
  fixed relative `.rs` suffix and no traversal. An arbitrary environment
  variable, absolute source path or unbounded expression is not an exemption.
- Automatic generators may emit module-path attributes needed for discovered
  implementations and owner-local conformance. That is generated wiring, not
  permission for a handwritten source-file list or private-source workaround.
- `include_str!` and `include_bytes!` remain permitted for data and fixtures.
  Reading Rust source as data for integrity or coverage analysis is distinct
  from compiling that source; do not turn data loading into a source-import
  workaround.

## Tests And Generators

- Colocate unit tests in an inline `#[cfg(test)] mod tests` beside the
  implementation. Integration and conformance tests belong under the owning
  crate's `tests/`, with ordinary module trees.
- Prefer an existing compatible domain target over a new integration binary.
  Keep separate binaries when process-global boot/logging state, counting
  allocators, socket/process lifecycle or cold-state measurements require it;
  document that isolation at the target root. Share fixture setup through its
  owner, not independent oracles, expected limits or immutable captures.
- When relocating tests, preserve assertions, feature gates and isolation,
  and compare before/after case names and actual dispatch counts. A filtered
  pass or silently skipped required-feature target is not acceptance. See
  [development and test ownership](../docs/testing/development.md).
- Generate Rust with `quote!` and `syn`, not hand-assembled source strings.
  Diagnostics, paths and intentionally malformed compiler-test inputs are not
  generated Rust implementations.
- `.expect()` is allowed in tests and explicitly test-only fixture helpers.
  Keep the production `clippy::expect_used` restriction; do not add a global
  production allowance.

## Enforcement

`auctioneer-bidder`'s workspace-wide `test_isolation` target checks source imports
in every crate, including nested attributes, expression-position includes and
generated-directory escapes. Run it after changing module or test wiring:

```sh
cargo test -p auctioneer-bidder --test test_isolation --locked
```

Do not narrow the scanned crates or weaken these controls to make a new source
import pass. Extend the owning API or use an ordinary module instead.

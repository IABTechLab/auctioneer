# GPP Envelope Module Implementation Plan

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement and qualify the extractable bounded GPP envelope parser required by privacy enforcement Tasks 4 and 7.

**Architecture:** A pure module returns borrowed section slices, with closed format/limit errors. Source selection, warnings, policy, and TCF/USP interpretation stay in the privacy enforcement resolver. No new crate or runtime dependency.

**Tech Stack:** Rust 1.95.0, standard library, existing proptest, pinned Go reference tools for test-fixture generation only.

**Design:** [GPP module design](../specs/2026-09-29-gpp-envelope-module-design.md). This is a subordinate plan, not replacement acceptance criteria for [privacy enforcement](2026-09-28-privacy-enforcement.md).

Plan review passed on 2026-09-29 after changing the generated header fixtures
from JSON to Rust constants, preserving the parser's dependency-free test
boundary. Task A is implemented and passed independent spec/quality reviews,
35 isolated tests, strict isolated Clippy, and native/three-WASM standalone
checks. Task B's pins, generated vectors, regeneration comparison, and layout
measurements are recorded. Native workspace tests and all three package/adapter
cross-target checks subsequently passed; final privacy enforcement resource acceptance is separate.

## Files and Boundaries

- Create `crates/auctioneer-privacy/src/gpp/mod.rs`: public limits, errors, envelope, bounded parsing orchestration.
- Create `crates/auctioneer-privacy/src/gpp/header.rs` if needed: private checked six-bit/Fibonacci reader, budget, range decoding.
- Create `crates/auctioneer-privacy/src/gpp/tests.rs`: independent parser unit/property tests, including private budget/mutation assertions.
- Create `crates/auctioneer-privacy/src/gpp/data/README.md` and `headers.rs`: pinned provenance and generated Rust header vectors, portable with module extraction and requiring no JSON test dependency.
- Create `crates/auctioneer-privacy/src/gpp/data/reference/`: a small Go fixture generator with `go.mod`/`go.sum` pinning go-gpp v0.2.0; not a production dependency.
- Modify `crates/auctioneer-privacy/src/lib.rs`: export the module, preserving prior USP/DV changes.
- Update only parser-related checkboxes/evidence in the privacy enforcement plan and verification record.

No edits to authorization, resolver, exchange, config, manifests, Cargo.lock, or runtime in this parser task. No encoding production API. Preserve pre-existing dirty files and do not commit without a separate request.

## Task A: Bounded Parser and Isolated Tests

- [x] Run the existing privacy tests as a fresh baseline with `cargo test -p auctioneer-privacy --locked --offline --target-dir /private/tmp/auctioneer-privacy-enforcement-baseline-target`.
- [x] Write tests against the public API before implementation. Representative assertions: `parse("DBACNY~tc~", Limits::default())?.section(2) == Some("tc")`, section 6 is `Some("")`, section 7 is `None`, and `raw()` is unchanged. Keep placeholder payloads explicitly labeled header-only tests.
- [x] Add independent vectors for `DBABM`/`DBABMA`, `DBACNY`/`DBACNYA`, and `DBADNb`/`DBADNbA`, zero sections, mixed ranges, unknown numeric IDs, 65535/65536, and malformed/limit categories. Add no-copy slice-pointer checks, opaque payload controls, and redacted diagnostics.
- [x] Run `cargo test -p auctioneer-privacy --lib gpp --locked --offline --target-dir /private/tmp/auctioneer-privacy-enforcement-baseline-target` and record a missing-API red result.
- [x] Implement private `Limits` fields with defaults from design section 4 and validated lower-only construction; invalid limit configuration is separate from input parse errors. Bound raw bytes/header bytes/raw slots before decoding. Store at most 64 entries inline; never allocate per encoded range size.
- [x] Implement a lazy six-bit reader over borrowed header bytes, with one alphabet validation charge per newly reached character, bit charges, fixed-width checked `u64` Fibonacci arithmetic, and typed format/limit errors. Cache the current six-bit value instead of revalidating it per bit. Stop on the first encountered error.
- [x] Read header type 3, version 1, 12-bit range-entry count, then positive cumulative offsets. For singleton: ID = previous endpoint + delta. For range: start = previous endpoint + delta; end = start + next delta. Check endpoints/cardinality/output capacity before writes; update previous endpoint cumulatively. Reject encoded counts above the output capacity before walking entries.
- [x] Validate the exact canonical or Reference-encoder header length and zero tail bits. Zip decoded IDs with exact borrowed payload slots, preserving trailing/consecutive empty slots. Return no envelope on failure. Neither sections 2/6 nor SID/policy receive special treatment here.
- [x] Add red/green tests for every bound, reduced budgets, precharge-before-mutation, long Fibonacci zero runs, native-independent overflow classification, invalid later alphabet versus earlier version, arbitrary strings, no panics, output capacity, and repeated parse independence.
- [x] Run the focused and complete privacy suites, strict scoped Clippy, and changed-file rustfmt. Complete independent spec review followed by code-quality review; fix and re-review any findings before marking Task A done. See the implementation checkpoint and independent review record; final full-workspace native verification also passed all parser tests.

## Task B: Reference and Portability Qualification

- [x] Record an immutable IAB specification revision/content hash and the pinned Go module checksum. Inspect the Go module license; preserve attribution as required for reference-derived code/fixtures.
- [x] Generate repository fixtures using the actual go-gpp v0.2.0 writer and reader, including cumulative singleton and mixed-range cases. Include the generator source, not temporary-harness output alone. Canonical-padding variants may be derived from known semantic bit counts, with provenance explicit. The files are present and reviewed; no Git commit or push has been made.
- [x] Mark specification-derived boundary/hostile vectors separately from Go parity; the pinned writer cannot encode Fibonacci values at 6765 or above. Do not use the Go decoder as a hostile-input safety oracle.
- [x] Make ordinary Rust tests import the generated `headers.rs` constants offline without Go or a JSON parser. The generator emits only ASCII header string literals and numeric ID arrays. Regenerate and compare those vectors with the pinned generator, preserving independent expected IDs.
- [x] Check `auctioneer-privacy --lib` with `cargo check --locked --offline --target-dir /private/tmp/auctioneer-privacy-enforcement-baseline-target` for `wasm32-unknown-unknown`, `wasm32-wasip1`, and `wasm32-wasip2`. All passed alongside the corresponding full adapter and foundation crates. These checks do not establish provider certification.
- [x] Measure result/scratch storage and retain it as Task 10 accounting input. Verify no dependency on other Auctioneer modules or ambient runtime and no input-sized allocation before bounds.
- [x] Review the qualification evidence and update privacy enforcement Task 4's parser status. Independent parser spec/quality reviews, generated-reference checks, and native/WASM checks passed; the privacy integration then passed native regression tests. This does not close privacy enforcement resource or provider acceptance.

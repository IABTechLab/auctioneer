# Schema Residency And Initialization Checkpoint

Date: 2026-11-21 (America/Los_Angeles; hosted timestamps are 2026-11-22 UTC).
Scope: PR [#262](https://github.com/IABTechLab/auctioneer/pull/262), partial implementation evidence for [#46](https://github.com/IABTechLab/auctioneer/issues/46), not final CI or adapter qualification.

## Source And Capture

- Source: `081b52116fcc6e690ad164121c5677ffd1441065`; tested PR merge: `a54022eeba59c5a6213ea63962ca9ccada9d5238`.
- [Run Tests 44966255810 / compiler-schema job 137035850296](https://github.com/IABTechLab/auctioneer/actions/runs/44966255810/job/137035850296), default-feature growth frame only.
- [Raw JSON](../../../crates/auctioneer-bidder/tests/conformance/fixtures/reference/schema-residency/hosted-growth-081b521.json) and [extraction provenance](../../../crates/auctioneer-bidder/tests/conformance/fixtures/reference/schema-residency/README.md): 1,620,650 bytes, including the final newline.
- Raw SHA-256: `6fff751282b3ea39ce70f4e5d7cb64acc65c9be594301035d22a2e63fb6e61e4`. Preserve these bytes; a later capture gets a new identity.
- Reference commit: `d26fcf7f566db6d1dce2e1847d79c9f44581edb8`; tree: `2d54c6f53ef3d062ca9292449a4546bb76e5ddd8`. It is verified Git-object data, not an executed historical build.
- One current measurement build: Rust 1.98.1 (`48a229ceaefd4985c50990b14116b6d856af0985`), `x86_64-unknown-linux-gnu`, little-endian 64-bit, debug, `test-support`, empty encoded rustflags. The record retains the exact 25 regex features, generator/options and consumed source witnesses.
- Measurement/reference/candidate lock SHA-256: `78118dcb53fd3f486a6032c47124ba249bd5fbf361a4a1973824e56d4f55001c`.

| Binding | Reference | Candidate |
| --- | --- | --- |
| Seed SHA-256 | `3a4880a48e86f05b668340ce0f153098c458fb5b06e573bb8ae16494b913194c` | `26f874cadc16e451bd44d9403438844672d80a2db682d7a887120f4e8f3eaca1` |
| Expanded graph SHA-256 | `824f3dd93c44c61eca757545b691fe9a60694e45ebbaf2cc88f6347fdd10dcc1` | `712d8c80324b859d43aacbdafe8328fe7d5e3b763c5b2ebbf20b2995a35b9306` |
| Recipe identity | `ce4abcc8aba35215e79f2c81e3c8dbbcf1c43cb534d0fba5840fa2af1e2faa8d` | `a78ee4aafa34a32ce1d7400240b5c8fead1b16127b2bad42ff138a3f9d203a8b` |

## Executed Checkpoints

Retained job logs, not patch filenames or static assertions, supply these results:

| Source Checkpoint | Executed Result / Limitation |
| --- | --- |
| `cb2ff769d7c65236f952101e4f2713f7aecc6e9d` | [Run Tests 44951423476](https://github.com/IABTechLab/auctioneer/actions/runs/44951423476), retained `pr262-cb2-schema.log`: 332 compiler controls passed; source run 745 passed / 21 failed / 6 filtered, reaching growth/constructor scaffold assertions. Not GREEN. |
| `a4bc6f02def6a21dacf659372a811b8f249c3d7d` | [Axum descriptor behavioral RED](https://github.com/IABTechLab/auctioneer/actions/runs/44956587925/job/137005944954): 0 passed / 1 failed / 792 filtered, `actual source descriptor refused: Coverage`. The separate unused-import failure is not behavioral RED. |
| `255f88a9349edc3aaa27a0e38e684ed88dc781df` | `pr262-255-schema.log`: 335 compiler controls passed; source 790 passed / 1 failed / 6 filtered, private-temp-directory refusal before matched measurements. `pr262-255-spin.log`: actual Spin artifact case passed in both feature lanes; not target fit. |
| `d2d69d3482f715c90f3897c253735c0c7751b9a8` | `pr262-d2-schema.log`: 335 compiler controls passed; source 794 passed / 1 failed / 6 filtered, actual growth `expanded schedule mismatch`. This did not reach metadata positivity assertions and is not their intended RED. |
| `4b4a08cfd731cca718183f7c303b1002258c138b` | `pr262-4b-schema.log`: 335 compiler controls and 803 source tests passed; 6 intentional exclusions; job then failed on 366 strict-Clippy diagnostics. Successful `eprintln!` output was captured, not a recoverable growth JSON record. |
| `081b52116fcc6e690ad164121c5677ffd1441065` | [Compiler-schema job 137035850296](https://github.com/IABTechLab/auctioneer/actions/runs/44966255810/job/137035850296): 335 compiler controls and 806 source tests passed, zero failed/ignored; 6 intentional exclusions; job then failed on 8 strict-Clippy diagnostics, exit 101. Complete default growth frame retained above. |

The six exclusions remain separately owned: four actual-application artifact cases, the compiler-private-layout case and the selected-schema link/execution probe. These counts do not credit those owners or the later development-metadata step. No local Cargo/build/test/fmt/lint/mutation execution is evidence for this checkpoint.

## Delivered Source Scope

Both captures contain 18 public definitions, 16 effective schemas, five distinct program owners (including shared hostname) and 19 linked/inline groups. The declared added/removed/changed owner sets are empty. Each projection reports `SourceComplete` / `TargetIncomplete`, `numeric_costs: null`, and false receipt, physical-fit and serving flags.

The descriptor re-expands the actual captured graph, binds production staging/native sources and complete model witnesses, and supplies exact required target slots and the #47 validation handoff. Missing/extra facts, cross-root ABI/type disagreement, missing/duplicate child assignments and hostile runtime-source mutations have executed default-lane controls in the 081 log. Source completeness is a conditional recipe, not an authenticated qualification receipt.

Capture-specific seed/graph/linked-source hashes remain distinct. Only common program/formula topology and linked-group identities, roots, inline children, budget owners and units are compared. There is no numeric linked-byte delta.

## Requested Allocation Observations

Reference and candidate have identical observations below; component-wise deltas are zero, not zero absolute storage. All values are requested bytes at the recorded native ABI, not allocator/RSS/guest/physical bounds.

| Surface | Cold Retained | Cold Peak Live | Lifetime Control |
| --- | ---: | ---: | --- |
| Five reconstructed Regex/LazyLock owners | 47,944 | 68,849 | Repeat allocates zero; final cleanup releases 47,944. This is reconstruction, not measurement of generated production singleton lifetimes. |
| Actual production metadata, fresh process | 12,501 | 12,501 | Repeat getter allocates zero; singleton remains until process exit, with no fake release. |
| Actual production registry, fresh process | 2,116 | 2,116 | Lookup allocates zero; registry drop releases 2,116. Metadata-prefix plus registry peak is 14,617. |

The supported library size-limit failure allocates 144,920 bytes cumulatively, peaks at 75,648, retains zero and caches failure without further allocation. With the already-live reconstructed prefix it peaks at 123,592 and preserves 47,944 retained bytes. This is not a claim that the admitted fixed production programs fail or that allocator OOM is recoverable.

The explicitly hypothetical layout evaluation gives C = 90,720, I = 405,111 and C + I = 495,831 requested bytes, with 5,211 separately supplied linked/inline attribution bytes. These are not observed target layouts and do not add linked bytes twice. Pure tests explicitly construct hypothetical `CatalogResourceCosts` to exercise the existing arithmetic; the source recipe does not authenticate or authorize installation of physical costs, establish fit or grant serving permission. Work ledgers retain their own source-visit units and exclusions, not latency/instruction/stack bounds.

## Main Dependency Integration

Main advanced through [#263](https://github.com/IABTechLab/auctioneer/pull/263)
at `05f2a56` during this implementation. The artifact/source guards refused the
synthetic merge's syntax 0.8.12 against this branch's exact 0.8.11 assumptions.
The normal main merge retains its compiler 0.62.0, engine 0.4.19 and SDK graph;
this PR does not select another dependency version or alter the vendor patches.

The syntax archives were independently checked against their lock checksums:
0.8.11 `d6f6ff9a378485b298a5286656da665ba74413d36db0979633275d2e708145d4`
and 0.8.12 `d902046e6a28a1e4c3cab97258a0ce11e994903f01c2d97a9447a0b3df85f4f6`.
The complete `hir/literal.rs` delta derives the same Prefix default instead of
implementing it manually. Literal/trie records and allocation, extraction and
destruction paths are unchanged. Only the added literal source witness changes;
the source-bound suffix formulas and independent expected totals do not.

The resolver now requires the current exact version/source/checksum tuple and
retains canonical package, edge, feature and vendor checks. Previous-release,
missing, duplicated and substituted tuples refuse. Compiler declarations must
explicitly disable the new evaluation method; both true and missing evaluation
options refuse. No legacy method tuple is admitted.

The frozen 081 capture above remains pre-upgrade evidence, not a certificate for
the integrated graph. Its pure-core consumer checks unchanged additive arithmetic
over that historical data only. Current common-source proofs and fresh matched
observations must execute against the integrated graph in the full hosted matrix;
their current-head results belong to PR #262. No old result is recertified by the
main merge or by changing a source witness.

## Final Acceptance

- Task 8: verify the captured-data pure-core consumer's multiplicity/refusal controls against unchanged production arithmetic in its existing owner; retain current `catalog_alloc` and historical continuity acceptance. Task 8's behavioral RED is already recorded in the captured-source path; these integration tests do not require a separate new RED for each test. The producer's `task8_complete: true` labels its reconstruction path, not completion of the entire approved task.
- Task 9: repair and rerun strict lint/hosted formatting, verify the real development growth frame and exact nonzero dispatch, all six separately owned probes, both fresh evidence producers and the unchanged complete feature/guest/packaging matrix at the final head. No final all-green result is recorded here.
- Spec section 10: finish evidence/issue-criteria reconciliation, advisory and independent review, eligible approval and maintainer merge authorization. The plan has Tasks 1-9, not a Task 10. #46 remains open.

Target physical/allocator and actual linked-artifact growth facts remain #72-#75; validation cache/scratch/search/work/stack remain #47; #48 and host/provider boundaries are unchanged. No adapter fit, guest readiness, native-only retirement or serving promotion is established. Existing reservations, limits and accounting remain unchanged. The [historical catalog captures](catalog-baseline-continuity-20261021.md) keep their original bytes and scope.

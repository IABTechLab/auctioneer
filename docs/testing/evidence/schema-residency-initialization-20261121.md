# Schema Residency And Initialization Checkpoint

Date: 2026-11-21 (America/Los_Angeles; hosted timestamps are 2026-11-22 UTC).
Scope: PR [#262](https://github.com/IABTechLab/auctioneer/pull/262), source-level implementation evidence for [#46](https://github.com/IABTechLab/auctioneer/issues/46), not adapter qualification. Dated checkpoints retain their original execution scope; current acceptance is recorded in GitHub.

## Post-Repair Proof Checkpoint: 2026-11-22

Proof-code source `4a863c6c796e55e0b17e1b3252b46060396d3c5a` was executed as merge `604cf03104f0d6581c0a75b49ea43a6467e02c04` over `05f2a56e9555be9c46a789c868100e37162abe5b`. All 25 checks passed: [Run Tests 45021426271](https://github.com/IABTechLab/auctioneer/actions/runs/45021426271), [format/lint 45021426434](https://github.com/IABTechLab/auctioneer/actions/runs/45021426434), and all three provider packaging workflows. This is the post-repair implementation tuple, not a recertification of 081. The later capture/consumer/documentation publication does not change measured code or model sources and receives its own full hosted matrix on the PR. A read-only publication inventory matched 112 captured measurement/handoff/metadata source-witness records to 69 unchanged current files per lane; it is continuity evidence, not another executed resource proof.

[Compiler-schema job 137204983421](https://github.com/IABTechLab/auctioneer/actions/runs/45021426271/job/137204983421) ran `bash .github/workflows/scripts/test/verify-schema-compiler-conformance.sh schema` and `bash .github/workflows/scripts/test/verify-schema-metadata-development.sh`, including strict explicit-target Clippy, exit 0. Verbatim libtest results from GitHub Actions:

```text
test result: ok. 337 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 650.14s
test result: ok. 840 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 368.38s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 719 filtered out; finished in 88.24s
```

The four application artifacts, private-layout and selected-schema cases retain their six separate successful owners. The [secure job](https://github.com/IABTechLab/auctioneer/actions/runs/45021426271/job/137204983530) executed the original dispatch fixture command, exit 0, with `libtest summary controls: 18 passed` and `schema residency dispatch controls: 27 passed`. It also executed native funding/expiry and fresh success, prelive-prefix, concurrent-construction and cached late-failure controls. The [Axum actual-artifact job](https://github.com/IABTechLab/auctioneer/actions/runs/45021426271/job/137204983409) passed its real descriptor/handoff case: `1 passed; 0 failed; 0 ignored; 0 measured; 845 filtered out`. The other three real artifact jobs and both fresh producers passed; native observations do not prove guest fit.

### Durable Captures And Independent Locks

The [capture README](../../../crates/auctioneer-bidder/tests/conformance/fixtures/reference/schema-residency/README.md) records byte extraction and provenance. Both records preserve the actual JSON frames without reserialization:

| Lane / Raw File | Bytes | SHA-256 |
| --- | ---: | --- |
| [default](../../../crates/auctioneer-bidder/tests/conformance/fixtures/reference/schema-residency/hosted-growth-4a863c6-default.json) | 1,624,271 | `71d1be5f96ef8ac6cb3f756cb13ff26efd06b57dd3f6a44a7e23c9beddba2a51` |
| [development](../../../crates/auctioneer-bidder/tests/conformance/fixtures/reference/schema-residency/hosted-growth-4a863c6-development.json) | 1,624,548 | `87da24f9e0a9644384ffe1f1c9bcb2eb2df5d3e52401d9937eeba897430d1f13` |

Reference provenance is `d26fcf7f566db6d1dce2e1847d79c9f44581edb8` / tree `2d54c6f53ef3d062ca9292449a4546bb76e5ddd8` / lock `78118dcb53fd3f486a6032c47124ba249bd5fbf361a4a1973824e56d4f55001c`. Candidate and current measurement lock are `ae849e777f8216f983112f631168bf585c4604076381d265459249d62f88df38`. Both lanes use Rust 1.98.1 (`48a229ceaefd4985c50990b14116b6d856af0985`), x86_64 Linux, debug, empty encoded rustflags and `test-support`; development additionally enables `insecure-bidders`. Every source, compiler/library/options/features witness remains inside the corresponding frame.

| Lane / Binding | Reference | Candidate |
| --- | --- | --- |
| Default seed | `e7b1d367ff1c297b2dcf7c0ab4183b8f4d0c556f5d11deec48c998358e02b143` | `c5012bd8cce24363e117abd2f4fa2074fec7cc22e07dc7aa24a125fde9af41ec` |
| Default expanded graph | `110cc9c3fe5f3007caf031b384b00d6e022441c5e2c095b7ad3fd42f87c28d45` | `c1880816459dba14d973ba1cd6ef80273b1ef29ac98a02fbf11b08745b06ef83` |
| Development seed | `eb6c4070b0df8dc3f51dccf9b997368e8794a51ba7b1a36cb71ae97fe7ca21f3` | `05d52f9c049c84c9b3562d5654b2bcea7183675d216e91fb49864b18df64c76e` |
| Development expanded graph | `78bdaa12683d5b1412c6a9d903bbcf064d2a3dbf792169f5960fa0b06939dc78` | `7b2ebab119f7b519ec8f3139d146f0642ec2f6907f1c3054e93f0b1825d4ae57` |

Both frames have 18 public definitions, 16 effective schemas, five programs, 19 linked/inline groups and empty declared owner deltas. The reference metadata/registry observations are explicitly **equal-input proxies of candidate constructors**, not reference executable observations. Exact metadata inputs must match; `supports_nonzero_metadata_registry_delta: false` means unequal metadata/registry inputs refuse, including a schema change that also changes those inputs. The candidate observations are actual current constructors in fresh processes. Both roles report metadata 12,501 retained bytes and registry 2,116 retained bytes, with zero repeat/lookup allocations; a zero component delta is not zero absolute storage or a physical bound.

The C/I example remains explicitly hypothetical: C = 90,720, I = 405,111, additive total = 495,831 requested bytes. Neither the raw records nor the `SourceComplete` / `TargetIncomplete` recipe supplies a physical cost, authenticated qualification receipt, fit or serving permission. The pure-core consumer checks both lane records, their distinct provenance/measurement locks and role/capability fields. It retains all 27 arithmetic cases per lane (54 total) and the zero-receipt refusal for each lane, without opening Git, running constructors or installing gates. Historical 081 bytes below are unchanged and are no longer its active input.

## Historical 081 Source And Capture

This section and its numeric observations preserve the historical default-lane
081 capture. They are not the later repair-head default/development captures or
a certificate for a newer source, dependency or model tuple.

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
| `55f38f1f7c97e95aa1ed8a90d7e17acc359ca179` | [Integrated compiler-schema job 137078706721](https://github.com/IABTechLab/auctioneer/actions/runs/44980134979/job/137078706721): 337 compiler controls, 809 default source tests and all 96 selected development cases passed, with zero failed/ignored. The job then exited 1 at the development dispatch reader: the framed JSON interrupted libtest's label/result line. The other 24 checks passed. This is not a passing full matrix. |

The six exclusions remain separately owned: four actual-application artifact cases, the compiler-private-layout case and the selected-schema link/execution probe. These counts do not credit those owners or the later development-metadata step. No local Cargo/build/test/fmt/lint/mutation execution is evidence for this checkpoint.

## Delivered Source Scope

The source and observation statements below describe the frozen 081 record.
Current repair execution and the interface review boundary are recorded in the
following supplement; historical bytes and identities are not rewritten.

## Review Repair Supplement: 2026-11-22

GitHub Actions executed the jobs below; CREATOR inspected their hosted logs. Compile/setup/lint failures are not behavioral REDs,
and passing cases inside a failed job are not a passing full matrix.

| Plan Task | Behavioral RED Or Characterization | Subsequent Executed Coverage |
| --- | --- | --- |
| 1: inventory | Read-only inventory/characterization; no behavioral RED is claimed. | Source, owner and CI inventory underpins the existing controls; this is not itself a test result. |
| 2: discovery authority | `8a7b59d`: the no-cohort identity and discovery-only projection assertions failed; secure config result was 418 passed / 2 failed. | Later passing integrated coverage is recorded at `bae1157` and the repair job below; no isolated per-task full-suite result is invented. |
| 3: captured seed and expansion | `8a65972`: the real snapshot control failed with `build seed must bind actual declarations to captured schemas`; 685 passed / 1 failed / 6 filtered. `58c4f51` also failed the unclassified auxiliary-storage refusal. | The named snapshot control and expansion controls passed in the `4b4a08c` source run (803 passed / 0 failed / 6 filtered), before that job's separate strict-lint failure. |
| 4: retained composition | `58c4f51`: four retained-owner assertions and explicit metadata/pool fact controls failed within the 706 passed / 14 failed / 6 filtered source run. | The retained controls passed at `cb2ff769`; explicit metadata/pool controls passed at `4b4a08c`. These are conditional source/layout arithmetic, not physical costs. |
| 5: constructor composition | `58c4f51`: suffix scratch controls failed. `cb2ff769`: eight program-construction controls failed within 745 passed / 21 failed / 6 filtered. | Constructor, literal and initialization composition controls passed in the later `4b4a08c` source run. |
| 6: actual lifecycle correspondence | The supplied logs establish lifecycle/source verification and characterization, not a separate intentional pre-implementation behavioral RED for this entire task. The Task 8 schedule RED is not relabelled as production lifecycle RED. | Initialization/lifetime and fresh reconstructed Regex observations passed at `4b4a08c`; these do not claim measurement of generated production singletons or a recoverable production OOM path. Native funding/publication invariants retain their existing owners. |
| 7: descriptor and handoff | `a4bc6f0` actual Axum case: 0 passed / 1 failed / 792 filtered, `actual source descriptor refused: Coverage`. The same source's compiler-schema unused-import failure is not that RED. `8ca4fc3` later failed four exact handoff-program membership controls. | Earlier descriptor/child-assignment controls passed at `4b4a08c`; the repair job below supplies current regression coverage, not downstream owner acceptance. |
| 8: matched reconstruction | `cb2ff769` executed all 13 growth assertion failures. Later `255f88a` private-directory refusal and `d2d69d3` schedule mismatch occurred before metadata positivity and are not claimed as its intended RED. | `4b4a08c` passed the closed-pair metadata/registry and source reconstruction controls; `081b521` retained the first frozen frame. The reference metadata/registry observation is an equal-input proxy of current production, not historical execution or support for unequal constructor inputs. |
| 9: CI/evidence ownership | `55f38f1` passed 337 compiler / 809 default / 96 development cases, then its dispatch reader exited 1. This is a transport regression, not a product-model behavioral RED. | Supplied `bae1157` handoff records all 25 checks successful and the corrected dispatch controls. Review repairs require their own source/results and fresh capture provenance; old GREEN is not transferred. |

Exact hosted owners for that map:

| Tasks | Source / Run / Job |
| --- | --- |
| 2 RED | `8a7b59d10264cac079984ba9ac4755809cf153c9`, [run 44877097116 / secure job 136764088139](https://github.com/IABTechLab/auctioneer/actions/runs/44877097116/job/136764088139). |
| 3 RED | `8a6597239b3418c8caadb066c4482d328ad601ed`, [run 44929751752 / schema job 136923553242](https://github.com/IABTechLab/auctioneer/actions/runs/44929751752/job/136923553242). |
| 4-5 RED | `58c4f51f95114bd3b0794c54f596765ed8be5512`, [run 44942703923 / schema job 136963263089](https://github.com/IABTechLab/auctioneer/actions/runs/44942703923/job/136963263089). |
| 5 and 8 RED | `cb2ff769d7c65236f952101e4f2713f7aecc6e9d`, [run 44951423476 / schema job 136990059483](https://github.com/IABTechLab/auctioneer/actions/runs/44951423476/job/136990059483). |
| 3-8 passing source slices, not passing lint | `4b4a08cfd731cca718183f7c303b1002258c138b`, [run 44962785608 / schema job 137025123014](https://github.com/IABTechLab/auctioneer/actions/runs/44962785608/job/137025123014). |
| 7 real-artifact RED | `a4bc6f02def6a21dacf659372a811b8f249c3d7d`, [run 44956587925 / Axum job 137005944954](https://github.com/IABTechLab/auctioneer/actions/runs/44956587925/job/137005944954). |
| 7 and evidence-name repair RED | `8ca4fc3e18489a34f91f751d33d9b1c415dff319`, [run 44995433901 / schema job 137125340340](https://github.com/IABTechLab/auctioneer/actions/runs/44995433901/job/137125340340): 809 passed / 5 failed / 6 filtered. |
| 9 transport failure | `55f38f1f7c97e95aa1ed8a90d7e17acc359ca179`, [run 44980134979 / schema job 137078706721](https://github.com/IABTechLab/auctioneer/actions/runs/44980134979/job/137078706721). |
| Prior complete GREEN | `bae1157ec48f4984d4c5b4cedf5898596a1996b2`, [run 44989194907 / schema job 137106310805](https://github.com/IABTechLab/auctioneer/actions/runs/44989194907/job/137106310805); not verification of later repairs. |

[Run Tests 45009943713, compiler-schema job
137169830933](https://github.com/IABTechLab/auctioneer/actions/runs/45009943713/job/137169830933)
at source `404135586d41bbe9126a67c9a4fd0cc142e7fe50`: 337 compiler,
840 default-source and 127 development cases passed, with zero failed;
the six default-source exclusions keep their separate owners and the
development selector reports 719 filtered. All test/provider jobs passed.
[Format 45009943738](https://github.com/IABTechLab/auctioneer/actions/runs/45009943738)
failed on two formatting hunks subsequently repaired at `4a863c6`. This
supplement does not assert a passing format rerun, recertify the formatted
head, or substitute old frame identities for fresh captures at that head.

### Interface Review Record

The source handoff to #47 maps each program's validation cache, mutable scratch
and search fallback, plus declaration/format work and stack children, to the
same source identities. Required program membership and child-assignment
completeness are distinct checks; the four `8ca4fc3` failures above record the
missing-membership regression before the repair. This handoff does not provide
validation work/cache/stack certification or install admission.

The #72-#75 handoff retains source/generator/options/features identity, target
ABI/layout slots, linked/inline attribution and allocator reconciliation.
Those owners must match the actual linked artifact and target tuple before
physical qualification. Common source completeness, hypothetical arithmetic
and native requested-allocation observations do not supply those facts.

This source-interface correspondence was statically reviewed by CREATOR against the actual graph and the #47/#72-#75 boundary on 2026-11-22. The repaired graph has five programs, 16 declarations and two distinct callbacks: `5 * 3 + 16 * 2 + 2 * 2 = 51` validation children. All use the declaration/helper/source/feature identities carried in the same recipe. Immutable programs, retained owners and initialization stay in C/I; validation caches, mutable scratch, fallback, declaration/callback work and stack stay with #47. Linked-group ownership and required ABI/layout/allocator facts stay with #72-#75. No source child is represented as a physical zero or assigned to both sides.

Independent review [6010775297](https://github.com/IABTechLab/auctioneer/pull/262#pullrequestreview-6010775297) identified the missing program-membership guard; its repaired controls and the full interface require REVIEWER acceptance in the original threads. This record is not a downstream certificate, formal approval, issue closure or merge authorization. GitHub retains those decisions.

## Historical Observed Source Scope

Both sides of the historical 081 capture contain 18 public definitions, 16 effective schemas, five distinct program owners (including shared hostname) and 19 linked/inline groups. The declared added/removed/changed owner sets are empty. Each projection reports `SourceComplete` / `TargetIncomplete`, `numeric_costs: null`, and false receipt, physical-fit and serving flags.

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

The frozen 081 capture above remains pre-upgrade evidence, not a certificate for the integrated graph. The original pure-core consumer used that historical data; the post-repair consumer uses both newly captured feature lanes and retains its non-installing arithmetic/refusal scope. Current common-source proofs and matched observations execute against the integrated graph; their exact provenance and hosted results are recorded separately. No old result is recertified by the main merge or by changing a source witness.

## Historical Acceptance Follow-Up At 55f38f1

The dispatch repair keeps the summary/count/filtered checks on the original raw
log. Its [transport-aware per-test checker](../../../.github/workflows/scripts/test/check-schema-residency-dispatch.sh)
requires every selected name exactly once, accepts one balanced development
frame only after a selected test's exact libtest label, and counts that test only
when libtest's actual next line is exactly `ok`. It never invents an outcome or
counts text inside the JSON as a result. The existing secure fixture owner adds
27 positive/refusal controls for framing, unknown/duplicate/missing names and
failed, missing or inexact outcomes. Their execution was outstanding at that
checkpoint; the supplied later `bae1157` handoff records successful execution.
Current review-repair results are distinguished in the supplement above.

At that historical checkpoint, Task 8's pure-core consumer, Task 9's final hosted matrix and independent review had not yet been accepted. Its old task-number status field is preserved only in the byte-pinned 081 record; active producers and post-repair consumers use `matched_reconstruction_complete`. The plan has Tasks 1-9, not a Task 10. Current delivered results and review decisions are recorded separately rather than retroactively changing that checkpoint.

Target physical/allocator and actual linked-artifact growth facts remain #72-#75; validation cache/scratch/search/work/stack remain #47; #48 and host/provider boundaries are unchanged. No adapter fit, guest readiness, native-only retirement or serving promotion is established. Existing reservations, limits and accounting remain unchanged. The [historical catalog captures](catalog-baseline-continuity-20261021.md) keep their original bytes and scope.

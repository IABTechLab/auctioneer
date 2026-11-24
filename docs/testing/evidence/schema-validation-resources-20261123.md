# Schema Validation Resource Implementation Evidence

## Scope And Status

Issue [#47](https://github.com/IABTechLab/auctioneer/issues/47), implementing the [specification](../../superpowers/specs/2026-11-22-schema-validation-resources-design.md) and [plan](../../superpowers/plans/2026-11-23-schema-validation-resources.md) merged by [#267](https://github.com/IABTechLab/auctioneer/pull/267) at `5fc32dcb9cc49d610f744113f60fabddb21e0aab`. This record begins with the implementation-base inventory, not a completed common proof or target qualification. No production, limit, dependency, reservation or serving change is authorized by this record.

Local verification remains source/diff inspection only. Cargo, compiler, tests, Clippy, executable shell controls and mutation checks run through GitHub's existing owners, not locally. On 2026-11-24 the maintainer separately permitted pinned rustfmt as an editing step on changed files; its successful editing is not product or proof verification. Hosted results below were executed by GitHub Actions; they are not author-executed local tests. The first handoff checkpoint at `82ff24e` deliberately left its refusal checks unimplemented to obtain assertion RED. The subsequent data-correspondence checks do not construct descriptor authority or issue a proof, cost, report or serving receipt. The actual sealed source/body/context join and remaining common derivations are still outstanding.

## Exact-Base Hosted Baseline

All 25 checks at the implementation base `5fc32dcb9cc49d610f744113f60fabddb21e0aab` completed successfully. Read using `gh api repos/IABTechLab/auctioneer/commits/5fc32dcb9cc49d610f744113f60fabddb21e0aab/check-runs?per_page=100`: exit 0, `total_count: 25`, every check `status: completed`, `conclusion: success`.

The [compiler-schema job](https://github.com/IABTechLab/auctioneer/actions/runs/45331234582/job/138232689975) supplies the default and development baseline. Actual principal result lines:

```text
test result: ok. 840 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 410.98s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 719 filtered out; finished in 84.31s
```

The first is the normal explicit source-conformance owner; the second is the `insecure-bidders` residency-prefix owner. Their populations are 846, not source-reference counts. Normal metadata checks in each feature lane ran two cases with 844 filtered out; staged metadata checks ran two with 839 filtered out, from their distinct 841-case graph. The temporary complete `--list` output was not uploaded by that run; do not present a reconstructed execution roster as that missing artifact.

The default owner preserves these six exclusions, each independently executed by its existing dedicated job with one passed, 845 filtered out and no failed, ignored or measured cases:

| Exact Case Under `tests::` | Hosted Owner |
| --- | --- |
| `actual_application_artifacts_axum_retain_every_selected_validator_in_both_feature_lanes` | [application artifact axum](https://github.com/IABTechLab/auctioneer/actions/runs/45331234582/job/138232690016) |
| `actual_application_artifacts_cloudflare_retain_every_selected_validator_in_both_feature_lanes` | [application artifact cloudflare](https://github.com/IABTechLab/auctioneer/actions/runs/45331234582/job/138232690236) |
| `actual_application_artifacts_fastly_retain_every_selected_validator_in_both_feature_lanes` | [application artifact fastly](https://github.com/IABTechLab/auctioneer/actions/runs/45331234582/job/138232689958) |
| `actual_application_artifacts_spin_retain_every_selected_validator_in_both_feature_lanes` | [application artifact spin](https://github.com/IABTechLab/auctioneer/actions/runs/45331234582/job/138232690193) |
| `compiler_private_layout_facts_cover_native_and_every_guest_abi` | [schema source private-layout](https://github.com/IABTechLab/auctioneer/actions/runs/45331234582/job/138232690224) |
| `selected_schema_probe_links_and_executes_the_source_bound_cohort` | [schema source selected-schema](https://github.com/IABTechLab/auctioneer/actions/runs/45331234582/job/138232690481) |

The [secure job](https://github.com/IABTechLab/auctioneer/actions/runs/45331234582/job/138232690040) also reported:

```text
libtest summary controls: 18 passed
schema residency dispatch controls: 27 passed
```

These are baseline results, not RED/GREEN evidence for added validation controls. The target remains explicitly owned because Cargo declares `test = false`. The new development prefix must execute before its first checkpoint can claim RED; zero selected, missing required cases, duplicate execution or wrong frame/filtered counts must refuse.

## Source Boundary And Retirement Inventory

The implementation base has the same production/source-conformance tree as approved head `8eed6f7bc04abf8db6a76c99cf520d02a8d34f63`. The baseline descriptor identifies 18 public definitions, 16 effective schema declarations, five program owners and 51 validation-handoff children. Those figures do not select membership or assert an executed-test count. Current-source controls must derive the exact sets from discovery and the sealed capture.

At this base, `schema_source_conformance.rs` has one `assert_selected_cache_heap` caller in the selected-schema artifact owner. That assembly calls `selected_cache_bundle` and writes `native_cache_heap_candidate` with kind `conditional_native_schema_cache_heap_candidate`. The manifest-only qualification-facing program enumeration is in `selected_half_search_certificates_cover_five_actual_owners_in_both_lanes`. Those replacements and their callers must be deleted when the common consumer is adopted, in this implementation PR; no translator, compatibility reader or failed-proof fallback is permitted. Other `selected_regex_patterns` callers remain independent component/observation controls, not another qualifier. Current guidance in `docs/bidders/schema-qualification.md` and `docs/testing/resource-proof-ownership.md` migrates at that cutover; archived bytes remain unchanged.

The existing controlled-native serving path is not being replaced here. It remains the only serving path until the separately qualified cutover owned by #72-#75/#88-#90 deletes it atomically. This implementation must not add a second serving path or use its reservations as proof credit.

## Attempt, Callback And Storage Ownership

Static source inspection at the implementation base establishes the following schedule, not numeric work, storage or frame-byte proof:

- `src/schema/native.rs` performs entry expiry, availability/owner checks, borrowed input measurement, checked cost, budget evaluation, selected fresh-cache validation and exit expiry, in that order. `src/schema/mod.rs` checks the original deadline before admission and debits before calling; false/panic gives no refund.
- Entry expiry, unavailable/unknown owner and input refusal each call expiry once without debit. Budget refusal calls it twice without debit. Normal admitted return, including false or exit expiry, calls it three times. Constructor/validator/drop panic reaches two expiry calls and retains debit. Unwind cleanup and abort are distinct artifact premises; a post-call deadline check does not interrupt synchronous regex work.
- Input measurement borrows recursively and counts values plus object keys. `auctioneer-runtime/src/json_size.rs` counts compact serialization without retaining output. It runs even when aggregate validation work is exhausted. Refused input and removed-impression/error cleanup remain attempted work outside admitted token debit; that work cannot inherit a depth-32 premise merely because input admission refused.
- Existing native input caps are 65,536 compact bytes, 4,096 nodes and depth 32. The actual admitted policy tuple is `(1024,16,128)`; single work is 4,194,304, auction work 67,108,864 and evaluation scratch 524,288 bytes. These are existing source premises, not newly approved coefficients, CPU units or sufficient funding for preflight.
- `schema_staging_build::native_dispatch` constructs one selected declaration cache inside the admitted closure, validates and drops it before returning. Every sibling cache field is constructed even when its branch is inactive. Partial construction, relocation and failure/drop require V ownership; immutable programs and initialization remain C/I.
- Hostname rejects lengths above 255 before constructing its fresh local search cache. Otherwise that cache overlaps the live generated wrapper and sibling caches. Serial callbacks accumulate work without multiplying simultaneous cache peaks. UUID scans uppercase then uses a 16-byte frame decode buffer, not another regex heap. Expanded control flow, not `calls = nodes`, must establish invocation bounds.
- Exchange visits impression scopes serially. Attempted calls, admitted closures and generated-validator calls are different populations; aliases count invocations separately from shared ownership. The bound must use the existing caller bidder/impression caps, not successful evaluations or a hardcoded adapter example.
- Exchange's expiry closure delegates through runtime `dyn Clock`, core `RequestClock` and SDK `MonotonicClock`. Its transient calls occur outside fresh validation caches; retained host state can overlap them. Common derivations require the exact identity and caller prefixes; actual host work/storage/retained-state/frame facts remain mandatory downstream inputs, never zero or fixture `|| false` assumptions.

## Implementation Results

Pending. No new common source completeness, token-policy adequacy, target fit or serving qualification is claimed. Added controls require completed hosted assertion RED followed by GREEN, exact source/feature identities and the unchanged full hosted matrix. Missing common derivations keep #47 open; downstream target facts do not excuse them.

### Execution-Owner Amendment, 2026-11-24

The original workflow's default success gating would skip development metadata after the intended secure assertion failure. The maintainer approved continuing with a narrow scheduling correction: name the compiler-conformance step and run the existing development metadata step on the schema lane after that step fails, while retaining the failed outcome and job status. Cancellation and earlier setup failures still prevent that execution. No `continue-on-error`, new job, matrix, timeout, test exclusion or acceptance waiver is introduced. This is not hosted RED evidence until both intended assertions actually execute.

The dispatch checker now requires `LOG SELECTED_NAMES EXPECTED_FRAMES REQUIRED_NAMES`. Residency explicitly requires one growth frame; validation requires zero. The independent four-name validation roster must be contained in the selected names, and the checker still requires each selected case to execute successfully exactly once. A deleted required case cannot disappear behind a smaller dynamic prefix. Existing growth-marker and summary refusals remain mandatory.

### First Checkpoint Formatting Failure

At tests-first head `82ff24eb56327c5ade06e1bffb4eadfc54ef48ed`, [the hosted formatting job](https://github.com/IABTechLab/auctioneer/actions/runs/45416541343/job/138524184141) ran `cargo fmt --all -- --check` and returned exit 1. Its emitted diff requests only line wrapping in `schema_validation/handoff.rs`; it is an unintended verification failure, not the required assertion RED. The literal hosted diff was applied manually without running a local formatter or changing assertions, committed as `89c1cc5`. The repair needs a new hosted result before it is considered verified. The original test run was retained through the secure/development assertion capture below, rather than cancelling it with an intervening push.

### Hosted Handoff RED, 2026-11-24

[Compiler-schema job 138524183739](https://github.com/IABTechLab/auctioneer/actions/runs/45416541240/job/138524183739) completed both intended feature-lane executions at `82ff24eb56327c5ade06e1bffb4eadfc54ef48ed`. Logs were retrieved with `gh api --allow-escape-sequences repos/IABTechLab/auctioneer/actions/jobs/138524183739/logs` and filtered to test names, assertions and result lines; retrieval/filter exit 0. These are completed assertion failures, not missing symbols or compiler errors.

The normal secure explicit owner, `bash .github/workflows/scripts/test/verify-schema-compiler-conformance.sh schema`, reported:

```text
test result: FAILED. 840 passed; 4 failed; 0 ignored; 0 measured; 6 filtered out; finished in 291.68s
```

The normal development owner, `bash .github/workflows/scripts/test/verify-schema-metadata-development.sh`, first preserved residency execution, then ran the new validation prefix:

```text
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 723 filtered out; finished in 51.22s
test result: FAILED. 0 passed; 4 failed; 0 ignored; 0 measured; 846 filtered out; finished in 0.01s
```

Both failing steps returned exit 101. The target population is 850: secure executed 844 and excluded its same six heavy cases; development residency executed 127 and filtered 723; development validation executed four and filtered 846. Both feature lanes actually executed the same four required names:

```text
schema_validation::handoff::tests::caller_context_is_not_a_regex_child
schema_validation::handoff::tests::every_child_requires_exactly_one_owner
schema_validation::handoff::tests::same_capture_is_required
schema_validation::handoff::tests::zero_regex_declarations_still_require_work_and_stack
```

The assertions saw `left: Ok(())` against, respectively, `Err(ContextCoverage)`, `Err(ChildCoverage)`, `Err(Binding)` and `Err(ChildCoverage)` in each lane. The defect demonstrated is acceptance of nonempty invalid correspondence. The follow-up delegates child coverage to the existing descriptor equality checker and adds exact parent-binding and distinct caller-role/source equality. It neither creates source authority from those data fixtures nor completes the sealed-body join.

Core and staged metadata steps skipped after secure/development failure are not counted as passing verification. The job remains failed, as required; the scheduling amendment only made development assertions reachable. The next implementation head must obtain hosted GREEN and ultimately the complete unchanged acceptance matrix. At this record point, GREEN for the follow-up is pending and no common completeness, policy adequacy or target qualification is claimed.

### Initial Correspondence GREEN

All 25 hosted checks completed successfully at `87fee6137c9d3e99cda20ff60e3df306c59cffc4`. [Compiler-schema job 138543823107](https://github.com/IABTechLab/auctioneer/actions/runs/45421968551/job/138543823107) executed every original handoff case in both normal feature lanes. Actual principal result lines, secure source target then development residency and validation selections:

```text
test result: ok. 844 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 373.24s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 723 filtered out; finished in 63.35s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 846 filtered out; finished in 0.00s
```

Normal metadata also executed two cases with 848 filtered out per feature lane; staged metadata executed two with 843 filtered out per lane. The [formatting/Clippy job](https://github.com/IABTechLab/auctioneer/actions/runs/45421968328/job/138543614790) succeeded at this same head, verifying the earlier formatting correction. This is real GREEN for data correspondence and the full checkpoint matrix, not source-body/work/storage/stack qualification. It does not complete Task 2 or #47.

### Next Source/Composition Checkpoint

The next tests-first checkpoint adds a borrowed, immutable descriptor view and a current-only fixture helper around the existing measurement/reconstruction/descriptor owners. It retains the private capture through callback and postchecks, does not read a historical report or enter catalog-reference reconstruction, and adds no child Cargo build. The reused source resolver still performs its existing offline locked metadata query; this is not a claim of zero subprocesses.

Real-capture tests require pristine declaration bodies from the same compiler expansion and reject seed/options/owner/marker/body-receipt drift. The body constructor deliberately refuses until its same-kernel visitor is implemented. Independent storage, stack and admission-composition models have explicit hypothetical fixtures and refusal stubs; they cannot mint source authority or target costs. Their source integration, complete work derivations and all remaining common obligations are still required. Module registration and the fixed minimum roster precede hosted RED; no new target or job is added. Results for these new controls remain pending.

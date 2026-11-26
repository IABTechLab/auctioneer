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

### Source/Composition Tests-First Checkpoint

The next tests-first checkpoint adds a borrowed, immutable descriptor view and a current-only fixture helper around the existing measurement/reconstruction/descriptor owners. It retains the private capture through callback and postchecks, does not read a historical report or enter catalog-reference reconstruction, and adds no child Cargo build. The reused source resolver still performs its existing offline locked metadata query; this is not a claim of zero subprocesses.

Real-capture tests require pristine declaration bodies from the same compiler expansion and reject seed/options/owner/marker/body-receipt drift. The body constructor deliberately refuses until its same-kernel visitor is implemented. Independent storage, stack and admission-composition models have explicit hypothetical fixtures and refusal stubs; they cannot mint source authority or target costs. Their source integration, complete work derivations and all remaining common obligations are still required. Module registration and the fixed minimum roster precede hosted RED; no new target or job is added. Results for these new controls remain pending.

### Source/Composition Checkpoint Warning Failure

At `350cdfe3a7055a2b7407559f93276a68ee9c3180`, the new storage tests caused a compiler warning rather than valid assertion RED. The [private-layout owner](https://github.com/IABTechLab/auctioneer/actions/runs/45440918239/job/138611454048) and [Axum artifact owner](https://github.com/IABTechLab/auctioneer/actions/runs/45440918239/job/138611454316) both failed while compiling the explicit source target. Actual diagnostic and result:

```text
warning: multiple variants are never constructed
 --> crates/auctioneer-bidder/tests/schema_validation/storage.rs:108:5
error: `auctioneer-bidder` (test "schema_source_conformance") generated 1 warning
error: warnings are denied by `build.warnings` configuration
Process completed with exit code 101.
```

The new negative fixtures already pattern-matched exact refusal variants and fields, but their constructor expressions appeared only inside matching guards. With the refusal-only evaluator stub, those uses do not establish enum-constructor liveness for Rust's dead-code analysis. The correction expresses the same exact expected errors with `assert_eq!` outside the match guards, without warning allowances, weaker checks or a premature evaluator implementation. This is an author-owned checkpoint failure, not proof of the desired behavior and not an external prerequisite. A new hosted run must reach the intended assertion failures before corresponding GREEN implementations proceed.

### Shared-Test Naming Failure

At the warning-repaired head `3b7700ebfccbb6fa7673a228c14413ca08fbd6f5`, the [secure-default owner](https://github.com/IABTechLab/auctioneer/actions/runs/45442850811/job/138618722164) reached `test_isolation` and found the coefficient enum `Axis` in `schema_validation/admission.rs`. That identifier collides with an actual catalogue identity under the existing case-insensitive source guard. Actual result:

```text
test tests::shared_test_sources_use_synthetic_bidder_inputs ... FAILED
production coupling in shared tests: [
    (
        "tests/schema_validation/admission.rs",
        {
            "Axis",
        },
    ),
]
test result: FAILED. 15 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.63s
error: test failed, to rerun pass `-p auctioneer-bidder --test test_isolation`
```

The repair renames only that internal type to `CoefficientDimension`. No bidder input, test name, expected value, guard or allowlist is changed. This author-owned naming failure is separate from the intended tests-first assertions; its correction still needs hosted verification.

### Hosted Source/Composition RED

The [compiler-schema job](https://github.com/IABTechLab/auctioneer/actions/runs/45442850811/job/138618722471) completed both intended assertion owners at `3b7700ebfccbb6fa7673a228c14413ca08fbd6f5`. The secure explicit source target and normal development validation prefix both returned exit 101. Actual principal result lines, secure target, development residency, then development validation:

```text
test result: FAILED. 844 passed; 34 failed; 0 ignored; 0 measured; 6 filtered out; finished in 386.82s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 757 filtered out; finished in 87.18s
test result: FAILED. 4 passed; 34 failed; 0 ignored; 0 measured; 846 filtered out; finished in 22.04s
```

The complete graph has 884 cases. Secure executed 878 and retained the same six separately owned exclusions; the normal development residency prefix executed 127, and validation executed 38. All four original correspondence controls passed. Both feature lanes executed the same newly required populations: four actual-body controls, ten storage cases, nine stack cases and eleven admission cases. The actual-body positives returned `BodiesUnavailable` instead of the complete 16-declaration receipt/visit roster. Storage positives returned `MissingSourceProof` instead of their independent supplied-fact peaks. Stack positives returned `MissingCommonProof(OperationGraph)` instead of checked path/expiry results, and admission positives reached their deliberate refusal stubs. These are assertion failures on missing behavior, not warning/compiler/setup failures or zero selected cases. Source-only schema disposition/work/callback integration remains unimplemented and is not established by these hypothetical controls.

The job kept both failure outcomes and skipped later success-gated acceptance; skipped strict Clippy/staged/core commands are not passing evidence. Separately, the existing compiler-control target reported `337 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` in 600.23 seconds before the source target. That is an existing owner duration, not evidence attributing runtime cost to the new body fixtures. No timeout, exclusion, warning policy or full-suite owner was relaxed. The naming repair and the new GREEN implementations require another complete hosted run.

### Approved Entropy-Service Amendment

On 2026-11-24 the maintainer approved adding an explicit host-entropy requirement, preserving all production behavior, limits and reservations. Source inspection traced fresh hybrid state-map creation under `std` to `HashMap` and per-thread `RandomState` seeds; process-wide initialization does not establish a warm validating thread. The pinned standard library's Linux entropy implementation contains an uncapped interrupted-call retry. The spec/plan now require exact source/feature/service binding and locally derived construction counts, with entropy-body work `N_seed*E_i` separate from local work and expiry bodies. Required transient storage, thread-lifetime retained-state multiplicity, stack and failure facts remain mandatory host inputs. This approval does not establish any of those facts or complete their common caller-side derivations. No runtime warming, hash replacement, dependency, new cache, resource cost or accounting-equation change is made.

### Body/Composition Implementation And Operation Controls

The next implementation uses one shared `visit_bodies` expansion kernel: unchanged captured options/compiler inputs produce pristine borrowed syntax before any mutable collector, and the original graph projection remains. The private checked-body result borrows both its seed and sealed descriptor, verifies parent/manifest/Rust/graph correspondence, checks the exact per-declaration receipts and rechecks snapshots and dependent witnesses. Callback results remain provisional until those checks finish. This establishes correspondence only, not work or completeness from an arbitrary callback result.

Storage, stack and admission now contain their checked supplied-fact implementations. They remain hypothetical: fixture labels cannot construct an actual common proof or install costs. The policy extractor parses the actual native work local with `syn`, independently extracts coefficients and requires its supported AST shape; varied literal, receiver, arithmetic, cfg and decoy controls do not equate local extraction with full coordinator/debit correspondence. Five additional storage boundary cases retain exact state, coverage, release and arithmetic ordering controls. All original positive/negative assertions remain.

Eight operation-recognition controls and one actual-capture consumer are registered before their own hosted tests-first run. The recognizer deliberately returns `ClassifierNotImplemented`; it cannot issue completeness. The actual consumer obtains every body from the sealed join, requires nonempty exact declaration coverage and attempts recognition for those bodies rather than selecting names. Recognition/announcing unresolved primitive obligations is not proving their work, runtime contexts, cleanup or stack. Unit definitions distinguish executed source events, bytes, probes and search processing from AST size, observations, CPU and native tokens. These definitions precede coefficient derivation; no measured maximum supplies a missing coefficient.

The explicit hash-seed service entry is `std::hash::RandomState::new`, including its warm/cold thread-local/platform body. Local map/cache setup, hashing, probing, growth and drop stay common obligations. The actual source recognizer, runtime primitive bounds, complete attempted-work formulas, entropy-site/lifetime proof, canonical report, actual artifact integration and hard-cut retirement remain unfinished. Hosted GREEN for the body/composition implementations and RED for the operation controls are pending at publication of this checkpoint. Full acceptance and review readiness remain blocked on completing and verifying that work, not on accepting the new scaffolds as certification.

### Body/Composition GREEN And Operation RED

At `db63e51bad25c44451f8054a369ebe8e27059d33`, 24 of the 25 hosted checks completed successfully. The [compiler-schema job](https://github.com/IABTechLab/auctioneer/actions/runs/45450803572/job/138647037671) failed on the nine intended operation-recognition assertions in both normal feature lanes. Actual result lines, secure source target, development residency, then development validation:

```text
test result: FAILED. 883 passed; 9 failed; 0 ignored; 0 measured; 6 filtered out; finished in 412.78s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 771 filtered out; finished in 88.42s
test result: FAILED. 43 passed; 9 failed; 0 ignored; 0 measured; 846 filtered out; finished in 46.94s
```

Both failing steps returned exit 101. The complete graph contains 898 cases. Secure executed 892 while preserving the same six separately owned exclusions; development validation executed 52. All eight handoff/body cases, 15 storage cases, nine stack cases and eleven admission cases passed in both lanes. That is GREEN for real body correspondence and conditional composition, not completion of actual primitive work/storage/stack proofs. The shared-test naming repair also passed its hosted owner.

The same eight `schema_validation::operations::tests` cases failed in both lanes: `unclassified_call_refuses_completion`, `changed_branch_multiplicity_changes_the_recipe`, `non_regex_declaration_has_a_nonempty_closure`, `changed_primitive_semantics_is_not_approved_by_rehashing`, `expiry_binding_is_explicit`, `unknown_callback_is_not_a_free_source_edge`, `new_nested_literal_owner_requires_a_supported_proof`, and `cold_entropy_has_exact_identity_lifetime_and_nonfree_obligations`. The actual-capture consumer `schema_validation::tests::current_body_shapes_have_declared_operation_obligations` also failed with `ClassifierNotImplemented`. The failures reached assertions, not compiler/setup refusals or an empty selection. The existing build-control owner separately reported 337 passed in 606.34 seconds. Later success-gated development Clippy and staged acceptance were skipped and are not credited as passing.

The next source recognizer implementation must preserve those controls and obtain hosted GREEN. It may declare unresolved primitive obligations; that alone cannot discharge them or issue common completeness. Complete attempt/caller binding, source-derived primitive work and lifetimes, stack closure, canonical integration, hard-cut retirement and final full-matrix acceptance remain required before review readiness.

### Source Recipes And Follow-up Tests-First Controls

The next checkpoint replaces the operation refusal stub with one owned, scoped recipe for supported emitted functions, call/control occurrences, typed borrowed origins, iterator protocols and cleanup obligations. Work and stack consumers borrow that recipe instead of parsing a second generated-function graph. The initial numeric projection remains deliberately partial: unresolved primitive calls, owned cleanup, guarded/complex pattern decisions and traversals do not become zero. Advisory source review identified failed-guard prefixes, owned-local cleanup, nested pattern decisions and unsupported executable attributes as rejection/unknown obligations rather than permission to undercount. These corrections still require hosted verification.

The same eight operation-test names remain. The actual-body consumer now reports every refused owner, without weakening its all-owners acceptance condition. Handoff controls additionally require mismatched captures/options to refuse before invoking the analysis callback.

Twenty follow-up contract cases are registered before their hosted tests-first execution: six caller/prefix controls, 13 local epsilon-site controls, and one actual public-name/disposition case. Their evaluators remain explicit refusal stubs. Caller fixtures independently vary compact bytes, value/key nodes, depth, attempt/admission populations, repeated branches/searches, clock prefixes and cleanup, retaining unresolved runtime/entropy obligations. The epsilon table binds a supported library AST and supplied epoch geometry; its site ceilings do not discharge NFA access, sparse-set, look, iterator, vector-growth, clone or drop bodies. The public-name census preserves the existing independent loader's typed local-schema-free disposition and requires executable-name/alias routing correspondence, including an independent ancestry-only route refusal. Candidate data cannot construct a sealed descriptor or authorize a source proof.

The fixed minimum validation roster grows from 52 to 72 names in the existing owner. No test target, matrix, exclusion, assertion, limit, reservation, dependency or serving path is removed or relaxed. Pinned rustfmt is used only for editing; local diff/source inspection is not test or proof execution. Compilation, strict lint, actual declaration recognition, the new assertion RED and final acceptance are pending at publication of this checkpoint. Common source completion, actual primitive work/storage/stack derivations, numerical policy/funding, target qualification and review readiness are not claimed.

### Source-Recipe Checkpoint Compilation And Formatting Failures

At `48631fa857cd77997f1a0e6994f1ac3458b9a222`, the [Axum source-artifact owner](https://github.com/IABTechLab/auctioneer/actions/runs/45467509990/job/138706271441) failed compiling the explicit conformance target before assertion execution. The same target is used by the sibling artifact/source owners. Actual diagnostics:

```text
error[E0609]: no field `reference` on type `&syn::Receiver`
  --> crates/auctioneer-bidder/tests/schema_validation/operations/resolve.rs:428:40
error[E0609]: no field `defaultness` on type `&ImplItemFn`
  --> crates/auctioneer-bidder/tests/schema_validation/operations/resolve.rs:504:19
error[E0609]: no field `unsafety` on type `&Signature`
  --> crates/auctioneer-bidder/tests/schema_validation/operations/resolve.rs:508:22
error: could not compile `auctioneer-bidder` (test "schema_source_conformance") due to 3 previous errors
Process completed with exit code 101.
```

These are author-owned uses of older Syn fields, not valid tests-first assertion RED. Locked Syn 3.0.10 represents receivers with `ReceiverKind`, impl defaultness under `modifiers`, and signature safety with the `Safety` enum. The narrow repair must preserve the accepted borrowed-receiver and initializer-signature constraints against those actual representations; it adds no dependency or broader grammar.

The [format owner](https://github.com/IABTechLab/auctioneer/actions/runs/45467489200/job/138706191135) separately returned exit 1 for the long array-pattern layout in `work/epsilon.rs:194`. Explicit pinned editing with both `--edition 2024 --style-edition 2024` produces the exact multiline layout requested by hosted Cargo formatting; specifying the syntax edition alone had retained the older style. This editing result is not credited as hosted formatting acceptance. No warning/lint policy is relaxed. Recognizer GREEN, the new 20 assertion failures, development strict lint and full acceptance remain unestablished at this head.

### Entropy Construction And Cleanup Source Audit

Read-only inspection on 2026-11-24 traced the selected `std` construction path through `meta::Regex::create_cache`, `Core::create_cache`, the hybrid wrapper, `hybrid::regex::Cache::new`, and its forward/reverse `dfa::Cache::new` calls. Each DFA constructor creates one `StateMap`; its `std` alias is `HashMap`, whose default constructor calls `RandomState::new` once. `Core` initializes `revhybrid` to `None`; the extra reverse-prefix constructor belongs to `ReverseInner` and requires its separately proved exclusion. Searches and map clearing do not themselves construct a new seed. These source facts are not yet a checked transitive proof.

For completed constructions, the conditional count is `2 * sum(R_d * C_d) + 2 * H`: `R_d` is the descriptor-bound regex-cache field count, `C_d` the declaration-cache construction count, and `H` the hostname callbacks reaching their local cache constructor. Eager sibling fields count even when never searched. An interrupted meta construction contributes the reached forward/reverse seed-entry indicators, each at most one, with reverse entry implying forward entry. Partial prefixes therefore contribute zero, one or two; completed siblings remain counted. Neither search count nor observed warm-thread behavior supplies this count. Thread/process-retained service state is a separate lifetime requirement, not a per-construction resident sum.

A separate caller audit found a bounded cleanup premise for successfully admitted core inputs with installed construction limits: each materialized scope passes whole-request byte/node/depth checks before scrub/schema processing, and Impression scrubbing only removes TID/shadow fields. Those checked scope dimensions can bound the disjoint Impression subtrees removed by schema filtering. This does not cover arbitrary low-level contexts without construction limits, original borrowed-input preparation/rejection work, or prove the work inside collection/type destruction. The actual source binding must include those admission, transformation, model and cleanup owners before claiming this premise.

The pinned std allocation-failure path can print through an interrupted-write retry; requested-storage bounds do not prove allocation success. The maintainer separately approved an explicit allocation-success premise on 2026-11-24: conditional validation bounds retain normal/error/unwind cleanup but exclude terminal allocation-failure handlers and fatal diagnostics. The spec/plan now require that exclusion and independently matched allocator/source/artifact support before actual target use. No new service boundary, bounded-abort claim, production change or common-completion claim follows. The source/report controls, hosted execution and missing common derivations remain required.

### Actual-Body Recognizer Stack Failure

At `24b556fde37b6e32d50b174924ebd58a880aa8d2`, 24 hosted checks passed, including formatting/Clippy and all four actual application-artifact owners. The [compiler-schema job](https://github.com/IABTechLab/auctioneer/actions/runs/45469232266/job/138712504225) failed in both normal source lanes. The eight operation unit controls individually reported `ok` in each lane, but the actual-body consumer aborted its process before the work/epsilon tests and before a result summary. Actual secure diagnostics, repeated with the same test name in development:

```text
test schema_validation::tests::current_body_shapes_have_declared_operation_obligations ...
thread 'schema_validation::tests::current_body_shapes_have_declared_operation_obligations' (38876) has overflowed its stack
fatal runtime error: stack overflow, aborting
Process completed with exit code 101.
```

Both processes reported `signal: 6, SIGABRT: process abort signal`. Secure announced `running 912 tests`; development announced `running 72 tests`. Neither invocation produced its required final result line, so these headers are not passing/executed population totals. The public-name case printed `FAILED` before each abort, but its deferred assertion diagnostic was not emitted; that alone does not establish its intended refusal reason. Work and epsilon assertion RED remains unexecuted, and actual declaration-recognition GREEN remains unestablished. The independent development residency invocation did complete:

```text
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 791 filtered out; finished in 110.96s
```

The existing compiler-control owner separately completed `337 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` in 758.10 seconds. Later success-gated lint/staged acceptance was skipped, not passed. The actual-body stack overflow is an author-owned recognizer failure, not a production-engine failure, target fact or completed tests-first checkpoint. Repair must preserve every owner/control and the existing stack allowance; no higher stack setting, skip or reduced source population is authorized.

The next repair extracts expression-variant lowering into smaller helpers so unrelated owned temporaries no longer share the recursively entered dispatcher frame, and replaces avoidable whole-operation-tree clones with binding-metadata clones plus moves. It preserves the accepted grammar, operation/child ordering and lexical sites. Exact frame use and the failure's precise machine frame remain unmeasured; hosted execution must establish whether the repair removes the abort. No stack setting is changed.

The mandatory roster adds `operations::tests::deep_boolean_source_preserves_every_occurrence`: an independent emitter-shaped conjunction with 32 ordered calls, 16 per helper, 31 conjunctions, 32 distinct sites, an independently specified comparison ceiling and explicit destruction. These are source-occurrence checks, not a claim that short-circuited calls all execute on one input. Static spec/quality review found no further issues in this delta; pinned rustfmt was editing only. The actual captured-body consumer, the remaining tests-first assertions, explicit lint and full matrix still require a new hosted result. The minimum validation roster is now 73 names, not an executed pass count.

### Source Recognition GREEN And Caller/Epsilon/Disposition RED

At `740d05b0209a3a86883cad768084fa4f27867fc2`, 24 hosted checks passed. The [compiler-schema job](https://github.com/IABTechLab/auctioneer/actions/runs/45477028137/job/138739848880) completed both normal source invocations without a stack abort. The nine operation unit cases, including the deep conjunction, and the actual captured-body consumer reported `ok` in both lanes. The consumer checked the exact nonempty declaration set through the sealed body join; this establishes recognition and correspondence, not the outstanding primitive work/storage/stack derivations.

Actual result lines, secure source target, development residency, then development validation:

```text
test result: FAILED. 893 passed; 20 failed; 0 ignored; 0 measured; 6 filtered out; finished in 479.56s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 792 filtered out; finished in 86.10s
test result: FAILED. 53 passed; 20 failed; 0 ignored; 0 measured; 846 filtered out; finished in 61.18s
```

Both failing source steps returned exit 101. The complete graph has 919 cases: secure executed 913 and preserved its six separately owned exclusions; development validation executed all 73 selected cases. Both lanes reached the same 20 planned assertion failures: the public-name positive returned `Err(JoinNotImplemented)` instead of `Ok(())`, the six work positives returned `Err(UnsupportedContext)`, and all 13 epsilon cases returned `Err(MissingSourceProof)` instead of their independent site tables or typed refusals. The pristine epsilon signature/body comparisons and public-name before-state checks completed before those failures. These are valid tests-first assertion failures, not compilation/setup failure, empty selection or an aborted process.

The existing compiler controls separately reported `337 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` in 763.56 seconds. Later success-gated explicit development lint and staged/core acceptance were skipped, not passed. The next implementations must preserve all positive/negative controls, obtain two-lane GREEN, and retain unknown ordinary primitive costs as incomplete until genuinely derived. No common-source completion, target qualification, numerical policy adequacy or review readiness follows from this checkpoint.

The maintainer approved running source conformance first on 2026-11-24. The subsequent script change moves each existing invocation together with its unchanged result checker in both `schema` and `all` modes. Successful runs still require source conformance, compiler controls and strict lint; an earlier failure leaves later commands explicitly not run. No exclusion, assertion, timeout, job, cache or resource setting changes. The 763.56-second previous compiler-control duration motivates earlier failure feedback, not a measured improvement for this new ordering; hosted results remain required.

### Caller, Epsilon And Public-Name Implementations Awaiting Execution

The next checkpoint implements the twenty contract evaluators whose assertion failures were recorded above. Public-name checks reuse the sealed descriptor graph and independent typed discovery, reject missing/extra/duplicate/misrouted names, preserve positive local-schema-free dispositions, and recheck the same source snapshot before and after analysis. Caller correspondence uses independent AST recipes for native dispatch, budget admission, input measurement, streaming serialization and exchange filtering; other caller identities stay explicitly opaque, not cost proofs.

Conditional recipe counts retain repeated calls, conservative short-circuit operands and checked iterator/constructor multiplicities. A second eagerly constructed but unsearched cache field independently varies cache construction/release from roots and searches. Guarded or-patterns, including nested alternatives, refuse rather than undercount possible guard retries; the existing branch case includes direct, tuple-nested and variant-nested refusal controls plus a supported single-pattern guard. Source cleanup duties borrow the existing binding classifier instead of duplicating its ownership rule. These counts preserve unresolved primitive obligations and are neither work weights nor common completeness.

The thirteen epsilon cases now check the supported source signature/body and checked site arithmetic under explicit supplied geometry. All seven ordinary helper obligations remain required even for a zero-execution evaluation. These site ceilings do not discharge the work inside NFA access, sparse sets, look, iterators, growth, clone or destruction. No primitive prototype or extra test roster is integrated in this checkpoint.

Compilation, the twenty-case GREEN, strict lint, the approved CI command ordering and the unchanged full hosted matrix remain pending at publication. Pinned rustfmt was used solely for editing and source/diff inspection is not executable verification. Actual source-derived primitive/caller/lifetime/stack closure, canonical reporting, four application joins and hard-cut retirement remain unfinished; the PR stays draft and #47 stays open.

### Contract Implementations Reach A Fixture Parser Failure

At `838b4cd477fba689cc08e54143d94a4d1f11d6f5`, the [schema job](https://github.com/IABTechLab/auctioneer/actions/runs/45501069679/job/138824185869) compiled the new evaluators and executed source conformance first. Its secure run completed with one failing case; the separate development residency and validation runs completed too:

```text
test result: FAILED. 912 passed; 1 failed; 0 ignored; 0 measured; 6 filtered out; finished in 752.17s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 792 filtered out; finished in 51.68s
test result: FAILED. 72 passed; 1 failed; 0 ignored; 0 measured; 846 filtered out; finished in 37.83s
```

Both failures returned exit 101 in `work::tests::expiry_prefixes_bound_real_call_sites`, at the same mutation-fixture construction:

```text
panicked at crates/auctioneer-bidder/tests/schema_validation/work.rs:1028:29:
unexpected end of input, expected semicolon
```

Syn 3's standalone statement parser requires a semicolon for that bare expression. The intended mutation retains `result` as the block's tail expression, so the repair constructs `syn::Stmt::Expr(syn::parse_quote!(result), None)` explicitly. It does not add a semicolon, remove the ordering mutation, change its before-state checks, relax the expected budget-source refusal or modify production code. All thirteen epsilon cases, the public-name case and the other five work cases passed in both lanes; the sixth case's positive prefixes ran before this fixture failure, but the whole case is not passing.

The failure occurred before compiler controls in the approved new order. Those controls and later success-gated lint/core/staged checks were not run, not passed. Development source diagnostics still ran under their existing failure condition. The tail-expression repair and unchanged full matrix require hosted verification at the next head. No twenty-case GREEN, common completeness or review readiness is claimed here.

### Primitive And Cache-Source Tests-First Contracts

The next tests-first checkpoint registers twelve NFA-access/sparse-set cases and four cache-source-lifetime cases under the existing source-conformance target and both normal feature lanes. All previous required names remain; the mandatory validation roster grows from 73 to 89. The primitive and cache-source evaluators deliberately remain `MissingSourceProof` and `NotDerived` refusal stubs until completed hosted assertion RED. The modified existing entropy case additionally requires both thread and process retention; its lowerer deliberately retains the older thread-only expectation for that RED. No new target, job, exclusion, source population reduction or limit change is introduced.

Primitive oracles require finite source/control/scalar event bounds for every supported ordinary helper, not all-unknown call-table scaffolding. Independent positive inputs include different root/dispatch populations and different sparse attempt/success/RHS populations. Data/source/receiver/invariant/compiler refusals include self-consistently rehashed owner-field/import/impl/index-macro drift and a signature-only change retaining the body. The builtin single-field equality lemma is documented against the exact compiler revision and vendor declarations. These three event components are separate from the eleven remaining work components, requested buffer extents, target frames and actual selected-program/invariant binding. No instruction, latency, complete-work or target claim follows.

The frozen unit boundary is source-level: compiler-inserted bounds checks within a matched intrinsic indexing operation are grouped under that operation's source visit, not enumerated as separate source predicate/control events. Explicit Rust predicates in matched bodies count normally. Matched-source and valid-ID premises remain mandatory. This clarification changes no unit, weight or count and assigns neither zero CPU cost nor an instruction/latency bound.

The cache-source contract consumes the sealed checked-bodies handoff and its existing typed operation recipe. Its actual-owner oracle independently checks the exact native wrapper and coupled roots, ordered field/full-program/site tuples, completed-sibling prefixes and complete program coverage. It includes a positively identified zero-cache owner, callback/wrapper overlap, malformed initializer order versus an extra constructor, escaping/reset/cloned/rebound-cache refusals, and before/after source/binding checks. This is a source-structure/lifetime prerequisite, not the full storage/cleanup proof or another engine heap model.

The narrow shared changes reuse the existing compiler probe and bounded regular-file reader. A conformance-local `WorkLedger::source_events` constructor keeps fields private and sets only the three supplied event components; all other components remain unproved. Actual rust-src interpretation still requires a consumption-window recheck and correspondence to the existing captured compiler identity, not just consecutive fixture snapshots or matching version strings. The whole-entry entropy contract now explicitly requires shared process retention alongside thread retention, with independent missing-process/thread-only refusals and no multiplication of retained state by seed calls. No new service exemption or serving path is added.

Static specification and quality review cleared these bounded contracts, including repair of a Syn 2-style impl-trait tuple before publication. Pinned rustfmt was editing only. Compilation, actual dispatch, intended assertion RED, subsequent implementations and all final acceptance remain pending; no expected case count is an executed result. The fixing head `c7bfd5c` was still undergoing its separate hosted matrix when this contract was prepared; its completed results are recorded below. Complete common derivations, canonical integration, four application joins and the old-report hard cut are still required before review readiness.

### Twenty Contract Cases GREEN; First Complete Strict Lint Fails

At `c7bfd5cf2997409484a5960abfa3acd47a9d8c6e`, 24 hosted checks passed and the [compiler-schema job](https://github.com/IABTechLab/auctioneer/actions/runs/45504266150/job/138835404692) failed at strict lint. Source conformance ran before compiler controls in the approved order. Actual secure source, compiler controls, development residency and development validation result lines:

```text
test result: ok. 913 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 1039.23s
test result: ok. 337 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 213.31s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 792 filtered out; finished in 86.82s
test result: ok. 73 passed; 0 failed; 0 ignored; 0 measured; 846 filtered out; finished in 64.42s
```

The tail-expression fixture repair therefore passes in both source lanes, and all twenty caller/epsilon/public-name contract cases have completed hosted GREEN. These are contract/correspondence results, not the still-missing complete common derivations. Both source and compiler-control tests ran successfully; their different durations do not establish a comparable performance improvement.

The now-reached explicit Clippy invocations failed with exit 101 in both normal feature lanes:

```text
error: could not compile `auctioneer-bidder` (test "schema_source_conformance") due to 320 previous errors
```

Default lint covered both `schema_build_controls` and `schema_source_conformance` with `--locked -p auctioneer-bidder -- -D warnings`; development lint explicitly selected `--features insecure-bidders --test schema_source_conformance`, also with `-D warnings` and the artifact-expectation variable unset. Diagnostics include shadowed bindings, exhaustive-pattern/style requirements, scoped field visibility, checked arithmetic, explicit numeric types, oversized functions and parent modules requiring `mod.rs`. They are author-owned failures, not valid tests-first RED or an external verification blocker. Later success-gated core/staged steps were skipped, not passed.

The repair preserves every assertion, independent literal, refusal and test name, uses ordinary directory modules with corrected data-include paths, and retains the same lint policy. No lint allowance, hidden cfg, ignored case, test/feature reduction or resource-limit change is authorized. Hosted verification must establish the repairs; static review and pinned formatting are not substitutes.

### Tests-First Checkpoint Stops At A Redundant Import

At `29b442a2dc8a7f1b5f8579f4fe66428e8b2afdbb`, the [private-layout job](https://github.com/IABTechLab/auctioneer/actions/runs/45512666791/job/138864077230) and [Axum artifact job](https://github.com/IABTechLab/auctioneer/actions/runs/45512666791/job/138864077495) both failed compiling the shared source-conformance target with exit 101:

```text
warning: unused import: `quote::ToTokens`
--> crates/auctioneer-bidder/tests/schema_validation/work/epsilon/mod.rs:299:9
error: `auctioneer-bidder` (test "schema_source_conformance") generated 1 warning
error: warnings are denied by `build.warnings` configuration
```

The outer module already provides the required trait scope; the child module's duplicate import is unused. The repair removes only that child import, retaining the outer import, every case, oracle, source recipe and the existing warning policy. These failed compilations produced no executed source-conformance result and establish no intended primitive/cache/entropy assertion RED. The checkpoint's new controls, lint repairs and unchanged full matrix still require hosted execution at the fixing head. This author-owned setup failure is not waived or relabelled as missing target evidence.

### Primitive/Cache/Entropy Execution And Separate Fixture Failures

At `50eca83fcb29d9899ece1e4f0a7b854eae200f1d`, the [compiler-schema job](https://github.com/IABTechLab/auctioneer/actions/runs/45513485379/job/138867011966) compiled and executed the source target. The secure script `bash .github/workflows/scripts/test/verify-schema-compiler-conformance.sh schema` and development script `bash .github/workflows/scripts/test/verify-schema-metadata-development.sh` both returned 101. Actual secure source, development residency and development validation results:

```text
test result: FAILED. 912 passed; 17 failed; 0 ignored; 0 measured; 6 filtered out; finished in 731.73s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 808 filtered out; finished in 55.89s
test result: FAILED. 72 passed; 17 failed; 0 ignored; 0 measured; 846 filtered out; finished in 45.04s
```

The graph contains 935 cases; secure executed 929 with the same six separately owned exclusions, and development validation executed all 89 required cases. Both lanes reached fourteen intended assertion failures: nine primitive cases returned `MissingSourceProof`, four cache-source cases rejected their positive schedules through the `NotDerived` stub, and the entropy case returned `ServiceLifetime(HostEntropy)` instead of accepting the combined process/thread contract. This establishes their missing-behavior RED, not completed source proofs or sensitivity of later negative assertions.

The other three failures are author-owned fixture failures, not valid missing-behavior RED: `nfa_access_matches_pristine_recipes_and_literal_counts`, `sparse_insert_matches_pristine_recipes_and_branch_counts`, and `known_helper_semantic_changes_are_refused` fail in `assert_index_edges` at `primitives.rs:908`, with zero matching index-template items versus the required one, before reaching the evaluator. The exact independent macro-template oracle must be repaired and rerun; its count assertion, source census and supported semantics must not be weakened. Compiler controls and later strict-lint/core/staged acceptance were skipped after source failure, not passed. The prior warning repair compiled successfully, but the earlier 320 strict-lint repairs still lack a reached current-head lint result.

Separately, the existing isolation target failed in all three full native lanes because the synthetic string `supplied-epsilon-owner` contains a real catalog identity. The guard reported only `tests/schema_validation/work/epsilon/primitives.rs` with identity `epsilon`. Actual secure, default-workspace and insecure-workspace result lines, respectively:

```text
test result: FAILED. 15 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.74s
test result: FAILED. 15 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.47s
test result: FAILED. 15 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.10s
```

The [secure](https://github.com/IABTechLab/auctioneer/actions/runs/45513485379/job/138867012149), [default](https://github.com/IABTechLab/auctioneer/actions/runs/45513485379/job/138867011907) and [insecure](https://github.com/IABTechLab/auctioneer/actions/runs/45513485379/job/138867012116) jobs returned 101; the two workspace commands retain `--no-fail-fast`. The repair changes only that synthetic label to `fake_closure_owner`, preserving all paired owner/epoch comparisons and the unchanged isolation guard. Source/protocol names remain genuine. Its hosted GREEN is pending; no test, warning or ownership policy is waived.

The next narrow checkpoint also repairs the index-template oracle and implements the demonstrated entropy requirement. Source inspection of locked proc-macro2 1.0.107 and quote 1.0.47 establishes that the source lexer preserves the `<` before `$name` as joint punctuation, while a literal `<` emitted by `quote!` is alone. All four independent expected items now interpolate that single explicit joint punctuator. Actual tokens are not normalized; exact whole-item equality and count one remain mandatory, and mismatch diagnostics add the expected and actual items. The primitive evaluator remains a refusal stub until those three repaired cases execute genuine missing-behavior RED.

For entropy, the lowerer now requires `ThreadAndProcess` and the separate `ProcessRetainedState` fact in addition to every prior host fact. The exact service entry, clock caller scope and cache-construction transient overlap remain unchanged. Static specification and quality reviews cleared both narrow deltas; only pinned rustfmt ran as an editing step. Their compilation, hosted GREEN/RED and strict-lint acceptance remain pending. Cache-source derivation is a separate in-progress implementation after its four executed REDs, not part of this narrow publication.

### Entropy/Fixture GREEN And All Primitive Contracts Reach RED

At `49882230f9804b073091e7f65cf1bc7e1c6532d0`, the [compiler-schema job](https://github.com/IABTechLab/auctioneer/actions/runs/45517049524/job/138879199064) completed both source invocations without an index-oracle failure. Every one of the twelve primitive cases now reaches `assert_local` and returns `Err(MissingSourceProof)` instead of its finite independent contract. All four cache-source cases retain their intended `NotDerived` failures. The entropy case reports `ok` in both lanes, including its independent missing-process and thread-only refusals. Actual secure source, development residency and development validation lines:

```text
test result: FAILED. 913 passed; 16 failed; 0 ignored; 0 measured; 6 filtered out; finished in 681.40s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 808 filtered out; finished in 53.47s
test result: FAILED. 73 passed; 16 failed; 0 ignored; 0 measured; 846 filtered out; finished in 44.25s
```

Both source scripts return 101. This establishes the repaired oracle's before-state checks and valid twelve-case missing-behavior RED, not primitive implementation or complete work bounds. The source-first secure failure skips compiler controls and later strict lint; development also stops before its strict-lint command. No new reached strict-lint success is claimed.

The unchanged isolation guard passes after the synthetic-label correction. The complete [secure lane](https://github.com/IABTechLab/auctioneer/actions/runs/45517049524/job/138879199322) and [insecure workspace lane](https://github.com/IABTechLab/auctioneer/actions/runs/45517049524/job/138879198966) both returned zero; their actual isolation-target result lines are:

```text
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.16s
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.93s
```

At recording, the default-workspace and evidence-producer jobs were still running; those pending statuses are not passes. Every required final-head check remains mandatory. Cache-source implementation and new primitive implementations still require independent static review and their own hosted GREEN; actual common closure, canonical reporting and hard-cut retirement remain incomplete.

### Cache-Source Derivation Awaiting Hosted GREEN

After the four executed cache-source REDs, the next bounded implementation replaces `derive_owner`'s refusal stub. It consumes the existing typed operation recipe: exact factory/wrapper/validation linkage, complete unique initializer program identities in lexical order, preceding constructed-sibling prefixes, and validation-reachable hostname/UUID callbacks. Unknown cache-bearing callbacks, extra constructors, cache escape, state-changing reset, cloning and rebinding refuse. The selected source-empty reset is accepted only with its exact receiver identity and empty body, not by a name exemption or zero-cost assumption.

The actual-source entry still performs fresh classification inside the sealed checked-bodies handoff; supplied or forged recipe data cannot become an actual-source receipt. All four unit cases, independent oracles, names and refusal variants remain unchanged. Separate static specification and quality reviews cleared this bounded implementation; source-file SHA-256 was `4f0881669f19a0bd7c230bc1c13af9001cde32cb692774ae64bc87d1dc60e8ec`. Pinned formatting was editing only. Compilation, the four-case GREEN, strict lint and full hosted acceptance remain pending. Ordered prefixes retain partial-failure cleanup obligations; they do not price or prove destructor work, release, complete heap bounds or target frames. Primitive implementation, logical stack/source closure and final integration continue separately.

### Cache-Source Contracts GREEN In Both Lanes

At `ad62c936b9c38f8cb3d31b0ae9b7d898cae6da4a`, the [compiler-schema job](https://github.com/IABTechLab/auctioneer/actions/runs/45520296628/job/138890145161) reports all four `storage::source` cases `ok` in both source lanes, including actual discovered owners, initializer ordering/partial prefixes, cache-effect refusals and zero-cache wrapper/callback duties. Actual secure source, development residency and development validation result lines:

```text
test result: FAILED. 917 passed; 12 failed; 0 ignored; 0 measured; 6 filtered out; finished in 1050.06s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 808 filtered out; finished in 87.99s
test result: FAILED. 77 passed; 12 failed; 0 ignored; 0 measured; 846 filtered out; finished in 77.38s
```

Both source commands still return 101 because the twelve primitive evaluators retain their intended refusal stub. No other source case fails. The four cache-source REDs therefore have executed two-lane GREEN; this is schedule/correspondence evidence, not complete heap, destructor work or common-source completion. Compiler controls and later strict-lint/core/staged commands skipped after source failure remain not run, not passed. Full final-head acceptance remains mandatory.

### Captured Logical-Stack Tests-First Contract

The next checkpoint retains all nine original stack cases and adds four `stack::source` cases under the existing target. The exact required validation roster retains all prior 89 names and adds these four, for a minimum of 93 names; this is a static roster size, not an executed count. `stack.rs` moves to the ordinary directory `stack/mod.rs` with the same namespace and only a new `mod source;` declaration.

The new contract consumes the sealed body handoff and existing Recipe without another parser. Independent oracles require complete source-anchored path/duty coverage, exact roots/call sites, argument evaluation before callee entry, nested versus serial/alternative paths, callback versus inline-loop boundaries, and distinct source-cycle/missing-callee versus target-frame failures. Omitted-path/duty, unattached-cleanup and wrong-active-caller controls guard the oracle itself. A positively classified clock case requires exact service binding, site and active-path retention. Its operations-owned `#[cfg(test)]` fixture produces data only, preserves private service fields and does not supply host facts or source authority.

Separate static specification and quality reviews cleared the repaired contract after finding and correcting missing path-coverage and service-positive controls. All original cases/formulas/refusals remain unchanged, every frame-byte slot stays unknown, and helper/protocol/cleanup closure remains unproved. The new shape deriver deliberately remains `Err(NotDerived)` until genuine hosted assertion RED. No job, target, exclusion, command, source population, resource limit or runtime behavior changes. Compilation, dispatch, intended RED, subsequent implementation and strict lint remain pending; pinned rustfmt was editing only.

### Finite Primitive Implementation Awaiting Hosted Execution

After the twelve genuine assertion REDs at `4988223`, the next implementation replaces the primitive refusal stub with finite supported-source recognition and a checked transitive callee table. The existing module moves to `work/epsilon/primitives/mod.rs`; `source.rs` owns source recognition and `events.rs` owns the separate call/event derivation. All twelve test identities, existing literal count oracles, receiver/invariant/compiler controls and refusal expectations remain. Additional rehashed controls reject const-qualifier drift and local `assert!` shadowing through macros or imports.

The implementation follows the selected NFA/SparseSet helpers through their concrete Arc, Vec, pointer, slice-index and StateID helpers. It derives only source visits, control visits and scalar comparisons; the other eleven WorkLedger components remain unknown. Requested byte extents, target frame sizes, actual selected-program invariants, builtin/prelude resolution and the authentic compiler/source consumption window remain separate unproved obligations. Matching supplied files or hashes grants no actual-source authority, complete common proof, numerical policy acceptance or target qualification.

Static specification review found two material issues before publication: locked Syn preserves const trait impls as `Item::Verbatim`, and local macros can shadow the assumed builtin `assert!`. The repaired recognizer uses a narrow fallible token projection retaining const/unsafe qualifiers, generic bounds, where clauses and selected typed members; it does not erase const syntax to manufacture an ItemImpl. The local macro/import scope refuses the reported shadowing shapes. Broader crate/prelude resolution is explicitly still required from the actual source owner. Independent specification re-review and quality review found no further concrete defects in this bounded implementation. Those are static reviews, not executed checks; pinned rustfmt was editing only.

Compilation, the twelve-case GREEN, reached strict source-target Clippy and complete final-head acceptance remain pending. The four stack-source controls published at `4e0d72d` are a separate tests-first contract, not an implemented stack proof. No dependency, job, target, ignored case, assertion, resource limit, engine, serving path or accounting equation changes in this primitive checkpoint.

### Four Stack-Source Contracts Reach Assertion RED

At `4e0d72dc778acb40f50c2176fe83fe564156a8f5`, the [compiler-schema job](https://github.com/IABTechLab/auctioneer/actions/runs/45523986811/job/138902757730) compiled and executed all four new stack-source cases in both feature lanes. Each fails its intended positive comparison with `Err(NotDerived)`, at `stack/source.rs:580`, `:809`, `:984` and `:740` respectively, rather than a setup, source-classification or compilation failure. The other twelve failures are the still-published primitive `MissingSourceProof` stubs. Actual secure source, development residency and development validation result lines:

```text
test result: FAILED. 917 passed; 16 failed; 0 ignored; 0 measured; 6 filtered out; finished in 1053.26s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 812 filtered out; finished in 90.45s
test result: FAILED. 77 passed; 16 failed; 0 ignored; 0 measured; 846 filtered out; finished in 83.92s
```

The graph contains 939 cases. Secure source executed 933 with the same six exclusions; development validation executed all 93 required names and filtered the other 846. Both source scripts returned 101. Compiler controls and explicit strict source-target lint were skipped after secure source failure; development lint was also not reached. Later success-gated core/staged steps were skipped, not passed. At recording, the evidence-producer job was still running; every other hosted check except compiler-schema had passed. No final-matrix success is claimed.

This genuine four-case RED permits implementing the finite logical source-shape derivation. It does not establish later negative-control sensitivity, helper closure, target frame bytes or complete stack bounds. The separately reviewed primitive implementation and the stack implementation still need their own hosted GREEN and the unchanged complete final-head acceptance matrix.

### Captured Stack-Path Derivation Awaiting Hosted GREEN

The next bounded implementation replaces `stack::source::derive_shape`'s refusal stub using the canonical Recipe. It inventories function/callback frame owners, call sites and primitive/service/cleanup duties; separately checks missing callees and cycles; and derives complete active-path data. Arguments are evaluated before callee entry, alternatives and serial calls retain separate activations, and callback protocols remain active through their body and cleanup rather than being treated as inline loops. Opaque helper obligations remain open and every physical frame-byte slot stays `None`.

All four source-stack test/oracle blocks are byte-identical to the tests-first head, and the nine prior stack cases/formulas remain unchanged. Separate static specification and quality reviews found no concrete defects in this finite shape implementation, whose source SHA-256 is `45ca64af7cd46248739ae4dc5a4bd8c5c2f33df3c74c207447530cd5eb66e4d7`. Pinned rustfmt was editing only. This is not transitive helper closure, target-frame evaluation or source-completeness authority. Compilation, four-case GREEN, strict target lint and complete final-head acceptance remain pending.

The `4e0d72d` hosted matrix subsequently settled with 24 successful checks and only compiler-schema failing on the sixteen recorded intended assertions. Those results belong to that predecessor, not the new implementation head.

### Source-Window, Rejected-Prefix And Seed Tests-First Contracts

The next checkpoint retains all 93 required validation names and adds eleven controls under the same existing target: four source-window, four rejected-prefix and three entropy-construction cases. The literal required roster contains 104 names; this is a static ownership count, not executed dispatch. No command, exclusion, checker, feature lane, target, job, timeout, assertion or resource limit is removed or relaxed. The `handoff.rs` directory move preserves the old namespace and assertions, adding only the ordinary source module; work adds ordinary prefix/entropy modules with inline owner-local tests.

The source-window contract requires expected compiler identity from the sealed manifest, the fixed twelve primitive source units, real bounded probes/regular-file readers and integrity rechecks around interpretation, including returned errors. Independent controls cover path/verbose/probe failures, missing/duplicate/extra sources, wrong paths, unreadable files, hash-only corruption and self-consistently rehashed post-read byte drift. A guarded control invalidates one disposable build-local schema snapshot during either successful or refused interpretation; both must return `Binding` and restore the valid snapshot. Containment, private-directory, regular-file, single-link and inode checks precede that mutation. Serving, vendor and rust-src files are never modified. Event traces and supplied read data cannot replace actual seed/runtime checks or approve source semantics.

The rejected-prefix contract keeps every new bound slot explicitly unknown until genuine hosted RED. Literal controls distinguish values from object keys, failing node attempts from successful charges, quoted raw length from escaped virtual output, iterator progress from container depth, and repeated work from simultaneous stack depth. Counts use reached preflight attempts, including refusals with zero debit; they are not successful SchemaInputStats. The raw UTF-8 plus quote quota covers only strings/keys passing the precheck and reaching serialization, including a final serialization whose byte charge fails. Counter `1/0/0` controls describe writes per nonempty/empty request and interrupted retries, not serde's request count. Serializer/helper work, map navigation/height and ordinary error/unwind cleanup remain explicitly unproved.

The entropy contract separately requires the constructor/default/std-import chain and ordered forward/reverse seed sites, eager inactive siblings, guarded hostname creation, supported strategy scope, zero-invocation obligations and checked overflow. Literal potential prefixes are `0/1/2` then `2/3/4`; two generated cache creations plus three hostname cache creations give a seed-entry ceiling of `10`. The counted entry is the whole `std::hash::RandomState::new`, not OS calls; existing operations-owned whole-service/lifetime tests are not duplicated. Ordinary map setup, hashing, growth and cleanup remain open. Branch labels, feature/compiler records, hashes and supplied Recipe/schedule data grant no actual-source authority. Every ledger component remains unknown here.

The source-window and entropy evaluators remain refusal stubs; prefix attachment is a no-op retaining `None` rather than inventing zeros. Static reviews identified and strengthened probe-error, hash-only, genuine post-capture/error-path checks and the raw-quota scope before publication. Pinned rustfmt is editing only. Compilation, all eleven intended assertion REDs, later derivations/GREEN, strict target lint and full final-head acceptance remain unexecuted at preparation. No complete common proof, numerical policy adequacy, target qualification, serving activation, review readiness or issue closure is claimed.

### Shared Compilation Failure Before New Assertion Execution

At `825bd50fe89576c550e8e0b70f395e68afaf3755`, the [private-layout job](https://github.com/IABTechLab/auctioneer/actions/runs/45531081376/job/138926248122) and [Axum artifact job](https://github.com/IABTechLab/auctioneer/actions/runs/45531081376/job/138926247874) independently failed compiling the shared source target with exit 101:

```text
error[E0432]: unresolved import `syn::visit`
--> crates/auctioneer-bidder/tests/schema_validation/work/epsilon/primitives/source.rs:16:10
the item is gated behind the `visit` feature
error: could not compile `auctioneer-bidder` (test "schema_source_conformance") due to 1 previous error
```

The root workspace enables Syn's `full` feature and bidder dev dependencies already enable `visit-mut`, not `visit`. The new primitive diagnostic walker incorrectly selected the unavailable API. The narrow repair uses the existing `VisitMut` API over a private AST clone; it changes only unknown-call diagnostic traversal, not supported source recognition, event arithmetic, assertions, features or dependencies. This is an author-owned compilation failure, not intended tests-first RED. The eleven new controls, primitive/stack implementations, strict lint and full acceptance still require hosted execution at the fixing head. No local executable check was run or acceptance waived.

### Synthetic Syn File Constructor Requires Its Current Field

At `185f8cb3bddffb931ce8c2887188926b7de617f0`, the [Spin artifact job](https://github.com/IABTechLab/auctioneer/actions/runs/45531703509/job/138928472001) and [selected-source job](https://github.com/IABTechLab/auctioneer/actions/runs/45531703509/job/138928472020) independently failed compilation with exit 101:

```text
error[E0063]: missing field `frontmatter` in initializer of `syn::File`
--> crates/auctioneer-bidder/tests/schema_validation/work/epsilon/primitives/source.rs:910:16
error: could not compile `auctioneer-bidder` (test "schema_source_conformance") due to 1 previous error
```

The visitor import failure no longer appears; compilation reaches the synthetic file used to inspect the index-macro implementation. The one-line repair initializes `frontmatter: None`, matching locked Syn 3.0.10's own `Parse for File`. Its current `parse_file` handles BOM/shebang then delegates to that parser; the empty non-exhaustive frontmatter marker is not populated by this source path. No source header is erased or newly accepted, and no unsafe or fabricated frontmatter fixture is added. Source inspection found only this new File struct literal in the validation owner. Recognition, event arithmetic, source populations, assertions, dependency features and limits are unchanged. This second author-owned compile failure establishes no new assertion RED or implementation GREEN; all required hosted execution remains pending at the fixing head.

### Logical Stack GREEN And Eleven New Contracts Reach RED

At `6c482079103b02144e77cc8a1ced358f8e191174`, the [compiler-schema job](https://github.com/IABTechLab/auctioneer/actions/runs/45532522843/job/138931211680) compiled and completed both source lanes. The approved source-first ordering is in effect. Both scripts returned 101; actual secure source, development residency and development validation result lines are:

```text
test result: FAILED. 921 passed; 23 failed; 0 ignored; 0 measured; 6 filtered out; finished in 1065.14s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 823 filtered out; finished in 88.65s
test result: FAILED. 81 passed; 23 failed; 0 ignored; 0 measured; 846 filtered out; finished in 113.66s
```

The target graph contains 950 cases: secure source executed 944 with the unchanged six separately owned exclusions; development executed all 104 validation cases. All four `stack::source` cases report `ok` in both lanes. This establishes their tests-first RED-to-GREEN progression, not complete transitive helper/frame closure. All eleven new cases reach intended positive assertion failures in both lanes: four source-window and three entropy cases return `MissingSourceProof`; four prefix cases retain `None` rather than their independent literal bounds. Their positive setup compiled and executed. Later negative assertions are not yet sensitivity evidence.

The remaining twelve failures are an implementation defect, not intended stub RED: all primitive cases return `UnsupportedBody(Nfa)` at their positive supported-source comparison. Static diagnosis locates an asymmetric optional-trailing-comma projection in `state_variants`: actual enum/field lists are rebuilt without trailing punctuation, while the expected recipe retains it. The repair must apply the same documented syntactic policy to both sides, preserve semantic matching and all independent event oracles, and obtain hosted GREEN; this diagnosis is not an executed repair result.

All 24 other hosted checks completed successfully at this exact head. The failed source step still skips compiler controls and strict source-target Clippy, and the development failure skips its strict lint and later normal/staged checks. Those skipped commands remain not run, never passes. Full final-head acceptance and a reached strict-lint result remain mandatory. Source-window, prefix and entropy implementation, complete work/storage/stack closure, canonical reporting and hard-cut retirement remain unfinished; the PR stays draft and #47 open.

### Next Bounded Implementation And Traversal Contract

After the executed source-window REDs, the real-I/O facade and shared private kernel now bracket interpretation with compiler, fixed-source, seed and runtime checks, including returned interpretation/read/probe errors. Integrity refusals precede consumer errors; a composite source-read/compiler failure control pins compiler precedence. Source identity still supplies no semantic/cost or completion authority. Hosted four-case GREEN and strict lint remain pending.

The primitive repair applies the documented declaration-list punctuation policy symmetrically to the actual and expected State enum. Three additional expected helper recipes now use the exact pinned Vec/Unique body punctuation, without adding global method-token normalization. Semantic matching, all twelve case names and independent event-count oracles remain unchanged. Supported prefix arithmetic now fills ten node/iterator/byte-charge/nesting ceilings using checked calculations; raw-string, Counter-protocol and tree-navigation slots stay unknown. All four prefix oracle blocks remain unchanged, so this partial derivation is not a claim of their GREEN or complete serializer/cleanup work.

Three reviewed tests-first traversal cases join the existing target and minimum development roster, retaining all prior 104 names for 107 static required names. They distinguish disjoint borrowed-child traversal across helper arguments from ancestor rescanning, vary cap seven to eleven while observed `J` remains four, and reject changed source bindings at both one and zero attempts. Literal coefficient/witness and overflow controls remain independent of the deriver. The new deriver deliberately returns `NotDerived` until genuine hosted assertion RED. No old counter is exposed as a fallback. Static review and pinned formatting do not substitute for compilation, executed RED/GREEN, strict lint or the unchanged full hosted acceptance matrix; none is claimed for this checkpoint at preparation.

### Exact Helper Templates Conflict With Formatting

At `102bf2f957d46afe098f6c69962a18da295e02ff`, the [format job](https://github.com/IABTechLab/auctioneer/actions/runs/45538145674/job/138949745528) failed `cargo fmt --all -- --check` with exit 1. Its actual diff adds trailing commas to the three authored expected templates: both Vec `aggregate_raw_ptr` argument lists and Unique's `_marker` field. No source-test result follows from this formatting failure; the separate source job was still executing when it was recorded.

The repair constructs these expected expressions as typed Syn ASTs, rebuilds only their expected argument/generic/field lists, and interpolates each complete expression into its existing recipe. This preserves the exact expected tokens while allowing rustfmt to edit the authored Rust. Actual method ASTs, event/count oracles, source acceptance and all tests remain unchanged; no formatting exemption or broad method-body normalization is introduced. Pinned rustfmt is editing only. Hosted format and source acceptance remain pending at the repairing head.

### Source Window GREEN And Traversal Contracts Reach RED

The completed [compiler-schema job](https://github.com/IABTechLab/auctioneer/actions/runs/45538145790/job/138949746957) at `102bf2f957d46afe098f6c69962a18da295e02ff` returned 101 from both source scripts. Actual secure source, development residency and development validation lines:

```text
test result: FAILED. 926 passed; 21 failed; 0 ignored; 0 measured; 6 filtered out; finished in 945.75s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 826 filtered out; finished in 93.41s
test result: FAILED. 86 passed; 21 failed; 0 ignored; 0 measured; 846 filtered out; finished in 128.45s
```

The graph contains 953 cases: secure executed 947 with the same six separately owned exclusions; development executed all 107 validation cases. All four source-window cases report `ok` in both lanes, including actual post-capture drift and error-path closure. Prefix's iterator-population/recursion case also reports `ok` in both; its other three cases still fail against unknown raw-string or Counter slots. All three new traversal cases reach their positive comparisons with `Err(NotDerived)` in both lanes, establishing valid missing-behavior RED rather than setup failure. Their source-binding mutations and later overflow controls are not yet sensitivity evidence.

All twelve primitive cases now reach `UnsupportedBody(Vec)` rather than `UnsupportedBody(Nfa)`. This remains an author-owned supported-source recognition defect, not expected stub RED, target uncertainty or a complete primitive proof. The three entropy cases retain their intended `MissingSourceProof` failures. No event/count oracle is changed to bypass the Vec failure; exact-stage diagnosis or bounded item/shape diagnostics must precede its repair.

The hosted matrix finished with 23 successful checks and two failures: compiler-schema and the independently recorded format check. Compiler controls, strict source-target Clippy and later success-gated commands were skipped after source failure and remain not run. No complete common proof, final-head acceptance, review readiness, qualification, activation or issue closure is claimed.

### Reviewed Entropy And Traversal Implementations Await Execution

After the recorded three-case REDs in both lanes, the bounded entropy implementation matches the selected constructor/default/import chain and derives ordered seed prefixes with checked invocation arithmetic. It reuses the existing owner schedule through borrowed getters; the narrow const-trait syntax parser is moved to one shared ordinary module, not copied or compiled through a source include. Static review found three missing receiver-type edges; the repair binds Core's hybrid field and both reverse strategies' core fields, with independent rehashed type-only controls preserving the dispatch methods and requiring exact `SourceShape(MetaStrategy)` refusal. All original names, numeric oracles and negative controls remain. Final reviewed entropy SHA-256 is `e4ead5aa67a46ad86eee6638d221daa0c83297af88d0e17c8cb7168cc74acf16`.

This contract supports one factory/one hostname-callback family only. Actual whole-catalog joins, absent-hostname/zero-cache support, inherited name/builtin resolution and ordinary map/cache/cleanup costs remain unfinished; all fourteen work units stay unknown. Source hashes, branch labels and fixture compiler/features do not supply actual-source or artifact authority. No complete common proof follows from the finite seed counts.

The traversal implementation derives typed formal/actual provenance, authenticates the canonical array/all binder, separates disjoint child depths from ancestor rescans and uses checked cap lifting rather than an observed J. All three test/oracle blocks are unchanged; zero attempts preserve source duties, unsupported grammar refuses, and no old-counter fallback is introduced. Final formatted and re-reviewed SHA-256 is `98a05e5e8b0b263bcd8c0ecc6ef90812aad4c07e9f28f0dcce6b61e17cb27f6a`; the entire test module remains byte-identical to its executed RED contract. Its supported unary borrowed-Value grammar is not yet the full actual validator closure or the final affine work report. Actual recipe/caller-cap binding and superseded-counter retirement remain required.

The primitive repair also supplies explicit joint closing-angle tokens in four authored const bounds that Syn retains verbatim. This corrects a deterministic expected/source lexical mismatch without rewriting actual bounds or erasing const qualifiers. It does not establish that this was the first executed Vec refusal. A stage observer on the same source-check kernel now supplies a static stage only in an assertion's lazy failure message; assertion operands, refusal classes and independent oracles remain unchanged. No direct printing, global/thread-local diagnostic state, new loader, second classifier or lint allowance is added. An initial direct-stderr diagnostic was removed before publication because static review identified the existing restriction lint; it was not waived.

Four reviewed tests-first caller cases join the same existing target and minimum roster, retaining all 107 prior names for 111 static required names. The contract records symbolic original-impression, resolved/funded/effective-bidder and whole-scoped-node expressions, explicit scope guards and an unchanged-parameter borrow duty. It does not manufacture numerical policy or panic facts from nonempty JSON. Cleanup, parameter-borrow preservation, concrete serialization/navigation and compiled-artifact bindings stay unknown. The deriver remains `MissingCallerSourceProof` until genuine hosted positive assertion RED. Oversized fixture functions and local shadowing found in static review were repaired without removing assertions, changing four case names or relaxing lint settings.

Separate static specification and quality reviews cleared these bounded deltas and their ordinary-module/required-roster wiring. Pinned rustfmt was editing only. Hosted compilation, new caller RED, entropy/traversal/primitive GREEN, reached strict lint and unchanged full final-head acceptance remain pending. Existing source-first order, commands, checkers, exclusions, feature lanes, jobs, timeouts, resource limits, serving behavior and accounting are unchanged.

### Primitive And Traversal GREEN; Caller Contracts Reach RED

At `66540dafe14cdb6cb5cf6421ca19faedc5694500`, the completed [compiler-schema job](https://github.com/IABTechLab/auctioneer/actions/runs/45545120821/job/138972765236) executed source conformance before compiler controls, as approved. Both source scripts returned 101. Actual secure source, development residency and development validation result lines:

```text
test result: FAILED. 941 passed; 10 failed; 0 ignored; 0 measured; 6 filtered out; finished in 1208.59s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 830 filtered out; finished in 89.38s
test result: FAILED. 101 passed; 10 failed; 0 ignored; 0 measured; 846 filtered out; finished in 264.29s
```

The target graph contains 957 cases. Secure source executed 951 with the unchanged six separately owned exclusions; development executed all 111 required validation cases. All twelve primitive, all three traversal and all four source-window cases report `ok` in both lanes. The primitive and traversal GREEN results establish the tested finite source contracts and their negative controls, not the remaining transitive work units, whole generated-validator grammar, artifact correspondence or complete common proof.

All four new caller cases reach the intended positive comparisons with `Err(MissingCallerSourceProof)` in both lanes, at `caller.rs:635`, `:708`, `:889` and `:1044`. Their setup executed successfully; this is valid tests-first RED. The source deriver may now replace that stub without changing the four independent oracles. Later negative-control assertions still need execution after implementation.

The three entropy cases fail their supported positive comparison with `Err(SourceShape(Hostname))`, an author-owned recognizer defect rather than intended stub RED or a target prerequisite. Static diagnosis locates `hostname_pattern` comparing a Syn-reprinted actual static against raw authored quote tokens: the nested generic closers and zero-argument closure pipes have different punctuation spacing. The narrow repair parses only the expected static into `syn::ItemStatic` and interpolates that whole item; it does not normalize actual source, alter the recipe, or relax any semantic or negative-control check. Its hosted repair result remains pending. Prefix's iterator/nesting case is GREEN in both lanes; its other three cases still fail against unknown raw-string or Counter-protocol bounds.

All 24 other checks passed, including hosted formatting and broad workspace Clippy. Explicit strict source-target Clippy was not reached after source failure in either lane; broad Clippy does not select this `test = false` target and is not a substitute. Compiler controls and later success-gated core/staged commands were skipped, not passed. The approved ordering changes only when a failure becomes visible: a successful head must still execute every existing command. No tests, assertions, exclusions, timeouts, limits or checks were weakened, and no local executable verification ran. The PR remains draft and #47 open; final-head full acceptance and complete common work/storage/stack derivations remain required.

### Caller Derivation And Next Tests-First Contracts

After the four executed caller REDs, the conditional caller recognizer matches the selected function/import/gate shapes and derives the symbolic impression, bidder and scoped-node expressions. Five re-export-root source slots extend the original nine, each covered by the missing/rehashed-source controls. The existing context-source checks are extracted into one source-only kernel; the binding-aware entrypoint retains both original binding checks before delegating. Parsing, literal-doc and token helpers move to the shared syntax owner rather than being copied or requiring fabricated binding JSON. The four original case identities and numerical oracles remain. An explicit unresolved-helper list retains parent/type ancestry, integer semantics, normalization preservation and collection protocols; parameter-borrow preservation, cleanup, navigation, panic mode and compiled-artifact binding remain unknown. This is a conditional source-shape result, not a completed caller/attempt theorem or common proof.

The expected-only hostname static repair preserves the recipe and actual AST unchanged. Static specification and quality reviews cleared both bounded implementations. Caller review found two shadowed executed-Rust locals; distinct names repair that strict-lint issue without changing quoted AST data, comparison order or assertions. These are static findings/repairs, not executed compiler or lint results.

Eleven reviewed tests-first cases join the same source target and development minimum roster, preserving all 111 prior names for 122 static required names. Four traversal cases extend the existing unary boundary to instance/cache argument bindings, object key/value provenance and independently varied ancestor-rescan caps. Three scalar cases use the exact locked serde/serde_core 1.0.229 and serde_json 1.0.150 sources plus pinned std sources as conditional DATA. They preserve the literal raw quotas, escaped/Unicode virtual lengths and Counter request protocol; registry/artifact authority, ordinary formatter work, map navigation and cleanup remain unproved. The prefix owner moves to its ordinary directory module with only scalar registration added; all four existing cases and oracles remain unchanged. New scalar derivation stays a refusal stub until valid hosted RED.

Four report cases freeze a strict current Incomplete envelope, source exclusions and fixed-false qualification/fit/permission flags. Positive setup consumes the sealed current descriptor, its same-capture checked bodies and the existing independent public-name census, without historical reconstruction or new child builds. Coverage controls independently change children, contexts, duties and program records. The output-format checker cannot deserialize authority or return costs. Source/report derivation remains refusal-only, and the actual old writers remain in place until this contract reaches hosted RED and its replacement is implemented.

The retirement control follows the complete ordinary conformance module graph through the existing bounded reader; missing or ambiguous modules refuse rather than silently skip. Static review caught root-only coverage, overly broad unit-writer exclusions and an omitted indexed-kind assignment form. The repaired controls cover moved declarations, calls, references, field/kind writers and both retired kinds; exemptions are limited to the exact private negative-fixture owners/signatures. The newly identified three-field `check_current_report_fields` checker is added to the same-PR retirement inventory, not retained as a second current contract. It is not deleted before tests-first execution. The census is not a general Rust dataflow proof or a legacy-report reader. Independent static specification/quality re-review cleared the frozen report contract.

Reviewed source SHA-256 values at preparation:

| Owner | SHA-256 |
| --- | --- |
| `work/caller.rs` | `d4e27cc0745d1470b57f94f861368844a19a5bf1db765c3c97de530f30d0c85b` |
| `work/entropy.rs` | `4ee0fd4e3ce323b33a11ad46249bea238bae65471d69926d8801810394e471e3` |
| `work/partition.rs` | `63d2138ea4ba2006bcf31e4303215fea9f9956e522544b8743f912613b629695` |
| `work/prefix/scalar.rs` | `551d983c24930c6d33974459cc4a83fdae2afe97fafd52947a7f48a209b2265f` |
| `report.rs` | `08a62db6fe68bc31b4e42a15c0c9074689b6d9d97ed55b18fed5845b98cdff89` |

Pinned rustfmt was editing only, and `git diff --check` supplies whitespace evidence only. Hosted compilation, the eleven intended assertion REDs, caller/entropy GREEN, reached strict target lint and complete final-head acceptance are pending. No local build, test, Clippy, proof, parser-control or mutation execution ran. Source-first order, commands/checkers, six exclusions, feature lanes, jobs, timeouts, assertions, serving behavior, resource limits, reservations and accounting remain unchanged. No common completion, qualification, activation, review readiness or issue closure is claimed.

### Caller Import Ownership Error Before Assertion Execution

At `4d7b8310f6a8089eca81efb317f38ba0a28f5b48`, both source lanes in the [compiler-schema job](https://github.com/IABTechLab/auctioneer/actions/runs/45555104818/job/139005160572) returned 101 during compilation. The independent [Fastly artifact job](https://github.com/IABTechLab/auctioneer/actions/runs/45555104818/job/139005160448) reports the same shared source-target failure:

```text
error[E0382]: borrow of moved value: `destination`
--> crates/auctioneer-bidder/tests/schema_validation/work/caller.rs:579:36
577 |                     found.push(destination);
    |                                ----------- value moved here
579 |                 if local == "*" && destination != "auctioneer_config::source::*" {
    |                                    ^^^^^^^^^^^ value borrowed here after move
error: could not compile `auctioneer-bidder` (test "schema_source_conformance") due to 1 previous error
```

The import matcher moves an owned destination into its result before the later glob-refusal check borrows it. The repair evaluates that unchanged refusal before the possible move, preserving the owned result, visibility/attribute checks and exact supported-import rule without cloning. This is an author-owned compilation defect, not tests-first RED, a source/target prerequisite or a runtime behavior change. Static review did not establish compilation. None of the eleven new assertion contracts ran, and caller/entropy GREEN is not established by this head. Hosted compilation, exact dispatch, later strict lint and full acceptance remain required at the repairing head; no assertion, check or limit is waived.

### New Traversal And Scalar Contracts Reach Assertion RED

At `a3c2f2c25e92751d87d47343ff7dab5f7ba770f2`, the completed [compiler-schema job](https://github.com/IABTechLab/auctioneer/actions/runs/45556166479/job/139008731306) compiles the source target and executes both feature lanes. Both source scripts return 101. Actual secure source, development residency and development validation result lines:

```text
test result: FAILED. 944 passed; 18 failed; 0 ignored; 0 measured; 6 filtered out; finished in 870.21s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 841 filtered out; finished in 51.47s
test result: FAILED. 104 passed; 18 failed; 0 ignored; 0 measured; 846 filtered out; finished in 174.93s
```

The graph contains 968 cases. Secure executes 962 with the same six separately owned exclusions, and development executes all 122 validation cases. All three entropy cases now report `ok` in both lanes, alongside the twelve primitive and three original traversal cases. These are bounded component results, not complete common work or source authority for target artifacts.

The four new traversal cases reach positive comparisons with `Err(CallBinding(Site { function: 1, lexical: [] }))`; the three new scalar cases reach positive comparisons with `Err(MissingSourceProof)`. Both feature lanes execute these intended missing-behavior failures. Their independent numeric expectations remain unchanged. This permits implementation of the supported parameter/cache/object grammar and conditional scalar bounds, not filling unrelated unknown work, cleanup or artifact slots.

Three report cases reach their intended `MissingSourceProof` comparisons. The fourth, `retired_assembly_has_no_active_callers`, fails its own positive census fixture before the actual retirement comparison: the visitor reports no field for a semicolon-terminated `json!` statement expected to contain `native_cache_heap_candidate`. Syn represents this as a statement macro, outside the expression-only hook. The narrow repair uses the shared generic macro visitor while keeping the `json` filter, exact fixture exemptions and all expectations unchanged. Canonical report implementation and hard cut remain gated on this case reaching the actual retirement assertion in both lanes.

All four caller cases instead reach `Err(SourceShape(CoreServices))`. Static diagnosis finds that method selection counts both an inherent `AuctionFunding` implementation and its distinct `Debug` trait implementation because it filters only by self type. The repair distinguishes the expected trait kind/path before uniqueness, then retains the full existing header comparison. Synthetic controls independently retain unrelated trait methods, duplicate the inherent implementation and replace it with a trait implementation. These are author-owned supported-source setup defects, not valid tests-first RED, target prerequisites or permission to relax an oracle.

The full hosted matrix completes with 24 successful checks and one failure, compiler-schema. Explicit strict source-target Clippy and compiler controls remain not run after the source failure. The source-first ordering is exercised; it does not waive later commands on a passing head. No local executable verification, changed assertion, new target/job, resource-limit change, serving change or accounting change is introduced. Full final-head acceptance, report retirement and complete common derivations remain required; this is not review readiness or issue closure.

### Retirement Census Reaches Actual Legacy Comparison

At `effa2440ae8ee5e11891cc3d9525d24bb0ef9a96`, the completed [compiler-schema job](https://github.com/IABTechLab/auctioneer/actions/runs/45560348617/job/139022288874) returns 101 from both source scripts. Actual secure source, development residency and development validation lines:

```text
test result: FAILED. 944 passed; 18 failed; 0 ignored; 0 measured; 6 filtered out; finished in 930.25s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 841 filtered out; finished in 54.27s
test result: FAILED. 104 passed; 18 failed; 0 ignored; 0 measured; 846 filtered out; finished in 184.30s
```

The 968-case graph and all 122 development validation cases remain executed. In both feature lanes, the retirement test now passes its synthetic census controls and fails the actual legacy-absence comparison at `report.rs:1330`. The exact active inventory contains declarations `assert_selected_cache_heap`, `selected_cache_bundle` and `check_current_report_fields`; calls/references to the first two; the `native_cache_heap_candidate` field; and kind `conditional_native_schema_cache_heap_candidate`. The expected inventory is empty. Together with the other three report cases' existing `MissingSourceProof` comparisons, this establishes valid four-case RED for the canonical replacement and atomic hard cut, not completion of retirement.

Caller matching passes the prior implementation-selection failure but all four cases now panic at authored expected syntax, `caller.rs:1382`: `unexpected end of input, expected semicolon`, in `syn::parse_quote::parse::<syn::stmt::Stmt>`. Parsing the bare tail `host` directly as a statement is not the supported Syn statement parser entry. This is another authored setup defect, not the intended positive caller result or an ordinary runtime failure. The repair must preserve the semicolon-free expression statement and all existing oracles; adding a semicolon to match the parser would change the expected program.

The four new traversal, three scalar and three earlier prefix failures remain; no unpushed implementation is credited by this head. Explicit source-target strict lint and compiler controls are again not reached after source failure. No final-head full acceptance, complete common proof, source retirement, qualification, readiness or issue closure is claimed.

### Reviewed Object Traversal And Scalar Protocol Implementations

The supported traversal implementation now verifies every formal/actual ordinal, distinguishes cache lineage from input-tree lineage, checks object key/value binder sites and preserves the cap of each active rescan axis. Array axes use `C - 1`; map axes use `floor((C - 1) / 2)` because each entry funds both a key and a value in J. All seven case/oracle blocks remain unchanged from their executed contracts. Separate static specification and quality reviews clear formatted SHA-256 `91aebae2f7c7f6f0bbc5eee7e3ae04b5b05e668b0a2d7f23d955da511b386b41`. This supplies conditional entry/callback bounds, not ordinary primitive work, authentic caller-cap binding or complete generated-validator closure.

The scalar implementation checks the existing twenty-source conditional tuple and derives the raw quota across attempts, bounded escaped-string length and concrete Counter request protocol. It reuses the existing input-source kernel and narrow const-impl parser; it does not read/authenticate new source paths, create a compiled receipt or attach unbound scalar data to the older prefix deriver. All three scalar and four existing parent-prefix tests/oracles remain unchanged. Registry/compiler/feature/artifact correspondence, ordinary formatter and helper work, map navigation, error/unwind cleanup and matched frames remain explicit missing obligations.

Static review catches three authored scalar defects before publication: an unavailable `syn::visit` feature, missing expected impl-header trailing commas and a denied wildcard match over the closed work-refusal enum. The reviewed repair uses enabled `VisitMut`, constructs formatter-stable expected-only where punctuation and enumerates the same refusal mapping. Actual source normalization, dependency features and oracles are unchanged. Separate specification and quality re-reviews clear SHA-256 `3f5475fa695b1b343876c37c8728d4fe2da82fe8d98f7a8d0c2a8d19b8b1ea4b`.

Caller expected syntax now constructs `Stmt::Expr(expr, None)` at all three semicolon-free tail sites rather than parsing each as a standalone statement. The source expressions and all four caller test/oracle blocks remain unchanged. Independent static specification/quality review clears SHA-256 `61f3997567405619ba8be5edf41602fa92080ffbafeee53a1f66b551a04c4798`; it is not an executed caller GREEN result.

Pinned rustfmt is editing only, and whitespace checking is not compilation. Hosted implementation GREEN, reached strict source-target lint and the unchanged full final-head matrix are pending. The canonical report remains a refusal stub in this checkpoint; its independently gated hard cut is separate unfinished work. No component review grants source completeness, numerical policy/funding, target facts, serving permission, review readiness or issue closure.

### Object Traversal And Conditional Scalar GREEN

At `aabe6b45f4681a080213296c5b95c65f640f85af`, 24 hosted checks pass and only [compiler-schema](https://github.com/IABTechLab/auctioneer/actions/runs/45564634292/job/139036348984) fails. Actual secure source, development residency and development validation lines:

```text
test result: FAILED. 951 passed; 11 failed; 0 ignored; 0 measured; 6 filtered out; finished in 1306.38s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 841 filtered out; finished in 90.68s
test result: FAILED. 111 passed; 11 failed; 0 ignored; 0 measured; 846 filtered out; finished in 337.68s
```

All seven traversal cases and all three conditional scalar cases report `ok` in both lanes, including their existing independent source-drift, zero-attempt and overflow controls. All 122 required development validation cases execute; the graph remains 968 cases with the same six secure exclusions. This is GREEN for the supported conditional contracts, not complete ordinary primitive work or artifact/source authority.

The four caller cases no longer panic while parsing tail statements. Their positive comparisons now return `Err(Context(SourceShape(ExchangeSplit)))`, requiring a precise supported-source diagnosis. This remains an authored recognizer failure, not caller GREEN, intended missing-behavior RED or a target fact. The other seven failures are the four canonical-report/retirement cases and three older prefix cases. Strict source-target Clippy and compiler controls remain not reached; no full acceptance or common completion is claimed.

### Application Attachment Oracle: Tests-First Checkpoint

Static review of the unpublished atomic report cutover identifies a missing integration oracle: the four real application tests do not inspect the new `validation_resources` attachment. The next checkpoint adds only a twenty-line assertion hunk to their existing shared report helper. Positive descriptor/handoff binding and identity checks precede the required validation object, exact kind, Incomplete status, three false permission/qualification flags and same-capture binding equality. It invokes no unpublished API and adds no test target, case, job or build.

Separate static specification and quality reviews clear this assertion hunk. The frozen full-worktree root hash is `f22120ea7eddd03dadcb981f95d0063c0ebeacf10571216c9440275fa90ee822`; removing only the hunk reproduces the earlier frozen root `0950cc037c500fd163406c766938104b8616d9983f9eb9e4b90a792de4490c15`. Only the assertion hunk is staged against the committed baseline, which has no report attachment. The cutover, moved observations, canonical checker and required-roster additions remain unpublished until this integration oracle reaches hosted assertion RED. Expected failure is the missing validation object, not a failed binding setup; this expectation is not yet executed evidence. No local executable verification ran and no assertion or limit was weakened.

### Four Real Application Owners Reach Integration RED

At `242c3029a00ffcf249cff739693e93257024eda2`, all four existing application-artifact jobs reach the new `assert!(validation.is_object())` at `schema_source_conformance.rs:863`, after their positive same-capture descriptor/handoff binding and identity checks. Actual result lines:

| Existing owner | Executed result |
| --- | --- |
| [Axum](https://github.com/IABTechLab/auctioneer/actions/runs/45569759953/job/139053229217) | `test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 967 filtered out; finished in 223.89s` |
| [Fastly](https://github.com/IABTechLab/auctioneer/actions/runs/45569759953/job/139053229229) | `test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 967 filtered out; finished in 213.61s` |
| [Cloudflare](https://github.com/IABTechLab/auctioneer/actions/runs/45569759953/job/139053229383) | `test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 967 filtered out; finished in 201.72s` |
| [Spin](https://github.com/IABTechLab/auctioneer/actions/runs/45569759953/job/139053229506) | `test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 967 filtered out; finished in 169.69s` |

Each selected test executes once and fails the intended missing-attachment assertion, not compilation, a zero-test filter or binding setup. Each first secure capture fails before its development capture; those later iterations are not run, not GREEN. Unrelated tests-first jobs may be superseded by the fix and supply no full-matrix success claim. The fixing head must still execute every feature lane and all original hosted checks.

### Canonical Report And Atomic Retirement Await GREEN

The reviewed replacement produces one exact sixteen-field current Incomplete envelope from the sealed view, same-capture checked bodies and independent public-name census. Seed, view and body closing checks run around interpretation, including error paths. Duty inventories and source exclusions remain obligations, not complete work/storage/stack proofs. Qualification, physical-fit and serving-permission flags stay false; output-format validation cannot deserialize authority or return installed costs.

The same commit deletes the old cache-candidate assembly, its field/kind and calls, plus admission's separate three-field checker. Its unique current-positive, retired, hybrid and heuristic-fallback controls now use the canonical checker. All four report oracle blocks are unchanged. Observation assertions move to `observations.rs`, retaining the 225 captured parseable cases, four generated helpers, one hostname owner, authored verdicts, phase/population checks, source-byte hashes and release controls. The two fake controls move byte-identically into that owner's inline tests. They are additionally required by the existing development roster, retaining all 122 prior names for 124 static required names; executed counts remain pending. Replay tests, measurement owners and the six heavy source exclusions are unchanged.

Each existing application capture calls the canonical producer while its seed/graph/recipe is live, attaching the result only after the existing graph-substitution controls. The current-only helper uses that same producer. No additional four-adapter build loop or historical reader is added. Opposite-lane probe observations remain component-only and receive no validation view. Static review catches a shadowed JSON-array binding and the loss of test-only Clippy scope during extraction; renaming the binding and explicitly marking `mod observations` with `#[cfg(test)]` restore the existing policy without an allowance or changed assertion. Separate specification and quality reviews clear the corrected bounded bundle, not full common completion.

Caller diagnosis also pins the current `ExchangeSplit` refusal: local-variable lookup accepts only `Pat::Ident`, while the real `bidders_on_imp: BTreeSet<&str>` is `Pat::Type`. The narrow repair admits one typed-identifier wrapper but returns the original Local, preserving full type/statement equality, attributes and duplicate rejection. All four caller oracles remain unchanged. Independent static specification/quality review clears SHA-256 `1e16864b91eea726a0162e320712a4c06d83db3479843c1fa7d36b8954737db4`. Hosted caller GREEN remains pending.

No local executable verification ran. Pinned formatting and whitespace checks are not tests. Actual retirement/canonical-report GREEN, both-lane dispatch, reached strict source-target lint and full final-head hosted acceptance remain required. Missing common ordinary work, typed byte traffic, cleanup, prefix integration, complete storage/stack and source/feature correspondence are still unfinished, not target facts or zero costs. No common-source completion, review readiness, qualification, activation, issue closure or merge is claimed.

### Canonical Report, Retirement And Caller GREEN

At `e799ecda70662415f7c6a8811c03ffefd3b036fc`, all 24 other hosted checks succeed and [compiler-schema](https://github.com/IABTechLab/auctioneer/actions/runs/45571808445/job/139060148777) fails only the three older preflight-prefix cases. Actual secure source, development residency and development validation result lines:

```text
test result: FAILED. 959 passed; 3 failed; 0 ignored; 0 measured; 6 filtered out; finished in 1107.93s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 841 filtered out; finished in 94.30s
test result: FAILED. 121 passed; 3 failed; 0 ignored; 0 measured; 844 filtered out; finished in 298.40s
```

Both feature lanes execute all four canonical-report/retirement cases, all four caller cases and both relocated observation controls successfully. The development validation prefix executes all 124 required names, with the unchanged 968-case graph and same six secure exclusions. The three failures remain the independently authored raw-quota and Counter-protocol expectations: `None` instead of `Some(196_608)`, `Some(65_536)` and `Some(1)`. Their conditional scalar lemmas already pass separately; the source/limit-bound prefix integration remains missing. No expectation was relaxed.

The same head's four actual application owners all pass the new report-attachment assertions:

| Existing owner | Executed result |
| --- | --- |
| [Axum](https://github.com/IABTechLab/auctioneer/actions/runs/45571808445/job/139060148865) | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 967 filtered out; finished in 385.70s` |
| [Cloudflare](https://github.com/IABTechLab/auctioneer/actions/runs/45571808445/job/139060148834) | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 967 filtered out; finished in 330.99s` |
| [Fastly](https://github.com/IABTechLab/auctioneer/actions/runs/45571808445/job/139060148942) | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 967 filtered out; finished in 318.84s` |
| [Spin](https://github.com/IABTechLab/auctioneer/actions/runs/45571808445/job/139060148884) | `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 967 filtered out; finished in 255.05s` |

Each application log explicitly records both secure and development capture labels, so both iterations executed at GREEN. The current report remains Incomplete and non-qualifying; those passes establish attachment and binding, not completed work, storage, stack, policy or target proofs.

The approved source-first order is exercised with unchanged commands, checkers, exclusions, jobs, timeouts and limits. The later compiler-control target and explicit strict source-target Clippy are not reached after source failure; the passing broad Clippy check does not select this `test = false` target. Full current-head acceptance, the remaining common derivations and independent whole-PR review remain required. PR #268 stays draft and #47 open. All executable evidence here comes from GitHub Actions; no local executable verification ran.

### Prefix/Scalar Bridge Tests-First Contract

The next checkpoint makes conditional `ScalarData` mandatory in the two private prefix entrypoints; no optional operand, default loader or old-signature fallback remains. The existing scalar fixture moves to its owning ordinary module so both consumers reuse setup without importing a private `tests` module or compiling handwritten source includes. The three scalar test bodies and four prefix numeric oracles are preserved; other callers receive the explicit operand mechanically. This is test-only DATA, not registry/compiler/feature/artifact authority.

Three new inline cases require exact native/input/json-size byte correspondence, all B/J/D limit links, every scalar source and feature/width/dispatch premise at zero attempts, and transactional count updates. A raw-quota-only overflow uses `A = 281_474_976_710_656`: `A*4_096 = 1_152_921_504_606_846_976` and `A*12_288 = 3_458_764_513_820_540_928` fit while `A*65_536` does not. This distinguishes the new bound from the old node arithmetic. A deliberately different `native_entries` value guards use of `preflight_entries`, not proportional test inputs.

The existing development required roster gains these three exact names, retaining all prior names for 127 static required validation cases. No target, job, limit, timeout or exclusion is added or removed. The bridge still ignores its new DATA operand and leaves all four scalar slots unknown until the new guards reach genuine hosted assertion RED. Expected failures are accepted mismatched tuples and the missing raw-overflow refusal; compilation/setup failure is not RED. These expectations are not executed evidence. Tree navigation, numeric formatter work, ordinary cleanup, actual source-window binding and all other unfinished common/target duties remain unproved. Static review, pinned editing and whitespace inspection do not establish compilation or readiness.

### Prefix/Scalar Bridge Reaches Hosted RED

At `dc306f3e82b03d3645b8f80ee6149952e83357fb`, the completed [compiler-schema job](https://github.com/IABTechLab/auctioneer/actions/runs/45581012800/job/139090773627) executes the three new bridge cases in both normal source lanes. Actual secure source, development residency and development validation results:

```text
test result: FAILED. 959 passed; 6 failed; 0 ignored; 0 measured; 6 filtered out; finished in 1137.09s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 844 filtered out; finished in 95.89s
test result: FAILED. 121 passed; 6 failed; 0 ignored; 0 measured; 844 filtered out; finished in 305.36s
```

The graph contains 971 cases. Secure executes 965 with the same six separately owned exclusions; development validation executes 127 cases. In each lane, positive setup first validates the existing scalar source lemma. The new link case then accepts an A=0 tuple instead of `Err(ScalarSource(ContextSourceMismatch(Native)))`; the source-duty case accepts missing Native data instead of `Err(ScalarSource(MissingSource(Native)))`; and the transactional case returns `Ok(())` instead of `Err(Overflow)` after proving that the old J/3J arithmetic fits. These are genuine assertion REDs, not compiler errors or failed setup. Later negative variants inside those failed cases are not yet executed acceptance.

The three older raw-quota/Counter-prefix failures remain; no original oracle is weakened. Both source scripts return 101, later compiler controls and explicit strict source-target Clippy are not run, and the job remains failed. The complete hosted matrix finishes with 24 successful checks and only compiler-schema failing, including passing application-artifact owners for all four adapters. This is not a complete passing matrix. The GREEN implementation may now connect the required DATA tuple through the shared scalar kernel, but it must retain all zero-attempt duties, checked arithmetic and error atomicity. No common completion, numerical policy/funding, target fit, serving permission or review readiness is established by RED.

### Coupled Factory/Entry Tests-First Frontier

The next three inline `work::partition::coupled_tests` cases freeze a narrow factory/entry result over the complete canonical Recipe and initialization map. Independent factory and validation populations remain distinct: three factory entries and two validation entries give five root entries, at most three inner-factory/initializer/map-callback entries, six two-program readiness probes, and three constructor calls per program. Ordered 0/1/2 completed-field prefixes retain partial-construction cleanup duties. The J=4 input witness and independently varied caps seven/eleven do not change factory bounds. Zero entries retain source/binding duties, and arithmetic overflow must refuse.

The existing ownership interpreter is reused through `owner_schedule_data`; a read-only roots accessor and parent-scoped module visibility permit ordinary sibling access without private test imports. All seven previous partition test bodies are preserved. The existing actual-body case buffers the new result from the same classified Recipe until handoff closing checks, comparing the exact owner/marker roster before asserting success. It creates no additional capture or child build. The new deriver remains an unconditional `MissingSourceProof` stub until valid hosted RED. Success is explicitly narrower than whole input partitioning, primitive/search work, source-call closure, cleanup or actual caller/artifact binding.

The same development roster gains these three names for 130 static required validation cases, preserving all previous cases and dispatch controls. Separate static specification and quality reviews find no concrete checkpoint issue; that is not compilation or execution evidence. Expected RED is the missing positive factory result, including in the existing actual-body case after its positive setup. No runtime path, limit, dependency, target, job or fallback is added. The prefix GREEN implementation and coupled-entry RED still require hosted execution at the next head.

The reviewed prefix implementation parses one `Sources` collection, checks the exact triplet and all native limits, and shares the existing semantic/arithmetic kernel with standalone scalar controls. All source/premise checks run at zero attempts before arithmetic; every fallible count finishes before any destination field is written. Child overflow retains the existing parent refusal, other scalar refusals retain their typed cause, and per-request `(1,0,0)` does not scale with attempts. Navigation and unresolved work/cleanup obligations remain unknown. Both test modules are byte-unchanged. Separate static specification and quality reviews clear `prefix/mod.rs` SHA-256 `3da1ccc6d8769a04eb65d4373f71137a71107d6cc4b8abd1790c315de8c33c01` and `prefix/scalar/mod.rs` SHA-256 `330453c9828d44be7ed5f996cb30f3769ec35787093bcb4c3bbbc7283c1c3751`; hosted GREEN remains pending.

Current qualification guidance now links the canonical Incomplete owner/evidence and separately labels the old cache-candidate description historical. Its anchor, original measurement figures and original qualification caveats remain; no pinned capture is rewritten. The resource index dates only its validation-row update and does not recertify earlier C/I/integration checkpoints. No local executable verification ran, and no current-head full acceptance, common completion or review readiness is claimed.

### Source-Bound Prefix GREEN And Coupled-Factory RED

At `3241de73320b7fde2056dd59102107d1bca6e415`, 24 hosted checks pass and only [compiler-schema](https://github.com/IABTechLab/auctioneer/actions/runs/45588687431/job/139116819860) fails. Actual secure source, development residency and development validation lines:

```text
test result: FAILED. 964 passed; 4 failed; 0 ignored; 0 measured; 6 filtered out; finished in 1052.93s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 847 filtered out; finished in 58.73s
test result: FAILED. 126 passed; 4 failed; 0 ignored; 0 measured; 844 filtered out; finished in 265.46s
```

The graph contains 974 cases; secure executes 968 with the unchanged six exclusions, and development validation executes all 130 selected cases. All seven prefix cases, all three scalar cases and all seven original partition cases report `ok` in both lanes. This is genuine prefix GREEN, including the source/premise refusal loops, raw-only overflow and transactional updates. The original numeric oracles remain unchanged. It does not prove ordinary serializer/primitive work, navigation, cleanup or actual compiler/artifact correspondence.

Both lanes execute all three new coupled-factory positives with `Err(MissingSourceProof)` instead of their independent expected bounds. The existing actual-body case also reaches that same refusal after exact whole-roster classification, ownership setup, handoff closing checks and canonical Incomplete-report checks. Its first factory result fails; later factory-result assertions are not yet acceptance. These four valid missing-behavior REDs permit implementation of the narrow entry/factory boundary, not a pruned input proof or unclassified-call fallback.

The source scripts retain exit 101 and the failed job; later compiler controls and explicit strict source-target Clippy remain not run. Broad Clippy, packaging and all four application-artifact owners pass at this head but do not substitute for those skipped commands or complete common derivations. No local executable verification, changed assertion, runtime behavior, workload limit, reservation or accounting change is introduced. #47 and full final-head acceptance remain open.

### Reviewed Coupled Factory Derivation Awaits GREEN

The replacement reuses `owner_schedule_data` once, checks the complete root/initialization metadata and all primitive source-call lists before arithmetic, then matches the executable eager initializer and exact program/binding/site correspondence. Probe lexical order is preserved separately from constructor-field order. Refusal precedence remains root binding, initialization, source calls, owner binding, initializer, then overflow. Checked F+V and F-times-probes arithmetic, per-program F ceilings, wrapper callback 0..F and ordered completed-field prefixes implement the frozen narrow contract.

Both inline test modules and the existing actual-body oracle are byte-unchanged. Separate static specification and quality reviews clear `work/partition.rs` SHA-256 `e99baa591c09cf6cb06bec54752408e1e495645e7c7319e99ee486894a75cced`. Pinned rustfmt is editing only and whitespace checks are not executable proof. Hosted GREEN and reached strict source-target lint remain pending.

The result still has `input_partition = None` and all eight explicit duties: caller populations, source binding, validation traversal, search multiplicity, primitive source calls, ordinary work, ordinary cleanup and partial-construction cleanup. It does not classify the whole generated-validator work closure, supply physical/callback facts, close #47 or grant serving/qualification. No fallback, filtered Recipe, additional capture/build, changed test, dependency, workload limit, reservation, accounting or production behavior is added.

### Coupled Input Traversal Tests-First Checkpoint

The next three inline cases borrow the complete coupled Recipe through its existing factory/owner foundation, then require a separate validation-root partition. Their fixture populations remain F=3/V=2; caps seven and eleven vary independently of the four-node array witness. An emitted-style object pass adds required/covered Bool locals, an `__object_pass` binding and two distinct leaf-call sites. Literal per-attempt coefficients remain separate from factory counts. Declaration, assignment-target and actual/formal-binding drift must refuse at both zero and one validation; source/initializer obligations and per-invocation arithmetic also survive zero workload.

The new entrypoint currently returns `MissingTraversal` after the factory foundation succeeds. The existing actual-body test buffers that new result until its original source/body closing checks, exact owner census, factory-positive checks and canonical Incomplete-report assertions complete. Expected RED is the missing traversal result, not compilation or failed setup. These expectations are not executed evidence. The future kernel must borrow the original Recipe and its checked validation root, without a caller-selected root, filtered clone, erased initializer map or coarse-counter fallback. All ten prior numeric and refusal contracts are retained; syntax-only lint repairs may rename locals or spell the same integer in hexadecimal.

The existing development minimum roster adds the three exact names for 133 required validation cases; its dispatch/result checkers, source-first ordering, six secure exclusions, jobs, timeouts and limits stay unchanged. Static review also identifies an unrelated local-name shadow in the preceding factory implementation; renaming the result to `bounded_programs` changes no behavior or test. Hosted strict lint remains required, not inferred from that edit. The prior `9b65ff2` full matrix was still running when this checkpoint was prepared; its completed results follow. No local executable verification, complete common proof, qualification or review readiness is claimed.

### Factory GREEN And First Executed Strict Source Lint

At `9b65ff2fa658936bb7288c2b4eb3574fbddf8549`, the completed [compiler-schema job](https://github.com/IABTechLab/auctioneer/actions/runs/45596200006/job/139142800882) executes source conformance before compiler controls, then reaches the explicit strict source-target lint. Actual secure source, compiler-control, development residency and development validation result lines, in that order:

```text
test result: ok. 968 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 1133.61s
test result: ok. 337 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 175.23s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 847 filtered out; finished in 96.20s
test result: ok. 130 passed; 0 failed; 0 ignored; 0 measured; 844 filtered out; finished in 322.72s
```

All three coupled-factory cases and the existing whole-roster actual-body case report `ok` in both feature lanes. The original traversal, prefix, scalar, caller, report and observation controls also pass. The narrow factory derivation therefore has genuine GREEN against its earlier four assertion REDs; input partitioning and ordinary primitive/work/cleanup proofs are still unfinished, not implied by those passes.

Both explicit source-target Clippy invocations then fail with the same result:

```text
error: could not compile `auctioneer-bidder` (test "schema_source_conformance") due to 199 previous errors
##[error]Process completed with exit code 101.
```

These are authored failures across the newly selected conformance code: visibility/import style, explicit enum-refusal arms, unused-value ownership, parameter/local names, literal notation and other strict lints. Broad Clippy had not selected this `test = false` target. The full matrix finishes with 24 successful checks and compiler-schema failed; no lint allowance, skipped command or passing full-matrix claim is justified. Repair the code while preserving all values, assertions, source-refusal predicates and limits, then execute the unchanged hosted checks again. All executable evidence is hosted; pinned formatting remains editing only.

The next checkpoint repairs those diagnostics in the conformance code without adding allowances or changing serving behavior. Private projection fields gain owner-local accessors where required; enum-refusal predicates, source-template tokens, formulas, numeric values and assertion contracts are preserved. The existing source-context counter now propagates overflow into its typed source refusal instead of panicking. Separate static specification and quality reviews clear the frozen code, including the three new input controls and intentional `MissingTraversal` stub. Whitespace inspection returns zero and pinned rustfmt remains editing only. Hosted compilation, strict lint and intended assertion RED are still pending; neither static review nor the previous head's GREEN is current-head acceptance.

### Coupled Input Reaches Hosted RED

At `f3af018e698d2c717632dc191720a29cc2b6fd2b`, the completed [compiler-schema job](https://github.com/IABTechLab/auctioneer/actions/runs/45605269986/job/139174405616) executes all three new coupled-input cases and the existing whole-roster body case in both source lanes. Actual secure source, development residency and development validation result lines, in that order:

```text
test result: FAILED. 967 passed; 4 failed; 0 ignored; 0 measured; 6 filtered out; finished in 877.33s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 850 filtered out; finished in 56.96s
test result: FAILED. 129 passed; 4 failed; 0 ignored; 0 measured; 844 filtered out; finished in 219.78s
```

The 977-case graph retains the same six separately owned secure exclusions; development validation executes 133 cases. Each new case reaches its first positive assertion with `Err(MissingTraversal)` instead of the required source-derived input partition. The existing actual-body case reaches that same refusal after the original sealed source/body closing checks, exact owner/marker census, factory positives and canonical Incomplete-report checks. These are genuine missing-behavior REDs, not compilation or setup failures. Later negative variants within those failed cases are not yet executed acceptance.

All other 24 hosted checks pass, including the four application-artifact owners. Compiler controls and explicit strict source-target Clippy are not reached after source failure in either lane. The preceding lint repairs therefore remain unverified by those commands; broad Clippy is not a substitute. Implementation may now replace the narrow input stub while borrowing the original complete Recipe, retaining all binding/source duties and preserving the frozen assertions. No local executable verification, complete common proof, qualification, serving permission or review readiness is claimed.

### Coupled Input Implementation Awaits Hosted Verification

The replacement shares the original provenance/counting kernel and selects only the validation root obtained from the checked factory owner. Both original roots, initialization, independent F/V populations and all eight duties remain intact. Bool locals preserve exact declaration/assignment/negation bindings. Scalar payloads and borrowed key strings never acquire Value-child provenance. Source-backed literal/type-union matches, labelled entry passes and map values/keys retain checked protocols and key-funded cardinality. Regex helpers require the factory-proven static program, matching cache field, exact optional binder and immutable `Input::new(...).earliest(true)` operand. The optional callback's body is bounded by its enclosing occurrence; no JSON-child axis or iterator callback is invented, and ordinary protocol/search work remains unknown.

Two new inline controls use independent literal coefficients with a J=5 witness and caps seven/eleven. Direct helper coefficients remain one; ancestor rescans yield three/five and total iterator coefficients four/six. Positive setup precedes independent cache-field, input-flag, formal, binder and label corruptions at zero/one validation. Canonical recipes contain only reachable functions, so only deliberately unreachable fixture helpers may be absent; every positive expected helper and total call-site count stays exact. All thirteen prior cases and their oracles are preserved. Both new names join the unchanged development execution checker, raising its required validation roster to 135. These new control expectations have not yet executed; the existing whole-roster missing-traversal assertion supplied the hosted RED for this implementation.

Pinned rustfmt remains editing only. The full unchanged hosted matrix, reached compiler controls, strict source-target lint and exact final-head results are still required. Source review and whitespace inspection do not prove compilation, source/artifact authority, complete ordinary work/byte traffic/cleanup/stack bounds, numerical policy adequacy or target fit. #47 remains open and PR #268 draft; no serving, workload, dependency, reservation or accounting change is introduced.

### Coupled Input GREEN And Erased-Wrapper Frontier

At `89a2c5951dea9db64f0af87c00307fe0b7db187c`, the complete matrix has 24 successful checks and only [compiler-schema](https://github.com/IABTechLab/auctioneer/actions/runs/45673380299/job/139414338981) failed. Actual secure-source, development-residency and development-validation result lines:

```text
test result: FAILED. 970 passed; 3 failed; 0 ignored; 0 measured; 6 filtered out; finished in 1323.17s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 852 filtered out; finished in 91.29s
test result: FAILED. 132 passed; 3 failed; 0 ignored; 0 measured; 844 filtered out; finished in 385.74s
```

All thirteen previous partition controls pass in both lanes, including the three original coupled-input positives and their source, zero-population and overflow variants. The 979-case graph retains the same six secure exclusions. Both new emitted-protocol cases and the existing actual-body case fail on `LocalBinding`, not setup/compilation: the actual-body frontier is `appnexus::StagedSchema0`, function 10 at `[0,1,0,0,0,0,1,1,0,0,0,1,1,0]`; both synthetic failures are function 6 at `[0,1,1,1,0,1,2,0]`. The latter is the rescan's negated parenthesized iterator expression. The canonical lowerer erases parentheses/groups through child zero, but the visitor demanded a direct child address. Later variants within those failed cases are not executed acceptance. Compiler controls and strict source-target lint remain not run after source failure; their pending status is not waived by the other 24 passing checks.

The follow-up accepts only canonical zero-suffix erasure at expression slots, preserving same-function/prefix checks and exact formal/declaration anchors. Iterator and optional binders use their syntactic child-one declaration slot, independently of an erased body-expression wrapper. A new positive-first inline geometry control independently rejects changed functions, prefixes, siblings and nonzero suffixes, and rejects a zero suffix on a local-declaration statement. It joins the existing required roster for 136 validation cases, with no command/checker/exclusion change.

Source inspection additionally identifies the count-cast template in `keywords/mod.rs::compile_count_limit`, used by current array `minItems` checks. Its new visitor validates one Scalar operand at its expression slot and a temporary Scalar result, visiting the operand once without adding helper/iterator populations. The supporting fixture gains this count check; all existing numeric oracles remain unchanged, with an independent nonzero-operand-site refusal at zero/one validation. This cast observation is source inspection, not a claimed separately executed RED. Scalar semantics, width, source/control/frame work and all eight outstanding duties remain unknown. Pinned formatting/static review are not hosted GREEN, qualification or review readiness; the next full matrix is still required.

### Whole-Roster Input GREEN And Remaining Strict Lint

At `c1d84c29d019d311080b5072c41e73e2cd8e2e8b`, the completed [compiler-schema job](https://github.com/IABTechLab/auctioneer/actions/runs/45680389874/job/139439075833) executes every original and supporting input control plus the actual whole-roster case successfully in both feature lanes. Actual secure-source, compiler-control, development-residency and development-validation lines, in that order:

```text
test result: ok. 974 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 1139.87s
test result: ok. 337 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 175.25s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 853 filtered out; finished in 96.55s
test result: ok. 136 passed; 0 failed; 0 ignored; 0 measured; 844 filtered out; finished in 321.56s
```

The 980-case graph retains all six separately owned secure exclusions. Both explicit strict source-target Clippy commands are reached and fail with `could not compile ... due to 14 previous errors`, exit 101. These are regular-file assertion spelling, block-semicolon placement, identical match arms, guarded indexing, shadowed names, an unnecessary lazy closure, one fixture integer's type fallback and a consumer-local inherent extension on the shared const-impl syntax type. The complete matrix has 24 successful checks and compiler-schema failed, not a full passing result.

The next repair uses equivalent `Metadata::is_file()` (never `!is_dir()`), merges identical arms without moving their guards, retains exact one-element dereference checks through `first()`, makes the same JSON integer explicitly `u64`, and turns the consumer-specific const-header method into a free helper with the same attribute/token checks. Other changes rename locals or move an equivalent semicolon. Static specification and quality reviews find no semantic, guard, source-token or oracle change. No allowances, removed assertions, rewritten captures, runtime changes or relaxed limits are introduced. Pinned rustfmt remains editing only; the next full hosted matrix and reached strict lint remain mandatory. These GREEN inputs still do not close ordinary work, bytes, source/artifact authority, cleanup, stack, numerical policy or target qualification.

### Redundant Fixture Scope Reaches Strict Lint

At `8c52e4fdd7f871a23e1a52e8d96fe7baf35b3975`, the [compiler-schema job](https://github.com/IABTechLab/auctioneer/actions/runs/45686428242/job/139460124063) records these secure-source and compiler-control results before reaching strict source-target Clippy:

```text
test result: ok. 974 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 1264.99s
test result: ok. 337 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 188.64s
error: could not compile `auctioneer-bidder` (test "schema_source_conformance") due to 2 previous errors
##[error]Process completed with exit code 101.
```

The two diagnostics are `semicolon_inside_block` and `semicolon_if_nothing_returned` in `public_names.rs::tests::seed_with_routes`. Moving the semicolon had removed the preceding `semicolon_outside_block` diagnostic but exposed the other two; strict lint therefore remains failing, not waived. The other thirteen preceding diagnostics are absent from the reached command. The latest local correction removes only that redundant lexical block and retains the exact iterator, unique-function assertion, generated quote tokens, mutation and manifest digest update. Ordinary non-lexical borrow ending permits serialization after the last mutable use; no explicit drop, new helper, allowance or assertion change is introduced. Pinned formatting is editing only. The complete fixing-head hosted matrix remains required; prior-head unfinished development/artifact steps are not claimed as acceptance.

### Lint Repair GREEN And Search Invocation Checkpoint

The complete hosted matrix at `b78077dcc74970fae8999a9ea17bbba6523502d1` finishes with all 25 checks successful and none waived or pending. The [compiler-schema job](https://github.com/IABTechLab/auctioneer/actions/runs/45692534848/job/139481451963) records secure source, compiler controls, development residency and development validation, in order:

```text
test result: ok. 974 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 1222.57s
test result: ok. 337 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 188.83s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 853 filtered out; finished in 66.20s
test result: ok. 136 passed; 0 failed; 0 ignored; 0 measured; 844 filtered out; finished in 307.75s
```

Both explicit strict source-target Clippy commands complete with exit 0, as do normal/staged core and metadata checks. The final [CLI job](https://github.com/IABTechLab/auctioneer/actions/runs/45692534848/job/139481452989) also passes. This closes the lint repair, not the common work/byte/cleanup/stack proof or #47.

The next tests-first checkpoint introduces explicit per-source-site and per-program invocation results, with `MissingSearchMultiplicity` after the already checked factory/input foundation rather than invented zero counts. Three new inline cases preserve prior factory/input oracles and independently vary F, V, declared caps and the J=5 witness. At the witness, direct/rescanned V=2 ceilings are 10/30/50; evaluating the same coefficients at declared caps seven/eleven instead yields 14/22/42/110. Neither is an observed exact search trace. One fixture reaches one static search site twice and another program once; another has eight distinct search sites. Unused programs and no-regex declarations retain source duties and require explicitly justified zero invocation results.

Positive setup precedes engine-name and primitive-role corruptions at V=0/1. The unchanged storage owner must reject these as `OwnerBinding` before the new numeric projection; the test does not pretend to reach a later guard. Separate aggregate-overflow controls retain passing input-only arithmetic: at cap 2^62 and V=0, input `2+3*cap` fits while eight search sites overflow before multiplication by zero; at cap seven and V=2^59, input `23*V` fits while search `56*V` overflows. These literal source ceilings are test DATA, not installed workload limits.

The existing whole-roster case buffers a new search result through the same closing source checks, owner census and canonical Incomplete-report assertions; no capture or build is added. All three exact names join the existing required development roster (expected 139 validation cases and 983 total source cases). Expected hosted RED is the explicit missing-search assertion in those cases and the whole-roster case, not compilation or failed setup. These expectations are not executed evidence. No local executable verification ran; pinned rustfmt is editing only. The eventual implementation must share the existing provenance walker, delete the interim missing-derivation stub, preserve source checks at zero workload, and keep haystack bytes, internal engine work, source/artifact authority, cleanup, stack and qualification separate.

### Search Invocation Reaches Hosted RED

At `dd12455907e99890d5332ae9db246fa7fdd197e8`, the completed [compiler-schema job](https://github.com/IABTechLab/auctioneer/actions/runs/45705995505/job/139527992864) records these actual secure-source, development-residency and development-validation lines:

```text
test result: FAILED. 973 passed; 4 failed; 0 ignored; 0 measured; 6 filtered out; finished in 943.40s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 856 filtered out; finished in 57.40s
test result: FAILED. 135 passed; 4 failed; 0 ignored; 0 measured; 844 filtered out; finished in 222.81s
```

All four failures in each source lane are `MissingSearchMultiplicity`: the three new cases reach their first positive search assertion, and the whole-roster case reaches its buffered search assertion only after original source closing checks, owner census, factory/input checks and canonical Incomplete-report assertions. This is genuine missing-behavior RED, not compilation or failed fixture setup. All thirteen previous partition controls pass. Later variants within a failed case are not yet executed verification. Compiler controls and both explicit strict source-target Clippy commands are not reached after their source failures; the next fixing-head matrix must still execute them.

The local implementation removes the missing-derivation variant/stub and records only top-level `Regex::search_half_with` after existing receiver/program/cache/input checks. It combines raw occurrences by exact source site before depth projection; distinct sites and programs then add. One `partition_analysis` supplies both input-only and search projections without cloning/filtering the original Recipe, deleting initialization, double-visiting operands or falling back to the coarse counter. Every factory program remains present, including source-proven unused programs. Site, program and whole-projection arithmetic is checked before multiplication by V; all eight unresolved duties remain present.

Two additional controls are prepared for GREEN, not claimed executed: a root-string search has positive constant coefficient `(1,0)` at independent caps/validation populations; and a cross-program fixture has eight sites per program, with one helper reached twice. At cap seven/V=2 its program totals are 112/224 and 56/112, whole totals 168/336. Cap `3*2^58` with V=0 and cap seven with V=`2^57` keep input and each program independently representable while the whole search sum overflows. These are conditional invocation ceilings, not haystack-byte or internal-engine work proofs. Full hosted GREEN, reached compiler/lint commands and final-head checks remain pending; no serving, workload, dependency, reservation or accounting change is introduced.

### Search Invocation GREEN

At `c672fab2035138410379d1c65243a30f4598c8b7`, the completed [compiler-schema job](https://github.com/IABTechLab/auctioneer/actions/runs/45711302017/job/139546114589) succeeds. Actual secure-source, compiler-control, development-residency and development-validation lines, in that order:

```text
test result: ok. 977 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 836.89s
test result: ok. 337 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 140.15s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 856 filtered out; finished in 55.75s
test result: ok. 139 passed; 0 failed; 0 ignored; 0 measured; 844 filtered out; finished in 218.14s
```

All three new search cases, their later constant/cross-program controls and the whole-roster case pass in both source lanes. The 983-case graph retains the same six separately owned secure exclusions. Both explicit strict source-target Clippy commands complete with exit 0, as do normal/staged core and metadata checks. The complete current-head matrix finishes with all 25 checks successful, including the [evidence-producer job](https://github.com/IABTechLab/auctioneer/actions/runs/45711302017/job/139546114976); none is pending, skipped as a waiver or dismissed.

This is executed GREEN for the conditional search-invocation projection, not the complete common work, byte-traffic, cleanup, storage, structural-stack, caller/source/artifact, numerical-policy or adapter proof. No local executable checks ran; pinned rustfmt remained editing only. #47 remains open and PR #268 draft.

### Source-To-Helper-Cost Tests-First Checkpoint

The next checkpoint extends the existing `handoff::source::tests::actual_sources_are_consumed_inside_the_bound_window`, without adding a case, target, source capture, build or required-roster change. A read-only consumed-input view exposes borrowed compiler/source data but no constructor, mutable access, path selection or I/O. The finite semantic recipes are shared with the supplied-data helper contract; the existing event evaluator gains a borrowed facts view instead of another cost table or fabricated source capture. The new join must return a projection retaining the actual private LocalContract, not a successful unit-valued source stamp.

Explicit conditional hypotheses independently vary NFA roots and dispatches, with literal source/control/scalar triples `(125,10,9)`, `(250,20,18)` and `(236,10,9)`. Sparse attempts two, successes one and RHS one require `(549,7,5)` and symbolic writes of two StateID elements and one usize element. Both families retain zero-call positives. Expected ledger arrays read the actual fourteen components against literal `Some`/`None` values, independently of the constructor used by the evaluator; eleven fields stay unproved. Helper-call, frame, buffer and invariant obligations remain present, including at zero calls. These supplied facts do not establish actual caller populations or receiver/invariant authority.

All original closing, compiler, seed/view and post-capture assertions finish before inspecting buffered cost results. An in-memory, self-consistently rehashed NFA semantic mutation must fail the cost join as `UnsupportedBody(Nfa)` for both active and zero-call demands while still completing the same source-window closing trace; no vendor, standard-library or serving source is mutated. Separate controls require exact missing-invariant, wrong-receiver, wrong-epoch and overflow refusals. The interim API returns `MissingSourceJoin`; expected hosted RED is the first positive cost assertion after valid source and hypothesis setup, not compilation or a failed source stamp. Later negative variants are not yet executed acceptance.

No local executable verification ran. Pinned formatting and static inspection are editing/review only. The source graph remains 983 cases, with the same six secure exclusions and 139 development validation cases. Hosted assertion RED is required before filling this integration; complete common work, byte traffic, cleanup, stack, numerical policy and target qualification remain unfinished.

### Imported Math Module Collides With A Catalog Identity

At `6e3e293ea021095b82a3bd390410437b67efdcbe`, the [secure native job](https://github.com/IABTechLab/auctioneer/actions/runs/45720734385/job/139578236984) fails the unchanged `test_isolation` owner:

```text
test tests::shared_test_sources_use_synthetic_bidder_inputs ... FAILED
production coupling in shared tests: [("tests/schema_validation/work/mod.rs", {"epsilon"})]
test result: FAILED. 15 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.06s
error: test failed, to rerun pass `-p auctioneer-bidder --test test_isolation`
```

The complete [default-workspace log](https://github.com/IABTechLab/auctioneer/actions/runs/45720734385/job/139578236780) and [insecure-workspace log](https://github.com/IABTechLab/auctioneer/actions/runs/45720734385/job/139578236935) identify the same sole failed target and the same import path. Their actual isolation result lines, in that order:

```text
test result: FAILED. 15 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.90s
test result: FAILED. 15 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.30s
```

The new ordinary `use epsilon::primitives` import triggers the lexical bidder-identity guard, even though it selects the local mathematical helper module. Qualifying it through `self::` would still contain the guarded path segment. The local repair renames that module/directory to `epsilon_closure`, preserves all four moved Rust files byte-for-byte, and updates its 25 exact development-case prefixes and current plan paths. No checker exception, source import trick, alias, test-body change, excluded case, relaxed assertion or count change is introduced. Historical evidence keeps the paths and result text actually observed at its recorded heads.

This is an authored integration failure, not the desired positive-assertion `MissingSourceJoin` RED. The source-cost job is still pending when this repair is prepared; it must independently establish that assertion in both source lanes. The fixing head must then execute the full unchanged matrix, including the isolation owner and all checks skipped after failures. Static review and byte-identical moves are not hosted GREEN.

### Consumed Source Cost Join Reaches Hosted RED

The completed [compiler-schema job at 6e3e293](https://github.com/IABTechLab/auctioneer/actions/runs/45720734385/job/139578237017) records these secure-source, development-residency and development-validation result lines:

```text
test result: FAILED. 976 passed; 1 failed; 0 ignored; 0 measured; 6 filtered out; finished in 1347.16s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 856 filtered out; finished in 91.75s
test result: FAILED. 138 passed; 1 failed; 0 ignored; 0 measured; 844 filtered out; finished in 393.20s
```

Both source failures are the existing actual-source-window case's first positive cost assertion: `conditional helper cost join 0: Err(MissingSourceJoin)`. All original closing/compiler/seed/view controls, semantic setup and independently checked conditional demand setup finish first. This is intended missing-behavior RED, not compilation, source setup, a unit-valued stamp or a zero-test filter. Later variants in that failed case are not yet executed acceptance. The graph remains 983 cases with the same six secure exclusions and 139 development validation cases. Compiler controls, explicit strict source-target Clippy and later staged checks are not reached after source failure and remain required on the fixing head.

The complete matrix has 21 successful checks and four failures: this intended schema failure plus the three native isolation failures documented above. None is dismissed. The local fix now checks consumed source semantics before every conditional demand, invokes the same borrowed-facts event evaluator as the original component contract, propagates its exact typed refusal and retains its real LocalContract in the result. The missing-join variant and stub are removed, not kept as a fallback. Zero calls cannot bypass source or invariant checks; all eleven unknown units and required frame/buffer/invariant duties remain unchanged.

The naming repair moves the same four files to `epsilon_closure`; only the primitive module's narrow join implementation changes their contents. All 25 required case-prefix changes preserve names below that module and the dispatch counts. No assertions, independent oracles, checker logic, exclusions, source captures, builds, workflow jobs, timeouts, dependencies, serving paths, reservations or limits are weakened or added. Pinned formatting and static review remain non-executable editing/review. Full fixing-head hosted GREEN and reached strict lint remain pending; this does not close #47 or make PR #268 ready for review.

### Consumed Source Cost Join And Isolation GREEN

At `627e4ca1e16a646abda7fd764c1df70c6d2f4a95`, all 25 hosted checks finish successfully, with no pending, dismissed or waived result. The completed [compiler-schema job](https://github.com/IABTechLab/auctioneer/actions/runs/45726730189/job/139598356678) records secure source, compiler controls, development residency and development validation, in that order:

```text
test result: ok. 977 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 1163.23s
test result: ok. 337 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 179.28s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 856 filtered out; finished in 96.34s
test result: ok. 139 passed; 0 failed; 0 ignored; 0 measured; 844 filtered out; finished in 329.15s
```

The actual-source-window case reports `ok` in both source lanes, including the later ledger/call/write, zero-call source-drift and exact refusal assertions. All 25 renamed required cases execute under `epsilon_closure`; the graph remains 983 cases, with six secure exclusions and 139 development validation cases. Both explicit strict source-target Clippy commands complete successfully: secure at `00:35:54Z` and development at `00:47:43Z` on 2026-11-26 UTC. Normal/staged core and metadata checks also pass.

The unchanged isolation owner executes all sixteen cases in [secure](https://github.com/IABTechLab/auctioneer/actions/runs/45726730189/job/139598356703), [default](https://github.com/IABTechLab/auctioneer/actions/runs/45726730189/job/139598356873) and [insecure](https://github.com/IABTechLab/auctioneer/actions/runs/45726730189/job/139598356742) lanes. Their result lines, respectively:

```text
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.44s
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.63s
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.84s
```

This is executed GREEN for the consumed-source-to-conditional-helper-cost bridge and the naming repair. It does not close the remaining common work/byte/cleanup/storage/stack, caller/invariant/namespace, numerical-policy or target obligations. Eleven helper units remain unproved, and all actual-use duties remain explicit. No local executable verification ran; pinned rustfmt was editing only. #47 remains open and PR #268 draft.

### Shared Source Coverage And Consumer Dependencies

The next tests-first slice prepares ordinary-library interpretation by extending the same matched source window from twelve to sixteen fixed files: `core/src/option.rs`, `core/src/lib.rs`, `core/src/prelude/mod.rs` and `core/src/prelude/v1.rs` join the existing twelve. It reuses the same bounded reader, descriptor capture and compiler/source/seed opening/closing checks; no loader, build, probe, target, capture or test case is added. Global coverage remains exact, not a minimum: each declared unit once, exact paths, valid digests, no extras and unchanged closing bytes.

The existing four source-window cases retain their assertions and now use independent literal-sixteen roster/count oracles. Their path/hash/read-error/closing-drift loops also cover the new files. A separate replay removes a present `CoreOption` record, which the NFA/Sparse helper does not consume. The owner must refuse it as exact `SourceCoverage`, run no consumer event and retain the closing trace. This guards global completeness independently of a consumer's narrower dependencies.

The helper's required twelve files and all literal cost, refusal and duty oracles remain unchanged. Its existing whole-window `source_count == PATHS.len()` guard is deliberately retained for hosted RED. The expected failure is the actual-source case's positive helper setup: a valid sixteen-file window produces `InvalidFacts("source_roster")` instead of success, after the preceding opening/closing and coverage controls. That is the unsupported consumer/global-set distinction, not a new missing-result stub or a cost proof for the added files. Expected RED is not executed evidence; it must be confirmed before removing that coupling. The eventual fix may require every explicit helper dependency, but must not weaken global coverage or substitute `>=12`.

These new source bytes are DATA only. In particular, the current core root also has later `#[macro_use]` providers `internal_macros` and `num`; merely loading the four preparation files cannot prove absence of namespace shadowing or close Option or builtin assert semantics. Those source/cost/namespace obligations remain unfinished. No local executable verification ran; pinned formatting and source review are not hosted RED/GREEN. Test names, the 983-case graph, six secure exclusions and 139 development validation cases remain unchanged.

### Shared Dependency Boundary Reaches Hosted RED

At `65857d0f3499806cffe08463384519373f6cfa92`, the completed [compiler-schema job](https://github.com/IABTechLab/auctioneer/actions/runs/45732868213/job/139618894759) records secure-source, development-residency and development-validation results:

```text
test result: FAILED. 976 passed; 1 failed; 0 ignored; 0 measured; 6 filtered out; finished in 1250.73s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 856 filtered out; finished in 66.26s
test result: FAILED. 138 passed; 1 failed; 0 ignored; 0 measured; 844 filtered out; finished in 341.25s
```

Both source failures are the actual-source case's positive helper setup: `Some(Err(SourceInterpretation(InvalidFacts("source_roster"))))` instead of `Some(Ok(()))`. The new global sixteen-file roster/count controls, unused-CoreOption opening refusal, prior closing/compiler/seed/view checks and post-capture control finish first. All three other source-window cases pass, including their extended sixteen-file read/path/hash/drift loops. This is intended consumer/global-count coupling RED, not compilation, missing-file setup, a zero-test filter or a semantic-proof claim for the new files. Later helper-cost/semantic variants within that failed case are not yet executed acceptance.

The complete matrix has 24 successful checks and only this expected schema failure. Compiler controls, explicit strict source-target Clippy and later staged checks are not reached after source failure; none is waived. The local correction removes only the helper's whole-window count equality. Its pinned-compiler checks, explicit twelve required dependency lookups, parse/root-attribute gates and shared semantic recipes remain unchanged. The owner still validates exact global sixteen-file coverage, paths, digests, opening/closing compiler/source/seed checks and all closing bytes before returning a result. No `>=12` replacement, optional dependency, alternate reader, fabricated inputs, fallback, test/oracle/filter change or authority conversion is introduced.

Pinned formatting and static review remain editing/review only. Full fixing-head hosted GREEN and reached strict lint are pending; all eleven unknown helper units and the remaining common/namespace/caller/target duties remain explicit. #47 stays open and PR #268 draft.

### Shared Dependency Boundary Reaches Hosted GREEN

At `a16d0276484b0e64924bbb60ba174591e74fcf91`, all 25 checks pass. The completed [compiler-schema job](https://github.com/IABTechLab/auctioneer/actions/runs/45737038146/job/139632661949) records secure source, compiler controls, development residency and development validation, respectively:

```text
test result: ok. 977 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 948.61s
test result: ok. 337 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 146.39s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 856 filtered out; finished in 54.42s
test result: ok. 139 passed; 0 failed; 0 ignored; 0 measured; 844 filtered out; finished in 243.13s
```

Both explicit strict source-target Clippy invocations and the unchanged normal/staged checks finish successfully. This completes the shared-coverage/dependency distinction only; the added standard-library files are not yet namespace or ordinary-helper proofs.

### Vendor Macro-Scope Controls Prepared For Hosted RED

The next controls extend the same window to twenty exact files with the vendored regex manifest, crate root, macro provider and util module. They retain all original coverage/path/digest/closing checks and the conditional helper-cost oracles. Owned replay mutations change these scopes and recompute their digests; no vendor, standard-library or build source is changed. The current helper ignores ancestor scope changes, so expected RED is a completed assertion accepting altered data where a fixed source-specific refusal is required, not a compilation or setup failure. Those expected failures are not executed evidence until hosted logs confirm them.

The supported vendor-scope recipe does not establish the remaining core export/prelude/builtin, selected cfg/artifact or actual caller/invariant facts. Complete validation work, byte traffic, cleanup, storage and stack remain incomplete. No cost field becomes zero, no serving flag changes, #47 remains open and PR #268 remains draft.

### Vendor Macro-Scope Controls Reach Hosted RED

At `9ed897dfb502ab0369c012bc1ba68bc537575b2d`, the completed [compiler-schema job](https://github.com/IABTechLab/auctioneer/actions/runs/45751111108/job/139678838452) records secure source, development residency and development validation:

```text
test result: FAILED. 976 passed; 1 failed; 0 ignored; 0 measured; 6 filtered out; finished in 1103.30s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 856 filtered out; finished in 60.76s
test result: FAILED. 138 passed; 1 failed; 0 ignored; 0 measured; 844 filtered out; finished in 289.86s
```

Both failures are the new `RootShadow` assertion: `Ok(())` instead of `Err(Interpretation(SourceInterpretation(VendorAncestry("RegexRoot"))))`. The existing actual-source case reaches this assertion after its original source/compiler/seed/closing checks, literal cost/refusal oracles and zero-call duties. The twenty-file opening and the three other source-window cases pass. This is missing ancestor-recognition RED, not compilation, unavailable data or a new stub. The remaining ten mutation rows and all 22 per-demand cost-refusal assertions are not executed acceptance until fixing-head GREEN. Compiler controls, explicit strict source-target lint and later staged checks are also not reached after source failure.

The fixing implementation adds one finite vendor-ancestry recognizer to the same existing helper-setup path before conditional cost evaluation. It borrows four already consumed records, uses the existing TOML parser for the edition/library-path fields, and compares ordered typed root/macro-provider/util syntax with independently written supported recipes. Only literal documentation is ignored; executable attributes, import/module order, paths and macro bodies remain checked. No filesystem reader, cost evaluator, capture, build, case, target, workflow change, dependency or serving behavior is introduced. All tests and literal oracles from RED remain unchanged.

Static spec and quality reviews find no concrete blocker; pinned rustfmt is editing only. Fixing-head hosted GREEN, explicit strict lint and the full matrix remain pending. The recognizer does not resolve core exports/preludes/builtins or establish the regex compilation unit's effective cfg/mode; in particular the preserved doctest invocation is not proved inactive by a bidder build-script environment or resolved-feature list. Those facts and all remaining common derivations stay open, not zero costs or qualification credit.

### Vendor Ancestry Assertions Pass; Strict Lint Refuses

At `8a54ea38fad17acd5290ee8151fd849d6aefed16`, the completed [compiler-schema job](https://github.com/IABTechLab/auctioneer/actions/runs/45754375576/job/139689522940) records secure source, compiler controls, development residency and development validation:

```text
test result: ok. 977 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 1179.06s
test result: ok. 337 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 174.46s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 856 filtered out; finished in 95.88s
test result: ok. 139 passed; 0 failed; 0 ignored; 0 measured; 844 filtered out; finished in 347.52s
```

The actual-source case passes in both lanes, including all eleven vendor mutations, all 22 positive/zero-demand cost-refusal assertions, exact closing traces and earlier cost/window oracles. However, both explicit strict source-target Clippy invocations fail with 15 diagnostics each: two oversized fixture functions, five wildcard enum matches, two `unreachable!` uses, two unchecked arithmetic sites and four shadowed names. Those are real failed checks, not successful verification. The completed matrix is 24 successful checks and this one failed schema check; no result is waived. Later staged acceptance after the lint failure is not reached.

The lint repair preserves the accepted four item classes and rejection of every other current/future item, using explicit `if let` branches and distinct names. The independent twenty-entry roster remains literal and separate from the implementation roster, now in a private test constant. The two before-state mutation operations are extracted without removing their assertions; the manifest-field mapping is exhaustive; checked arithmetic retains exact removal and 22-execution assertions. All eleven mutation identities, expected outcomes, test names, source reads, costs and limits remain unchanged. No lint allowance, test removal, compatibility path or workflow change is added.

Full hosted tests and strict lint on the repairing head remain pending. These passing ancestry assertions close no core/prelude/cfg/artifact or remaining work/storage/stack obligation; #47 stays incomplete and the PR stays draft.

### Vendor Ancestry And Strict Lint Reach Full Hosted GREEN

At `ec302cf91cdfb0991bd0f4e75847ab122e688c64`, all twenty-five hosted checks pass. The completed [compiler-schema rerun](https://github.com/IABTechLab/auctioneer/actions/runs/45759128227/job/139721386288) executes the existing source/compiler scripts and every normal/staged secure/development artifact stage. Actual principal result lines, secure source, compiler controls, development residency and development validation:

```text
test result: ok. 977 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 1231.85s
test result: ok. 337 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 183.81s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 856 filtered out; finished in 95.03s
test result: ok. 139 passed; 0 failed; 0 ignored; 0 measured; 844 filtered out; finished in 356.63s
```

Both actual-source executions complete every earlier assertion, all eleven vendor mutations and all twenty-two positive/zero-demand refusal assertions. Both explicit strict source-target Clippy invocations pass; secure/development normal/staged core checks each execute four cases, and normal/staged metadata checks each execute two. The final development staged result is `2 passed; 0 failed; 0 ignored; 0 measured; 976 filtered out; finished in 3.09s`. The rerun completes in 40 minutes 21 seconds. The twenty Run Tests jobs succeed, as do the five same-head formatting/Clippy/provider checks. Completed logs and exact-head checks were downloaded/read back; no local executable verification ran.

The [first attempt](https://github.com/IABTechLab/auctioneer/actions/runs/45759128227/job/139704697824) at that same head exceeded the unchanged forty-five-minute deadline during the final development staged compilation. It is cancelled, not passed or waived. That attempt's principal results were secure source `977/0` in 1378.06 seconds, compiler controls `337/0` in 199.18 seconds, development residency `127/0` in 91.85 seconds and development validation `139/0` in 430.17 seconds. Both strict source-target lint invocations completed before cancellation. No command, assertion, exclusion, timeout, limit, reservation or workflow changed between attempts. A successful rerun is not evidence of a performance improvement or a reproduced-base finding about the timeout's cause.

This completes the supported vendor-ancestry substep only. Core/prelude/builtin routes, ordinary helper semantics, actual cfg/source/artifact/caller correspondence and the remaining work/storage/stack derivations are not supplied by that GREEN.

### Core Assert-Route Controls Prepared For Hosted RED

The next tests-first checkpoint adds fourteen independent core-route changes to the same actual-source case, after all existing assertions. It reuses the same descriptor, twenty source records, bounded source reader, compiler probe, replay/closing kernel and conditional cost evaluator. There is no new source/capture/build/test target or actual test case; the full graph remains 983 cases and development validation remains 139. New conformance-support helpers use ordinary child modules, not handwritten source imports.

The controls independently redirect or disable the core root's prelude and macro-provider modules; redirect or disable the v1 prelude file; disable or reroute the 2021 prelude; alter each relevant root's cfg; alias the explicit assert import; disable the builtin module; and relocate its direct assert child below another inline module. Only owned replay bytes change and are rehashed consistently. Each control preserves NFA/Sparse/ID bodies, checks exact closing events, and expects one fixed source-specific `CoreAncestry` refusal plus the same refusal at positive and zero sparse demands. The exact per-demand assertion count is twenty-eight. Every earlier literal helper-cost, invariant, buffer, frame and closing oracle remains.

No core-route implementation is present at preparation: intended RED is the old checker returning `Ok(())` for an altered route, not a compile/setup error or an unused test. The finite routing lemma will remain conditional on successful compilation of the same supported core source by the pinned compiler. It will not prove that compilation/source/artifact premise, vendor-leaf name lookup, actual invocation/receiver/cfg identity, builtin expansion costs, ordinary cleanup or qualification. Unsupported or missing common semantics stay incomplete, not zero or new host services. The canonical report remains `Incomplete`; #47 stays open and PR #268 stays draft.

### Core Assert-Route Controls Reach Hosted RED

At `c26507984b2630dc7b74ea5c8840157de1ebc1f3`, twenty-four hosted checks pass and the [compiler-schema job](https://github.com/IABTechLab/auctioneer/actions/runs/45773315345/job/139750894378) reaches the intended assertion failure in both feature lanes. Completed logs, fetched with `gh run view 45773315345 --job 139750894378 --repo IABTechLab/auctioneer --log`, report secure source, development residency and development validation:

```text
test result: FAILED. 976 passed; 1 failed; 0 ignored; 0 measured; 6 filtered out; finished in 1427.21s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 856 filtered out; finished in 91.81s
test result: FAILED. 138 passed; 1 failed; 0 ignored; 0 measured; 844 filtered out; finished in 427.38s
```

Both failures are `assertion left == right failed: LibPreludePath`: `left: Ok(())`, `right: Err(Interpretation(SourceInterpretation(CoreAncestry("CoreLib"))))`. Both failing steps return exit 101. The same actual-source case reaches that assertion after every original source/cost/invariant/window/vendor control. The complete graph is still 983 cases and the same six secure exclusions remain. This is missing route recognition, not a missing symbol, parser/setup error, layout warning or zero-test selection. Compiler controls, strict source-target lint and later success-gated stages are not reached in this failed job and are not credited as passing. The remaining thirteen core changes and all twenty-eight cost-refusal assertions require fixing-head GREEN.

The follow-up recognizer consumes the same twenty-source window after the existing body recipes and vendor checks. It reuses the already parsed, body-checked macro file rather than reading/parsing it again or adding another assert-body classifier. Supported root attributes, core module declarations, the complete 2021 prelude module and explicit v1 assert import are compared structurally with independently authored typed recipes. The unconditional builtin header and direct assert child are required. Only literal documentation is ignored; unknown header fields, executable attributes, paths and token changes are preserved by whole-item comparison. No actual compiler/cfg/caller/leaf-lookup/builtin-work obligation is discharged by this conditional routing lemma.

The handoff and primitive source owners move mechanically to their `source/mod.rs` locations under the existing module-file rule; module identities, test names and data paths remain unchanged. Child modules use ordinary declarations. No original assertion, test selection, cost/duty oracle, reader, capture, build, dependency, workflow, timeout, reservation, limit or serving path is changed. Static review and pinned rustfmt are not executable verification. Full fixing-head hosted tests, both strict lint lanes and artifact acceptance remain pending; #47 and the draft PR remain incomplete.

### Core Route Assertions Pass; Documentation Lint Refuses

At `5a0cb008cd3dad833735cc9c955b23939ad1c35b`, the completed [compiler-schema job](https://github.com/IABTechLab/auctioneer/actions/runs/45781340318/job/139777289033) executes secure source, compiler controls, development residency and development validation. Its complete log was fetched through `gh api --allow-escape-sequences repos/IABTechLab/auctioneer/actions/jobs/139777289033/logs` after the job completed; the flag permits capturing the log's ANSI bytes as data. Principal result lines, in that order:

```text
test result: ok. 977 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 1002.16s
test result: ok. 337 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 143.29s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 856 filtered out; finished in 54.39s
test result: ok. 139 passed; 0 failed; 0 ignored; 0 measured; 844 filtered out; finished in 269.32s
```

The actual-source case passes in both source lanes, including all fourteen core controls, all twenty-eight per-demand refusals, every earlier vendor/window/compiler/seed check and the original literal cost/duty oracles. This is executed assertion acceptance, not complete CI GREEN. Both explicit strict source-target Clippy invocations return exit 101 with the same sole diagnostic: `doc paragraphs should end with a terminal punctuation mark` at the second reference URL in `primitives/source/core_routes/mod.rs:16`. The source checks and compiler controls pass; the lint does not. Later success-gated normal/staged acceptance is not reached and is not credited. The correction adds the missing terminal period without changing a matcher, test, assertion, lint rule, command, timeout or limit. Fixing-head hosted lint/full acceptance is still required.

### ID Import Projection Controls Prepared For Hosted RED

A static follow-up audit finds that the existing ID-source matcher requires its `Vec` import but ignores additional imports. The next tests-only checkpoint retains the same actual-source case/capture and adds six independent owned replay changes to `Ids`: named, renamed, grouped-renamed, glob, existing-core-import expansion and cfg-gated imports. Every variant starts from the independently authored exact three-import shape, keeps the alloc-gated `Vec` import and all non-import ID syntax unchanged, preserves the other nineteen source records and recomputes its digest. Replay supplies identical changed bytes at opening and closing. There is no vendor or standard-library source edit.

A literal-documentation positive retains the existing conditional cost projections and duties through the same evaluator. Each negative requires exact `UnsupportedBody(Ids)` from setup and from both positive and zero sparse demands after the existing closing trace; the independent demand-refusal count is twelve. Earlier core/vendor/window/cost controls and all test names/counts remain unchanged. These controls add no case, target, reader, build, capture, dependency, evaluator, cost field or serving route. Their ordinary child module stays with the conformance owner.

The import recognizer is intentionally unchanged at preparation. Expected RED is its wrong acceptance of the first added import, not a compilation, pristine-shape, documentation-positive or zero-dispatch failure. That expectation is not executed evidence until hosted logs confirm it. Static spec/quality reviews and pinned rustfmt are not executable verification. This slice supports only an import-shape projection: custom macro/attribute producers, actual derive/compiled-source/cfg/caller/artifact identity and complete work/cleanup/storage/stack remain unproved. The canonical report stays `Incomplete`, all qualification/fit/serving flags stay false and #47 remains open.

### ID Import Projection Reaches Hosted RED

At `157e374a6b7c25dc02748df7456d20b48fe9f085`, the completed [compiler-schema job](https://github.com/IABTechLab/auctioneer/actions/runs/45787361711/job/139797419785) reaches the intended import assertion in both source lanes. Its full completed log, retrieved through `gh api --allow-escape-sequences repos/IABTechLab/auctioneer/actions/jobs/139797419785/logs`, reports secure source, development residency and development validation:

```text
test result: FAILED. 976 passed; 1 failed; 0 ignored; 0 measured; 6 filtered out; finished in 917.66s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 856 filtered out; finished in 57.15s
test result: FAILED. 138 passed; 1 failed; 0 ignored; 0 measured; 844 filtered out; finished in 250.14s
```

Both failures are `assertion left == right failed: Named` in `handoff/source/tests/leaf_imports.rs:248`: `left: Ok(())`, `right: Err(Interpretation(SourceInterpretation(UnsupportedBody(Ids))))`. Both failing steps return exit 101. The same actual-source case first completes the earlier core/vendor/window/compiler/seed checks, literal cost/duty assertions and the new documentation-positive projections. The new independent pristine-import assertion also passes before the failure. This is observed wrong acceptance, not a missing symbol, parser/setup error or zero-test selection. The remaining five import variants and all twelve cost-refusal assertions still require fixing-head GREEN. Compiler controls, strict source-target lint and later success-gated normal/staged checks are not reached in this failed job and are not credited as passing; the punctuation repair's strict-lint verification remains pending too.

The full matrix must still be completed and re-read before its final result is reported. No failing check is dismissed. The narrow correction now matches the complete supported ID-source import set through the existing shared recipe path. It compares whole imports after the existing supported literal-documentation normalization; sorting preserves duplicates while ignoring use-item order. Both supplied-data and consumed-source paths reach it before conditional work interpretation, including at zero demand. The original direct required-`Vec` import check, Sparse gate, helper/body recipes and all tests/oracles are byte-identical. No second evaluator, source reader, capture, build or cost unit is introduced.

Static spec and quality reviews find no concrete issue in the 36-line code change; pinned rustfmt is editing only. Full fixing-head hosted GREEN, both strict lint lanes and later normal/staged acceptance remain required and unexecuted at this checkpoint. The import projection supplies no actual builtin-derive or source/caller/cfg/artifact authority, ordinary cleanup bound, common completeness, physical fit or serving permission. #47 and the draft PR remain incomplete.

### ID Import Projection Passes The Full Hosted Matrix

At `c8be05b8dc6aaba1c3d0037e1a45bde4b73d18cc`, all twenty-five hosted checks pass. The completed [compiler-schema job](https://github.com/IABTechLab/auctioneer/actions/runs/45793901843/job/139819522420), downloaded with `gh run view 45793901843 --job 139819522420 --repo IABTechLab/auctioneer --log`, records secure source, compiler controls, development residency and development validation:

```text
test result: ok. 977 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out; finished in 1051.92s
test result: ok. 337 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 163.12s
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 856 filtered out; finished in 55.09s
test result: ok. 139 passed; 0 failed; 0 ignored; 0 measured; 844 filtered out; finished in 287.22s
```

The actual-source case completes all six import controls and twelve positive/zero-demand refusals in both lanes, after every earlier source/cost/window control. Both explicit strict source-target Clippy invocations pass. Secure/development normal/staged core checks each execute four cases and metadata checks each execute two. The last development staged result is `2 passed; 0 failed; 0 ignored; 0 measured; 976 filtered out; finished in 1.85s`. Every Run Tests, formatting/lint and provider check succeeds at that exact head. This is full hosted acceptance of that partial implementation, not completion of #47; its canonical report still says `Incomplete`.

### Whole-Feature Completion Gate Prepared For Hosted RED

The next tests-first checkpoint makes the existing current-source report case invoke the real `current_report` entrypoint after its original diagnostic, coverage and refusal controls. It requires `CommonSourceComplete` and complete storage, work, structural-stack, shape-correspondence and attempted-work derivations. It still requires `TargetIncomplete`, unevaluated numerical policy, unestablished attempted-work funding and false qualification/fit/serving flags. The existing current-body and actual-application attachment expectations are reconciled to the same canonical completion contract; no-proof diagnostic and forged/hybrid refusals remain separate and unchanged.

This deliberately removes the false completion signal from a suite whose real-source assertions accepted an incomplete diagnostic. It adds no test case, capture, source reader, build, target, job, limit, dependency or serving path. Existing GitHub owners must execute the semantic RED and then the complete implementation's GREEN. No local executable verification is run. At preparation, expected RED is the existing canonical reporter returning `Incomplete`, not a compiler/setup error or zero-test selection. It is not executed evidence until the hosted logs confirm it. Later completion assertions are not credited until reached. A status-only change cannot satisfy the required underlying source derivations, and no missing common proof may be moved to a target-fact placeholder.

### Refreshed Archive Reaches The Whole-Feature Assertion

The first whole-feature checkpoint at `aa8f5182f1f786a403d236fe7e6a66ddd632cb2a` cannot reach source tests: the baked GCL expires before compilation. The [compiler-schema job](https://github.com/IABTechLab/auctioneer/actions/runs/45889238565/job/140147510968) reports `baked GCL is stale: expired 2026-11-26 16:01:09.631887112 +00:00:00`, with exit 101. That is a prerequisite failure, not semantic RED.

The maintainer merged #269 into main as `7f0ee8a97dab947cf9c75024a47a980fb8411262`. A normal main merge integrates its four archive-file changes without conflicts into `e03d90c8cdb88bd7128422ad82ace50bb86a119c`. `git diff HEAD^ --check` returns exit 0. No freshness override, test, timeout, workload or resource policy changes.

The completed [Axum application job](https://github.com/IABTechLab/auctioneer/actions/runs/45891310955/job/140154411686) now builds and executes the real application attachment case. Its log, downloaded with `gh api --allow-escape-sequences repos/IABTechLab/auctioneer/actions/jobs/140154411686/logs` (exit 0), records:

```text
thread 'tests::actual_application_artifacts_axum_retain_every_selected_validator_in_both_feature_lanes' (5572) panicked at crates/auctioneer-bidder/tests/schema_source_conformance.rs:848:9:
assertion `left == right` failed
  left: String("Incomplete")
 right: "CommonSourceComplete"
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 982 filtered out; finished in 187.85s
Process completed with exit code 101.
```

The source-projection output precedes that assertion; this is the intended missing common-completion behavior, not compilation, stale-input or zero-test failure. The first failing feature lane aborts that case; later lane assertions are not credited. Other current-run jobs remain pending at this recording, so this is not a full-matrix result. #47 and the draft PR remain incomplete.

### Helper Logical Paths Prepared For Hosted RED

The next tests-first checkpoint extends the existing conditional helper contract, not another source reader or stack evaluator. The original matched-callee event rows already own the concrete helper frames and sequential calls. Ordered simultaneous root-to-leaf alternatives are still missing, so the seam deliberately returns `None` until its semantic assertion RED is observed. The final successful contract must always carry the derived paths; the missing-path seam is not a fallback to retain.

Independent hand-authored oracles require eight NFA paths and sixteen sparse-insert paths, exact frame union and unit-weight peaks eleven/thirteen. Changing the Arc and Vec frame weights independently proves sequential alternatives, not simultaneous summation. Sparse equality weights thirteen/eleven force peak twenty-six and retain the live Insert/Contains ancestors around the frameless RHS fragment. Zero demand retains every structural obligation. Private row corruptions cover missing/cyclic/unreachable nodes, an ordinary frame disappearing and a fragment acquiring an invented frame. These are source/path controls and fictional algebra, never physical frame facts or a target qualifier.

All prior helper counts, three partial work components, symbolic writes, required buffers, invariant/source/window controls and test identities remain. The existing actual-source projection oracle checks paths only after its earlier assertions; no test case, reader, capture, build, target, job, dependency, limit or serving behavior is added. This local lemma does not complete common storage, work, stack, attempt or token correspondence. Fresh hosted helper-path RED and subsequent implementation/GREEN remain required; no local executable verification runs.

### Helper Logical Paths Reach Hosted RED

At `8305f69061244612c74e9e428482bf27957f4a39`, the existing [compiler-schema job](https://github.com/IABTechLab/auctioneer/actions/runs/45893630821/job/140162180276) builds and executes the source suite. Its emitted log is downloaded through `gh api --allow-escape-sequences repos/IABTechLab/auctioneer/actions/jobs/140162180276/logs` (fetch exit 0). The completed secure source invocation records:

```text
thread 'schema_validation::handoff::source::tests::actual_sources_are_consumed_inside_the_bound_window' (34128) panicked at crates/auctioneer-bidder/tests/schema_validation/work/epsilon_closure/primitives/mod.rs:813:29:
matched helpers still lack simultaneous frame paths
test result: FAILED. 962 passed; 15 failed; 0 ignored; 0 measured; 6 filtered out; finished in 1238.59s
Process completed with exit code 101.
```

The existing NFA, sparse-insert, zero-invocation and positive-first refusal controls also fail their extended `assert_local` equality at line 1728: the contract returns `None`, not the independently expected paths. Twelve primitive cases and the actual-source case expose that missing local feature; the two canonical-report/body cases retain their separate `Incomplete` versus `CommonSourceComplete` failures. This is semantic RED after compilation and the earlier per-case assertions, not a source/path/setup error.

The job subsequently completes with failure. Its final log records development residency, followed by development validation:

```text
test result: ok. 127 passed; 0 failed; 0 ignored; 0 measured; 856 filtered out; finished in 90.81s
test result: FAILED. 124 passed; 15 failed; 0 ignored; 0 measured; 844 filtered out; finished in 296.20s
Process completed with exit code 101.
```

Both source lanes reach the same missing-path assertion and the separate whole-feature failures. Compiler controls and explicit strict source-target lint are not reached after source failure. Normal/staged secure core, secure metadata and development core stages are skipped; they are not passing evidence. No failing check is dismissed and no requirement is weakened.

The final same-head check readback reports twenty successful and five failed checks, all completed: the source/compiler owner and the four real-application attachment owners fail. No cancelled or pending check is counted as passing. The later correcting-head test and lint results remain required, independently of these predecessor results.

The narrow implementation now replaces the missing-path seam with a checked walk over the original row table, retaining parent frames across each nested alternative and attaching only the two source-bound frameless fragments. Successful contracts must carry concrete paths; the temporary `Option`/`None` state is removed rather than kept as a compatibility fallback. Work dimensions and remaining source/target obligations are not changed. Static review, full fixing-head assertions and strict lint remain required; no local executable verification runs.

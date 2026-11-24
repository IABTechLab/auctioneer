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

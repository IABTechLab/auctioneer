# Ordered Fastly deployment implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deploy every eligible main merge to the selected standalone Fastly
service in source order, using immutable releases, EdgeZero actions and private
runtime configuration.

**Architecture:** Preserve the existing parallel CI owners, add credential-free
release production, and select one pending main push head per serialized
coordinator invocation. GitHub Deployment records retain ordering and recovery;
the protected consumer privately retrieves and validates the Environment's TOML
and delegates provider mutations to EdgeZero.

**Tech stack:** GitHub Actions, EdgeZero's existing composite actions, Python
standard-library workflow helpers, Bash entry points and the existing Auctioneer
CLI/Fastly adapter. No new application dependency or public Rust API.

**Approved specification:** [Fastly main deployment design](../specs/2026-11-18-fastly-main-deployment-design.md),
approved at `9024c1c93a6de22a8e7dc7d63aba3a50fdbcd9c1` for
[#247](https://github.com/IABTechLab/auctioneer/issues/247).

This is a planning document. All commands and pass/fail expectations below are
future implementation steps, not executed evidence. Implement only after this
planning PR is reviewed and merged into the actual default branch. Create a
separate implementation worktree and PR with `Closes #247`; this PR uses
`Refs #247`. Publishing this planning PR is authorized; merging or deploying is
not authorized by that publication decision.

---

## File ownership and fixed interfaces

| File | Responsibility |
| --- | --- |
| `.github/workflows/ci-fastly.yml` | Preserve current guest/vendor/size checks; publish push receipts; expose the exact package artifact identity. |
| `.github/workflows/prepare-fastly-release.yml` | Credential-free CLI build and immutable release assembly from the matching Fastly package. |
| `.github/workflows/deploy-fastly.yml` | Register main push intents, serialize selection and protected lifecycle actions, and perform bounded catch-up/recovery. |
| `.github/workflows/test.yml`, `format.yml`, `ci-cloudflare.yml`, `ci-spin.yml` | Preserve main CI per SHA and PR supersession; host the relevant helper/fixture checks without removing existing checks. |
| `.github/workflows/scripts/deploy-fastly/github_api.py` | Fixed-host, bounded GitHub metadata transport; explicit read-only versus controller methods. |
| `.github/workflows/scripts/deploy-fastly/coordinator.py` | Pure selection/transition rules plus private workflow commands for intent, admission and outcome records. |
| `.github/workflows/scripts/deploy-fastly/release.py` | Validate push/release receipts and bind archive/artifact/run/attempt identity. |
| `.github/workflows/scripts/deploy-fastly/private_config.py` | Private Environment input retrieval, masking and bounded typed validation. |
| `.github/workflows/scripts/deploy-fastly/smoke.py` | Bounded service-owned hostname selection and Auctioneer-specific hosted probe assertions. |
| `.github/workflows/scripts/deploy-fastly/verify_log_safety.py` | Inspect completed fixture-job logs, summaries and artifact allowlists for independent sentinels. |
| `.github/workflows/scripts/deploy-fastly/tests/run_units.py` | Discover inline helper units, require the independent critical-case roster and nonzero dispatch, and reject skips/errors/failures. |
| `.github/workflows/scripts/deploy-fastly/tests/exercise-edgezero-wrappers.sh` | Integration owner for real pinned action wrappers with upstream capable fake fixtures. |
| `.github/workflows/scripts/README.md` | Invocation and prerequisites for the new workflow helpers. |
| `docs/deployment/fastly-main.md`, `README.md` | Operator setup, guarantees, recovery and discoverability. |

Keep pure Python unit cases inline in their production helper under
`unittest.TestCase`. Importing a helper must not execute its workflow command.
Keep multi-process/action integration controls under the owning `tests/`
directory. Use standard-library transport and JSON serialization; no PyPI
dependency, runtime API-host override, YAML-to-Rust snapshot or copied provider
implementation. The hosted runner must provide Python with `tomllib`; check that
prerequisite explicitly before accessing configuration.

Use the reviewed action reference
`e11ea3082f66ec864aabdd6007b15c4926c73b82` consistently for this adoption. Before
implementation, verify that object is available and review any proposed upgrade
as one change across every action checkout and reference. Application Rust and
tool versions remain owned by workspace metadata, `Cargo.lock` and
`.tool-versions`.

### Persisted and in-job data

Use schema version 1 with strict known fields. A main push receipt contains
`repository_id`, `ref`, `before`, `after`, `workflow_id`, `run_id` and
`run_attempt`. A release receipt additionally identifies the immutable release
artifact name/ID and archive/package digests. Bind each receipt to its GitHub
producer metadata; never trust its claimed SHA alone.

Use separate controller task kinds for enrollment, intent, CI-ineligible
disposition, provider attempt and completion/checkpoint. Records identify their
stream, predecessor, source push head, owning controller run/attempt, accepted CI
attempts and release. Provider attempts also bind the config digest and target
fingerprint. A safe checkpoint can carry known numeric provider versions and
closed phase/mutation/recovery verdicts. No raw TOML, service ID, token, provider
body or parser excerpt belongs in a record or artifact.

Treat provider recovery as the first decision, independent of newer CI results.
The selection function's precedence is:

```python
def select_next(stream, receipts, ci, releases, attempts):
    unresolved = attempts.unresolved_for_stream(stream.id)
    if unresolved is not None:
        return Recover(unresolved)  # Never becomes CI-ineligible after mutation.
    candidate = receipts.next_after(stream.cursor)
    if candidate is None:
        return Idle()
    if not ci.all_required_attempts_terminal(candidate.after):
        return Wait(candidate)
    if not ci.all_required_attempts_successful(candidate.after):
        return RecordIneligible(candidate, ci.accepted_attempts(candidate.after))
    release = releases.verified_for(candidate.after)
    if release is None:
        return Block(candidate, "release_unavailable")
    return Admit(candidate, release, ci.accepted_attempts(candidate.after))
```

Define these result types as frozen dataclasses; keep this a private workflow
boundary, not a reusable deployment framework. All side effects happen after the
pure decision and under the same workflow lock. A blocked decision never updates
the cursor. Native Deployment statuses/control records must distinguish queue
metadata from a healthy provider deployment.

## Task 1: Receipt validation and private GitHub transport

**Files:** `github_api.py`, `release.py`, and their inline unit tests.

- [ ] Write independent rejection controls for a wrong repository/ref, PR event,
  wrong head/run/attempt, duplicate/unknown receipt fields, malformed SHA/digest,
  ambiguous artifact, expired artifact, missing page and truncated response.
  Vary these independently from one known-good dictionary fixture.
- [ ] Add fake-transport cases for 401/403/404/429/5xx, redirect to another host,
  response over 1 MiB, timeout and the 60-request budget. Assert a failed read
  cannot look like an empty result, and error output contains no response body.
- [ ] Add the unit launcher under `tests/run_units.py`. Discover `*.py` from the
  helper directory, flatten suites, require nonzero cases and an independent
  roster initially containing `github_api.ApiTests.test_failure_not_empty` and
  `release.ReleaseTests.test_attempt_identity`. Extend that hard-coded roster in
  Tasks 2 and 4 when their owning cases land; no runtime phase/skip switch is
  added. Run every discovered case and reject skipped/error/failed cases; a
  missing case from the current committed roster is not a passing run.
- [ ] Run the new helper unit command below. Expected before implementation:
  the named cases fail because the transport/receipt validators are absent;
  retain the actual failing output and nonzero status, rather than accepting a
  missing launcher/import error as the intended failing branch.
- [ ] Implement fixed `https://api.github.com` resource construction with encoded
  path components, redirect refusal and 10-second request timeouts. Pass tokens
  as parameters from their owning step, never CLI arguments. The Environment
  reader permits GET only; the controller permits only its required metadata and
  Deployment operations. Do not reuse the read credential for controller writes.
- [ ] Validate GitHub artifact digests and receipt provenance before accepting
  the archive's expected hash. Use attempt-specific names and explicit artifact
  IDs. Preserve a release selected before provider mutation across all retries.
- [ ] Rerun the unit command and require every owned case to execute. Expected:
  zero failures/errors, including the independent good path. Commit these
  focused helpers and controls with an imperative title.

Unit command for every helper task, from the worktree root:

```sh
python3 .github/workflows/scripts/deploy-fastly/tests/run_units.py
```

Report actual discovered/executed counts; do not invent a frozen count before
the cases exist. Avoid naming a helper `queue.py`, which would shadow Python's
standard-library queue when capture or fake-transport controls use threads.

## Task 2: Source-order queue, enrollment and restart rules

**Files:** `coordinator.py`, `release.py`; inline pure tests and controlled Git-history
fixtures under `tests/`.

- [ ] Construct a three-push chain A, B, C with distinct before/after identities.
  Complete C and B CI first and assert no provider admission before A is
  classified. Then assert the admitted order is A, B, C, once each.
- [ ] Add rebase/multi-commit controls: a push interval can contain intermediate
  first-parent commits, and only its resulting head is a candidate. Locate the
  workflow-introducing change inside the first enrolled push interval. An
  out-of-order notification must not move enrollment to B and lose A.
- [ ] Add missing receipt, broken predecessor, force-push, duplicate wakeup,
  exhausted pagination and changed-target controls. Assert an unresolved provider
  attempt wins over a later failing CI rerun. Test CI-ineligible advancement and
  refusal to deploy an old rerun after a successor has been processed.
- [ ] Extend the launcher's committed required roster with
  `coordinator.QueueTests.test_source_order` and
  `coordinator.QueueTests.test_recovery_precedes_ci_failure`. Keep Task 1's
  required cases and all discovered tests.
- [ ] Run the helper command and retain the intended failures. Implement strict
  schema/ancestry/owner validation and the selection precedence above. Use the
  controller task plus selected Environment to find its current validated record
  and follow explicit predecessor IDs. Inspect history forward from the cursor,
  not the entire deployment history on every wakeup.
- [ ] Persist intent/disposition/attempt/completion state through GitHub native
  Deployment records with `auto_inactive: false` on controller-owned status
  updates. Validate the owning workflow/run before trusting a cursor. Record
  provider checkpoints without changing an immutable attempt's release/config
  identity. Missing or conflicting state remains blocked.
- [ ] Rerun all queue cases, including a simulated process death after provider
  success but before completion recording. Expected: the next selection recovers
  A, with no B mutation. Commit the queue and its executed evidence.

## Task 3: Preserve CI and produce exact application releases

**Files:** the five existing main workflows, new
`prepare-fastly-release.yml`, `release.py`, and the workflow-script README.

- [ ] Add receipt producer controls and review the actual YAML graph. The main
  source SHA is always the checked-out push head. PRs continue to execute all
  existing checks but cannot create a deployable main receipt or access an
  Environment. Preserve each existing required check name and workload.
- [ ] Change main concurrency to include its source SHA; keep PR concurrency
  based on the PR ref with cancellation enabled. Do this across tests, format,
  Fastly, Cloudflare and Spin; leaving one main producer cancellable breaks
  eligibility for rapid merges.
- [ ] Publish an immutable push receipt at the beginning of Fastly main CI,
  before heavy build steps. Preserve its artifact on later test/build failure.
  Give package artifacts attempt-specific identities and expose their name/ID
  to the assembly job; update their actual consumers together.
- [ ] Build `auctioneer-cli` with its default application feature using
  EdgeZero's `build-app-cli` in an unbound, credential-free job. Keep native CLI
  compilation off the existing Fastly guest/packager critical path. Use the
  repository's toolchain and download-cache policy.
- [ ] Invoke credential-free CLI production and assembly for PRs as well as main,
  so the real packager/lifecycle compatibility is exercised before merge. Bind a
  PR candidate to its actual checked-out merge-ref SHA and mark its receipt's
  event accordingly. Only a verified main **push** producer can supply the
  deployment queue; a successful PR candidate archive is not deployable input.
- [ ] Assemble only after the matching vendor build and CLI jobs succeed. Use
  `package-application-release-fastly`, with the source SHA's `edgezero.toml` and
  `fastly.toml`, and the already measured `auctioneer.tar.gz`. Publish the strict
  archive and receipt, retain the upstream 14-day window, and record actual
  artifact expiry. No credentialed job builds or substitutes a package.
- [ ] Run `actionlint` on the five edited and two new workflow files and the
  helper unit command. Verify actual upstream packager execution with the real
  application CLI in CI. Keep these checks separate from provider execution.
  Commit the complete producer/CI change without raising the package ceiling.

The assembly mapping is fixed:

```yaml
uses: IABTechLab/edgezero/.github/actions/package-application-release-fastly@e11ea3082f66ec864aabdd6007b15c4926c73b82
with:
  app-cli-archive: release-inputs/cli/app-cli.tar
  fastly-package: release-inputs/fastly/auctioneer.tar.gz
  application-manifest: application/edgezero.toml
  adapter-manifest: application/fastly.toml
  source-revision: ${{ needs.select_source.outputs.source_revision }}
  artifact-name: application-release-${{ github.run_id }}-${{ github.run_attempt }}
```

`select_source` above denotes the credential-free producer's verified checkout
identity. Define that concrete job/output when wiring the reusable producer;
never source it from the publisher Environment. Retain the unchanged Fastly
vendor build and size gate before making its package an assembly input.

## Task 4: Private variable retrieval and typed validation

**Files:** `private_config.py`, `github_api.py`, inline unit controls and private
process integration fixtures.

- [ ] Add independent config/token sentinels and capture every output stream.
  Test a valid multiline value, percent/CR/LF escaping, empty/whitespace-only
  config, output-delimiter collision, optional selector absence versus blank,
  invalid UTF-8/JSON/TOML, denied API access, oversized responses and cancellation.
- [ ] Extend the launcher's committed required roster with
  `private_config.PrivateConfigTests.test_no_input_before_mask`. The resulting
  complete roster, including Tasks 1 and 2, is mandatory in final CI.
- [ ] Implement retrieval from the frozen selected Environment using
  `AUCTIONEER__DEPLOY__ENVIRONMENT_READ_TOKEN` with repository Environments read
  access. Fetch `AUCTIONEER__CONFIG_TOML` once. Fetch the two optional canonical
  selectors for `auctioneer_config` and `auctioneer_secrets` through the same
  reader. An optional-variable 404 is absence; a required-config 404 is failure.
- [ ] Register masks before any raw value reaches an action input or step `env`.
  Use escaped runner commands, with no diagnostic copy of their payloads:

```python
def escape_command_data(value):
    return value.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")

def register_mask(value, command_stream):
    command_stream.write("::add-mask::" + escape_command_data(value) + "\n")
    command_stream.flush()
```

  Register the full value and nonempty lines. After successful `tomllib` parsing,
  mask string leaves as defense in depth. These are intercepted workflow
  commands, not log lines; test the rendered GitHub logs rather than claiming a
  raw local command-stream capture is already redacted.
- [ ] Create one private directory (0700) with input/capture files (0600). Invoke
  the already verified CLI from the extracted release, with its bundled manifest:

```sh
auctioneer-cli config validate --manifest "$release_root/edgezero.toml" --app-config "$private_toml" --no-env
```

  Capture both streams privately with a 1 MiB cap per stream and a 60-second
  validation deadline. A timeout or overflow terminates/reaps the owned process
  and fails closed. Emit only a fixed code and numeric exit status on error.
  Test that cleanup is observed on normal/refused paths and record best-effort
  limits on abrupt runner loss.
- [ ] Write a collision-checked multiline **step-local** output only after
  validation succeeds; the same original bytes supply EdgeZero's
  `app-config-inline`. Pass `no-env: 'true'`. Do not create a job/workflow output,
  upload/copy raw input into source, or invoke the committed-file input path with
  an untracked file that upstream correctly rejects.
- [ ] Run the helper controls and real-action integration fixture. Expected:
  validator success/exit/failure paths execute, and no sentinel reaches rendered
  logs, summaries or artifacts. If upstream output leaks, stop for its correction
  rather than bypassing the action or changing the input contract. Commit the
  private-input path with that evidence.

## Task 5: Coordinator and protected lifecycle orchestration

**Files:** `deploy-fastly.yml`, `coordinator.py`, `release.py`, the owning integration
fixture and the log-safety verifier.

- [ ] Add coordinator entry controls for main push, allowed completed main CI,
  scheduled catch-up and manual retry. Reject PR/fork/other-branch notifications,
  forged workflow identity and arbitrary source/release override inputs.
- [ ] Define one workflow-wide `auctioneer-fastly` lock with `queue: max` and
  `cancel-in-progress: false`, covering selection through outcome recording.
  Notifications can be coalesced/lost; pending push intents cannot. Run at most
  one candidate per invocation, without holding a runner polling earlier CI.
- [ ] Resolve `vars.DEPLOY_ENVIRONMENT` in an unbound preflight and verify the
  Environment already exists before scheduling the bound consumer. Thread its
  frozen name through `needs.preflight.outputs.environment`. Require all three
  secrets without printing values or silently substituting repository variables.
- [ ] Download the selected artifact IDs from the exact producer attempt and
  verify them before executing a bundled CLI or exposing provider credentials.
  Snapshot and privately validate config. After any Environment approval delay
  and immediately before the first mutation, re-read accepted main CI attempt
  states and release/artifact identity/expiry under the same lock. A new running
  or failing rerun invalidates fresh admission. Test both transitions between
  preflight and mutation and assert zero provider calls.
- [ ] Only after that final fresh-admission check succeeds, create the immutable
  provider-attempt record with its expected identities, before the first possible
  provider mutation. A pre-mutation admission refusal must not create an
  unresolved attempt. Already-existing unresolved attempts continue through the
  recovery path; later CI failures cannot reclassify them as CI-ineligible.
- [ ] Wire EdgeZero's config/deploy/health/rollback actions with the same archive,
  digest and expected SHA. Step-scope both provider credentials and the read
  credential. The config action's inline source comes only from the already
  masked and validated step output, never directly from `vars`.
- [ ] Persist mutation-attempted and known provider versions through allowlisted
  checkpoints guarded with `always()`. Do not serialize whole `steps`, `vars`,
  `secrets` or environment contexts. A missing output after an action starts is
  unknown, not proof of no mutation.
- [ ] Reconcile interruption using the same verified release and known attempt.
  Use the existing CLI's read-only `active-version` where appropriate and verify
  provider package/version identity before recovering success. If identity cannot
  be proven, block for the operator. Roll back only a known distinct valid target;
  never report config restoration from version rollback. Block the queue on
  unresolved config mutation or a changed retry config digest/target fingerprint.
- [ ] Exercise success, early validation refusal, partial config push, failed
  deployment without a version output, first deploy, failed smoke, rollback
  failure and process interruption. Expected: at most one ordered mutation chain,
  correct pending/recovery state and no secrets in logs. Commit the consumer.

Use the same action-reference pattern for all four lifecycle actions. The config
binding must have this shape inside the Environment-bound consumer:

```yaml
- id: push_config
  uses: IABTechLab/edgezero/.github/actions/config-push-fastly@e11ea3082f66ec864aabdd6007b15c4926c73b82
  with:
    app-release-archive: ${{ steps.release.outputs.archive }}
    app-release-sha256: ${{ needs.preflight.outputs.archive_sha256 }}
    expected-source-revision: ${{ needs.preflight.outputs.source_revision }}
    fastly-api-token: ${{ secrets.FASTLY_API_TOKEN }}
    app-config-inline: ${{ steps.private_config.outputs.toml }}
    no-env: 'true'
    store: auctioneer_config
    deploy-to: production
```

Here `production` is EdgeZero's active-publication selector for the enrolled
standalone test/staging service. It grants no customer-production readiness.
Define the corresponding `release` and `private_config` steps and preflight
outputs as owned typed helper results, never publisher-selected identities.

## Task 6: Bounded hosted smoke and logging integration

**Files:** `smoke.py`, `tests/exercise-edgezero-wrappers.sh`,
`verify_log_safety.py`, workflow fixture jobs.

- [ ] Add private response fixtures for unique/ambiguous/missing service domains,
  a hostname not owned by the checked version, HTTP redirects, invalid TLS,
  response overflow, slow/trickled bodies and contradictory config-health headers.
  Derive the real hostname from provider domain inventory; no new mandatory
  hostname variable or Environment-name inference.
- [ ] Run EdgeZero's healthcheck action, then an Auctioneer status probe that
  accepts only a 200/204 response with one unambiguous `ok` or `ok-cached` config
  header. Reject HTTP 200 with `degraded`, missing or duplicate/conflicting values.
  Use three attempts within a 45-second total budget, 10 seconds per attempt and
  a 16 KiB response cap. Inspect privately; print only a fixed verdict.
- [ ] Add the deliberately malformed JSON auction decoder control, capped at
  the same response/time limits, and require 400 without bidder egress. This is
  a lexical negative control, not a valid JSON fixture or successful auction.
- [ ] Reuse pinned EdgeZero fake fixture builders and real wrapper scripts for
  action-level controls. Keep the fake provider/CLI capable enough to model
  activation, store mutation, versions, failure and recovery. Do not use a real
  bidder, service or token as a fake. No runtime API-host override is introduced.
- [ ] Add `deployment-wrapper-fixtures` and dependent `deployment-log-safety`
  jobs to the existing `Run Tests` workflow. They run on its ordinary PR/main
  events without any GitHub Environment or real provider credentials. The first
  job privately acquires independent sentinels,
  masks them before actual action consumption, and exercises rendered runner
  banners and failure paths. The dependent observer job downloads the completed
  fixture job's logs with Actions read access and checks known sentinel encodings
  and artifact allowlists. Use the job-specific plaintext log endpoint. Its
  signed-storage redirect requires a separate bounded download path: accept the
  one HTTPS Location from the authenticated GitHub response, send no GitHub or
  provider credential to storage, refuse another redirect and never log the
  signed URL. Keep the ordinary API reader's redirect refusal unchanged. The
  observer prints verdicts/counts only, never matching log lines. Cap the private
  download at 16 MiB and fail on overflow; do not print or upload the captured log.
- [ ] Check summaries separately: they are runner attachments, not ordinary job
  log text. Wrapper-script fixtures use a private `GITHUB_STEP_SUMMARY` sink and
  independently check its bytes against the summary allowlist. The workflow's
  own summary-writing step verifies its actual command-file contents before the
  step ends. Do not upload raw summary/config captures; the observer receives only
  the closed audit verdict/digest and checks the artifact allowlist. Review the
  pinned lifecycle actions for additional summary writers and instrument or
  reject any uncovered writer. A local stdout grep is not rendered-log acceptance.
  Both fixture jobs' conclusions join the existing main CI gate.
- [ ] Run the helper suite, wrapper fixture and actual hosted fixture-log
  observer. Expected: all controls execute and the observer finds no sentinel
  material. On any leak, retain diagnostics privately and stop publication of
  real provider automation until corrected. Commit the probes and fixture owner.

## Task 7: Runbook and complete verification

**Files:** `docs/deployment/fastly-main.md`, `README.md`, workflow-script README;
the implementation PR's evidence.

- [ ] Document `DEPLOY_ENVIRONMENT` setup and the exact three secrets/one required
  variable. Explain Environments read token scope, existing provider stores and
  secret-entry provisioning, optional store selectors and the real-domain
  prerequisite. Examples are fictional; never copy local operator settings.
- [ ] Document genesis/receipts, CI-ineligible versus provider-blocked state,
  safe reruns, late older CI success, changed targets, config compatibility,
  versionless config reconciliation and artifact expiry/retention. Manual retry
  uses the same oldest-pending queue and lock, not an arbitrary SHA or raw deploy.
- [ ] Run focused helper/script/workflow checks using the commands below. Run
  affected existing CLI config and Fastly store contracts. Do not change the two
  recorded baseline lint findings as unrelated implementation cleanup, and do
  not suppress their diagnostics or represent the continuation decision as a
  check dismissal.
- [ ] Push the implementation and execute the repository's full existing GitHub
  PR matrix. Inspect the current workflow files for the complete commands and
  prerequisites; do not replace compiler/evidence/guest owner jobs with a broad
  native Cargo pass. Match the tested head/merge ref and record actual job/step
  results, executed counts and blocked/not-run checks with attribution.
- [ ] With separate explicit deployment authorization and a provisioned test
  Environment, observe automatic activation after the implementation merges.
  Exercise three rapid eligible merges with out-of-order CI completion and retain
  safe release/provider/health/recovery evidence. The planning push/PR approval
  does not authorize this hosted exercise. Keep #247 open until its hosted
  acceptance and documentation are delivered.

Focused future commands, from the implementation worktree:

```sh
python3 .github/workflows/scripts/deploy-fastly/tests/run_units.py
python3 -m py_compile .github/workflows/scripts/deploy-fastly/*.py
shellcheck .github/workflows/scripts/deploy-fastly/tests/*.sh
actionlint .github/workflows/test.yml .github/workflows/format.yml .github/workflows/ci-fastly.yml .github/workflows/ci-cloudflare.yml .github/workflows/ci-spin.yml .github/workflows/prepare-fastly-release.yml .github/workflows/deploy-fastly.yml
cargo test --locked -p auctioneer-cli --bin auctioneer-cli application::config_contract::
cargo test --locked -p auctioneer-adapter-fastly --test manifest_health_stores
cargo fmt --all -- --check
git diff --check
```

The native Fastly command does not execute the ignored guest case. Execute that
case through the actual `CI: Fastly` owner after its Viceroy install and test-guest
build; preserve the explicit dispatch/result. Provider CLI installation and
vendor packaging also remain mandatory in that workflow.

For the full prescribed suite, execute the normal PR-triggered workflows
`Run Tests`, `Run Format`, `CI: Fastly`, `CI: Cloudflare` and `CI: Spin`, plus the
new workflow contract/fixture owner. Their existing YAML/scripts are the source
of truth for every native feature, compiler, evidence, conformance and guest
command. These inspection commands are evidence collection, not test execution:

```sh
gh pr checks "$pr_number" --repo IABTechLab/auctioneer --json name,state,workflow,link
gh run list --repo IABTechLab/auctioneer --commit "$head_sha" --limit 50 --json databaseId,headSha,workflowName,status,conclusion,event,url
gh run view "$run_id" --repo IABTechLab/auctioneer --json headSha,conclusion,event,jobs,url
gh run view "$run_id" --repo IABTechLab/auctioneer --log-failed
```

Do not claim completion from a queued/skipped check or from these commands merely
succeeding. Read actual current-head results. Local fixture passes, package size,
guest execution and separately authorized hosted acceptance remain distinct.
The planning PR carries only these documents and their actual validation record;
implementation tests named here do not exist yet and are not claimed as passing.

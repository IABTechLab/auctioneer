# Deploy Auctioneer to Fastly after main merges

Issue: [#247](https://github.com/IABTechLab/auctioneer/issues/247), under epic
[#12](https://github.com/IABTechLab/auctioneer/issues/12).
Status: proposed specification for maintainer review; implementation has not started.

## Outcome and accepted constraints

After each eligible merge into `main`, activate that exact Auctioneer release on
the configured standalone Fastly service. Preserve successful merge order even
when later CI finishes first. The maintainer explicitly chose every successful
merge in source order, with no pending-deployment supersession.

`DEPLOY_ENVIRONMENT` selects an existing GitHub Environment. Its Fastly token and
service ID are secrets. `AUCTIONEER__CONFIG_TOML` stays an Environment variable.
The maintainer chose additional read-only GitHub API access to retrieve that
variable privately, because GitHub logs unmasked action inputs and step `env`
values before their scripts execute. Deployment variables, credentials and raw
configuration must not be echoed or included in uploaded diagnostics.

The maintainer authorized continuing planning with the observed baseline
`actionlint` failures recorded. That decision does not waive validation or merge
checks. The baseline findings are SC2086 at
`.github/workflows/check-openrtb-drift.yml:24` and
`.github/workflows/refresh-gvl.yml:52`. Locked dependency acquisition, metadata
and formatting passed at `d26fcf7f566db6d1dce2e1847d79c9f44581edb8`; a local full
suite has not run. These observations are planning evidence, not qualification.

## Current contracts and ownership

- `.github/workflows/ci-fastly.yml` executes local guest regressions, runs the
  Fastly vendor packager, enforces the existing package ceiling and uploads the
  package. It does not build a complete application release or deploy it.
- The five existing main CI workflows cancel older runs. This must change for
  main commits while retaining PR supersession and every existing check.
- `auctioneer-cli` delegates lifecycle operations to EdgeZero and typed config
  operations to the shared Auctioneer model. No replacement deployment CLI,
  config model, Rust dependency migration or provider implementation is needed.
- `edgezero.toml` declares `auctioneer_config` and `auctioneer_secrets` as the
  logical defaults. Deployment binds physical resources through EdgeZero.
- `/status` deliberately remains available on a config fault. HTTP success
  alone is insufficient: `x-auctioneer-config-status: degraded` is unhealthy.

EdgeZero owns immutable-release verification, provider tooling, store-link
reconciliation, activation, healthcheck and rollback. Auctioneer owns the CI
producer, ordered selection, Environment input handling, its additional smoke
assertions and deployment records. The existing Trusted Server deployer is a
reference for that producer/consumer split, not a new Auctioneer dependency.

## Scope

Deliver one main-to-Fastly automation path, its tests and its operator runbook.
Use the configured service's active publication target so its real endpoint
serves the new version. Enrollment is for an operator-approved standalone test
or staging service; activation is not a declaration of customer-production
readiness. Existing production restrictions and resource qualification remain
with their current owners.

Do not add other providers, alter bidder or privacy policy, read a developer's
local TOML, provision provider resources implicitly, embed operator values in
builds, or deploy a PR/fork. Existing resource workloads and ceilings stay fixed.
[#73](https://github.com/IABTechLab/auctioneer/issues/73),
[#88](https://github.com/IABTechLab/auctioneer/issues/88),
[#99](https://github.com/IABTechLab/auctioneer/issues/99) and
[#102](https://github.com/IABTechLab/auctioneer/issues/102) retain their acceptance
and qualification requirements. A smoke pass does not close them.

## Configuration interface

`DEPLOY_ENVIRONMENT` is configured before job scheduling, using the repository
Actions variable of that name. An unbound preflight resolves it and checks the
Environment already exists. Empty, whitespace-only, malformed or nonexistent
selection fails without creating an Environment or accessing Fastly. Do not
derive a default from a branch, domain or service ID.

The selected Environment supplies:

| Kind | Name | Contract |
| --- | --- | --- |
| Secret | `FASTLY_API_TOKEN` | Token scoped to the selected Fastly lifecycle operations. |
| Secret | `FASTLY_SERVICE_ID` | Required destination; never read from `vars`. |
| Secret | `AUCTIONEER__DEPLOY__ENVIRONMENT_READ_TOKEN` | GitHub API credential restricted to this repository and the required Environments read permission. |
| Variable | `AUCTIONEER__CONFIG_TOML` | Required, nonempty typed runtime TOML, fetched privately from this exact Environment. |

Keep provider-owned names intact. Optional canonical EdgeZero physical-store
selectors retain their existing semantics: omission uses the logical ID;
present blank/invalid selectors fail. Fetch the config and secret store selectors
for the declared logical IDs through the same private input path, not an unmasked
job-level `env` map. An absent optional variable remains absent. This
specification introduces no second Auctioneer bootstrap policy.

Missing credentials, invalid service IDs and invalid typed config fail before
provider mutation. Keep secret *values* in the provisioned Secret Store; runtime
TOML contains only the secret references accepted by Auctioneer's model.
The read-only GitHub credential is used only by the private retrieval step and
is not supplied to builds or the application CLI.

## Producer and immutable release

Preserve the existing Fastly build/test/size owner. Add credential-free native
CLI production and release assembly, using EdgeZero's `build-app-cli` and
`package-application-release-fastly` actions. Assembly consumes the exact
package produced and measured by Fastly CI, the CLI built from that same main
SHA, and the unchanged manifests from that SHA. It does not rebuild the package.

Use one reviewed immutable EdgeZero action revision throughout. Verify the
application CLI lifecycle contract through the upstream packager. Toolchain and
application dependency versions continue to come from repository metadata,
`.tool-versions` and `Cargo.lock`; do not copy the reference deployer's older
tool versions or pins. Preserve the repository's download-cache policy rather
than restoring a whole-workspace compiled target archive.

Publish these producer artifacts with attempt-specific names:

1. The upstream immutable release archive, containing its strict `release.json`,
   CLI archive, vendor package, `edgezero.toml` and referenced Fastly manifest.
2. A small receipt recording format version, source SHA, producer workflow/run/
   attempt identity, release artifact name, release archive SHA-256 and package
   SHA-256. It contains no runtime config, destination or credential.

The consumer resolves artifact IDs from that exact trusted producer run and
attempt, checks GitHub's artifact identity, then verifies the receipt against
the selected SHA and the archive. Every lifecycle action receives the same
archive, digest and expected revision. An Environment cannot select or replace
the source revision, manifests, CLI or package.

The initial artifact recovery window is the upstream action's 14-day retention
from publication, with actual `expires_at` recorded in the deployment evidence.
The runbook must identify that expiry and how an operator retains the unchanged
archive and digest for longer recovery. Missing, expired, ambiguous or mismatched
artifacts block the pending revision; do not rebuild or silently skip it. A new
producer attempt may replace a release only before any provider mutation for
that revision; retries after mutation retain the original release identity.

## Ordered coordinator

Keep main CI parallel, but give each main source SHA a distinct concurrency
identity so a later merge cannot cancel its checks. PR concurrency continues
to cancel superseded PR runs. Change the existing five workflows together.

Add a coordinator workflow woken by main push events and completed runs of those
workflows, with scheduled catch-up and a manual retry entry point. Notifications
are wakeups, not the authoritative queue. Reconstruct pending work from main push
receipts and durable GitHub Deployment records, so a lost/duplicated wakeup or
GitHub's pending-run queue limit cannot discard a source revision.

The deployment candidate is the resulting **head of a main push**, not every
intermediate commit introduced by a rebase or multi-commit update. The repository
allows rebase merges and requires linear main history; those intermediate
commits need not have individual push-triggered CI runs. At the start of Fastly
main CI, publish a small immutable push receipt before heavy checks, containing
the event's `before`, `after`, repository, ref and run/attempt identities. Register
the same intent from the coordinator's push event and deduplicate by the exact
before/after pair. Receipt creation is credential-free with respect to Fastly
and publisher Environments. Preserve it even if later build/test steps fail.

Enrollment begins with the main push whose before/after interval introduces the
coordinator workflow. Determine the introducing change from first-parent Git
history and bind it to that push's receipt; the candidate is its `after` SHA,
which can follow the introducing commit in a rebase update. Do not choose genesis
from event arrival or CI completion order, and do not replay earlier history.
Record the genesis, selected Environment and target fingerprint in an enrollment
record. Reuse it on restart. A changed service or store mapping must not reuse
an existing stream silently: stop for explicit operator re-enrollment and record
the new start revision.

Order push heads by their verified before/after chain and main's first-parent
ancestry, not timestamps, commit dates, merge-request numbers, workflow run IDs
or CI completion order. A receipt's `before` must connect to the previous enrolled
push head. Intermediate commits covered by that interval are not missing queue
items. A missing push receipt or ancestry link, force-push, inconsistent
enrollment or unprovable cursor blocks the worker. Main history and all API
listings used for decisions must be complete; an omitted page is not no work.

Each invocation selects at most one oldest outstanding enrolled revision:

| Observation | Disposition |
| --- | --- |
| Prior provider attempt exists without a completed healthy success | Recover/retry that same revision and release, or remain blocked. This takes precedence over any later CI classification. |
| Required CI missing, queued, running or being rerun | Wait; no provider mutation and no later revision overtakes it. |
| All required accepted CI attempts complete, with at least one unsuccessful conclusion | Record `ci-ineligible` with exact run/attempt identities and conclusions; advance without a provider attempt. |
| All required CI successful, release complete and identity verified | Admit exactly this revision to the protected consumer. |
| Successful CI but missing/invalid/expired release | Block this revision; do not classify it as CI-ineligible. |
| This revision already completed healthy deployment | No duplicate deployment; continue selection from the durable cursor on the next wakeup. |

Accept only push runs from this repository's actual main branch, with their
head SHA on the enrolled first-parent chain. Match the five workflow files and
IDs, not arbitrary check names. Select and record the latest complete accepted
attempts after excluding currently rerunning results. Require the whole existing
matrix, including guest/build/resource owners; a PR merge-ref pass is not a main
SHA pass. Recheck the admission facts immediately before the first mutation.

CI-ineligible disposition is explicit and durable. It applies only before any
provider attempt; a failed CI rerun after mutation cannot turn an unfinished
rollout into an ineligible revision or release the queue. A later CI rerun can make a
revision eligible while it remains at the cursor. Once a successor has been
processed, an older rerun cannot publish out of order; recovery needs a new main
revision. A provider failure is never a CI-ineligible disposition.

### Durable records and serialization

Use repository-native GitHub Deployment records under a fixed Auctioneer task
namespace. Record schema version, stream/enrollment ID, source revision,
predecessor record, accepted CI attempt IDs and release identity. A provider
attempt also records the submitted config digest and a target fingerprint,
never raw config or service/secret values. Record statuses and link the owning
workflow attempt and safe evidence. Distinguish enrollment/control metadata,
CI-ineligible dispositions, provider attempts and healthy completion explicitly.
Control records must not be reported as successful provider deployments.

Only the coordinator writes these records. Validate their namespace, schema,
source ancestry and owning workflow identity before using them as a cursor.
Retain status history and disable automatic inactivation for controller-owned
statuses so a later deployment cannot erase an earlier success fact. Incomplete
or conflicting state blocks; do not infer success from the newest record alone.

Query records by the controller task and selected Environment. Use the current
validated controller record and its explicit predecessor IDs, and examine source
history only from that cursor forward; do not replay all historical deployments
on every wakeup. An incomplete page, unknown schema or exhausted request guard
blocks with a fixed diagnostic. It never truncates history to obtain a candidate.

A single repository-wide Fastly concurrency group covers selection, config
push, publication, health and recovery, with `cancel-in-progress: false` and
`queue: max`. Using one coarse lock also covers Environment changes without
exposing a secret service ID in the concurrency key. GitHub's FIFO waiting order
does not replace source-order selection. All manual paths use the same lock;
uncoordinated external service/store mutations remain an operator exclusion.

CI failure records can advance only after their evidence is complete. Provider
success advances only after bounded health/smoke acceptance and a durable
completion record. If a process dies between provider success and recording,
the next invocation must reconcile that exact attempt; it cannot assume no
mutation or immediately start the next revision.

## Private variable retrieval and logging

The protected consumer binds the preflight's frozen Environment name. Fetch
`AUCTIONEER__CONFIG_TOML` from the fixed GitHub Environment Variables API using
the read-only credential. URL-encode the Environment path component, restrict
the host, refuse credential-bearing redirects, cap responses and emit only
closed error codes on HTTP/JSON failure. Never print the response body.

Fetch and retain the value once per attempt. It must never first enter a `with`,
`env` or `run` expression through `vars.AUCTIONEER__CONFIG_TOML`: GitHub would
log that value before the script could mask it. Register properly escaped masks
for the full value and nonempty lines before any runner-visible use; include
config string values in masking after successful TOML parsing. Masks are
defense in depth, not permission to print the data.

Write the captured TOML to a private file. Use the verified release CLI and its
manifest to perform typed validation with `--no-env`, capturing stdout/stderr
privately. On failure emit an allowlisted failure code and exit status, never
source excerpts, raw parser diagnostics or the captured log. The original bytes
validated are the bytes sent to config push.

After validation, a masked **step-local** output may supply `app-config-inline`
to EdgeZero's config action in the same job, with `no-env: 'true'`. Use the
runner's private output file with a collision-checked multiline delimiter;
never legacy stdout output commands. Do not forward the TOML as a job/workflow
output. Inspect the pinned action and actual rendered runner logs: passing shell
tests alone does not establish that GitHub hid its automatic input/`env` banners.

No environment dumps, shell tracing, debug payloads, provider response bodies,
raw config diffs, secret values or input-value annotations are allowed. Scope
credentials to their owning steps. Temporary input and diagnostic directories
are mode 0700 and files 0600; remove them on all observed exit paths. Cancellation
or abrupt runner loss has only best-effort cleanup, so use ephemeral runners.
Never include these files in release, cache or diagnostic artifact uploads.

If the pinned upstream actions leak a sentinel after this input handling, stop
and obtain an upstream correction before claiming delivery. Do not bypass the
immutable-release contract or silently change the variable to a secret.

## Fastly lifecycle and smoke acceptance

Use the existing declared stores and EdgeZero's canonical selectors. Existing
resources and required secret entries are operator prerequisites. No config
payload can choose a different provider service or release identity.

Use `config-push-fastly`, `deploy-fastly`, `healthcheck-fastly` and
`rollback-fastly` at the same reviewed action revision. Config push runs before
publication, so config must remain compatible with the currently serving
release. An incompatible cutover requires a separately reviewed store/version
strategy; this worker does not invent one.

Select a real HTTPS probe hostname from the configured service's provider
domain inventory, with a unique provider Compute hostname preferred. Require
one unambiguous service-owned selection and verify it belongs to the version
being checked. A missing/ambiguous hostname fails; do not guess a hostname from
the Environment name or service ID. Document this one-time service prerequisite.

Hosted acceptance requires the EdgeZero health action plus an Auctioneer probe
that accepts only the expected success status and `x-auctioneer-config-status`
of `ok` or `ok-cached`. Reject `degraded`, missing or contradictory headers.
Keep HTTP/TLS/redirect, retry, total-time and response-size bounds explicit.

Exercise `/openrtb2/auction` with a fixed deliberately malformed JSON decoder
control and require its bounded 400 rejection. It must reach no bidder and
carry no consent, cookie or identifier. Inspect responses privately and emit
only a fixed smoke verdict. This is route/decoder smoke, not a successful bid,
bidder compatibility test or resource qualification.

Record the new and previous provider versions as soon as known. A failed health
check triggers EdgeZero rollback only with a known, distinct valid target. A
first deployment has no previous version. Missing outputs, ambiguous provider
state, failed rollback or interruption after mutation require reconciliation;
none is a healthy success. Keep the revision pending until it completes healthy
deployment or an explicitly reviewed scope change records otherwise.

Config Stores are versionless. Version rollback does not undo config push.
Record mutation-attempted signals even when a later step emits no key/version.
After any partial config mutation, require safe config reconciliation before a
retry or later revision. A retry that would change the recorded config digest
or target fingerprint must stop for operator reconciliation.

## Bounds and evidence

Use explicit job timeouts and no unbounded polling for earlier CI. Each wakeup
examines the next candidate and exits waiting/blocked when it cannot proceed.
The initial coordinator/API guard is 60 requests, 10 seconds per request and
1 MiB per JSON response; exhausting a guard is blocked, not empty/success.
Config input is bounded by GitHub's documented 48 KiB variable limit. Private
CLI captures and probe captures are bounded and checked before any readback.
The implementation plan must give their concrete limits and rejection tests.
Do not change application resource ceilings or existing CI workload coverage.

Safe evidence includes source/release/package identity, accepted CI run IDs,
provider version numbers, action/tool identities, closed config/health/recovery
verdicts and artifact expiry. Keep submitted config digests distinct from proof
of a runtime-served digest: `/status` does not attest the latter. Never relabel
a liveness response or another artifact's old results as deployment acceptance.

## Acceptance and validation

1. Actual workflow review proves source/attempt binding, no credentialed build,
   no PR/fork deploy path, existing check preservation and a frozen Environment
   selection. Shell bodies stay beside their workflow; do not add Rust snapshots
   of YAML or shell source.
2. Focused coordinator controls independently vary CI completion order, missing
   pages, duplicate notifications, CI failure, reruns, late older success,
   expired artifacts, cursor corruption and target changes. Three rapid merges
   with later CI completing first still produce source-ordered provider calls.
3. Private input controls cover multiline/percent/CR/LF values, output-delimiter
   collisions, invalid TOML, API denial/redirect/oversize, whitespace/empty values
   and failed validation. Independent sentinels must be absent from rendered
   logs, summaries and uploaded evidence, including failure/cancellation paths.
4. Lifecycle controls exercise real action wrappers with capable fake provider
   and CLI fixtures, exact immutable release identity, partial config writes,
   missing version output, failed health, first deploy, rollback and recovery.
   They must not mutate a real service.
5. Run the full repository-prescribed matrix from the actual workflows, including
   compiler/evidence owners and all adapter builds/guest checks. Attribute GitHub
   runner results separately from local checks and report every blocked/not-run
   row. Baseline permission is not a check dismissal.
6. Separately authorized hosted acceptance observes the first enrolled main
   deployment and rapid-merge behavior on the configured standalone service.
   Retain source/config-submission/provider identities and recovery outcomes.
   No hosted deployment has been performed during specification work.
7. The runbook documents Environment setup, API credential scope, service/store
   and hostname prerequisites, enrollment, failed-CI disposition, blocked-queue
   recovery, config compatibility, artifact expiry and safe retention. It must
   distinguish activation of the selected test service from production readiness.

The plan/spec PR uses `Refs #247` and remains a planning change. Implementation
uses a separate worktree/PR after the reviewed plan merges into main. No issue
closure or implementation readiness follows from writing these documents.

## References

- [Current Fastly workflow](../../../.github/workflows/ci-fastly.yml)
- [Development and test ownership](../../testing/development.md)
- [EdgeZero action adoption](https://github.com/IABTechLab/edgezero/blob/e11ea3082f66ec864aabdd6007b15c4926c73b82/docs/guide/deploy-action-adoption.md)
- [EdgeZero lifecycle actions](https://github.com/IABTechLab/edgezero/blob/e11ea3082f66ec864aabdd6007b15c4926c73b82/docs/guide/deploy-github-actions.md)
- [Reference producer](https://github.com/stackpop/trusted-server-deployer/blob/ea942b1ea8492ea8f9dc270b9a7030d7312d97da/.github/workflows/prepare-release.yml)
- [Reference deployment](https://github.com/stackpop/trusted-server-deployer/blob/ea942b1ea8492ea8f9dc270b9a7030d7312d97da/.github/workflows/deploy.yml)
- [GitHub concurrency and queue behavior](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/control-workflow-concurrency)
- [Environment Variables API](https://docs.github.com/en/rest/actions/variables#get-an-environment-variable)
- [Runner action input logging](https://github.com/actions/runner/blob/main/src/Runner.Worker/Handlers/Handler.cs)

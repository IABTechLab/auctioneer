# Limited-Bidder Publisher Acceptance

Status: reusable procedure; no publisher deployment or live rollout has been
accepted by this document. Execution belongs to publisher pilot, after Trusted Server embedding and the selected
bidder wave. diagnostic qualification owns the native qualification tooling and this preparation.

## Evidence Boundary

Use the [resource qualification contract](../superpowers/specs/2026-10-05-diagnostics-and-qualification-design.md#101-deployment-resource-qualification-q2)
and the [native runner](../testing/resource-qualification.md). Local acceptance
and deployment qualification are separate results. A passing local run is not
permission to send publisher traffic, certify a bidder, or assert a provider
memory/delivery guarantee.

The selected target must complete all six workload families: cold/warm baseline,
maximum admitted auction work, response/decode overlap, all debug states,
concurrent auxiliary/config work, and saturation/recovery. Use the real generated
App and shipped adapters. Do not substitute a private handler, a fixture-only
engine, or the generator's memory measurements.

An artifact with loopback bidders, conformance helpers, or fixture endpoint
overrides is development evidence only. The production-intended artifact must
have its own identity and applicable acceptance evidence.

## Predeclare The Candidate

Before measurement, commit a versioned safe profile and assign an operator,
independent reviewer, and observation window. Record:

- Artifact digest, source revision, compiled features, SDK pin/runtime, target,
  relevant reference-data and baked-configuration digests.
- Actual deployment memory/concurrency/time budgets, hardware/OS/meter identity,
  safe effective SDK settings, explicit catalog residency/initialization and
  per-auction validation/context costs, accounted resource total and required headroom.
- Serving runtime-configuration digest, backend refresh/restart mode, controlled
  fixture digests/seeds, all scenario IDs, repetitions and complete windows.
- Offered and admitted work, required successes, allowed refusals/errors/timeouts,
  exact latency population/start/end/percentiles, high-water memory limits,
  retained-growth limits and healthy-recovery criteria.
- Rollout traffic fractions, monitoring windows, maximum response/config propagation
  waits, and numerical stop thresholds. A missing value means execution is not ready;
  do not substitute a generic performance target.

Do not record raw configuration, environment dumps, cookies, credentials, consent,
identifiers, URLs or debug bodies in the profile/report. Keep provider credentials
and rollback configuration in the operator's existing restricted storage.

Changing an artifact/feature, effective settings/configuration, SDK/runtime,
relevant reference data, or platform budget invalidates affected evidence.
Rebuild/reinspect and rerun affected rows against the new identity. Do not edit a
passing report to describe a different process or deployment.

## SDK And Product Preconditions

Q1, Q3 and Q4 remain EdgeZero-owned controls, not new Auctioneer implementations:

| Boundary                 | Deployment evidence required                                                                                                                                     |
| ------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Q1 raw ingress/admission | The selected listener's enforced limits and actual overload behavior, including allocation before application admission.                                         |
| Q3 config pre-extraction | The selected backend's bounded read/allocation behavior and applicable integrity/visibility semantics. A typed-push success is not a bounded backend-read proof. |
| Q4 response delivery     | Actual driver deadline, cancellation, terminal ownership and provider handoff behavior. Application-set header limits exclude provider-added framing.            |

Use the pinned SDK's capability/evidence records and the
[foundation follow-up ownership](../superpowers/plans/2026-10-06-production-fanout-final-review-fixes.md).
A required pending/unsupported guarantee keeps the selected deployment
nonqualified, even when local application acceptance passes. Unrelated targets
can remain pending without blocking development.

Before publisher traffic, verify the selected real bidders' credentials,
parameters, aliases, supported non-video formats, timeout/failure behavior,
consent/identity enforcement, targeting, and creative rendering. Check reference
freshness and the real cookie-key rotation/access policy. Controlled mock responses
do not close this work. Embedding acceptance remains Trusted Server embedding.

## Preserve A Known-Good Pair

Capture both the known-good artifact/version and the runtime configuration that
it actually serves. Preserve the typed configuration source and its validated
digest in restricted operator storage; retain only the digest in safe evidence.
Record the safe effective settings and a successful bounded health/auction probe.

The repository's Cargo executable is named `auctioneer-cli`; its commands below are
the existing application wrapper, not a second config or promotion mechanism.
Use the version shipped with the selected release and confirm its help at the
recorded SDK pin.

For Fastly's versioned lifecycle, capture before changing the active version:

```sh
auctioneer-cli active-version --adapter fastly --service-id "$SERVICE_ID"
```

Retain the returned version as `GOOD_VERSION`. Do not infer it from a newer
draft. Record the complete immutable application-release bundle, not only a
component/package. Cloudflare and Spin use their approved provider-native version
capture/restore procedure; do not assume Fastly-only flags implement it.

Validate and push the known-good/candidate sources through the existing typed
operations. In isolated native testing:

```sh
auctioneer-cli config validate --manifest "$MANIFEST" --app-config "$CONFIG" --no-env
auctioneer-cli config push --adapter axum --manifest "$MANIFEST" --app-config "$CONFIG" --no-env --no-diff --yes
```

The manifest must select the intended isolated binding/key. A local native push
is already local-only. For a provider, select its approved deployment/binding
and supported flags explicitly; never reuse a shared or production store for
corruption tests. Do not run provider writes as part of diagnostic qualification.

## Stage And Observe

1. Inspect the exact artifact and safe effective profile under the same sanitized
   environment used to start it. Verify every digest before load.
2. Install the candidate configuration through typed push. Start/restart or refresh
   using the backend's actual contract; wait for bounded observations of the
   intended serving revision, gates/clamps and aliases.
3. Execute all required workload rows and the separately authorized hosted-handler
   corruption/recovery cases on dedicated nonproduction bindings. Require actual
   fixture dispatch and restoration/recovery observations, not only successful
   push validation.
4. Evaluate safe observations with the real qualifier. Missing rows, shortened
   runs, underload, identity mismatch, unexpected failures or unsupported required
   guarantees cannot become a passing deployment result.
5. Only after Q2-D and publisher prerequisites pass, start the predeclared limited
   traffic fraction. Hold each fraction for its full monitoring window before
   advancing. Preserve a working rollback path throughout.

Fastly staging uses its existing version-aware lifecycle:

```sh
auctioneer-cli deploy --adapter fastly --application-release "$RELEASE_ROOT" --service-id "$SERVICE_ID" --staging
auctioneer-cli healthcheck --adapter fastly --service-id "$SERVICE_ID" --version "$CANDIDATE_VERSION" --domain "$DOMAIN" --path /status --staging --retry 3 --retry-delay 5 --timeout 10
```

These retry values bound only the example probe. They are not the acceptance
windows or thresholds. Non-Fastly adapters reject this staging flag; follow the
selected provider's recorded staging procedure instead.

## Selected-Target Execution Handoff

publisher pilot must bind a versioned workload driver and a real service-only meter to the
selected deployment. The native local runner is not a Cloudflare/Fastly/Spin
resource meter. diagnostic qualification does not claim to ship or execute a universal provider Q2-D
driver. If the selected driver/meter is unavailable, Q2-D remains pending.

In the deployment record, store the exact executable/job revision and argument
vector as `Q2D_WORKLOAD_ARGV`. Its recorded inputs must select the immutable
artifact/service revision, `PROFILE`, safe meter descriptor, isolated fixture
identities, and the new `OBSERVATIONS` destination. The meter descriptor names
the actual source, byte units, version, high-water method, service PID/deployment
identity, complete cold/warm/scenario windows and known limitations. It cannot
substitute generator memory, sampled averages or guest-only memory for a required
provider/process peak.

After explicit publisher pilot authorization, the parameterized execution/evaluation handoff
is:

```bash
set -euo pipefail
test "${#Q2D_WORKLOAD_ARGV[@]}" -gt 0
test -x "${Q2D_WORKLOAD_ARGV[0]}"
test -f "$METER_DESCRIPTOR"
test ! -e "$OBSERVATIONS"
test ! -e "$REPORT"
auctioneer-cli qualify validate --profile "$PROFILE"
"${Q2D_WORKLOAD_ARGV[@]}"
auctioneer-cli qualify evaluate --profile "$PROFILE" --observations "$OBSERVATIONS" --output "$REPORT"
```

Resolve the argument vector from the selected driver/job's checked documentation
and help, not guessed generic provider flags. Require it to execute all predeclared
subcases/repetitions/windows and write the qualifier's bounded safe observation
schema, including variant startup identity, actual admitted work, outcome/latency
populations, effects, service-only high-water readings and recovery. Preserve the
exact command vector and safe inputs with their digests for reruns; credentials
remain in the existing operator secret bindings, not command arguments/evidence.

The profile must use deployment scope and the production-intended artifact.
The report destination must be new. Exit 1 is not qualified: it can mean passing
local acceptance with missing/unsupported SDK evidence, as well as failed work.
Require a qualified profile result and complete publisher prerequisites before
traffic; a generated report or passing local result is insufficient. Never
manufacture missing measurements or copy local observations into deployed rows.

## Stop And Roll Back

Stop traffic advancement immediately on a privacy/identifier disclosure,
configuration-integrity failure, unexpected serving revision, broken identity/
consent flow, unsupported required capability, or breached predeclared resource,
latency, outcome or recovery criterion. Privacy/correctness failures do not wait
for a statistical window. Pause/route traffic through the operator's existing
control, preserve bounded safe evidence, and restore the known-good pair.

| Backend                      | Coordinated restoration                                                                                                                                                                                                                                              |
| ---------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Native Axum startup snapshot | Typed-push the preserved known-good configuration, stop the candidate process, restart the intended known-good artifact with its recorded safe settings, then observe that serving snapshot and healthy auctions. File replacement alone changes no running process. |
| Fastly                       | Restore the intended configuration through its typed binding, use the captured known-good version/bundle, then verify config visibility and actual version/health. Staging deactivation and production reactivation are different operations.                        |
| Cloudflare/Spin              | Use the selected deployment's recorded provider version rollback plus typed configuration restoration and its verified refresh/restart semantics. Observe the serving revision; do not substitute the store's latest digest.                                         |

Existing Fastly operations:

```sh
auctioneer-cli rollback --adapter fastly --service-id "$SERVICE_ID" --version "$CANDIDATE_VERSION" --staging
auctioneer-cli rollback --adapter fastly --service-id "$SERVICE_ID" --version "$CANDIDATE_VERSION" --rollback-to "$GOOD_VERSION"
```

Use the staging command only for a staged candidate, or the production command
only for an explicitly authorized live rollback. Neither rolls back stored
runtime configuration automatically. Restore configuration in the recorded
backend-safe order and keep traffic stopped until the pair is verified.

After rollback, repeat bounded health, valid auction, consent-denial, targeting,
and selected creative-rendering probes; observe resource/latency/recovery for the
full predeclared rollback window. A process restart, push acknowledgement, or
liveness response alone is insufficient. Record artifact/config/settings digests,
serving revision, scenario results, and operator disposition. Failed cleanup,
unobserved configuration restoration, or failed recovery means rollback acceptance
failed; do not resume pilot traffic.

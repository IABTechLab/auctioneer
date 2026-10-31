# WASM Resource Envelope Remediation Design

Date: 2026-10-17
Status: Implementation in progress; proof-backed accounting corrections with unchanged workload authorized by AG on 2026-10-18; workload or shared-equation changes still require specific approval
Milestone: M7-R0, delivered within the M7 branch and PR #21

**Scope extension approved 2026-10-30:** the
[controlled-native M7 amendment](2026-10-30-m7-controlled-native-completion-design.md)
moves complete source/ABI worst-case schema residency, initialization, scratch,
work and stack certification here. It is a distinct R0 obligation alongside
absolute WASM fit and missing platform facts. Native M7 allocation/stress tests
and frozen reservations are not those proofs. No target becomes ready from the
scope change, and no shared equation, workload or deployment ceiling is changed.

## 1. Purpose And Boundary

Make the declared WASM workloads defensible under their actual memory scopes.
This is the absolute-fit prerequisite identified in [M7 section 11.2](2026-10-16-m7-bidder-catalog-design.md#112-target-resource-acceptance),
not a requirement to make catalog migration repair every resource problem.
The [implementation plan](../plans/2026-10-17-m7-r0-wasm-resource-envelope.md)
starts with reproducible accounting and an explicit feasibility checkpoint.

M7 owns catalog-attributable growth. M7-R0 owns the existing runtime/core/provider
envelopes, allocation lifetimes and any separately approved workload changes.
M7.6 owns credentialed hosted execution, corrupt-config injection/recovery,
actual deployment qualification and the publisher pilot. None is implied by
another's passing build. Keep all three dispositions visible per target.

This document does not authorize smaller workloads, weaker privacy, omitted
charges, larger platform ceilings or a different strict-only equation. It does
not authorize speculative charge reductions. AG's execution authorization in
section 4.2 permits verified unchanged-workload implementation improvements
and independently proved accounting corrections.
No new crate, executable, allocator framework or provider HTTP stack is needed.

## 2. Source-Grounded Baseline

Planning baseline: M7 commit `871c72e2`, Rust 1.98.1, EdgeZero PR #275 revision
`f6c708ccb5be19ba9fa92575db3c08c0f17e0d72`. These are repository/SDK declarations,
not a claim that today's hosted service configuration has been measured.

| Target              | Existing subtotal lower bound, bytes | Declared ceiling, bytes | Minimum headroom, bytes |
| ------------------- | -----------------------------------: | ----------------------: | ----------------------: |
| Cloudflare          |                       58,064,844,288 |             128,000,000 |               8,000,000 |
| Fastly              |                       10,259,037,696 |             128,000,000 |               8,000,000 |
| Spin hosted profile |                       10,259,037,696 |             134,217,728 |               8,388,608 |

These are older golden subtotals in each provider's `src/envelope.rs`;
`expected_required_bytes()` adds further positive terms and actual boot also
counts baked defaults. They are neither exact current totals nor measured
resident GB. Task 1 must capture exact normalized totals, including baked
defaults, without replacing or relabeling historical M6 evidence.

The complete source decomposition below includes the current normalization and
debug increments, rather than presenting two small costs as the cause of the
older subtotal. Let `b` be the effective baked-default byte count and let
`L_t = fixed_per_auction_bytes + reusable_scratch_bytes +
50 * retained_per_scope_bytes` from the actual target's core construction
descriptor. The numeric portion of one auction is **19,588,723,204 bytes**:
`6,258,324,480 + 2,064,573 * (36 + 16 + 50 * 128) + 69,120 + 9,704,608`.
The source row therefore charges that numeric portion plus `L_t + 128*b`.

| Term             |                 Cloudflare, bytes |                  Fastly, bytes |             Spin hosted, bytes |
| ---------------- | --------------------------------: | -----------------------------: | -----------------------------: |
| B                |                        27,328,512 |                     27,328,512 |                     27,328,512 |
| G                |                         7,168,000 |                      3,072,000 |                      3,072,000 |
| Q                | `117,532,339,224 + 6*L_t + 768*b` | `19,588,723,204 + L_t + 128*b` | `19,588,723,204 + L_t + 128*b` |
| E                |                       453,009,408 |                     50,335,744 |                     50,335,744 |
| A                |                    19,327,385,600 |                  3,221,233,664 |                  3,221,233,664 |
| T                |                            48,640 |                          9,216 |                          9,216 |
| P                |                           122,880 |                         90,112 |                         90,112 |
| H                |                         1,428,480 |                        238,080 |                        238,080 |
| S                |                       698,405,888 |                    698,405,888 |                    698,405,888 |
| C / I increments |                             0 / 0 |                          0 / 0 |                          0 / 0 |
| Total            | `138,047,236,632 + 6*L_t + 768*b` | `23,589,436,420 + L_t + 128*b` | `23,589,436,420 + L_t + 128*b` |

These are source-derived expressions at the planning baseline, not executed
target observations. `b=0` selects the existing pure wrapper; boot uses the
resolver's actual `b`. Keep target layout in `L_t`, not a substituted native
size. Task 1 must reconcile this table with actual named-term reports.
Zeros for C/I are current incremental descriptors, not free existing catalog.

Even before the positive `L_t` and baked-default terms, the declared-ceiling
planning gaps are:

| Target              | Numeric demand lower bound, bytes | Gap including minimum headroom, bytes | Approximate demand reduction |
| ------------------- | --------------------------------: | ------------------------------------: | ---------------------------: |
| Cloudflare          |                   138,047,236,632 |                       137,927,236,632 |                      99.913% |
| Fastly              |                    23,589,436,420 |                        23,469,436,420 |                      99.491% |
| Spin hosted profile |                    23,589,436,420 |                        23,463,607,300 |                      99.467% |

Percentages are rounded to three decimal places; gap bytes are exact. These
use `max(0, total + minimum_headroom - declared_ceiling)`; Task 1 must
add the actual layout/baked terms and any larger selected headroom. They are
planning lower bounds, not completed platform validation. No independently
justified class-1/class-2 package closes these gaps at this baseline. Their
magnitude does not prove every unchanged-workload improvement impossible, but
does rule out assuming that ordinary optimization will suffice. Task 3 must
present class-3 workload/profile proposals for all three WASM targets at the
initial checkpoint, alongside any evidenced unchanged-workload alternative;
do not defer the product decision until after implementing small savings.

Q and A dominate all three rows. In particular, the 768-MiB auxiliary peak
generates `8 * (768 MiB * 3 + 4,096) = 19,327,385,600` bytes on Cloudflare:
four auxiliary plus four independent probe-config permits. Fastly/Spin each
fund `2 * (768 MiB * 2 + 4,096) = 3,221,233,664`. It is not just another
body cap. One auction alone exceeds every ceiling even with all other isolate
terms removed. These costs, not only smaller parser/GVL/egress terms, need an
explicit disposition at the feasibility checkpoint.

All three WASM declarations currently allow 32 calls, 256-KiB decoded bodies,
65,536 parsed nodes, 1-MiB backend and blob config caps, 8,192 config nodes,
and a 768-MiB auxiliary-route peak. Fastly/Spin already admit one auction.
Two concrete lower bounds explain why concurrency reduction alone cannot work:

- Config, before baked-default bytes: `1,048,576 * (1 + 128 + 128) +
8,192 * 623 = 274,587,648` bytes per admitted auction.
- Response parse: `32 * (262,144 * 256 + 65,536 * 320) = 2,818,572,288`
  bytes, before retained bodies, assembly, headers or other auction work.

Even one auxiliary work allowance is larger than a target ceiling. These are
conservative accounting expressions, not proof that their full allocations
occur simultaneously. Establishing a smaller valid bound requires inspecting
and, where necessary, changing the owning implementation, not just a constant.

### 2.1 Memory Scope And Unknowns

| Profile     | Pinned SDK scope                                                 | Required disposition                                                                                                                         |
| ----------- | ---------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| Cloudflare  | 128,000,000 bytes per instance                                   | Bound all resident/in-flight populations sharing the instance; the outbound connection limit of six is not an ingress concurrency proof.     |
| Fastly      | 128,000,000 bytes per execution                                  | Attribute guest and host allocations to the documented scope; one request does not mean one response body or one auxiliary/probe allocation. |
| Spin hosted | 134,217,728 bytes per execution in the hosted-default descriptor | Identify the deployment using this descriptor; generic Spin's runtime-configured memory is unknown until explicitly supplied and verified.   |

The current baseline, parser-transient and transmit-state allowances contain
native-derived or unmeasured assumptions. A numeric row is not evidence of
enforcement or measurement. Preserve its charge during the audit, record its
provenance/uncertainty, and withhold target-readiness claims until resolved.
SDK/platform facts must be tied to the selected artifact and memory scope;
local guest execution and upload-size checks do not prove hosted memory fit.

At the planning baseline Spin installed the hosted envelope but dispatched
with generic `SPIN_PLATFORM` metadata. The adapter now passes the shared
profile's `AKAMAI_FUNCTIONS_PLATFORM` to `run_app_with_platform`, matching AG's
selected Akamai Functions hosted deployment. Actual artifact/deployment
provenance remains unproved; matching metadata does not make an arbitrary
generic deployment hosted. Fastly's
separately declared stack is not extra heap capacity. All three SDK descriptors
leave host ingress/framing accounting unpublished. Record those unknowns even
after the guest's additive arithmetic fits.
At the pinned SDK revision, host-ingress accounting is unknown on **all four**
targets, including Axum. Axum additionally publishes neither its memory ceiling
nor inbound population; Cloudflare's inbound population is also unknown.
Consequently no unchanged platform descriptor can produce four `Fits` results,
even after the application demand is repaired. Section 5.3 assigns these
external prerequisites; local remediation must not change an unknown to known
or a nonpassing SDK result to passing to obtain a green command.
AG owns the Spin deployment-profile decision; the Auctioneer Spin maintainer owns
dispatch/envelope alignment and its target proof. Task 3 must name the selected
hosted or explicitly configured generic deployment and its next evidence action.
Until resolved, retain the mandatory Spin row as blocked with that owner; choosing
generic/unknown is not permission to remove Spin or close its fit obligation.

## 3. One Accounting Contract

Keep the current checked additive model in `auctioneer-runtime/src/budget.rs`:

```text
Q = N * per_auction_peak
S = W * per_stream_peak
total = B + G + Q + E + A + T + P + H + S + C + I
```

| Symbol | Existing owner and multiplicity                                                                                                                                  |
| ------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| B      | `baseline_bytes`, once per declared memory scope; includes its documented existing reserves.                                                                     |
| G      | Distinct parsed GVL versions: `(cache entries + admitted auctions + auxiliary permits) * parsed-list bound`.                                                     |
| Q      | The six prepaid `AuctionCharges` receipts, times admitted auctions N; catalog validation V and context K remain inside projection.                               |
| E      | Successful egress permits times final-response handoff and transmit state.                                                                                       |
| A      | Auxiliary plus independent probe-config permits, each funding the auxiliary peak/handoff/state; retain the probe-config minimum when auxiliary permits are zero. |
| T      | Terminal refusal permits and their handoff/state.                                                                                                                |
| P      | Resident cached probe output plus independently gated in-flight probe responses.                                                                                 |
| H      | Larger of H1/H2 pre-admission parser peaks times the platform-enforced population at the same memory scope.                                                      |
| S      | Active outbound body streams times transport/decoder/header-stream peak.                                                                                         |
| C, I   | Disjoint catalog resident and initialization increments, each once; retain I even when auction work dominates.                                                   |

Here W is the global active-body-stream population. The named term set,
currently eleven summands, must come from one runtime-owned `IsolateTerms`
record used by the checked fold and exhaustive report adapters. Do not freeze
an unrelated count or report-key list in CLI/tests. Adding a runtime term must
force its fold/export/audit mappings to be updated; use exhaustive destructuring
without `..` and an added-field compile control. For every owned term, a unit
increment must change the checked total by exactly one. Zero-valued terms
remain rows.

There is no startup/steady partition or `max(startup, steady)` in this contract.
Do not subtract existing baseline reserves to manufacture space for C/I.
`CatalogResourceCosts::UNCHANGED` means zero increments beyond a matched
baseline, not free existing metadata, code, schema or initialization storage.

Use `auctioneer-core::host_primitives::resolve_budget_inputs` for boot, export and
strict checking. Its effective backend/blob/node caps and checked serialization
of exactly `(host_default(), account_default())` are required inputs to the same
`derive_isolate_budget_with_baked_config` function with construction and catalog
descriptors. No private raw-zero-cap or zero-baked-byte shortcut is allowed.

Extend M7's planned `auctioneer-cli bidders resources --check --all-adapters` with
an explicit `--require-fit` policy, using the same inputs, terms and report. The ordinary
catalog-relative check may pass while recording blocked WASM absolute fit;
`--check --all-adapters --require-fit` returns nonzero if any of its four adapter rows lacks
required platform facts or justified separate-stack requirements, or fails
primary fit/headroom or a separate-stack limit. Missing rows, stale identities
and malformed inputs refuse both modes. This is an arithmetic and bound-fact
gate, not an overall deployment-qualification command.

The operator-facing selector is `--adapter`, matching build, deploy and serve:
`auctioneer-cli bidders resources --check --adapter fastly`. Accepted adapter names
are `axum`, `cloudflare`, `fastly` and `spin`; compiler targets/ABIs are derived
from each adapter's profile, never a second selector or an operator mapping.
Require exactly one of `--adapter <name>` and `--all-adapters`. A single-adapter
report declares `selection.scope = "adapter"` and its selected identity;
`--all-adapters` declares `selection.scope = "all_adapters"`. The report's
`adapters` rows must exactly match that selection, and `--require-fit` applies
only to the declared selection. Single-adapter success cannot establish global
R0 completion. Canonical capture, audit/plan validation and frozen review always
require `--all-adapters` and exactly four adapters. Audit rows and change packages
use `adapter` identities, not compiler target names. This is a hard cutover of
the unmerged resource-record format; historical qualification schemas/captures
are unchanged.

For effective ceiling `c = min(configured_isolate_bytes, platform_memory_bytes)`,
require `total + headroom <= c`, with checked arithmetic. New strict acceptance
will require WASM headroom `ceil(c/16)` and native headroom
`max(ceil(c/16), 268,435,456)`; compute the ceiling division without overflowing
`c + 15`. Neither floor is currently enforced in production: native has the
`shipped_baked_budget_leaves_256_mib_headroom` test, not a shared acceptance
policy. Section 3.1 adds that policy. Pin the independent headroom literals and
test exact/under/over boundaries with unequal ceilings. Missing scope or an
unknown platform ceiling cannot become a passing result through a sentinel.

### 3.1 Shared Headroom Policy

Add one pure `minimum_headroom_bytes(c, HeadroomClass)` policy to the existing
runtime `budget.rs`, with explicit `Native` and `Wasm` classes mapped from the
validated target identity. It implements the floors above using
`c / 16 + u64::from(c % 16 != 0)` for a known nonzero effective ceiling.
Pin the independently computed nondivisible WASM case
`c = 100,000,001 -> 6,250,001` bytes. The current provider ceilings are all
divisible by 16 and cannot detect removal of the rounding term; test both the
helper and real resource/qualification callers with a WASM identity for this
case. Also pin native `c = 4,294,967,297 -> 268,435,457`; native's fixed floor
would hide the remainder in the smaller case.
This policy does not change the additive derivation or boot's warning policy.

Both the resource checker and qualification `Profile::validate` must call this
policy. Qualification profiles with `headroom_bytes` below the floor are invalid,
not silently clamped or accepted because the value is positive. A resource row
without a declared override selects the floor; an override below it is invalid.
An accepted larger declaration remains authoritative. Qualification `account`
and `check_memory` use that validated declaration for accounted and measured
fit, respectively. The resource checker reports the floor and selected headroom;
the qualification report's `required_headroom_bytes` remains the selected value.
Reject unknown/contradictory target identities before choosing a class.

In particular, the current `headroom_bytes != 0` validation and evaluator's
unchecked trust in that value are not the completed contract. Require actual
`qualify validate`, `qualify evaluate` and resource-checker controls,
not only helper tests. With all other conditions valid, floor-minus-one refuses,
the floor is accepted when demand fits, and a larger declaration can independently
make an otherwise fitting demand fail. Preserve historical captures unchanged;
do not add a legacy-headroom exception to current acceptance.
Policy acceptance is not aggregate acceptance: a real resource command may
accept the headroom declaration and still refuse on external facts. Assert
both outcomes explicitly; all-known synthetic SDK facts belong only in the
adapter's test fixture, not a command-line override for a real target.

### 3.2 Platform And Separate-Stack Validation

Use the pinned SDK's `PlatformMetadata::validate_memory_envelope` at the CLI's
platform-validation boundary, preserving source-bound platform facts and every
indeterminate reason. Adapt the already-derived primary demand without a second
population multiplication: `MemoryEnvelope.fixed_bytes = total + selected_headroom`
(checked), `per_live_request_bytes = 0`, and the reviewed memory-domain scope.
The configured-ceiling comparison still applies independently. This SDK adapter
does not move or remove any summand in the shared additive derivation.
Before adaptation, require the budget's pre-admission population to cover the
selected SDK's known inbound bound at that same scope. Passing zero per-live
bytes cannot excuse an under-declared population or an unknown fact.

When the selected platform publishes a separate stack, require an independently
justified, nonzero stack-peak upper bound tied to artifact, ABI, toolchain and
covered paths. Supply it as `separate_stack_bytes`, not as primary-memory capacity
or an extra primary summand. Fastly's current allowance is exactly **1,000,000
bytes**. A missing/unproved requirement stays pending; a bound above that allowance
fails even when primary memory plus headroom fits. Do not substitute zero or
the published allowance itself for an unmeasured/unjustified requirement.
Targets publishing no separate stack pass `None`; this does not exempt their
stack storage from the audit of what their primary allowance includes.

Only SDK `Fits` plus the configured-ceiling/headroom checks can pass required-fit
mode. `Exceeds`, every `Indeterminate` reason, validation errors and future
unrecognized outcomes remain nonpassing, including unknown host ingress
accounting and `MissingSeparateStackRequirement`. Preserve the reason/result in
the bounded source-bound resource report; a successful primary comparison must
not overwrite a missing or excessive stack verdict. Reuse the existing SDK
contract rather than requesting another platform validator from EdgeZero.

Report local numeric fit and outstanding external facts separately from the
SDK's exact verdict. Overflow of the configured ceiling is a Auctioneer demand
failure whether or not the platform ceiling is known. In particular, with no
published ceiling the SDK cannot report primary `Exceeds`; the independent
configured-cap comparison is the sole numeric ceiling-failure signal. Compare
demand plus selected headroom when available; demand alone above the configured
cap already proves a local failure without inventing effective-platform
headroom. Retain pending effective headroom/gap when the platform ceiling is
unknown, alongside that local failure. Passing the configured comparison does
not prove platform fit. With a known platform ceiling, exceeding that ceiling
is also a demand failure. Otherwise-fitting arithmetic with only unpublished
required facts is an external-prerequisite block, not a pass. Both failures
and external blockers can be present: the SDK returns `Exceeds` before
gathering indeterminate reasons, so retain the source-fact inventory even in
that case. Require controls for demand
failure alone, external facts alone, and both together, plus a genuinely
all-known fitting control. Never map `Indeterminate` to success. At this pin,
aggregate exit zero additionally depends on the section 5.3 external work;
it is not a promised outcome of Auctioneer-only patches.
Include unknown-platform-ceiling controls with a fixed known declared headroom
and demand-plus-headroom below, at and above the configured cap. All retain
the external blocker, while only the overflow adds a local ceiling failure;
no owner is hidden by SDK `Indeterminate`. This known sum is not a substitute
for the pending effective-platform floor or gap.

Boundary controls must exercise the actual Auctioneer-to-SDK mapping with independently
known test facts: missing, zero or unproved stack requirements; proved bounds of
`999,999`, `1,000,000`, and `1,000,001` bytes;
scope mismatch; unknown host/population facts; unexpected separate-stack shape;
and a per-instance population greater than one that must not multiply the
already-derived total again. Test facts prove the adapter, not hosted readiness.

Boot currently warns and installs an over-ceiling envelope; strict acceptance
rejects it. A headroom-only shortfall will refuse strict acceptance, not boot;
the existing boot validator still compares derived demand without a headroom
term and its warning policy is unchanged. Headroom is a separate acceptance
inequality, not a twelfth allocation summand. R0 does not bypass health routes
or turn a malformed profile into an unauthenticated diagnostic. Any proposed
production startup refusal needs a separate explicit behavior amendment.

## 4. Audit And Feasibility Checkpoint

Before allocation, workload or admission changes, produce one bounded,
source-bound report for each target and a native control. Reuse M7 Task 4's
CLI/resource-report owner; do not build
a second calculator. Record raw and normalized limits, baked bytes, target and
construction/catalog inputs, each term, ceiling/headroom and fit status.
Record source/lock/toolchain/features/ABI, SDK and profile identities. Do not
embed secrets, request bodies or arbitrary runtime configuration in the report.
Include separate-stack allowance/requirement/proof identity and SDK validation
status/reasons; missing proof remains visible. Audit/tooling work may add the
pure shared policy and CLI validation above before the remediation checkpoint,
but may not change allocation ownership, declared workloads or boot policy.

The audit must map each allocation category to its concrete owner, acquisition,
retention and release points; enforced population and memory scope; separately
funded overlapping copies; independent bound evidence; and unresolved facts.
At minimum inspect configuration extraction and snapshots, privacy projection,
response parsing and retained bids, debug capture, GVL eviction, guest/host
handoff, auxiliary and probe work, ingress parsing, and registry initialization.

The feasibility record produces an explicit decision table, not a promise that the
current workload can fit. For each proposed change name the owning functions,
old/new lifetime or population, independent proof/test, projected term delta,
remaining fit gap and affected targets. Classify it as:

1. **Implementation improvement:** bounded sharing, scratch reuse or earlier
   release with unchanged accepted workload and semantic output.
2. **Accounting correction:** demonstrated overcount or wrong scope, supported
   by independent allocation/lifetime evidence, not merely low sample RSS.
3. **Workload/profile change:** explicit old/new numeric caps and product impact;
   requires user approval before implementation, even if it seems necessary.
4. **External prerequisite:** exact missing SDK/platform bound, enforcement
   hook or observation, with owner and next verification action.

Class-1 changes and proof-backed class-2 corrections may proceed under AG's
authorization below. Preserve the accepted workload and the single shared
additive total. A smaller coefficient, corrected unit or split handoff charge
requires an independent bound for every affected allocation and population,
not just a passing sample or a desired fit. Workload changes and replacement
of the shared additive equation still require specific approval.

### 4.1 Required Audit And Feasibility Record

Commit `crates/auctioneer-core/tests/fixtures/resource_envelopes/audit.toml` as the
machine-readable source for the human audit, not a prose-only sign-off. Require
exactly the four target rows, raw/normalized inputs and identity fields above,
all runtime-owned term IDs (including zeros), the six auction receipts,
construction/baked inputs, scope/fact/stack results, headroom and fit gap.
Each term names its allocation owner, acquisition/release, enforced population,
ABI evidence and unresolved-fact owner/next action. Generate the human tables
from this record; the CLI resource owner checks completeness and rederives sums.

Each adapter owns one typed core `ResourceProfile`, populated alongside its
existing envelope declarations. Bootstrap borrows the raw boot envelope from
that profile, and the CLI aggregates the same exports. Platform and actual
dispatch facts, intended ABI, accounting scope, headroom class and construction
descriptors come from the adapter, not a provider-specific table in the CLI.
Configurable dispatch passes the profile's dispatch metadata to the SDK; SDK
wrappers with fixed metadata use their published constant. Compare metadata,
not descriptive labels, for deployment/dispatch alignment. Core retains input
normalization and runtime retains every receipt, summand and fit policy.
The source record stores the accounting scope so frozen audits do not read
later adapter declarations when validating old owner/scope arithmetic.

Use bounded typed TOML records (`schema_version = 1`); reject unknown fields,
duplicate identities/terms and missing required values. The required schema is:

| Record              | Required fields                                                                                                                                                                                                                                                                                                   |
| ------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Audit header        | `code_base_commit`, `sdk_revision`, `lock_sha256`, `toolchain`, `features`, frozen independent probe populations in `runtime_facts`, and exactly four `adapters`.                                                                                                                                                 |
| Adapter audit       | `adapter`, artifact/source/SDK/lock/toolchain/features/ABI identity, raw/effective limits, baked bytes/digest, construction/catalog descriptors, named `terms`, `auction_receipts`, frozen component/auxiliary receipts, `owners`, platform/stack proof status, selected headroom, checked total and `gap_bytes`. |
| Owner row           | Runtime `term_id`, allocation/source path, acquisition/release, population/enforcement, ABI/bound evidence, disposition, and unresolved owner/next action when pending.                                                                                                                                           |
| Change-plan header  | `code_base_commit`, `audit_sha256`, and nonempty `packages`; each package targets a present audit row.                                                                                                                                                                                                            |
| Remediation package | Unique `id`, `adapter`, ordered changes with before/after named terms and evidence, projected total/residual gap, exact `files`, `test_commands`, and `full` or `partial` disposition.                                                                                                                            |

Workload/profile changes additionally record typed old/new declared limits,
product impact and the selected deployment scope. These caps replay from the
preceding proposal state, just as the named terms do. Do not reject a mixed
implementation/workload proposal by applying the old implementation's
ownership coefficients to its projected future state; these are reviewed
projections, not claims that future allocation bounds have already been proved.
Implementation must later reconcile the realized shared derivation and owning
proofs with the approved projection.

Term/target IDs use the owning enum/record, not free-form labels. Apply the
existing bounded CLI input policy (at most 1 MiB per file), with at most 128
exact file paths per package. Audit rows must explicitly distinguish native
layout evidence from actual WASM ABI evidence; a native control cannot fill an
unobserved guest-layout field. Unknown evidence is a typed pending disposition.
An unknown scope/ceiling also leaves the effective headroom/gap typed pending,
never zero. Such a target cannot claim a full projected-fit package; any useful
partial work retains that uncertainty and its named next action.

The current-source check uses
`bidders resources --check --all-adapters --audit <path> --plan <path>`. It requires a clean,
matching compiled source identity and all four actual source profile digests,
then rederives the audit through the shared runtime owner. It does not accept
self-consistent invented baked bytes, layout charges or limits as the current
shipped row. This mode checks the records; it does not approve a package or
establish allocation or deployment fit. It is mutually exclusive with review
and `--require-fit` modes.

`bidders resources --check --all-adapters --capture-audit` emits the canonical typed TOML
from the four real source rows, with source-grounded lifetime owners and
explicitly pending artifact/ABI proofs. Dirty or mismatched compiled sources
refuse before any output. It changes no profile, coefficient or production
allocation. `--check --audit <path>` without a plan validates the same current
source binding and generates the human Markdown tables; adding `--plan` keeps
the package-validation mode above. Capture cannot mix with audit, plan, review
or fit modes. Generated tables identify nested component rows as explanatory,
not additional summands, and never authenticate change approval.

For each target, a decision row is mandatory for every term greater than 5% of
the exact normalized demand (`term_bytes > total_bytes / 20`), including its
auction/auxiliary subcomponents above that same isolate-total threshold. Do not
select terms using rounded percentages or the older subtotals. Record smaller
terms too, with an explicit
unchanged disposition where appropriate. Missing owners/rows refuse the check.

Freeze an ordered package of proposed changes with complete before/after term
values and checked aggregate savings against
`gap = max(0, total + selected_headroom - effective_ceiling)`. Reconcile overlaps
by applying each change to the preceding state; do not sum two independent
savings estimates against the same original bytes. The final projected terms
must re-sum under the shared equation. Approval of a limit/model change requires a package with
an explicit full or partial disposition, not an assumption that implementation/
accounting improvements close the gap. A **full** numeric package must close
the target's entire gap, not six unrelated small reductions; it still grants
no missing platform fact. Given section 2's approximately 99.47-99.91%
reduction requirements, an explicitly AG-approved **partial** package is the
normal first disposition, not an exceptional fallback after weeks of
unsuccessful unchanged-workload optimization. Present the three class-3
proposals before freezing a limit-changing package; this does not block verified
unchanged-workload improvements. Partial approval must record its
residual gap, dominant unresolved terms, named owner and next action; it grants
no fit or closure. No workload reduction is authorized by the existence or
approval of this design alone.

### 4.2 Approval And Scope Enforcement

**Execution authorization (AG, 2026-10-18, explicit user replies):** "Proceed with
unchanged-workload improvements; ask before limit/equation changes," followed
by "Approve proof-backed accounting corrections; preserve workload limits."
The latter expressly covers correcting config-node units and separating
response-handoff storage from config/privacy work when independent bounds
prove the change. These authorizations
supersedes the earlier requirement for an approved frozen package before every
production ownership change. Such improvements may proceed through ordinary
scoped implementation, red/green tests, independent review, commit and push.
Preserve accepted workload, privacy/output semantics, all declared workload
limits and the single runtime-owned additive total. Record observed allocation changes separately from
accounted savings; an implementation improvement need not reduce a charged
term. No per-patch human reapproval or frozen-window CI binding is required
for this authorized category.

No per-patch frozen approval window is required for an independently proved
accounting correction within this authorization. Record the old/new units,
allocation owners, enforced populations, independent proofs, exact charged
delta and remaining gap before claiming savings. In particular, an auxiliary
receipt reduction alone does not reduce an unchanged declared isolate slot.
Do not credit a handoff split until all contributing routes and uninitialized
or stale-App contexts are bounded or refused before extraction.

**Unfunded-context decision (AG, 2026-10-18, explicit user reply):** "Refuse
unfunded contexts; retain bounded liveness." Before runtime-config extraction,
the single core resolver requires an installed budget and captured SDK backend,
blob and value-node caps no larger than that budget's normalized caps. Refuse
before any store read when either precondition fails. Apps created before boot
do not acquire larger funded caps merely because boot subsequently succeeds;
a newly configured App may recover. A missing store binding continues through
the existing missing-config policy because it performs no extraction.
Adapter clock/timer/RNG initialization remains a dispatch prerequisite;
"preboot" here means before budget installation, not before platform primitives
exist. Do not mask absent adapter initialization with fallback primitives.

Expose a fixed `unfunded_extraction` config diagnostic, with no input values or
secret/config contents. When a decision route reaches this resolver refusal,
it fails closed with a bounded HTTP 500 and no retry hint; earlier admission
or static-response refusals retain their existing statuses. This initialization
error is not a transient backend failure. `/status` keeps its bounded liveness response and reports the
same degraded code rather than reading unfunded config. Normal boot-before-App
entrypoints and all accepted workload caps remain unchanged. This boundary
closes unfunded extraction, not the remaining typed/merge, all-route output,
handoff or guest-allocation proofs.

AG remains the required human approver before changing workload limits or
replacing the shared additive equation. The implementer may prepare a proposal
but cannot self-approve it. For changes outside the authorization, freeze
`crates/auctioneer-core/tests/fixtures/resource_envelopes/remediation-plan.toml`
with the audited code-base commit, audit digest, target/package IDs, ordered
numeric deltas, exact repository-relative changed-file paths, proof/test
commands, residual gaps and full-versus-partial disposition. No path globs,
directory-wide grants or automatically approved ownership-table entries.
Include necessary test, workflow, human-audit and evidence updates in that
exact list, not just runtime implementation files.

AG's actual approval must identify the immutable review commit containing both
records, via a PR review or explicit user message. Retain its author, date and
external reference in the human audit; a self-written `approved=true` is not
authorization. Keep approval pending until that record actually exists.
Mechanical checks do not authenticate a human approval; verify the external
record before beginning the affected limit/model patch and before pushing it. No extra signing
layer or new approval service is required.

Commit any concrete spec/plan amendments before choosing the audited code-base
commit B. Bind/rebind the audit to B, then create records-only review commit D;
do not append new plan steps inside the B-to-D records-only interval.

Extend the existing CLI resource owner with `--check --review-commit <sha>`.
Read the frozen records from that Git commit, not mutable worktree copies;
verify the audit digest and reviewed code-base continuity. The audited code-base
must be its ancestor; only the two records and the human audit may change
between those commits. Code/profile/dependency drift requires a fresh audit.
The review must also be an ancestor of the checked head. With `--staged`,
check committed changes since that review plus the index; with `--head <sha>`,
check the complete review-to-head diff. Include added/deleted paths and both
sides of renames. Every changed file must appear in the frozen exact-path list;
the records themselves cannot be changed inside this implementation window.
For an approved frozen package, run the staged check before each commit and the head check before each push,
and require the head check in CI. Missing/invalid review commits, absent paths,
changed records or stale audit baselines refuse; extend scope only through a new
record and a new AG approval. This checks scope, not semantic correctness.
The staged/head selectors are mutually exclusive and one is required in review
mode. CI binds the externally approved review SHA; it cannot choose the current
head as its own approval. Projections and audit checks are not fit qualifications.

Keep current-source validation separate from frozen-review validation. Once
AG has approved D, an allowed accounting patch at H may legitimately change a
coefficient used by the calculator. The scope checker must not recompute B's
old inputs with H's new coefficients and thereby invalidate its own approved
baseline. Instead it validates the frozen record's checked named-term and
receipt sums, ordered projections and metadata, together with Git's B-to-D
records-only continuity and D-to-H scope. Recorded component and independent
probe populations remain frozen too. Initial current-source validation and
AG's external review establish the baseline; the later mechanical scope
check does not repeat or replace them. No alternate budget equation is added.
Repository-local diff settings must not hide gitlinks; Git scope checks reject
symlinks, gitlinks and unresolved index entries rather than treating them as
grants over regular files. Unstaged user edits remain outside staged scope.

Serialize R0 package commits on the M7 branch while that window is active;
unrelated M7 changes wait until it closes, or require an explicitly renewed
checkpoint rather than an automatic exclusion. AG approves the final scope
endpoint H and the closing CI-binding change after the D-to-H check passes.
That closing change may update only the approved workflow binding and human
audit, not bundle further implementation. Closed CI keeps checking frozen D-to-H
using the explicit approved endpoint instead of the moving M7 HEAD. Later M7
work can then proceed; a new R0 package opens a new reviewed window. Closing a
window does not close a target or qualify later artifacts: source-bound fit and
evidence checks still apply independently.

Review and approve any patch list outside the authorization before implementing it.
If it cannot close a target's gap without a workload change, present that
numeric proposal to the user and amend this spec/plan first. Until then retain
the old row as blocked; never guess a smaller profile or claim R0 completion.
If a phase-aware equation is proposed later, it requires a separately reviewed
amendment changing shared derivation and all consumers together, backed by an
enforced lifetime separation. This document authorizes no such equation.

## 5. Implementation Constraints

### 5.1 Ownership Before Coefficients

Prefer removing a real redundant allocation to reducing its charge. Shared
immutable config needs bounded retained versions, keying/invalidation and
failed-refresh ownership; an `Arc` alone does not bound distinct snapshots.
Existing request-consistent config/secret snapshots, signing-key rotation,
privacy decisions and fail-closed config recovery remain unchanged.

Streaming parse or scratch reuse must still fund retained successful bids,
headers, errors, correlation data and final output. `StreamGate` bounds active
outbound streams, not all completed results. Replacing call multiplicity by
stream width without changing/proving ownership is forbidden. Preserve full
accepted response semantics, per-bid error isolation and media attribution.

Maintain receipt reservation before work, release on every failure/cancellation,
and leases through suspension and response serialization/handoff. Reuse the
existing `AuctionBudget`, admission/stream/egress/auxiliary gates and EdgeZero
paved handler/body APIs. Do not create an allocator-based security guarantee:
the receipts are accounting/admission, not interception of every allocation.

Decoder windows/layers, node/depth caps, checked layout on 32-bit guests,
metadata and signed-cookie limits, deadlines, exactly-once completion,
terminal/probe availability and privacy disclosure rules remain protected.
Do not truncate valid bids or disable debug/privacy work to improve a meter.

### 5.2 Target And Catalog Continuity

The audit must distinguish native layout, actual WASM ABI layout and host-owned
buffers. Native `size_of`, `include!` envelope tests, package sizes and Viceroy
boot/health results are not whole-target heap evidence. Every reduced bound
needs a stated target-applicable argument and, where executable, observation.
Unknown host overhead remains an explicit blocker, not zero.

Rebase M7's matched baseline and candidate together after an approved R0 change;
preserve the former reports and exact catalog deltas. Smaller unrelated costs
cannot compensate for a breached C/I/V/K/A catalog allowance. Keep the native
passing workload/headroom control, source catalog and enabled bidder set intact.

### 5.3 Auctioneer And EdgeZero Responsibilities

Auctioneer owns the arithmetic, real allocation ownership, limits/profiles, receipts,
handler integration and tests. EdgeZero/provider follow-ups own missing enforced
pre-handler population bounds, body/parser/handoff memory facts at the declared
scope, and measurement hooks that application code cannot supply. Each request
must name the adapter, contract, enforcement point and independent acceptance
control; do not request a new signing layer or assert that a pending hook exists.
Portable work can proceed against reviewed interfaces, but no unresolved external
fact is promoted to a proved target bound. Keep feedback and status separate.

The EdgeZero adapter maintainers own the source-bound contract for publishing
host-ingress/framing memory accounting on Axum, Cloudflare, Fastly and the
selected Spin deployment. They also own publishing/enforcing Cloudflare's
inbound population and exposing verified operator-configured ceiling,
scope and population facts for Axum (and generic Spin if selected). AG owns
supplying/reviewing the deployment-specific operator declarations; a Auctioneer
numeric limit alone is not that evidence. Provider-unpublished storage facts
require provider evidence or a justified enforced bound, not a fabricated
descriptor. Each remains an explicit external prerequisite with owner and
next verification action until the selected SDK/artifact actually supplies it.
Update the dependency and rerun all four SDK mappings when that work lands;
the existing API is reused, and no future implementation is assumed here.

### 5.4 External Contract Acceptance

These are separate project requirements, not Auctioneer patches or declarations
that the pinned SDK already supplies the facts. The latest PR #275 head was
checked on 2026-10-18 and still matched the pinned `f6c708cc` revision.

| Owner                       | Required contract and enforcement point                                                                                                                                                                                                                                                                                                                                                                            | Independent acceptance control                                                                                                                                                                                                                                                                                                 |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| EdgeZero: all four adapters | Publish justified `host_ingress_memory_accounting` at the selected memory scope, covering parser/framing and retained ingress storage. State which provider-owned bytes count toward that ceiling; do not equate guest heap with total charged memory without evidence.                                                                                                                                            | Leave the fact unknown in an otherwise-fitting row and require `Indeterminate`; supplying a supported fact may clear only that blocker. Exercise cold, rejected and cancelled requests as well as successful dispatch.                                                                                                         |
| EdgeZero: Axum runner       | Expose operator-configured ceiling/scope and the same resolved connection population that the HTTP runner enforces before allocating a parser. `build_app_for_dispatch` currently uses static `AXUM_PLATFORM`; metadata must describe the actual resolved runner configuration, not an unrelated default. Operator memory declarations need explicit provenance; Auctioneer's 4-GiB arithmetic cap is not an OS quota. | Independently vary the configured memory ceiling and connection bound. Verify dispatch metadata matches the resolved enforcement settings, a bound above the funded population refuses, and invalid overrides fail before serving. Unknown operator facts remain unknown.                                                      |
| EdgeZero: Cloudflare        | Supply a justified per-instance inbound population bound, enforced before the counted guest ingress allocation, or retain it as unknown. Document separately any provider-created request storage excluded from that guest enforcement point. Six outbound connections are not an inbound bound.                                                                                                                   | Concurrent ingress beyond the declared population must not create another counted parser/body reservation. Vary inbound and outbound populations independently; changing the outbound allowance must not change the advertised inbound fact. If provider storage cannot be bounded/classified, keep strict acceptance pending. |
| Auctioneer: hosted Spin         | Align the actual dispatch metadata with AG's selected Akamai Functions hosted Spin contract. Generic `SPIN_PLATFORM` is not hosted evidence. This is an implementation/proof package owned by Auctioneer; no extra SDK interface is assumed.                                                                                                                                                                           | Compare metadata structurally, retain the generic/hosted mismatch negative control, and bind the hosted execution observation to its actual artifact/deployment. Selecting a constant alone does not prove memory fit.                                                                                                         |
| Auctioneer: Fastly              | Establish the compiled artifact's separate-stack requirement across initialization, admitted depth, errors and cancellation, against the published 1,000,000-byte allowance. The missing application upper bound is not an EdgeZero implementation defect.                                                                                                                                                         | Test below/at/above the justified requirement with primary memory independently fitting. Missing proof, zero requirement and stack overflow remain nonpassing.                                                                                                                                                                 |

Each EdgeZero follow-up must retain provenance and unknown states. A fact-only
update must not lower Auctioneer's demand or alter its receipt populations. On any
dependency update, repeat the four adapter mappings, unequal-ceiling and
independent-population controls, then update source-bound records. Do not turn a
provider-unavailable guarantee into a hard-coded positive fact merely to close
R0.

## 6. Proof And Failure Semantics

Reuse M7's pure `catalog_resource_profile` controls and fresh-process
`boot_strict_parity` target rather than parallel installers. Each installing
row runs in a new bounded child of a single-test binary and asserts no prior
boot gates. Neither a mutex nor `--test-threads=1` resets the four OnceLocks.
Compare real boot retention with the actual CLI strict path, not two calls to
the pure derivation. Cover all-zero caps, each zero separately, unequal explicit
caps and independent nonzero baked serialization counts.

For each reduced cost or multiplicity, require independent positive and negative
controls: exact cap and cap+1, unequal populations, at least two input sizes,
and overlap/cancellation/failed-refresh cases where applicable. Assert work
actually occurred. A coefficient-removal or population-widening mutation must
fail the owning control for the intended reason. Expected byte bounds cannot
be computed solely from the production constant being tested.

Separate four outcomes in the existing report/evidence ownership:

| Outcome                  | Passing evidence                                                                                                                                      | Does not establish                                  |
| ------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------- |
| Accounted fit            | Exact shared additive demand plus policy-compliant headroom fits the verified primary scope; SDK validation also accepts any separately capped stack. | Physical allocations or runtime enforcement.        |
| Allocation bound         | Independently bounded owning paths and target-applicable layouts/lifetimes.                                                                           | Missing provider-owned storage or hosted execution. |
| Target observation       | Digest-bound executable guest/runtime measurements with declared workload, scope, meter and overlap.                                                  | A different target/deployment or larger workload.   |
| Deployment qualification | M7.6's complete applicable hosted acceptance.                                                                                                         | Unexecuted publisher/video/catalog capabilities.    |

Failures and missing/stale evidence retain failed/blocked/pending dispositions;
they cannot yield an overall qualified verdict. Preserve current qualification
schema-2 identity, bounded input, exact workload reconciliation and no-clobber
evidence publication. A changed wire contract needs its own hard version cutover;
do not casually add optional readiness defaults to historical formats.

CI must run the owner tests, source-bound report/strict controls and actual
three WASM target builds/lints already in the workflows. Preserve the existing
Cloudflare upload, Fastly package and Spin component ceilings. Pin required
commands/features, unconditional measurement steps and workflow triggers through
the existing semantic CI guard. A missing runner/meter is reported, not ignored.

## 7. Acceptance And Handoff

AG's section 4.2 authorization permits verified unchanged-workload
implementation improvements and independently proved accounting corrections.
Workload changes and replacement of the shared additive equation still require
the specific approval above. Authorization and passing tests do not themselves establish
deployment fit or R0 completion.

Per-target R0 closure requires exact source-bound boot/strict parity, fit and
headroom, justified separate-stack fit where published, independently justified
allocation/population bounds, and executable target-applicable observation for
changed paths. Any unavailable meter or
unenforced required population leaves that target pending. Global R0 completion
requires all three affected WASM targets; partial closure must name them.
Partial closure is progress recorded in individual target rows, not an exemption
from the aggregate CLI policy: `--require-fit` still exits nonzero until native
and all three WASM rows pass. Reports must retain the passed subset and remaining
blockers. An applicable-target M7.6 handoff may use that target's complete proof;
it must not relabel the failed aggregate result as a passing command.

Distinguish **approved Auctioneer package complete; external prerequisites pending**
from per-target acceptance and global R0 completion. That work-status record
requires all approved local patches/proofs and numeric comparisons to be
reconciled, with each remaining external fact, owner and next action named;
unknown-ceiling platform fit stays pending, while independently detected
configured-cap overflow remains a local failure. No synthetic platform ceiling
is supplied. It does not close R0, grant target readiness or permit aggregate exit
zero. Aggregate success also requires EdgeZero's missing all-target ingress
facts, Axum ceiling/population and any other selected-platform facts to be
published and verified under section 5.3. Native's unknown metadata must not
be bypassed merely because R0's allocation changes focus on WASM.

Update `ISSUES.md`, the roadmap and M7 status with actual results and blockers.
Hand M7.6 the approved artifact/profile/SDK/toolchain identities, workload and
limits, retained accounting reports, target observations, unresolved external
facts, and the existing hosted acceptance runbooks. Do not mark credentialed
deployed-handler checks, corrupt-config recovery or publisher readiness complete
without executing their separately authorized M7.6 acceptance.

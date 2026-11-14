# Schema Compiler Qualification

For the current source-bound closure, implemented components and remaining proof owners, see [resource proof ownership](../testing/resource-proof-ownership.md). Historical checkpoint prose below retains its original scope; it is not current target certification.

**2026-11-10 publication amendment:** Mocktioneer is now compiled in both
feature lanes, disabled by default, with an explicit endpoint required to enable
it. Current shipped source checks require 18 native bindings (four guest
bindings), 18 selected roots/members and 16 effective schemas in each lane.
Earlier 15/16-cohort measurements below retain their original historical scope;
the new publication does not requalify those linked artifacts.

**2026-10-30 delivery amendment:** AG approved
[controlled-native acceptance](../superpowers/specs/2026-10-30-controlled-native-bidders-completion-design.md)
using tested reservations, enforced limits and real handler/allocation/stress
checks. Complete worst-case certification remains unproven and moves to resource qualification.
The inspection feature still refuses serving; normal native builds now compile
the selected validators for separately funded native admission. New guest
publications remain excluded. The checkpoint narrative below predates this
amendment and is not a statement that the formal engine gate has passed.

**Status: patched candidate under qualification.** The published 0.58.6
generator bypasses numeric type restrictions in two committed schemas. The
approved bounded compiler patch below repairs those controls; complete engine
and resource qualification remain pending. This record covers structural,
native compatibility and linked compiler controls, not completion of the
engine gate or permission to enable
request-time schema validation. No production registry or auction path uses the
candidate. Normal serving dependency graphs exclude the candidate runtime;
the inspection-only staging feature also links it for native and guest checks
and explicitly refuses application startup and serving admission.

## Shipped Qualification Scope

The [2026-10-29 amendment](../superpowers/specs/2026-10-29-shipped-schema-qualification-design.md)
scopes new bidder catalog serving/resource acceptance to the complete generated shipped
closure `S`, including planned priority and existing published bindings,
aliases, references and shared helpers in each adapter/feature lane. Existing
full-source correctness, compatibility, integrity and component controls below
remain intact; do not repin their 261-schema counts to a smaller cohort.
Their historical whole-catalog investigations are not an instruction to prove
every unshipped backend before finishing bidder catalog. Required shipped-path bounds
remain mandatory, measurements stay observations, and newly needed engine
surfaces belong to their bidder expansion wave or residual remaining catalog closure publication gate.

### Selected Source And Generated Membership

The 2026-10-29 native source-conformance checkpoint derives the union of
generated publication declarations, the frozen 13-row priority wave and the
separately witnessed supplemental priority root. It resolves ancestry and
effective schema ownership before expanding the selected schemas. Runtime
disablement is not an exclusion; a feature-gated definition is absent only
from the lane that does not compile it. Factory sharing does not import every
schema owned by that factory. Explicit local schema-free bindings stay explicit.

| Source/Generated Population | Secure Lane | Development Lane |
| --- | ---: | ---: |
| Planned priority roots | 14 | 14 |
| Published plus planned roots | 16 | 17 |
| Members including ancestry | 17 | 18 |
| Effective parameter schemas | 15 | 16 |
| Generated regex helpers | 4 | 4 |
| Shared hostname regex | 1 | 1 |

Both lanes require the hostname and UUID format callbacks. The manifest retains
source/schema digests, binding/ancestry membership, expansion options, compiler
and target identity, lock/workspace-manifest identities and verified vendor
source witnesses. Expansion refuses `compile_error!`; absence of extracted
helpers is not treated as successful compilation after a generator error.
The unchanged all-source inventory still contains 261 main schemas, with the
two separately witnessed supplemental schemas outside that census.

Run the focused controls without rebuilding the full-catalog child probes:

```sh
cargo test --locked --offline -p auctioneer-bidder --test schema_source_conformance schema_selection -- --test-threads=2
cargo test --locked --offline -p auctioneer-bidder --test schema_source_conformance --features insecure-bidders schema_selection -- --test-threads=2
```

Each lane passes 34 controls, including exact current membership, independent
root/schema/helper omissions, reordering, feature/ancestry and malformed-input
refusals. These are source/generated checks, **not** linked-artifact acceptance
or resource bounds. The report explicitly sets `linked_or_serving_qualification`
to false. No candidate is promoted and serving validation stays uninstalled.

Final independent dispatch checked both named manifest tests as part of the
documented filter. Strict targeted Clippy, formatting, isolation (16 controls)
and the raw-fixture gate pass. The unchanged full-source literal-prefix control
also passes after sharing the expansion helper. A final rebuild encountered
GCL expiry during the session; the existing maintenance CLI refreshed that
archive separately in `223d16bd`, with 20 configuration/archive checks passing.
All 555 core library tests pass against the refreshed archive. No freshness
override, schema change or historical evidence repinning was used.

| Remaining Acceptance Surface | Required Next Evidence |
| --- | --- |
| Per-adapter artifact and feature closure | Match the executed schema probes below to actual application artifacts and finish native section inspection |
| C/I/V program, initialization and evaluation costs | Reachable source/library/compiler bounds and bounded cold/warm/changed/failed probes; observations alone are insufficient |
| Registry and K/A composition | Nonzero complete descriptors, precharged cached startup, indexes and schema admission on the real handler path |
| Priority promotion | Current producer evidence and composed onboarding for all 14 flows |

Reconcile the five selected regex owners with the locked constructor policy
before extending nested literal/backend proofs. Unselected strategies do not
create new bidder catalog obligations; any owner actually required by the selected cohort
still does. Target readiness and authorized hosted observations retain their
separate resource qualification/publisher pilot owners.

### Selected Schema Link Probes

The next checkpoint links the same 15/16-schema cohorts into standalone native,
`wasm32-unknown-unknown`, `wasm32-wasip1` and `wasm32-wasip2` probes. It shares
the existing primary-validator emitter, exact compiler options, profiles,
patched dependency verification, bounded child runner and artifact parser.
The all-261 compatibility probes remain unchanged in scope. Selected probes
exclude alternate schema-control wrappers and unselected workload data.

```sh
cargo test --locked --offline -p auctioneer-bidder --test schema_source_conformance selected_schema_probe_links_and_executes_the_source_bound_cohort -- --test-threads=1 --nocapture
```

The command runs both schema-owner lanes. These are **schema probes, not the
four application artifacts** or proof that bidder feature flags are enabled in
those applications. Reports explicitly deny application/resource qualification.
They bind full source/selection witnesses by a hashed membership marker,
verify that marker in every artifact, pin the actual compiler executable,
check native dispatch membership and record resolved regex features, generated
and auxiliary Rust-source hashes, child manifest/lock identities, artifact
digests and WASM code/data/memory declarations. Schemas are copied into the
private child workspace only after bounded byte/digest checks, and compilation
reads those staged copies, not a mutable source tree.

Both cohorts execute cold and repeated initialization, every schema's null
refusal, and the selected subset of the pinned external parameter tables.
The latter executes 236 cases across eleven families per cohort: 79 accepts,
146 schema refusals and eleven JSON refusals. Every family is checked against
its independently pinned verdict counts; all syntactically valid cases also
run in ordered mixed-input sequences. No expected outcome is recomputed from
the candidate validator. Four secure-lane schemas lack cases in this particular
capture; the development lane adds one more. The report names them rather than
claiming complete parameter semantics. Their owner-local controls and remaining
branch coverage retain their bidder catalog obligation.

Artifact bytes and static declarations include the CLI, JSON parser and
observation runtime. They are observations, not retained-program or runtime
heap bounds. Native section sizes remain unmeasured. At this initial link
checkpoint, guest execution was unclaimed: the available local Wasmtime was
45.0.0, not the repository's pinned 48.0.2. The later
[WASI execution checkpoint](#selected-wasi-command-execution) addresses two
command targets without rewriting that earlier evidence. Linking/validating
a guest binary is not running it. Application
membership, resource ownership, funded serving integration and priority
promotion remain open; no schema, dependency, serving limit or capture changed.

Final verification: 193 synthetic harness controls, 34 source-selection controls
in each feature lane, the two-cohort/four-target link-and-execution control and
16 source-import isolation controls pass, with no failures or ignored tests.
Strict targeted Clippy passes in both feature lanes; formatting and the
188-fixture/54-file raw-JSON gate pass. The initial sandboxed broad run passed
191 controls and failed two existing loopback-only reference controls at socket
creation; the complete permitted rerun passes all 193 without test changes.
The complete 261-schema source-conformance suite was not rerun in this round.

### Selected Regex Retained Heap

The next native candidate composes the five selected regex helpers, not all
schema state. It keeps six requested-heap terms separate: immutable metadata,
the strategy Arc, shared forward/reverse NFA trees, a conservative one-pass
table envelope, the automatic prefilter matcher and unused pool bookkeeping.
Metadata already owns the two root Arcs; engine handles share their NFA and
prefilter allocations. Static inline storage is reported separately.

Four helpers are start-anchored, excluding automatic literal prefilters. The
remaining helper has an end anchor and one extracted one-byte prefix. The
pinned constructor chooses the scalar matcher first; reverse/fallback paths
reuse it rather than constructing nested literal matchers. The existing public
`Prefilter::from_hir_prefix` and `resource_layout` APIs supply a same-type
layout witness. This is not a query of hidden allocation identity. Pinned
source establishes one matcher Arc despite its cloned handles. Other pattern
classes refuse this model rather than becoming a zero-byte success. No new
vendor patch, matcher, schema, feature or runtime limit is introduced.

On Rust 1.98.1, `aarch64-apple-darwin`, the reconstructed five-owner candidate
totals 100,792 requested retained heap bytes, with 120 static inline bytes
separate. The release probe's constructor-only observations retain 47,944
bytes in each cohort. This is corroboration, not a peak, CPU or physical-memory
bound; subtracting those values does not derive initialization scratch.
The new constructor-only command invokes the existing cold/repeat initializer
measurement without cache construction or validation searches. Per-schema and
shared-format observations require successful initialization, nonzero work for
funded owners, tight retained-byte checks and zero-allocation repeats.

The report binds native build/library identities and derivation sources,
explicitly excludes non-regex schema state, and keeps
`application_or_resource_qualification = false`. Test-binary layouts do not
establish release-child or guest layout correspondence. Code/data residency,
remaining schema state, constructor work/peak, search/cache work and the actual
serving-profile/target proofs remain open. No descriptor is installed by this
candidate. The existing full-catalog controls and frozen evidence stay intact.

Verification passes 82 component/observation controls, the 20 program/literal
controls again with `insecure-bidders`, both selected cohorts on all four
compiler targets, all 193 synthetic harness controls and 16 isolation controls.
Strict Clippy passes in both feature lanes, along with formatting and the
unchanged 188-fixture/54-file raw-JSON gate. The scalar and composition stubs
first failed three and eight controls respectively; the real probe then failed
on its missing constructor-only command before the route was added. Separate
spec-compliance and code-quality reviews found no actionable issues within this
checkpoint's stated premises. The complete 261-schema suite was not rerun.

## Selected Validation-Cache Heap

The selected probe reuses the existing serial `MetaCacheHeap` derivation and
`CacheBundleHeap` composition rather than adding another engine or private
layout patch. Each generated wrapper funds every sibling pattern's full
lifecycle peak, its compiler-reported inline slots and its required format
callback storage. Equal patterns in different generated owners are not
deduplicated. The shared hostname program remains a separate retained owner;
its callback constructs and drops a fresh cache per invocation. UUID's fixed
decode buffer still belongs to the separate call-frame obligation.

The native probe now embeds only the selected families' parseable captured
parameter cases, retaining original input strings, order and expected booleans.
Existing selected bidder-owned workloads follow with distinct `authored_`
identities and unchanged expectations/transforms. They exercise the regex owner
without an external parameter table and retain exact byte/node-limit controls;
they are authored controls, not newly claimed reference captures. The common
bounded workload loader is reused before schema staging, so the private staged
schema tree need not contain another copy of owner test assets.
The existing raw-input consumer separately exercises malformed JSON. Empty
objects remain construction/search controls, not a substitute for the capture.
Generated workload data is native-only and changes the source/artifact digest;
it does not add guest validators, alternate wrappers or unselected workloads.

For each schema, populations of one, two and four independent cache owners
execute cold, repeated and changed-input phases. The consumer carries retained
bytes between phases and adds each phase's incremental peak to its prior live
storage. Every expected input and verdict is checked on every owner, and each
reverse-order release must return that owner's live storage to zero. Separate
hostname cases exercise valid, invalid and over-limit inputs. Tight synthetic
controls independently vary prior storage, transient growth, retained changes,
owner/input counts, verdicts and release order; a zero-iteration check cannot
stand in for exercising a populated history.

The report is explicitly a
`conditional_native_schema_cache_heap_candidate`, with
`application_or_resource_qualification = false`. Its native model assumes a
fixed program/cache, serial earliest searches and bounded full haystacks;
reset, cloning and rebinding are outside its contract. Optional-prefix and
fallback storage remain conservative existing supersets. Actual wrapper
layouts are reported by the child compiler, but parent test-profile engine
layouts are not thereby proved equal to the release child's or guests'. The
probe's enum/container storage is excluded from allocation windows and
reported separately, not installed as the production owner representation.

These observations do not prove CPU/work limits, complete call frames,
constructor peak/work, code/data residency, other schema state, allocator/RSS
behavior or target/application fit. Those remain bidder catalog obligations where required
by the shipped set. No measured peak becomes a production descriptor, and no
serving schema or limit changes in this checkpoint.

Captured and authored populations are counted separately in the report, as are
the probe's enum storage and observed requested heap. Each cohort has four
generated regex helpers and one hostname-using schema. Five standalone hostname
inputs are each invoked twice. Guest artifacts compile and retain their
membership checks, but guest execution remains unclaimed.

The Rust 1.98.1 / aarch64-apple-darwin probe runs 225 parseable captured cases
and 22 existing authored cases per cohort, plus each schema's empty control.
Populations of one, two and four owners produce 3,668 secure-lane and 3,682
development-lane schema-validation calls, separately from the ten hostname
callback calls. The maximum conditional bundle storage candidate stays at
316,705 bytes. Maximum observed requested heap is 46,026 bytes; it excludes
inline/container storage and is not a tighter bound. The probe's 4,200-byte
enum remains separately reported, not silently charged as an actual serving
representation.

Verification includes 207 shared synthetic harness controls and 75 focused
controls in each feature lane. The workload merge/loader stubs first failed
eight controls; the earlier capture/phase/population stubs failed their own
controls, and the real probe first rejected its missing cache report. The final
helper extraction retains the same owner/input/verdict, maxima and release
checks without adding lint exemptions. Strict Clippy passes in both lanes;
16 isolation controls, formatting and the unchanged 188-fixture/54-file
raw-JSON gate pass as well.
Separate source/spec and quality reviews found no actionable issue within the
stated conditional scope. The full 261-schema source suite and serving
application qualification were not rerun or claimed by this checkpoint.

### Five-Helper Half-Search Certificate

The separate [half-search certificate](../../crates/auctioneer-bidder/tests/schema_selected_half_search/mod.rs)
checks the source-bound selected owners in both secure and development cohorts:
OpenX helpers 0/2, Rubicon helper 0 and shared-hostname use Core forward
half-search; OpenX helper 1 uses ReverseAnchored. Under the unchanged pinned
configuration (hybrid available, full DFA unavailable), mandatory HIR Start/End
properties, strategy ordering and both successful primary hybrid builders
establish these routes. ReverseAnchored internally sets
`Anchored::Yes` for the ordinary input. Nonempty NFAs and the complete
non-quitting capacity/ID proof exclude UTF-8 empty-match retries, clears and
fallback, not eager allocation.

The lifetime premise is one fixed cache, serial whole UTF-8 haystacks through
`Input::new(...).earliest(true)`, then drop; no reset, clone or rebinding.
For positive finite caller-supplied call count `N` and byte ceiling `B`, checked
arithmetic reserves cumulative byte progress `N * B` and position arithmetic
`B + 1`. EOI transitions do not add a byte-progress charge. This is an explicit
caller premise, **not a production-enforced cache lifetime or CPU-work bound**.
Generated-owner controls use `B = 65_536`; hostname uses a fresh one-call cache
with `B = 255` for each callback observation.

The native requested-heap composition is `eager + complete_active_peak +
fresh_inactive_peak`. It preserves capture/possible one-pass storage and the
inactive direction's allocations. The immutable scalar prefilter remains
separately C-owned. Equal patterns in different generated owners remain
distinct; shared-hostname is counted once by shared owner identity, not by
callback iterations. The existing broader `MetaCacheHeap` derivation and its
published bundle totals are unchanged.

Central RED verified six expected missing-certificate failures without compile
errors; central GREEN then verified **6 passed, 0 failed, 0 ignored**. Controls
cover finite/missing/zero lifetime budgets, endpoint and cumulative overflow
(including synthetic 32-bit counter arithmetic), strategy/empty-match and
quit/incomplete-graph refusals, component correspondence and actual selected
owner changed-input replay in both cohorts:

```sh
cargo test --locked --offline -p auctioneer-bidder --test schema_source_conformance selected_half_search -- --test-threads=1 --nocapture
```

This is a conditional native component certificate, not completed I/V or
serving qualification, guest-layout correspondence, complete frames or physical
memory accounting. Constructor scratch/work and failure cleanup remain open.
In particular, Rubicon's schema spells `\d`, which is Unicode under default Rust
regex settings; the converter translates patterns before emitting helpers.
The I audit must bind the actual emitted pattern and lowered class before
excluding any reachable Unicode constructor path. This checkpoint supplies no
ASCII-only constructor certificate.

## Selected WASI Command Execution

The selected source-conformance producer now requires the Wasmtime version in
`.tool-versions` before building its cohorts. A missing runner or a different
version refuses the test; there is no ignored or zero-work execution pass.
The invoked program's canonical path, byte count, SHA-256, version and host
identity are reported. Its identity is rechecked around each cohort's guest
observations. This is an invoked-program witness on a trusted test host, not
attestation of a version-manager launcher's transitive executable image.

Both existing WASI command artifacts execute their ordinary `main` entrypoint,
not Rust-reference exports through `--invoke`. The runner checks the linked
artifact's exact size/hash before and after every call. Inventory names,
empty-object verdicts and encoded membership must match the linked cohort;
generated regex initializer hooks must succeed on both passes. Initializer
results exclude custom format initialization and search-cache construction.

The same captured-case driver is used for native and WASI execution, preserving
each original raw input, independent expected verdict, per-family count and
parseable sequence. To avoid a fresh VM compilation per raw row, a family can
be sent as a table of its original strings. The guest parses every string
independently; malformed JSON produces its own refusal without dropping later
siblings or being normalized into a parsed sequence value. Table length, order
and every verdict remain checked. An encoded table over the existing 64-KiB
argument cap falls back to scalar calls for every row, retaining the original
per-input cap instead of truncating or shrinking the workload. A trace-driven
fake runner independently proves batching, complete scalar fallback and
count-mismatch refusal. Each table/sequence reuses one generated cache owner;
separate process invocations do not share initialization or caches.
Source-only and uncovered cases retain their existing explicit status, not
invented reference expectations. The common wire verdict type is now named
`RawVerdict` rather than suggesting that its JSON shape belongs only to native
execution.

Every invocation uses the existing ten-second process deadline, four-MiB
per-stream output cap and process-group cleanup. An explicit empty configuration
and cleared host environment exclude ambient Wasmtime settings; compiled-code
caching is disabled. There are no directory preopens or debugger flags, and
guest environment inheritance, stdin inheritance, network inheritance, DNS,
TCP and UDP are explicitly disabled. Standard WASI command services such as
stdio, clocks and randomness remain available. Neither a guest memory ceiling
nor fuel limit is configured or claimed by this observation.

The nested report is versioned `local_wasi_execution_observation` with
`application_or_resource_qualification = false`. Its artifact rows retain the
link-time execution status separately from subsequent observations. The
`wasm32-unknown-unknown` artifact was retention-only at this checkpoint: its
Rust-reference exports are not a supported validation invocation ABI. The
[integer-ABI checkpoint](#selected-integer-abi-guest-execution) below adds a
separate safe invocation path without calling those reference exports.
Native/release/guest layout correspondence, initialization/work bounds,
descriptors, serving admission and provider execution remain separate gaps.

Local execution uses the official [Wasmtime 48.0.2 release](https://github.com/bytecodealliance/wasmtime/releases/tag/v48.0.2)
from temporary storage; the existing global 45.0.0 installation is unchanged.
The aarch64-macos archive digest checked before extraction is
`8abc55958f04678bf01d4d0c46868f45aa8083bacc29e5a1685f971f110fbc14`.
CI's x86_64-linux archive is separately pinned to
`f2b0ad1ce9253f2f9a38793c2c42cd1cba4e90b27dc40d685eaf723dc8438d94`.
Its installer reads the version from `.tool-versions`, verifies that checksum,
and must precede every existing schema-build consumer. Workflow mutation
controls reject missing, relocated, skipped or weakened prerequisite commands.
Hosted CI execution is not claimed by checking those workflow definitions.

Invoked-program hashing has a separate 128-MiB file-reading cap: the verified
Linux executable is 70,741,896 bytes and therefore cannot use the 64-MiB WASM
artifact cap. Hashing streams through an 8-KiB buffer; the file cap is not a
guest memory allowance. A sparse-file control at the published executable size
first failed under the coupled cap, and an independent one-over control retains
the new cap's refusal. Guest limits and the accounting equation do not change.

Both secure/development cohorts pass actual command execution on WASIp1 and
WASIp2. Each target checks 236 original raw inputs across eleven families
(79 accepts, 146 schema refusals and eleven JSON refusals), then all 225
parseable cases in owner-reused sequences. Inventory and generated initializer
checks pass for all 15/16 schema owners. The initial scalar run took 605.63
seconds; the raw-table replay took 192.88 seconds with unchanged cases and
expected outcomes. These are diagnostic host wall times, not benchmark gates
or production latency guarantees.

Thirteen runner controls and all 120 workflow controls pass. Missing runner,
file drift, failing guest despite native success, wrong result counts/types,
UTF-8 byte limits, large Linux program hashing and complete scalar fallback
have explicit controls. No new ignore, lint exemption, dependency, engine,
schema, guest profile or production resource limit is introduced.

The full shared harness passes 220 controls, and all 16 source-import isolation
controls pass. Strict targeted Clippy passes for the bidder harness in both
feature lanes and for the workflow controls. These checks are not the complete
261-schema source suite or actual adapter-application acceptance; those broader
claims remain open with their existing capability owners.

## Selected Inline Cache Layouts

The same explicit-cache probe now exposes its existing `cache-layouts` command
on both WASI targets, instead of limiting it to native execution. The strict
version-2 DTO adds the compiler's pointer-word size and retains the actual
`meta::Cache` size/alignment and each generated wrapper's size/alignment.
The enclosing local-WASI observation is also version 2. Earlier reports are
not reinterpreted or rewritten.

The consumer checks ordered schema identity and exact row counts against the
selected binding, and wrapper multiplicity against independently expanded
helper counts. Empty wrappers must have zero bytes and alignment one;
nonempty wrappers must have exactly one compiler-sized cache slot per helper.
Native release results must match the parent compiler's base cache layout.
WASI results must declare four-byte words; their cache and wrapper sizes are
read from the guest, not copied from a native table.

This closes only compiler-reported inline cache/wrapper correspondence for
those probes. It does not establish private nested layouts, guest requested
heap bounds, constructor/search work, complete frames, allocator or physical
memory, or actual adapter-application correspondence. In particular the
native heap candidates are not installed or relabeled as guest bounds.
The unknown-unknown target was still retention-only at this checkpoint; its
later integer-ABI execution is recorded separately below.

Both selected feature cohorts pass the complete replay with this query:
`meta::Cache` is 1,400 bytes with eight-byte alignment in the native release
probe, versus 752 bytes with eight-byte alignment in each WASI probe. Pointer
words are eight and four bytes respectively. Every 15/16-schema wrapper row
matches its own target's cache size and independently expanded helper count.
The matrix also reruns all 236 raw captures and 225 parseable sequence cases
per guest/cohort. These sizes are compiler layout observations, not measured
heap peaks or complete guest resource bounds.

Verification passes all 224 shared compiler/protocol controls, 16 import-isolation
controls and both selected cohorts on all four build targets, with actual WASI
execution as above. Four new layout controls independently vary target words,
helper counts, identities, field shapes, sizes, alignments and arithmetic
overflow. The correspondence stub first failed three controls; the guest query
also first failed its command-admission control. The extended synthetic runner
checks a successful typed layout query and malformed-result refusal. Strict
targeted Clippy passes in secure and development-feature lanes without new
exemptions; formatting and the unchanged raw-JSON gate pass. Full 261-schema
source execution and actual application qualification are not claimed here.
Focused development-feature reruns also pass all four layout controls and
thirteen runner controls, including the added typed-query cases.

## Selected Integer-ABI Guest Execution

Both selected cohorts now execute on `wasm32-unknown-unknown` as well as native
and WASIp1/P2. Seven fixed C exports take only integer words and return one
integer word; the host never calls the old Rust-reference retention exports or
passes pointers into guest memory. The ABI has an explicit version and strict
schema, case-count, packet-padding and refusal checks. Unsigned argument bits
are represented as signed decimal for Wasmtime's i32 command interface.

The original captured input strings are compiled into the probe, including
malformed JSON and duplicate-key spelling. Their exact bytes must occur in a
linked data section of the identified module, not just an unrelated custom
section. Expected verdicts are not compiled into the guest. Sixteen-row packed
responses preserve every schema/JSON classification and its position. Fresh
mode constructs a cache per parseable row; reused mode replays the complete
original prefix with one cache for each batch. It does not claim cache
persistence across separate Wasmtime processes.

The unchanged `jsonschema` runtime dependency retains three unavailable
binding imports in this standalone module. The probe's import check admits
only `__wbindgen_describe`, `__wbindgen_externref_table_grow` and
`__wbindgen_externref_table_set_null` under their exact binding namespaces,
rejecting duplicates and nonfunction imports. Bare invocation uses Wasmtime's
trapping-only unknown-import policy, never default return values. A valid
control module succeeds with its import unused; the same module calling it
must produce an execution trap. These constrained observations do not establish
Cloudflare handler execution or the behavior of those imported services.

The source-bound matrix passes both 15/16-schema cohorts across all four build
targets. Every guest/cohort preserves the selected 236 raw captures and 225
parseable sequence results; schemas without captured cases remain explicit.
Initialization is checked twice per schema in one invocation. The unknown
guest reports four-byte words and a 752-byte, eight-byte-aligned base cache;
its wrappers match independently expanded helper counts. These are observed
compiler layouts and executed behaviors, not heap/work or provider bounds.

The new integer-ABI observation is version 1. The enclosing WASI observation
is version 3, removing its obsolete unknown-target pending field in favor of
the separate result. Historical observations are not rewritten. All 243 shared
compiler/protocol controls pass: 241 in the sandbox and two offline-reference
controls in a loopback-enabled rerun after the sandbox refused their listener.
Independent review found no remaining correctness issue in this boundary.

Actual application correspondence, remaining required semantics, native code
sections, constructor/evaluation bounds, nonzero C/I/V descriptors, funded
registry/schema admission and priority onboarding remain bidder catalog work. No dependency,
compiler patch, engine, schema, workload limit or serving binding changed in
this probe increment.

## Selected Owner Semantic Execution

Every selected owner now supplies `tests/schema/semantics.toml` beside its own
bidder implementation and parameter schema. Strict, bounded loading checks
exact shapes, explicit verdicts, namespaced case identities and schema/data
digests. Authored rows are schema-derived controls, not external captures. The
14 imported expectations keep their independent source revision, blob, offset
and capture digest. No expected outcome is recomputed by Auctioneer.

The secure batch contains 139 authored plus 14 imported rows: 57 accepts and
96 rejections. Development adds eight authored rows: 61 accepts and 100
rejections across 161 rows. All selected owners have cases, including owners
without the earlier source-table captures. Controls exercise required and
alternative branches, nested element types, minima, formats and additional
properties without changing schemas or their matcher.

Both batches pass native, WASIp1, WASIp2 and unknown-unknown execution with fresh
and reused caches. Each target compares to the same fixed batch and exact row
counts. The bare guest uses separate integer count/replay exports and an
input-only table whose exact bytes must occur in linked module data. Expected
booleans are never compiled into that table. Its existing 236 captured inputs,
exports and provenance remain separate and unchanged. Reused bare batches
replay their original prefix, rather than claiming cache persistence between
separate runtime invocations.

The consumer rechecks source-bound inputs after child builds. Negative controls
reject missing/extra results, wrong verdicts or lexical classes, lost owner rows
and mismatched reported counts. These are behavioral checks, not exhaustive
schema equivalence, resource bounds, provider authorization or serving admission.

## Actual Application And Layout Inspection

The source-conformance acceptance target builds the real Axum, Cloudflare,
Fastly and Spin applications in both selected feature lanes. Native object and
WASM parsers reconcile every selected owner with mapped descriptor data and
actual validator/cache/initializer references. Decoy strings, unrelated custom
sections and imported or missing function references fail the controls. Spin's
component is inspected through its nested core modules. All eight builds and
inspections pass locally.

The rows use a fixed `repr(C)` layout with compiler assertions: 160 bytes on
32-bit guests and 176 bytes on the native target. Pointer fields are read with
the artifact's word width and resolved through its own symbols/relocations or
WASM tables. The parsers are the workspace-managed `object`, `gimli` and
`wasmparser` libraries; synthetic WASM controls use the matching encoder.
Inspection dependencies are native-only test dependencies.

These applications remain explicitly unqualified and refuse before ordinary
serving initialization. Their retained code/data is evidence of staged linkage,
not a measurement of the eventual admitted application or its residency delta.
Final shipping-artifact correspondence remains mandatory after funded admission
is implemented.

The companion layout target reuses the normal application's dependency receipts
and verifies exact compiler, source, feature and extern identities before a
static DWARF query. It covers eight syntax layouts and six automata layouts on
native and the three guest targets. Guest relocations are resolved by the pinned
linker; no guest execution, debugger or handwritten private-layout guess is used.
All eight package/target queries pass. The actual syntax graph includes
`default` alongside its nine concrete feature flags; the automata graph retains
its existing 25 flags. No new regex backend or serving feature is enabled.

Reports deliberately remain `Unqualified`, with
`application_artifact_correspondence = not_established`. Private inline slots
close only one premise of the conditional construction models. Complete
initialization/search work, machine-stack frames, target heap composition,
nonzero C/I/V descriptors and prepaid registry/schema admission remain bidder catalog work.

## Non-Serving Application Staging

The internal Cargo feature `auctioneer-bidder/schema-artifact-staging` generates the
selected schema union through existing discovery and the compile-time converter.
It is the sole staging switch: dependency selection and mandatory unqualified
mode cannot disagree. It adds no runtime setting, operator option or publication.
Normal application dependency graphs exclude the unqualified runtime; staging
must not enable its transitive JSON-number parser features in normal builds.
The same selector is shared with source-conformance controls; declarations are
validated before feature filtering, and the emitted private table is exactly
the selected effective schema owners. Required formats keep the candidate's
existing engines, options and explicit cache ownership.

The compiler reads content-addressed snapshots of the verified schema bytes
under its own `OUT_DIR`, not reopened source paths. Every snapshot is also a
build-script input: an incremental edit reruns identity validation and conflicting
bytes refuse rather than compiling beneath a stale sidecar. Source paths remain
watched. The sidecar records source, generator, patched dependency, lockfile,
compiler, target, profile, feature and build-environment identities. It explicitly
labels linked membership, imports, resources and serving integration as pending.
Generated membership is not proof that a linker retained it.

The private retention root holds function pointers only. It must not initialize
patterns, construct caches or call a validator. Staged builds refuse public
registry construction, App configuration and resolved boot/resource inputs with
a typed unqualified error before installing gates. Normal builds emit no staged
table or sidecar; `Normal` describes that build mode, not qualification.
The CLI preserves this cause as `unqualified_schema_artifact`; an actual staged
resource-export invocation exits 2 with empty stdout rather than emitting a
profile. Pure arithmetic evaluation of separately supplied historical evidence
is not activation of the staged application.

Paired checks must supply an independent runtime expectation rather than derive
the expected answer from the build flag:

```sh
env -u AUCTIONEER__TEST__EXPECT_SCHEMA_ARTIFACT cargo test --locked -p auctioneer-core --lib schema_artifact_
AUCTIONEER__TEST__EXPECT_SCHEMA_ARTIFACT=unqualified cargo test --locked -p auctioneer-core --features auctioneer-bidder/schema-artifact-staging --lib schema_artifact_
AUCTIONEER__TEST__EXPECT_SCHEMA_ARTIFACT=unqualified cargo test --locked -p auctioneer-bidder --features schema-artifact-staging --test schema_source_conformance tests::staged_
```

Build the CLI together with the bidder package to select the inspection feature,
then invoke that binary directly:

```sh
cargo build --locked -p auctioneer-cli -p auctioneer-bidder --features schema-artifact-staging
target/debug/auctioneer-cli bidders resources --check --all-adapters
```

The second command must exit 2, emit no stdout and report only
`unqualified_schema_artifact` on stderr. Confirm the bidder artifact's enabled
features in Cargo's `--message-format=json` output when auditing this build;
running a previously built normal CLI does not exercise the staged refusal.

Repeat both modes with `insecure-bidders` added to their feature selection; normal source-conformance
checks also require absent staging metadata. The real-input snapshot control
checks all selected snapshot watch paths and their mutation refusal. This
staging implementation creates inspectable inputs and retains the fail-closed
boundary. It is not a resource descriptor, linked-artifact report, final shipping
artifact or permission to activate the selected bidders. All four actual
application/package correspondence and funded admission gates remain required.

The paired admission checks pass four tests per mode/feature combination; the
exact metadata filter passes two per combination. Build-support and workflow
gates retain explicit result-count checks. Staged Cloudflare, Fastly and
Spin adapter library checks compile on their actual guest targets. These are
compilation checks, not application linkage, packaging, deployed execution or
resource-bound observations. The generated module has one local lint
expectation for the converter's mandatory absolute custom-format paths; no
workspace lint policy is widened.

## Current Test Ownership

Shared `schema_*` integration targets now use independent synthetic schemas and
metadata under `tests/support/fake_bidder/`. They test compiler and accounting
mechanisms without using production bidders as examples.

`schema_source_conformance` separately runs the complete pinned production
inventory, actual generated validators, source captures and resource controls.
Bidder-specific cases are discovered from each owner's `tests/schema/`.
Run both scopes:

```sh
cargo test --locked -p auctioneer-bidder --test schema_build_controls --test schema_source_conformance
```

Historical commands and observations below retain their original scope and
counts; rerunning a former shared-target command alone no longer establishes
complete production-source coverage. The [isolation plan](../superpowers/plans/2026-10-25-bidder-test-isolation.md)
records the cutover and its verification. Neither scope qualifies serving
validation or target memory fit.

## Candidate Identity

`jsonschema = "=0.58.6"`, with default features disabled and only `macros`
enabled. No HTTP, file-resolution, TLS, async-resolution or arbitrary-precision
feature is enabled on that dependency. This does not mean every transitive
package avoids arbitrary-precision types; the allocation proof is still pending.

The reviewed published-package checksums are listed below. All except
`jsonschema-macros-core` remain registry entries in Cargo.lock; that compiler
now resolves through the checksum-verified local override described below.

| Package                | Version | SHA-256                                                          |
| ---------------------- | ------- | ---------------------------------------------------------------- |
| jsonschema             | 0.58.6  | f622c574a29fb3034a0a90645afee194dfedc75fc124228801b6e77041075bd1 |
| jsonschema-macros      | 0.58.6  | 5553b59276ad81f2f49c80cd381a6d6d873e74139cdeb2bef0f9eb28a2ded2b6 |
| jsonschema-macros-core | 0.58.6  | bf3ebe067f2c25a981a7d1b2f696fc213e10dbf80ba5fe997f7dc2e4fe261f26 |
| jsonschema-regex       | 0.58.6  | a968fffd387b2ab005bf00c1851f5d26e90df05b6f7ad609ed0d620033cca5e7 |
| jsonschema-value       | 0.58.6  | 39b7670345e5ce87371339d13705dbb9276b7e3e934bbe1efa2c6734a1f65a96 |
| referencing            | 0.58.6  | 558f3855529c0a7e78a48e409fa653456e55f1b167eab4109c16a255b7727e3f |

The probes additionally use maintained `regex = "=1.12.4"` and
`uuid-simd = "=0.8.0"`, with the latter's UUID integration feature disabled.
No hand-written schema compiler or parameter validator was introduced.

Resolution requires two existing shared lockfile upgrades: `regex-automata`
0.4.14 to 0.4.18 (`fancy-regex` now requires at least 0.4.16), and `zmij`
1.0.21 to 1.0.23 (required by `jsonschema-value`). These changes affect shared
dependency resolution even though the candidate itself is test-only. Resource
and behavioral regressions must therefore be checked separately from the new
compiler controls. These dependencies are not added to production guest builds;
the temporary linked harness has its own explicit cross-target dependency graph.

## Structural Inventory

The audit reconciles all raw schema files against both committed source
witnesses and the reference import manifest, including the literal source
revision `d7f8515b86258688304b0d9b6668c6a0e258bc9e`.

The captured inputs and their witness files are in
[static catalog witnesses](../../crates/auctioneer-bidder/tests/conformance/fixtures/reference/catalog/),
shared by the native CLI import checks and schema controls. They are not
runtime metadata or promoted schemas. Reports remain here. The
[approved asset migration](../superpowers/plans/2026-10-23-bidder-owned-assets.md)
moves each schema and its TOML provenance into the owning bidder folder, with
explicit associations for multiple public schemas sharing an implementation.
That cutover and the qualification reader update are implemented. Relocation
is not serving-validation qualification or promotion.

- Exactly 261 schemas, 164,518 raw bytes; the maximum is 3,778 bytes at
  `appnexus.json`, blob `37909b12066ad9239b488b9751d070212e90d61a`.
- 260 schemas declare Draft 4. `ogury.json` declares Draft 2020-12 and uses
  conditional requirements. Applying Draft 4 to every schema loses those rules.
- Actual format keywords name only `hostname` and `uuid`. A property named
  `format` is not itself a format-validation keyword.
- No reference or unknown keyword occurs at a schema position in this snapshot.
  Annotation payloads, defaults and instance examples are not schema positions.

The audit is data inspection, not instance validation. Its defensive controls
also cover local pointer references and cycles. Anchors, nested URI scopes and
dynamic references remain explicitly unresolved, not treated as supported or
retrieved. A future snapshot introducing them needs additional qualification.

## Native Compatibility Findings

The production-source conformance controls use actual committed schema paths, explicit draft
selection, the linear regex engine, 64-KiB regex and DFA limits, and the
generated boolean `is_valid` method. Separate synthetic type-dispatch controls
additionally exercise `validate` and `iter_errors`.

Two controls first failed with the default candidate behavior:

1. Under Draft 4, `validate_formats = true` alone leaves `uuid` unrecognized and
   ignored. An invalid UUID was accepted. The test-only format hook uses
   maintained UUID parsing plus the reference's lowercase, hyphenated policy;
   it rejects simple, braced, URN, uppercase and malformed representations.
2. The built-in hostname checker caps names at 253 bytes, while the pinned
   reference format accepts up to 255 with 63-byte labels. The test-only hook
   applies that reference constraint using the maintained regex engine, with
   independent total-length and label-length controls.

These constraints come from the pinned reference's dependency on validator
version 1.2.0 and its
[format policy](https://github.com/xeipuuv/gojsonschema/blob/v1.2.0/format_checkers.go).
They are compatibility requirements, not claims that the reference defines the
latest IAB standard. The hooks are qualification proposals, not production
policy approval or a measured resource bound.

Positive and negative controls also exercise missing versus null, integer/string
alternatives, oneOf/anyOf, required nested fields, unanchored patterns, ASCII
versus non-ASCII digits, explicit format disablement, and Ogury's newer draft.

### Disqualifying Type Bypasses

The unpatched published callbacks accepted both of these invalid instances:

| Schema            | Instance                                    | Required Result                           | Observed Result |
| ----------------- | ------------------------------------------- | ----------------------------------------- | --------------- |
| `yandex.json`     | `{"page_id":"x","imp_id":"x"}`              | Reject: both properties declare `integer` | Accept          |
| `consumable.json` | `{"networkId":"123","siteId":1,"unitId":1}` | Reject: `networkId` declares `number`     | Accept          |

These expectations follow the captured schemas and Draft 4's
[type validation rule](https://json-schema.org/draft-04/draft-fge-json-schema-validation-00#rfc.section.5.5.2).
A string-only constraint such as `minLength` or `pattern` does not add `string`
to the explicitly allowed numeric types. The maintained runtime validator,
using the same raw schema and explicit draft, rejects both instances. That
differential is an additional control, not an external reference capture or
permission to substitute runtime schema compilation.

Source inspection locates the disagreement in
`jsonschema-macros-core 0.58.6/src/codegen/dispatch.rs::compile_typed`: the
string arm is emitted when a string keyword exists even if the declared
type excludes strings. `object_schema.rs::compile_object_schema` then
suppresses a separate type check because typed dispatch was emitted.

The initial required-rejection test failed on the actual Yandex callback.
The published-candidate disagreement is historical evidence, not accepted
validation policy. The patched-candidate control
`generated_checks_enforce_declared_numeric_types_with_string_keywords` now
requires both invalid instances to be rejected, retaining numeric positive
controls and the maintained runtime validator differential.

Do not change the captured schemas, relax their types, add a handwritten
validator/type-prefilter, drop the affected bidders, or silently switch to
startup compilation. Follow section 7.1's recovery path: a reviewed maintained
generator alternative or bounded upstream macro fix, followed by complete
compatibility and resource qualification. Independent foundations may proceed;
request-time schema integration and Tasks 12/17 acceptance remain held.

### Approved Compiler Patch

AG approved the bounded compiler repair on 2026-10-19. The root
`[patch.crates-io]` points to `vendor/jsonschema-macros-core`, excluded from the
workspace. The original package archive is retained with the checksum above.
All 280 original members are preserved. That repair changes
`src/codegen/object_schema.rs`; the separately approved constructor extension
below also changes `src/codegen/emit_root.rs`. The upstream MIT license is added from source revision
`55ac1664384793c5d3b20745d3f9c169dfe2c06f`.

The repair separates keyword dispatch from explicit type enforcement, using
the maintained type-keyword compiler rather than a Auctioneer type prefilter.
This also preserves integer/boolean/null union checks when inapplicable
string, array or object keywords are present. Literal positive and negative
controls exercise all three generated validation APIs. The patched source
SHA-256 is
`c5fe64170d5154c5967280d28c2fcb269c24d49ef756322d269ac8eb8998d912`.

The portable [patch and upgrade instructions](../../vendor/README.md#upgrades)
are part of the same change. A control applies the patch to fresh published
source and compares the result byte-for-byte. Standalone qualification
manifests explicitly carry the override and verify Cargo's actual resolved
compiler package, source and manifest path; a pathless lock entry alone is
not sufficient. Archive and expanded-source controls reject missing, added,
changed or symlinked compiler inputs before constructing the harness.

The schemas and source captures are unchanged. This repair does not enable
production validation, complete the instance oracle or establish all-path
work/cache bounds. Generated code size and extra type-check work remain part
of the outstanding qualification.

Patch verification on 2026-10-19, Rust 1.98.1:

- `cargo test --locked --offline -j 2 -p auctioneer-bidder --test schema_build_controls -- --test-threads=1 --nocapture`: 45 passed, none failed or ignored. Includes actual patch application, standalone Cargo resolution, four linked artifacts and isolated native allocation probes.
- `cargo test --locked --offline -j 2 -p auctioneer-bidder --lib --test schema_compiler_controls --test schema_source_inventory`: 128 library, 18 compiler/provenance and 24 inventory tests passed.
- `cargo clippy --locked --offline -j 2 -p auctioneer-bidder --all-targets -- -D warnings`: passed. Existing GVL deletion and dependency future-incompatibility warnings remain.

All 261 callbacks remain reachable in each linked qualification artifact:

| Target                 | Artifact Bytes | SHA-256                                                            |
| ---------------------- | -------------- | ------------------------------------------------------------------ |
| Native                 | 2,472,664      | `0bde1d3ae1c39e6393b8c3a2decaa78f9d91d48d119c0bb3e5d31056cf86e822` |
| wasm32-unknown-unknown | 1,521,959      | `9125a5a64d1f1d08eaf3625d3a33bf09d23578ae2075b2404e1a0a4a19e7b267` |
| wasm32-wasip1          | 1,578,043      | `a22e985450cfafe178cbcf6fa1555f038b4e6e79ca8a70b84d9ea0419b2132cb` |
| wasm32-wasip2          | 1,635,402      | `0d6e798a0353a2ee3a1d5ba7a5a866e4102cc8e42ca85df8028dcdde42eea713` |

These are complete probe-artifact sizes, not disjoint resident code/data
bounds. Native sampled first-pass allocation remains 895,247 cumulative,
371,978 peak-live and 166,738 retained requested bytes over 323 inputs and
261 callbacks; repeat requests eight bytes cumulatively and retains none.
This does not cover every lazy cache, concurrent invocation, allocator
metadata or guest memory. Existing challenge thresholds and workloads did
not change.

### Explicit Pattern Constructor Initialization

AG approved this separate compiler extension on 2026-10-21. The additional
portable patch changes only `src/codegen/emit_root.rs`, whose SHA-256 is
`0fe3dfb41dd37dab3fa8fe7613fb20084b91550b100798da2c21adbcd21a6730`.
The original archive, numeric repair, schemas, engines and builder limits are
unchanged. Each generated validator exposes `initialize_regexes() -> bool`:

- Every generated pattern constructor is forced, even after an earlier
  constructor returns an error. No-pattern validators return true.
- Initialization and all validation methods use the same lazy storage.
  Repeat/concurrent calls construct each regex once; errors remain cached
  as the same validation-denying `None` rather than being retried.
- Custom-format hooks, URI-format caches and search-cache growth are not
  covered by this API. It is not a runtime schema compiler or admission gate.

`schema_initialization` executes actual macro-generated code, with observed
runtime builders delegating to the maintained engines. Constructor counters,
literal builder limits, a deliberately failed first constructor and a cold
eight-thread initialization control exercise lifecycle behavior independently
of matching results. An empty object first fails required-field checks without
constructing either pattern. Before the patch, a temporary no-op initializer
fallback produced the intended failing assertion: zero constructors versus two;
that fallback was removed after implementation.
Additional controls cover validation-first reuse, mixed eight-thread
validation/initialization and concurrent initialization with a cached failure.
Changing the initializer to short-circuit fails both engine controls (one
constructor skipped in each); deleting the linear engine's size-limit setter
fails its positive initialization control. Both mutations were restored and
the exact approved compiler checksum rechecked before final verification.

The disposable all-schema harness keeps initialization separate from
empty-object dispatch and existing allocation modes. Its initializer table
pairs every source identity with that identity's generated method. Native
`initialize` executes first and repeated initialization in a fresh process;
the unknown-OS WASM artifact also exports a parameter-dependent initializer,
and independent export-removal controls prevent dead-entry qualification.
WASM builds demonstrate linkage, not executed constructors on those hosts.

These controls close constructor reachability only. Complete source-bound
work/scratch/cache/concurrency and target C/I/V/K qualification, custom-format
ownership and production integration remain open. They do not promote any
bidder, establish physical fit or complete Task 3.

Verification on 2026-10-21 with Rust 1.98.1 and the unchanged SDK/lockfile:

- The affected bidder slice (`--lib`, `schema_build_controls`,
  `schema_compiler_controls`, `schema_initialization`, `schema_source_inventory`)
  passed 283 tests under secure defaults and 294 with `insecure-bidders`.
  Both used `cargo test --offline --locked -j2 -p auctioneer-bidder` and
  `-- --test-threads=1`, with the named target selectors. No test was ignored.
- Both lanes linked native and three WASM artifacts, executed the 261 native
  initializers twice and preserved the 236 captured case outcomes (79 accepts,
  146 schema refusals, 11 JSON refusals). These are not 261-schema independent
  semantic oracles or executed WASM constructor observations.
- The initial restricted run passed the links and instance cases but failed
  when the existing offline-reference test bound its loopback listener
  (`EPERM`). The complete successful runs allowed that local listener; the
  refusal test was neither skipped nor weakened.
- After race controls and lint-only helper extraction, final constructor/
  compiler controls passed 21 secure-default tests; constructor controls
  passed two with `insecure-bidders`. The final source-ownership/provenance
  subset passed 30. Workspace all-target Clippy with
  `insecure-bidders,conformance-suite` and `-D warnings`, formatting and the
  raw-JSON gate (168 fixtures across 50 files) passed. Separate spec and code
  reviews approved this constructor-only scope, not full engine qualification.

The historical artifact/allocation tables below retain their original source
identities. No threshold, capture or production resource descriptor was updated
to turn this constructor increment into qualification.

### Constructor Allocation Observations

The `initialized-catalog` allocation mode executes the existing custom
hostname-format constructor and every generated initializer in a fresh native
process, then repeats both. The UUID hook has no owned heap constructor. The
format pattern, builder limits and length-before-match behavior are unchanged;
its explicit initializer is native probe support, not production integration.
Per-owner reports preserve the exact source schema order and identify all 261
callbacks, including no-pattern initializers. Callback count is not a count of
distinct regexes. Three known allocating pattern owners and the shared format
owner provide positive controls; every repeated initializer must allocate zero.

Constructor measurement wraps only the synchronous initializer call. Result
vectors, instance construction, parsing and report serialization are outside
the windows. The existing independent allocator controls remain mandatory.
The test rederives the aggregate from per-owner rows, retaining signed releases
and chronological prefix residency in peak accounting. It does not derive
transient scratch by subtracting final retention from an earlier peak.

The same process subsequently runs the unchanged 323 sampled inputs (261
empty-object calls and 62 named inputs), with independently expected outcomes.
The report exposes both measured outcomes for every input. Tests compare the
261 empty-object pairs with cold inventory results and all 62 named pairs
with the separate cold workload reports; a repeat-consistency flag alone is
not accepted as equivalence evidence.
The legacy cold and sampled modes still start separately without initialization.

Observed native requested-heap bytes on 2026-10-21, Rust 1.98.1:

| Phase                                         | Gross Allocation | Peak Live | Retained | Allocation Calls |
| --------------------------------------------- | ---------------: | --------: | -------: | ---------------: |
| All generated constructors plus shared format |        1,918,618 |   517,274 |  193,206 |            5,070 |
| Constructor repeat                            |                0 |         0 |        0 |                0 |
| Sampled validation after constructors         |           96,528 |    61,662 |   61,554 |              564 |
| Same-input validation repeat                  |                8 |         8 |        0 |                1 |

These observations fit the necessary, predeclared 12-MiB retained cache/data
and 20-MiB joint cache/data-plus-initialization checks; neither comparison is a
qualified upper bound. In particular, 517,274 minus 193,206 is **not** an
initialization-scratch descriptor. The validation phases exclude preexisting
constructor residency, and show that constructor completion does not imply
cache saturation or allocation-free validation. The separately exercised
fractional-number refusal continues to allocate on repeat.

The TLS meter records single-thread requested heap, not allocator overhead,
executable/static data, CPU/work, other threads or physical memory. Complete
source-bound, target-specific C/I/V/K and concurrency proofs remain open. No
threshold, SDK/compiler pin, production descriptor, workload or bidder status
changes in this increment.

Reproduce the native observations with:

```sh
CARGO_INCREMENTAL=0 cargo test --offline --locked -j2 -p auctioneer-bidder --test schema_build_controls native_generated_callbacks_have_isolated_cold_and_repeat_allocation_reports -- --nocapture
```

Verification of this increment:

- The same five-target bidder slice listed above passed 283 secure-default
  tests and 294 with `insecure-bidders`, with no failures or ignored tests.
  Both lanes retained the four linked artifacts and 236 source-captured cases.
- Removing prefix residency from aggregate peak accounting failed the new
  independent row-total comparison: the mutated report understated 517,274
  bytes as 334,904. Returning true without constructing the shared format
  failed the cold-retention positive control. Both changes were restored;
  an isolated final allocation run passed after restoration and formatting.
- Strict workspace all-target Clippy with `insecure-bidders,conformance-suite`
  and `-D warnings`, workspace formatting, explicit formatting of the probe
  support files, the raw-JSON gate (168 fixtures across 50 files) and diff
  whitespace checks passed. Separate spec and code reviews approved this slice.
- One clean rerun, concurrent with Clippy, hit the unchanged ten-second
  allocator-control child watchdog before the new measurement. Its isolated
  rerun passed. The cause is unconfirmed, not attributed to the compiler or
  treated as proof of parallel-run stability; see the open harness issue.
  No timeout, assertion or limit was relaxed.

## Allocator-Control Watchdog Diagnostics

The one-off control timeout recorded above remains unexplained. Native control
children still have a ten-second watchdog, build children 300 seconds, and
each output pipe a 4-MiB limit. No retries were added. Process-group teardown,
reaping and reader joins still precede a deadline refusal; that refusal now
retains at most the last 4 KiB of stderr, including after lossy UTF-8 decoding.
An output-limit refusal is not accepted as a measurement.

The control emits fixed main-thread phase markers at entry and after each
allocation window: positive allocation, retained allocation, release, outside
work, thread spawn, thread completion and report completion. `thread-spawned`
means spawn returned, not that the worker was scheduled. An empty trace cannot
establish that the child reached its first marker. Markers never echo input or
enter a measured allocation window. The linked test checks their complete
stderr sequence and the existing independent exact allocation controls.

On 2026-10-21, a one-off native Rust replay held the existing catalog-artifact
lease, checked one baseline and 64 fresh `allocations controls` processes,
and independently bounded each child at ten seconds. All 64 returned the exact
control metrics and complete phase trace while a workspace Clippy process
(`--offline --locked -j2 --workspace --all-targets --features
insecure-bidders,conformance-suite -- -D warnings`) remained active before and
after each child. The slowest observed launch-to-exit took 55,931 microseconds;
Clippy completed successfully in about 88 seconds. Process overlap is not a
CPU-pressure measurement or proof of scheduling stability, and this replay
did not reproduce the original timeout. No synchronization defect was found
in the release/acquire handshake. Investigate any recurrence using the retained
phase, not a larger deadline or a retried successful measurement.

This is test-harness diagnostics only. The SDK/toolchain, compiler patch,
allocation assertions, qualification ceilings and production behavior remain
unchanged; the outstanding source-bound work/cache/resource proofs remain open.

Verification of this increment:

- The affected five-target slice (`--lib`, `schema_compiler_controls`,
  `schema_source_inventory`, `schema_build_controls`, `schema_initialization`)
  passed 285 secure-default tests and 296 with `insecure-bidders`, with no
  failures or ignored tests. The final readable phase-trace assertion also
  passed a focused secure-default replay. Both full lanes retained all four
  linked catalogs and the 236 source-captured cases.
- A forced-deadline test failed before stderr retention was implemented;
  the actual linked control failed before its phase trace was implemented.
  Both then passed. Invalid UTF-8 and oversized stderr exercise the final
  diagnostic byte cap and tail preservation; excessive stdout and stderr
  independently retain their output-limit refusals.
- Strict workspace all-target Clippy with
  `insecure-bidders,conformance-suite` and `-D warnings`, workspace formatting,
  explicit probe-helper formatting, the raw-JSON gate (168 fixtures across
  50 files), and diff whitespace checks passed. Separate spec and code reviews
  found no remaining issues in the diagnostic change.

## Schema Work Admission

`auctioneer-bidder/src/schema/mod.rs` provides the scalar charge-before-call boundary.
Composition must share one non-`Clone` budget across the auction's callers.
The guard uses exactly the composition owner's single-evaluation work, auction
work and funded scratch limits. A fallible cost projection can refuse overflow
without entering the validator. The original-clock check precedes overflow,
zero-work, single-work, scratch and aggregate-work checks. A refusal neither
calls the validator nor debits the balance. Positive exact-limit charges are
admitted; work is debited
before invoking the callback and is not refunded for invalid data or a panic.
Scratch is a per-evaluation peak comparison, not cumulative auction work.

`SchemaWorkBudget` does **not** authenticate a qualification record, derive engine
work, measure input, enforce allocations or install a serving-path validator. The
composition/build owners must supply a correct source-bound cost, admitted
input shape, funded limits and original auction clock. They remain required
before Task 12 integration. No qualified engine-cost function is published by
this increment, and the predeclared resource ceilings remain unchanged.

Colocated controls independently vary unequal limits and exercise exact/over
work and scratch, retained invalid/panic charges, overflow, zero work, exhaustion
and expiry. Integration controls use actual generated Appnexus and Ogury
callbacks with **synthetic** costs, literal validity expectations, one shared
balance and invocation counters. They prove admission behavior, not that those
synthetic charges bound either schema. No candidate dependency moves into a
production dependency graph, and no auction call site is enabled.

The accompanying source audit corrects the pool-multiplicity description below:
CPU-sized shards can retain independent high-water cache populations, and
contention can cause new search allocations even after constructors and sample
searches have run. Complete cache/work/scratch and target descriptors remain
open; native finite observations do not substitute for those proofs.

Verification on 2026-10-21 with Rust 1.98.1:

- The five-target bidder slice passed 309 secure-default tests and 320 with
  `insecure-bidders`, without failures or ignored tests. Both retained the four
  linked catalog artifacts and 236 captured-instance verdicts.
- After lint cleanup, the complete library and compiler-control targets passed
  again: 181 + 21 secure-default tests and 192 + 21 feature-enabled tests. The
  cleanup used explicit generics/types and checked counters, removed an
  unnecessarily fallible test helper and strengthened the panic-payload check;
  it added no lint suppressions or weakened assertions.
- Moving the deadline check below cost-overflow handling made the new collision
  control fail with `CostOverflow` instead of `Deadline`. The reviewed order was
  restored before the full runs above.
- Fresh `cargo check --offline --locked -j2 -p auctioneer-bidder --lib --target`
  runs passed for `wasm32-unknown-unknown`, `wasm32-wasip1` and `wasm32-wasip2`
  after cleanup. These compile the guard; they are not guest execution,
  deployment observations or physical-memory qualification.
- Strict workspace all-target Clippy with
  `insecure-bidders,conformance-suite` and `-D warnings`, formatting, the unchanged
  168-fixture/50-file raw-JSON gate and diff whitespace checks passed. Separate
  spec and quality reviews found no in-scope gaps; the quality recheck also
  approved the lint cleanup and the stated qualification boundaries.

## Borrowed Input Measurement

The schema framework now measures a borrowed final `serde_json::Value` through
`measure_schema_input`. Its caller supplies independent inclusive serialized
byte, syntactic node and container-depth limits. There is no defaulting,
clamping, schema selection or serving call site. The approved serving ceilings
remain 65,536 bytes, 4,096 nodes and depth 32; the measurement API refuses a
declared depth above 32 before traversing the value.

Every value, including a container, counts as a node; object keys count as
additional nodes. A scalar root has depth zero and a root container depth one.
The result records exact compact JSON bytes, nodes and maximum container depth,
not the original wire spelling, discarded duplicate keys or allocated tree
capacity. Measurement happens on the final owned-by-caller representation, not
by reparsing ingress or cloning an auction.

The bounded walk borrows child values and uses the existing runtime JSON-size
helper only for individual scalars and keys. Serde remains responsible for
escaping and numeric encodings. Before scanning a string or key, the walker
checks its raw UTF-8 length plus the two quotes against remaining bytes. A
string admitted by that lower bound can still fail after escaping; its scan
is bounded by the supplied byte allowance, not by an arbitrary original
string length. Container punctuation and all counters use checked arithmetic.
No serialization buffer or traversal heap is retained. Fixed errors discard
serializer diagnostics and contain no keys or parameter values.

This is input admission, **not** a qualified schema cost function, a runtime
allocator limit or proof of stack size on each ABI. The caller still owns the
already-funded input tree, its cleanup and measurement work, the original
deadline, qualified engine costs and scratch. Only caller-supplied limits bound
byte/node work; raising them is not an independently qualified production
configuration. The existing scalar work guard remains unchanged and must share
one budget across the auction when serving integration is qualified.

### Input Measurement Verification

Verification on 2026-10-22 uses Rust 1.98.1, locked/offline dependencies and two
build jobs.
The 23 colocated controls cover independent byte/node/depth boundaries, exact
escape/numeric sizes, sibling nesting, zero limits, checked overflow and depth
31/32/33. A 10,000-container input is refused using a borrowed walk and then
torn down iteratively by its test owner, not by a recursive destructor.

The existing allocator target retains its independent allocation/zeroing/resize
positive controls. It observes zero allocation calls or bytes around successful
and refused measurements over all scalar kinds, empty containers, three
independently sized nested inputs and oversized strings/keys. The generated
AppNexus callback test distinguishes structural refusal (zero callback calls
and no work debit), admitted valid data and admitted invalid data. Its projected
costs are explicitly synthetic, not engine-work bounds.

Before implementation the complete 23-test unit set failed behaviorally against
the stub. The separately executed callback and allocator tests also failed with
`Representation` instead of their independently specified outcomes. After the
module-layout/lint cleanup, the secure-default five-target slice passed 256
tests: library 204, allocator 4, compiler controls 22, initialization 2 and source
inventory 24. No test in that slice was ignored. The command was:

```sh
CARGO_INCREMENTAL=0 cargo test --offline --locked -j2 -p auctioneer-bidder --lib --test schema_compiler_controls --test catalog_alloc --test schema_initialization --test schema_source_inventory -- --test-threads=1
```

The same complete invocation with `--features insecure-bidders` passed 267 tests:
215 library tests and the same 52 integration controls, with no failures or
ignored tests. Both final replays followed the lint cleanup; no assertion or
numeric input value was weakened.

Strict workspace all-target Clippy with `insecure-bidders,conformance-suite`
and `-D warnings`, formatting, the unchanged 168-fixture/50-file raw-JSON gate
and whitespace checks passed. Three actual guest-library checks passed with
`cargo check --offline --locked -j2 -p auctioneer-bidder --lib`, specifying
`wasm32-unknown-unknown`, `wasm32-wasip1` and `wasm32-wasip2`. These compile the
input walker; they do not execute guests, measure per-ABI stack, link all-schema
harnesses again or establish physical fit. Existing GVL and dependency
future-compatibility diagnostics remain visible.

Separate static spec and code-quality reviews approved the scoped increment.
The work budget moves to `schema/mod.rs` only to follow the repository's module
layout rule; its precharging behavior and tests are unchanged. No new
dependency, feature, lint suppression, profile, serving call site or resource
equation is introduced. Task 3 qualification and Task 12 integration remain open.

## Captured Parameter Expectations

The [parameter capture](../../crates/auctioneer-bidder/tests/conformance/fixtures/reference/schema-reference/README.md)
contains every accepted/rejected literal from fourteen pinned parameter-test tables,
not expected results generated by Auctioneer. Source labels, order, duplicate entries,
whitespace and malformed-JSON inputs are retained. No reference program
was executed to produce or validate these expectations.

| Schema       | Source Accepts | Source Refusals |
| ------------ | -------------: | --------------: |
| appnexus     |             16 |              26 |
| bidstack     |              4 |               3 |
| criteo       |             16 |              19 |
| feedad       |             19 |              20 |
| kargo        |              4 |               9 |
| msft         |             12 |              21 |
| openx        |              7 |              15 |
| pubmatic     |              6 |              20 |
| sharethrough |              7 |              14 |
| sovrn        |              5 |               6 |
| thetradedesk |              3 |               9 |
| unruly       |              2 |               8 |
| visx         |              4 |              14 |
| yahooAds     |              1 |              10 |

All 300 cases run through the native raw-input probe in the existing linked
compiler target. Its 287 syntactically valid inputs reach the actual generated
callbacks; thirteen JSON refusals occur before schema evaluation. The probe reports
JSON refusal separately from
schema refusal: the required split is 106 accepts, 181 schema refusals and
thirteen JSON refusals. Independent literal counts also fix this split for each
family; an aggregate total alone cannot mask a missing or misrouted family.
The probe retains its 64-KiB input ceiling, including exact/
one-over controls; an unknown schema refuses even with malformed JSON input.

The owning reader pins the capture bytes, revision and all fourteen source witnesses.
It requires object-only root/source/case shapes, complete field sets, exact group
membership/order/indexes and source-aligned booleans. Unknown, duplicate or missing
fields, positional records and renamed/removed groups refuse. Original bytes still
reach the DTO decoder after object-shape preflight; preflight cannot normalize
duplicate fields away. The capture is `-text` protected, with actual Git checkout
filter coverage alongside the existing raw reference artifacts.

When `AUCTIONEER__BIDDER__SOURCE_DIR` is supplied, the target additionally reads the
immutable revision/path blobs through Git, not working-tree files. Exact lengths,
digests, reviewed table spans, literal offsets/content and complete table gaps
must match; changed source bytes outside the tables and changed captured literal
bytes both refuse. This verifies bounded byte witnesses, not a general source
parser or a recapture generator. The native CI job already supplies that checkout.
Without it, capture integrity and callback behavior still run, but there is no
fresh source-blob comparison; the test reports the actual source-blob count.

This is complete table coverage for these fourteen families only. The priority
names `rise`, `rubicon` and `triplelift` have no corresponding parameter-test
table at this pin; no captured expectations are invented for them. The selected
`ttd` flow uses its canonical `thetradedesk` table. AppNexus remains an additional
shipped-family control. This does not
qualify the other 247 schemas, every branch of the fourteen schemas, work/cache/
allocation bounds, guest execution, resource descriptors, live parameter
validation or bidder catalog/resource qualification readiness. All 261 markers remain linked in four artifacts;
that link coverage is distinct from native compatibility coverage. Original
schemas, compiler patch, workloads, thresholds and historical observations are
unchanged. Dated verification records below distinguish each capture expansion.

### Original Source-Case Verification

The following records the original October 20 three-family capture, not the
expanded eleven-family run. Its historical logs and observations are unchanged.

The compiling raw-probe stub first failed the linked control. The capture owner
then failed three of seven metadata tests before validation was implemented; the
literal-table witness control failed independently before byte verification.
The positional-array control demonstrated a real Serde fallback, and passed after
object preflight. The actual Git filter test failed on the new capture path before
its `-text` rule was added. None of these REDs is a compile error or zero-match run.

Rust 1.98.1, locked/offline resolution and two jobs are used throughout. The
full secure bidder/schema invocation passes 256 tests: 159 library, 55 linked/
capture controls, 18 compiler controls and 24 inventory controls, with no failures
or ignored tests. Its log is `/private/tmp/bidder-catalog-schema-reference-full-secure.log`.
The linked control reports `source_blobs=3`, `accepted=29`, `schema_rejected=59`,
`json_rejected=2`, `total=90`. The existing isolated cold/repeat allocator controls
also pass; they remain finite observations, not new all-path bounds.

The same four targets with `--features insecure-bidders` pass 267 tests: 170
library, 55 linked/capture controls, 18 compiler controls and 24 inventory controls,
again with no failures or ignored tests. The log is
`/private/tmp/bidder-catalog-schema-reference-full-insecure.log`; it records the same exact
source-blob and case counts. Both invocations link all 261 callbacks into the
native and three guest artifacts; only the native artifact executes these cases.

The complete CLI import target passes five tests, including all nine protected
checkout paths and its unprotected conversion control. The log is
`/private/tmp/bidder-catalog-schema-reference-import-final.log`. The source checkout is supplied
explicitly as `AUCTIONEER__BIDDER__SOURCE_DIR`; this path grants no provider/network
access, and only the existing local reference/Unix socket controls require host
permissions.

Commands are `cargo test --offline --locked -j2 -p auctioneer-bidder --lib --test
schema_build_controls --test schema_compiler_controls --test
schema_source_inventory -- --nocapture` and `cargo test --offline --locked -j2
-p auctioneer-cli --test bidders_import`. RED logs use the `probe-red`, `owner-red`,
`witness-red`, `shape-red` and `checkout-red` suffixes under
`/private/tmp/bidder-catalog-schema-reference-*.log`.

Final cross-workspace checks also pass:

- `cargo clippy --offline --locked -j2 --workspace --all-targets --features insecure-bidders,conformance-suite -- -D warnings` (log `/private/tmp/bidder-catalog-schema-reference-workspace-clippy.log`). This is host all-target lint coverage, not guest execution.
- `cargo test --offline --locked -j2 -p auctioneer-core --test ci_workflows`: 103 passed, no failures or ignored tests (log `/private/tmp/bidder-catalog-schema-reference-ci-workflows.log`). The existing protected native step still runs the linked compiler controls with an explicit source checkout.
- Workspace formatting, direct formatting of the embedded probe source, `git diff --check`, and the raw-JSON gate (168 fixtures across 50 files).

Existing GVL deletion and dependency future-incompatibility warnings remain.
These are local verification results, not hosted CI or deployed qualification.

### Priority Coverage Verification

On October 21, the new eleven-family membership check first failed against the
original three-family capture: one test executed and failed on the exact missing
set. An initial private-field compile error was corrected before this behavioral
RED; it is not counted as test evidence. The expanded capture is 48,194 bytes,
still under the unchanged 64-KiB document ceiling. Its original ninety rows and
source witnesses also have an independent frozen semantic digest.

With Rust 1.98.1, `CARGO_INCREMENTAL=0` and the explicit local source checkout,
the existing four named targets and bidder library pass 278 tests: 159 library,
77 linked/capture controls, 18 compiler controls and 24 inventory controls.
The command is the original `cargo test --offline --locked -j2 -p auctioneer-bidder
--lib --test schema_build_controls --test schema_compiler_controls --test
schema_source_inventory -- --nocapture`; the log is
`/private/tmp/bidder-catalog-priority-schema-secure.log`. The linked driver reports exactly
`source_blobs=11`, `accepted=79`, `schema_rejected=146`, `json_rejected=11`,
`total=236` and all per-family counts match the independent literal table.

The same invocation with `--features insecure-bidders` passes 289 tests:
170 library, 77 linked/capture, 18 compiler and 24 inventory controls, with the
same source-blob and verdict counts. Both runs have zero failures or ignored
tests. Its log is `/private/tmp/bidder-catalog-priority-schema-insecure.log`.

The full CLI import target passes five tests, including the real protected-
checkout positive control and refusal to repair altered staging files
(`/private/tmp/bidder-catalog-priority-schema-import.log`). The core workflow target passes
104 tests, preserving the existing required schema-control command and source
checkout (`/private/tmp/bidder-catalog-priority-schema-ci.log`).

Independent spec and quality reviews approved this scoped increment. The first
strict lint pass then reported ten test-helper style diagnostics. Binding-name,
import, range-type and positive-result assertion cleanup added no suppression
or behavioral change. All fifteen capture-owner tests and the family-membership
test passed again afterward (`/private/tmp/bidder-catalog-priority-schema-final-owner.log`
and `/private/tmp/bidder-catalog-priority-schema-final-membership.log`). The complete 278/289
runs above preceded that lint-only cleanup.

Final `cargo clippy --offline --locked -j2 --workspace --all-targets --features
insecure-bidders,conformance-suite -- -D warnings` passes
(`/private/tmp/bidder-catalog-priority-schema-clippy-final.log`), as do workspace formatting,
diff checks and the unchanged 168-fixture/50-file raw-JSON gate. Existing GVL
deletion and dependency future-incompatibility diagnostics remain visible;
neither is suppressed or claimed fixed here.

All 261 callbacks link for native and the three guest targets. Only native
executes the captured instances. Existing cold/repeat allocator controls retain
their thresholds and workload; these observations are not conservative all-path
work, cache or memory bounds. No source schema, compiler patch, dependency,
toolchain, production limit, accounting coefficient or live registry changes.
Full Task 3 qualification and Tasks 12/17 acceptance remain open.

## Format And Pattern Source Cases

The next capture increment adds the complete pinned `bidstack`, `feedad` and
`visx` parameter tables to the existing source owner and generated-callback
driver. The original eleven families' 236 rows and all provenance reconstruct
the exact prior 48,194-byte capture, including its original SHA-256. The earlier
90-row preservation control also remains unchanged. No source schema, compiler
patch, validator expectation or capture-admission limit changes.

| Added Source | Accept | Schema Refusal | JSON Refusal |
| ------------ | -----: | -------------: | -----------: |
| bidstack     |      4 |              3 |            0 |
| feedad       |     19 |             20 |            0 |
| visx         |      4 |             12 |            2 |
| Added Total  |     27 |             35 |            2 |

These tables cover UUID-format inputs, integer/string patterned identifiers,
FeedAd's repeated regex groups and nested SDK-property types. FeedAd's duplicate
members and Visx's `nil` and empty lexical inputs remain literal source rows,
not normalized or replaced. Expected booleans come only from source table
membership. The immutable source-blob and complete table-span checks also apply
to each added family; native JSON parsing only distinguishes lexical refusals
from schema refusals, never supplies the expected validation boolean.

The current capture contains 300 rows from fourteen families: 106 accepts,
181 schema refusals and 13 JSON refusals. Its 62,945 bytes remain below the
unchanged 65,536-byte whole-capture limit. The [capture record](../../crates/auctioneer-bidder/tests/conformance/fixtures/reference/schema-reference/README.md)
owns the exact digest, source lengths and spans. This extends native
compatibility evidence, not production registration, migration or schema
acceptance. The remaining 247 schemas, untested branches within these fourteen,
and the work/cache/allocation/target-resource gates remain open.

### Format And Pattern Verification

The new family-membership control first failed with the old eleven-family list
instead of the independently specified fourteen-family list. The capture-owner
tests separately failed on the old 236/300, 11/14 and 0/64 counts before the
data expansion. The old-capture reconstruction control passed before and after
the additions; no previous input, offset, expectation or provenance was weakened.

On 2026-10-22, Rust 1.98.1 and locked/offline dependencies, the six-target
secure-default slice passed 339 tests: library 204, allocator 4, linked/compiler
controls 83, direct compiler controls 22, constructor controls 2 and schema
inventory 24. No test in that slice was ignored. The full linked control recorded
all 261 reachable validators in native plus three WASM artifacts, and the actual
native case driver recorded exactly:

```text
source_blobs=14, accepted=106, schema_rejected=181, json_rejected=13, total=300
```

The invocation explicitly supplies the separate pinned reference checkout:

```sh
CARGO_INCREMENTAL=0 AUCTIONEER__BIDDER__SOURCE_DIR=/path/to/pinned-reference cargo test --offline --locked -j2 -p auctioneer-bidder --lib --test schema_build_controls --test schema_compiler_controls --test schema_source_inventory --test schema_initialization --test catalog_alloc -- --test-threads=1
```

The existing local loopback no-retrieval fixture ran without sandbox socket
restrictions. No external source program or provider endpoint was executed.
Linked guest artifacts are compilation/link evidence, not guest execution,
stack bounds, physical fit or completed engine qualification. The native cold/
warm constructor and allocation controls retain their existing workloads and
challenge ceilings; no new memory bound is inferred from this replay.

The identical six-target invocation with `--features insecure-bidders` passed
350 tests: 215 library tests and the same 135 integration controls. It also
linked all four artifacts and drove all 300 native cases with fourteen
immutable source witnesses. No test in either slice was ignored. The recorded
logs are `/private/tmp/bidder-catalog-schema-pattern-source-{secure,insecure}.log`.

After those full runs, lint cleanup strengthened the lexical-input assertion to
require `Syntax` for `nil` and `Eof` for the empty string. The complete seventeen
capture-owner tests passed again in each feature lane. These final replays did
not rerun the four-artifact build or allocation probes; their code, source
inputs, observations and verdict assertions were unchanged.

The CLI import/source integration targets passed 5 + 11 tests with the pinned
checkout, including actual checkout-filter byte preservation and independent
wrong-revision/reforged-digest refusal. Strict workspace all-target Clippy with
`insecure-bidders,conformance-suite` and `-D warnings`, formatting, the unchanged
168-fixture/50-file raw-JSON gate and diff whitespace checks passed. Existing
GVL/dependency future-compatibility diagnostics remain visible.

Separate static spec and code-quality reviews approved the scoped expansion and
conditional cache lemma. No production code, dependency, engine, workload,
compiler patch, funding equation or limit changed. Full engine/resource
qualification and serving integration remain open.

## Generator Source Inspection

Relevant source within the exact registry packages:

- `jsonschema-macros-core/src/lib.rs`: input paths/resources, registry creation
  and emitted source-change tracking.
- `referencing/src/retriever.rs`: the default retriever refuses retrieval.
- `jsonschema-macros-core/src/codegen/keywords/format.rs`: draft-sensitive
  formats and explicit custom-format dispatch.
- `jsonschema-macros-core/src/codegen/regex.rs` and `codegen/emit_root.rs`:
  translated patterns and lazy runtime regex construction with configured caps.
- `jsonschema-value/src/`: runtime helpers whose code, data and temporary
  allocations still need qualification.

Static inspection alone is not a remote/file-reference refusal test. The linked
harness adds executed controls described below. The explicit constructor API
below can initialize generated pattern regexes, but complete search-cache and
custom-format initialization/residency still need qualification and funding
before requests if this candidate is selected. Warm calls must not repeat
schema or regex compilation; the finite probes are not complete cache bounds.

### Runtime Helper Scope

The following source observations apply to the committed schemas and the
boolean-only, `serde_json::Value` backend, not every feature of the library:

- `codegen/keywords/properties.rs` and `items.rs` emit borrowed object/array
  traversal. Their error-collection branches are not the selected boolean
  API. Combinators can revisit the same instance; the work derivation must
  count every possible branch, including failed branches.
- The captured schemas have no `uniqueItems` or `multipleOf` keyword, and
  their enums contain strings. The arbitrary-precision and complex-enum
  helpers present in the dependency sources are not evidence that these
  validators execute those paths. Numeric bounds use primitive literals;
  `codegen/keywords/minmax.rs` selects primitive comparisons with the
  reviewed features. This is a source observation, not an allocation bound.
- Draft 4 integer checks can allocate even after regex initialization:
  `jsonschema-value/src/serde_number.rs::is_written_as_integer` formats
  floating-point representations. Invalid `adkernel` input `{"zoneId":1.5}`
  must therefore be measured separately from an integer positive control;
  boolean-only validation is not synonymous with allocation-free validation.
- A regex program-size limit and DFA-cache limit do not bound the complete
  regex object, compilation scratch or pooled search caches. In locked
  `regex-automata 0.4.18`, `meta/regex.rs::Config::get_pool_capacity` defaults
  to available CPU parallelism. This configures shards, not a retained-cache
  ceiling: `util/pool.rs` has an owner cache and independent shard stacks.
  Successive nonowner threads can populate different stacks even when only
  one search runs at a time. Failed shard `try_lock` calls can construct
  disposable caches during otherwise warm searches, and return contention
  can discard reusable caches. Single-thread warming therefore certifies
  neither complete residency nor allocation-free later calls. A complete
  descriptor must account for shard high-water populations, transient caches,
  the other search engines' storage and their target-specific multiplicities.
  These owners remain unqualified, not proven unbounded; the two configured
  limits alone are not their byte bound.

  The next source-level derivation has a concrete conditional population bound:
  with `S` actual shard stacks and at most `q` simultaneous search operations
  over the regex's lifetime, including cache construction and return, same-thread
  acquisition/return permits at most `q` retainable caches per shard, plus the
  one owner cache. Only an observed empty shard creates a retainable cache;
  failed-lock caches carry `discard: true` and cannot inflate the retained
  population. Thus `1 + S*q` retained cache objects plus at most `q` transient
  objects is conservative under those premises. The selected synchronous
  `search_half` returns its guard before leaving the call; this is not a general
  promise about movable guards or all regex APIs. Actual `S`, the complete
  deployment ownership bound `q`, container capacities, byte costs and growth
  overlap still require qualification. In particular, the current memory-report
  methods are not capacity proofs: some stack reports use lengths, hybrid map
  storage is approximated and the meta-cache report omits `capmatches`.

  Source witnesses for this conditional lemma, checked on 2026-10-22 against
  both the local source and members of the cached archive whose SHA-256 matches
  Cargo.lock's `ad8553b9b26413251cbf30e620595c7a41b3887f03da04579c0e6b0d6a06b4b2`:

  | `regex-automata 0.4.18` Member | SHA-256                                                            |
  | ------------------------------ | ------------------------------------------------------------------ |
  | `src/util/pool.rs`             | `6aaebecd38d4b4ea9d70468b45f67dffa479737f60fb1947bfe9dd8a79c787f3` |
  | `src/meta/regex.rs`            | `da85acff388ff352f4ddc2ce282c9b63c5581f25f3aa2ca9af6ee43811f9d864` |

In particular, calling all 261 validators on an empty object is dispatch
coverage, not proof that all lazy initializers ran: missing required fields
can short-circuit before a regex or format check. Later allocation controls
must identify the paths they actually exercise. Neither a zero-allocation
repeat nor a passing finite workload replaces the source-bound work/cache
derivation required by the full gate.

## Linked Harness

`schema_build_controls` generates a disposable standalone crate from the
verified inventory, using the same test-only format hooks as the native probes.
The child-process controls run on Unix hosts; they do not qualify other native hosts.
All 261 function pointers remain in a runtime-selectable dispatch table; native
execution checks the exact ordered names and invokes every row. The production
release and wasm-release profile tables, including package overrides, are
copied structurally from the workspace, not approximated with defaults.

The workspace lock is only a resolution seed. An offline Cargo metadata pass
materializes the actual standalone closure before any locked build. Every child
registry package's name/version/source/checksum must match the reviewed lock;
the standalone package must be present exactly once. A fresh unseeded resolution
selected `aho-corasick 1.1.5` instead of the reviewed 1.1.4 and was correctly
refused during development. It was not accepted as a new qualification pin.

The unknown-OS WASM target cannot use the command-line interface: its argument
stub let optimization discard the original table. The isolated harness now
exports the parameter-dependent Rust entry `auctioneer_schema_validate` plus
`auctioneer_schema_membership`, with two explicit linker export arguments emitted
by its build script on that target only. These are link-retention controls,
not a deployed FFI or JavaScript interface. A maintained WASM parser validates
the artifacts and requires both function exports on the unknown-OS target.
Every artifact must contain the complete inventory stamp; an empty valid WASM
module cannot pass. Each required export is also independently renamed in a
valid copy of the artifact and must trigger its exact refusal, so neither
export check is hidden by its sibling. Native and WASI keep the ordinary binary entry point.

An independent Rust AST check verifies every schema's canonical source path,
public name and corresponding marker callback against the frozen inventory.
Routing every row to `Schema0` and separately compiling every marker from the
AppNexus schema were both observed failing that check, then restored.

Child processes have five-minute build deadlines and bounded pipe retention
(4 MiB per output stream); the process group is cleaned up before joining the
readers. Competing test invocations lease the shared target cache rather than
overwriting an artifact while another invocation checks it. The disposable
manifest and sources are removed when their owner drops; build caches remain
under the existing workspace `target/` directory.

The native CI job installs all three WASM targets and performs native plus
explicit three-target locked fetches before each build-control consumer.
Each native/WASM dependency-fetch step has its own enforced ten-minute limit.
Structured workflow tests reject missing targets, removed or skipped setup,
unlocked fetches and setup moved after any consumer. No new ignored test,
required feature, production schema validator or workspace crate was added.

### Guest Section Measurements

The linked control's test-only artifact reader uses the locked `wasmparser`
validator and recursive parser. Its observation distinguishes complete artifact
bytes from function-body bytes and data-segment payload bytes. Function bodies
include their local declarations and operators, not code-section framing; active
and passive data payloads exclude segment headers and offset expressions. Code
section lengths are not added again to function-body lengths.

A component's nested core modules contribute once each. The enclosing component
or module-section byte ranges are not added to code or data totals. The reader
separately counts defined and imported memory declarations; defined initial
memory bytes describe declared minima, not maxima, physical resident bytes or
host-owned imported memory. These observations include the complete linked
harness and any component adapters, not just schema-generated code.

Outer format must agree with the linked compiler target: core modules for
`wasm32-unknown-unknown` and `wasm32-wasip1`, a component for `wasm32-wasip2`.
Native artifacts have no WASM measurement. This does not fill the native
code/data measurement gap, certify target residency or provide C/I/V/K
descriptors. Full engine qualification and runtime integration remain pending.

Observed on 2026-10-21 with Rust 1.98.1 and the reviewed lock/profiles, after
linking all 261 callbacks in each artifact:

| Target                 | Core Modules / Components | Defined Functions / Bodies | Body Bytes | Active / Passive Data Bytes | Defined Initial Memory Bytes |
| ---------------------- | ------------------------: | -------------------------: | ---------: | --------------------------: | ---------------------------: |
| wasm32-unknown-unknown |                     1 / 0 |              1,235 / 1,235 |  1,000,151 |                 376,336 / 0 |                    1,441,792 |
| wasm32-wasip1          |                     1 / 0 |              1,379 / 1,379 |  1,032,853 |                 396,282 / 0 |                    1,507,328 |
| wasm32-wasip2          |                     3 / 2 |              1,426 / 1,426 |  1,082,217 |                 392,842 / 0 |                    1,507,328 |

All three contain two data segments and one defined memory, with zero imported
memories. Their imported-function counts are respectively 3, 7 and 33. These
counts include library/toolchain helpers, not one function per validator.
The corresponding complete file sizes/hashes are:

| Target                 | File Bytes | SHA-256                                                            |
| ---------------------- | ---------: | ------------------------------------------------------------------ |
| wasm32-unknown-unknown |  1,521,959 | `9125a5a64d1f1d08eaf3625d3a33bf09d23578ae2075b2404e1a0a4a19e7b267` |
| wasm32-wasip1          |  1,579,636 | `7c45a8a74f9904eaccc8b6c3c846d9f1069724fd4e10cb4e405c51dcf786c2c5` |
| wasm32-wasip2          |  1,637,108 | `2323ff598260850702c6654a99641d25917ea2d69f31ea20061cfba9234c7336` |

#### Initial Measurement Verification

These section-reader verification runs preceded the priority capture expansion;
the newer eleven-family results are recorded under Priority Coverage Verification.

The reader's compiling stub first failed 12 of 15 controls. The old target-format
check failed its new control by accepting a module as `wasm32-wasip2`. Separately,
the full four-artifact link test failed when `LinkReport` still supplied `None`
instead of measurements. These REDs compiled and executed real assertions; none
is a compilation error or zero-match run. The logs are
`/private/tmp/bidder-catalog-artifact-{reader,format,integration}-red.log`.

The final secure invocation passes 272 tests: 159 library, 71 linked, 18 compiler
and 24 inventory controls, with no failures or ignored tests
(`/private/tmp/bidder-catalog-artifact-secure-verified.log`). This includes all 15 reader
controls, target-format checks, real four-artifact builds, the existing
cold/repeat allocation controls, and all 90 captured cases with three immutable
source blobs verified. The source checkout was explicitly supplied through
`AUCTIONEER__BIDDER__SOURCE_DIR`. Measurements are emitted as structured JSON;
the observed numbers above are unchanged by the final lint-clean refactor.

The feature-enabled invocation passes 283 tests: 170 library, 71 linked,
18 compiler and 24 inventory controls, with no failures or ignored tests
(`/private/tmp/bidder-catalog-artifact-insecure-verified.log`). It produces the same guest
measurements, verifies three source blobs and dispatches all 90 captured cases.
These results use `cargo test --offline --locked -j2 -p auctioneer-bidder --features
insecure-bidders --lib --test schema_build_controls --test schema_compiler_controls
--test schema_source_inventory -- --nocapture`. The secure linked invocation
omits `--features` and retains all four targets.

Colocated binary-fixture tests pin unequal body and data lengths, nested modules,
opaque custom-section payloads, imported/defined memory separation, memory64
conversion and sum overflow, exact/one-over admission, custom-page refusal,
truncation and semantically invalid code. Nested invalid cases carry correct
enclosing lengths so framing errors cannot mask their semantic checks.
The production normal/build dependency tree still excludes the candidate,
UUID helper and WASM parser (`/private/tmp/bidder-catalog-artifact-production-dependencies.log`).
No new dependencies, captured fixtures, feature gates, production inputs, workload limits
or budget coefficients are introduced by this measurement step.

Final checks also pass:

- Strict workspace all-target Clippy with `insecure-bidders,conformance-suite` and `-D warnings` (`/private/tmp/bidder-catalog-artifact-workspace-clippy-verified.log`), after fixing style errors without adding lint suppressions.
- Core `ci_workflows`: 103 passed, no failures or ignored tests (`/private/tmp/bidder-catalog-artifact-ci-workflows.log`). The existing required native control target executes the reader; no new test target or workflow is needed.
- Workspace formatting, `git diff --check`, and the raw-JSON fixture gate (168 fixtures across 50 files).

Spec review approved the bounded measurement contract. Code-quality review
checked the observations and also approved the final counter-dispatch extraction
and JSON logging. Existing GVL deletion and dependency future-incompatibility
warnings remain. These are local results, not hosted CI, deployed execution or
target-readiness evidence.

### Observed Artifacts

The linked-only preparation at `9da4a168`, before the allocation modes below,
on `aarch64-apple-darwin`, Rust 1.98.1
(`48a229cea`, 2026-09-01), produced the following complete harness artifacts.
These are observations, not golden files, generated-code-only sizes, per-target
residency bounds or guest execution results. The native digest can change with
the disposable source path retained in its line-level debug information.

| Target                 | Validators | Artifact Bytes | SHA-256                                                          |
| ---------------------- | ---------: | -------------: | ---------------------------------------------------------------- |
| native                 |        261 |      2,119,528 | 0d74e01d772ed0717610910eae3f8ca668a8bcf8cdf1690eff87e3d8fbdb8e4a |
| wasm32-unknown-unknown |        261 |      1,520,686 | 4c8a79f17fe748178bf63caf527e6f021a946c8b6fc8ad85ffb9caeecfd57a96 |
| wasm32-wasip1          |        261 |      1,576,536 | 6342d5e133feeeaea28037ce8275dff7ce3b450210b7a6ca973ade7d3513f4f4 |
| wasm32-wasip2          |        261 |      1,633,835 | 37fe19c97fab9366ac134751f554a9ab77af9294f35d77553fdd113f89c3b50f |

After the workflow's target and locked-fetch setup, reproduce the controls with:

```sh
cargo test --locked --offline -p auctioneer-bidder \
  --test schema_build_controls -- --nocapture
```

Native execution covers all dispatch rows plus positive
and negative instance cases. The macro is also checked against an available
file and a listening HTTP endpoint: both require the default retriever's
specific refusal, and the listener must observe no connection. A successful
local-source compiler control prevents unrelated compiler failures from
satisfying those negative cases.

Final linked-slice verification on 2026-10-17:

- Secure-default `cargo test --locked --offline -p auctioneer-bidder --all-targets -j 2 -- --quiet`: 228 passed, zero failed or ignored, including 30 build-control tests.
- `cargo test --locked --offline -p auctioneer-core --test ci_workflows -j 2 -- --quiet`: 94 passed, including independently mutated setup, target and prefetch-duration controls.
- Workspace `cargo clippy --locked --offline --workspace --all-targets --features insecure-bidders,conformance-suite -j 2 -- -D warnings`, formatting and raw-JSON checks passed. The normal/build bidder dependency tree still excludes the candidate, UUID helper and WASM parser.

Both review stages approved this bounded compiler slice only. No full-schema
work/scratch/cache or physical WASM memory qualification was performed.

## Native Allocation Controls

The disposable native executable also owns a test-only `System` allocator
delegate. Each measurement encloses only the synchronous generated callback
on an already constructed, borrowed value. Input construction, JSON parsing,
shape inspection, callback lookup, result storage and serialization are
outside the window. Counters are thread-local; overflow refuses the report.
Positive controls pin ordinary/zeroed allocation, growing/shrinking realloc,
deallocation and signed retained deltas. A separately allocated worker and
outside-window setup must not affect the callback's counters.

`allocated_bytes` counts complete successful allocation/reallocation requests;
`peak_live_bytes` is the maximum incremental logical requested heap relative
to the window's starting point. It excludes baseline caches, allocator
bookkeeping and any internal old/new physical overlap during realloc.
`retained_bytes` is signed, so freeing a preexisting block cannot disappear
through saturation. These are requested-heap observations, not process RSS,
stack measurements or complete allocation upper bounds.

Separate processes distinguish empty-object dispatch across 261 callbacks
from 62 named inputs covering 11 validators. The named inputs exercise
64-KiB serialized values, 4,096-node collections/objects, depth 32, changed
inputs after short-input initialization, Unicode, late-invalid array/object
values, FeedAd's nested-repeat regex and Draft 4 fractional-number refusal.
Each process repeats the same complete input set after its first pass.
Consumable's numeric fields stay numeric in these workloads; its type bypass
is recorded separately above, never treated as valid input policy.

The existing 512-KiB scratch, 8-MiB initialization and 12-MiB cache/data
ceilings are used as independent challenge thresholds, not derived from the
observations. The sampled first-pass cumulative allocation is checked against
the initialization ceiling, but does not partition initialization from
resident memory and scratch. Not every catalog cache is reached, and no
concurrent cache-population, generated static data, physical WASM memory or
conservative work bound is certified by this test.

On the same native Rust 1.98.1 host, a fresh sampled process observed:

| Pass               | Requested Bytes | Incremental Peak Bytes | Retained Bytes | Allocation Calls |
| ------------------ | --------------: | ---------------------: | -------------: | ---------------: |
| First pass         |         895,247 |                371,978 |        166,738 |            3,260 |
| Repeated input set |               8 |                      8 |              0 |                1 |

Changed hostname inputs after short-input initialization retained another
31,264 bytes for the sampled LimelightDigital paths and 30,248 for OpenX;
the next identical pass retained zero. This is evidence of input-dependent
cache growth, not an all-cache initialization strategy. Adkernel's repeated
fractional refusal requested eight bytes; the integer positive control
requested zero. These observations are not golden memory bounds.

Verification of this increment:

- Secure-default bidder all-target suite: 230 passed, zero failed or ignored;
  includes 31 linked/allocation controls and eight compiler controls. The
  latter include the explicitly failed-candidate record, not an acceptance.
- Workspace all-target Clippy with `insecure-bidders,conformance-suite` and
  `-D warnings` passed. Existing GVL deletion and dependency future-incompatibility
  warnings remain; no suppression was added for the new compiler controls.
- The raw-JSON fixture gate remains 169 fixtures across 50 files. No captured
  schema or external expected result changed.

Separate spec and code-quality reviews approved this bounded increment after
removal of an unnecessary allocator lint exception. They did not approve the
failed candidate, a complete resource derivation or runtime integration.

## Explicit Cache Native Observations

The native explicit probe now uses the same counting allocator and literal
workloads as the default probe. Separate child-process reports distinguish
shared constructor initialization, per-schema bundle creation, first and
changed-input searches, same-input reuse, and bundle release. All 261 schemas
have independent populations of 1, 2 and 4 probe owners. Empty-object dispatch
plus 62 literal workload inputs exercise 4,522 searches; valid/invalid outcomes,
membership, population sizes and full measured lifetime balance are asserted.
The allocator's positive, zero, release and cross-thread controls run in this
artifact too. Native-only source guards exclude the meter from guest builds.

Reproduce the observation, not a resource qualification:

```sh
CARGO_INCREMENTAL=0 cargo test --offline --locked -j2 -p auctioneer-bidder --test schema_build_controls explicit_cache_measurements_separate_construction_growth_reuse_and_release -- --test-threads=1 --nocapture
```

Historical pooled-hook observations at `72d7dbae` on
`aarch64-apple-darwin`, Rust 1.98.1, on 2026-10-22:

| Phase                                              | Requested Bytes | Incremental Peak | Retained Delta |
| -------------------------------------------------- | --------------: | ---------------: | -------------: |
| Shared constructors, including format construction |       1,895,958 |          497,825 |        173,198 |
| Separate hostname first search                     |          12,913 |           11,264 |         11,208 |
| Separate hostname changed input, 255 bytes         |          55,080 |           29,848 |         29,848 |

Shared constructors made 5,001 allocation requests; their repeated calls made
none. The sampled 4,522 bundle searches requested 82,775 bytes in aggregate;
this is neither a peak nor a derived upper bound. Per-owner construction,
search and release deltas balance for each measured population. The native
probe enum is 4,208 inline bytes per owner, reported separately from requested
heap; its container storage, inputs and report serialization are outside the
windows. This enum layout is not the selected production-owner layout.
In that historical layout the hostname pool was warmed separately on the same
thread before bundle searches, so its initial growth was not attributed to
the pattern bundles.

### Per-Call Hostname Cache Observations

AG approved a separate qualification-only follow-up on 2026-10-22. The hook
retains one immutable meta-regex with the same exact pattern, UTF-8 boolean
matching and both 65,536-byte limits. It now creates, searches and drops an
explicit cache inside every callback. Its unused shared pool bookkeeping is
fixed to one shard, not the host CPU count; search never acquires that pool.
The callback signature, UUID helper and `<256` byte guard are unchanged.
The guard executes before cache construction. Both generated modes use this
same hook; no serving validator or compiler patch changed.

The added zero-retention assertion first failed against the old hook with
11,208 retained bytes. The replacement passed the same reproduction command
above, with these native requested-heap observations on the same host/toolchain:

| Phase                                              | Requested Bytes | Incremental Peak | Retained Delta |
| -------------------------------------------------- | --------------: | ---------------: | -------------: |
| Shared constructors, including format construction |       1,894,769 |          496,849 |        172,222 |
| Short hostname, 9 bytes, each call                 |          12,913 |           11,264 |              0 |
| Valid hostname, 255 bytes, each call               |          67,993 |           41,056 |              0 |
| Invalid tail, 121 bytes, each call                 |          10,953 |           10,164 |              0 |
| Overlong label, 67 bytes, each call                |          38,569 |           25,080 |              0 |
| Overlong hostname, 256 bytes, each call            |               0 |                0 |              0 |

Shared constructors made 4,997 allocation requests; repeats still made none.
Each initial and repeated call matched its literal verdict and retained zero
requested heap. Every sub-256-byte sample allocated inside its measured call;
both 256-byte calls rejected without allocation. Exact row membership and
length boundaries are asserted, so an omitted or newly unexercised case fails.
The 4,522 enclosing bundle searches now requested 3,851,701 aggregate bytes;
per-call format scratch is included, not warmed outside those windows. Bundle
construction/search/release balances remain zero across every population.
The probe enum remains 4,208 inline bytes, separate from the requested heap.

Removing retained thread-history storage trades it for repeated construction,
search and destruction work. These samples neither upper-bound that work nor
the peak across all inputs. The eventual resource derivation must include
format scratch overlapping the enclosing validator bundle; zero retained
bytes alone is not a zero charge or proof of physical allocator reclamation.

### Locked Source Obligations

The `regex-automata 0.4.18` source audit retains these independent obligations:

- `meta/strategy.rs`'s core cache owns captures and multiple engine caches.
  Lazy PikeVM/backtracker initialization can add retained storage alongside
  existing hybrid/one-pass storage rather than replacing it.
- `hybrid/regex.rs` constructs forward and reverse caches; reverse strategies
  can require another prefix-reverse cache. One 65,536-byte configured limit
  is not a bound on the whole meta cache.
- `meta/regex.rs::Cache::memory_usage` omits captures and inline storage.
  Engine reports also omit or approximate retained capacities, map storage,
  allocation headers and old/new growth overlap. Source-derived NFA states,
  capture slots, union fanout and target container layouts must supply bounds.
- `util/pool.rs` still allocates bookkeeping for the unused meta pool.
  Explicit matching bypasses acquisition, not construction. The hostname hook
  now owns a separate per-call cache; construction, growth and destruction,
  overlapping enclosing bundle residency, still need complete storage/work bounds.
- Deployment admission and execution lifetimes must bound constructing,
  active, idle-retained, cancelled and replacement bundle owners. The probe's
  1/2/4 populations do not select or prove that production layout.

Finite samples, allocator release and linked guest artifacts do not prove
retained native/WASM physical memory or complete work bounds. Resource
descriptors and serving integration remain disabled pending those proofs.

## Forward NFA Logical Storage

The next source-derived component uses the locked `regex-automata 0.4.18`
public Thompson compiler to reconstruct each **translated emitted literal**,
not its raw schema spelling. It preserves the forward meta builder's UTF-8
syntax/empty handling, all captures, no shrink, newline look matching and
65,536-byte NFA limit. The reconstruction does not change the selected engine
or disable an optimization in generated validators.

The new native `schema_cache_bounds` target walks the same audited 261-schema
inventory and finds 23 regex constructors in 14 generated validators. Every
expansion must have exactly one cache owner, including the 247 validators
without regex helpers. Within each owner, named meta-cache fields, constructor
symbols and `as_ref()?.create_cache()` initializers must be a bijection.
Unmatched, duplicated, missing, wrongly typed or differently initialized slots
refuse rather than disappear from the profile. Generated non-regex mutable
state is unsupported by this component and requires a further derivation;
the pinned schema snapshot has no reference/URI state to account here.

For the final forward NFA, let `N` be its number of states, `S` its capture
slot count, and `P` its pattern count. Let `B` sum each union's fanout minus
one (a binary union contributes one). Let `C` count capture **states**, not
distinct slots; bounded repetition can emit several capture states for one
slot. The checked logical projection is:

| Component                                 | Logical Length Or Upper Bound                  |
| ----------------------------------------- | ---------------------------------------------- |
| Two active sets' dense and sparse vectors | `4*N` StateID entries                          |
| Two active sets' slot tables and scratch  | `2*(N*S + max(S,2*P))` optional-offset entries |
| Meta `capmatches`                         | Another `S` optional-offset entries            |
| Epsilon exploration/restore stack         | At most `max(1,B+C)` frames                    |

These follow the locked source's `nfa/thompson/pikevm.rs` `ActiveStates::reset`,
`SlotTable::reset` and epsilon-closure routines, `util/sparse_set.rs::resize`,
and `meta/strategy.rs::Core::create_cache`. The closure pops its seed before
expanding states, deduplicates visited states, and drains its stack. Each union
adds at most fanout-minus-one pending explorations and each capture state at
most one restore frame. Stack-frame payloads are separate from the slot arrays.

`meta/regex.rs::Config::to_thompson_config` supplies the reconstruction's
configuration. `Core::search_half_nofail` delegates to `search_nofail`, whose
fallback passes `Captures::all` slots to PikeVM; boolean results do **not**
license assuming zero capture slots. Literal-only meta strategies may bypass
this NFA entirely. This projection bounds the initialized forward-NFA/PikeVM
component if it is used, not every possible meta strategy.

At the current source pin, generated helpers have at most six capture slots
(`feedad`), and their largest pair of slot tables contains 384 entries
(`admixer`, 47 states and four slots). The separately shared hostname pattern
has `N=265`, `S=8`, `P=1`, `B=125`, `C=8`: 1,060 sparse-set entries, 4,256
slot-table entries, eight extra meta-capture entries and at most 133 stack
frames. Its test reconstructs the actual shared pattern constant, not a copied
pattern. These are logical counts, not native/WASM byte allowances.

Controls vary each dimension independently, pin both active sets and meta
captures, exercise repeated capture states and overlapping-literal branches,
and refuse arithmetic overflow at multiplication, scratch addition, final
doubling and stack addition. Disjoint leading literals can become sparse
transitions rather than epsilon unions and are not used as a branch control.
The added cache-field mutation first passed the old constructor-only audit;
the new reconciliation rejects it. Unknown constructor/config shapes refuse.

Reproduce these component controls:

```sh
CARGO_INCREMENTAL=0 cargo test --offline --locked -j2 -p auctioneer-bidder --test schema_cache_bounds --test schema_compiler_controls -- --nocapture
```

This establishes neither retained `Vec` capacities nor layout/allocator bytes,
reallocation overlap, constructor scratch, whole-meta work, reverse or other
engine caches, or production multiplicities. Clearing a logical length does
not release its capacity. Those obligations remain open; this component cannot
produce a usable C/I/V/K descriptor or authorize serving validation.

## Bounded-Backtracker Logical Storage

The next component is specific to the locked `regex-automata 0.4.18` meta
fallback for our explicit `earliest(true)` searches. In
`meta/wrappers.rs::BoundedBacktracker::get`, the engine refuses selection when
the **full haystack** exceeds 128 bytes, even for a shorter searched span.
It separately checks the visited capacity. This does not make inputs above
128 bytes unsupported: the meta engine selects another strategy. No engine
or search setting changed.

For a searched span of `H` bytes, define `Q=H+1` possible offsets, including
end-of-input. Using the forward-NFA dimensions above, the checked projection
in `tests/schema_backtrack_bounds/mod.rs` is:

| Component                 | Logical Length Or Upper Bound                                          |
| ------------------------- | ---------------------------------------------------------------------- |
| Visited bitmap            | `ceil(N*Q/word_bits)` machine words, independently for 32/64-bit words |
| Exploration/restore stack | At most `max(1,Q*(B+C))` frames                                        |
| Captures                  | The existing meta `S` slots; no second backtracker-owned capture table |

`nfa/thompson/backtrack.rs::Visited::setup_search` caps `N*Q` at 2,097,152 bits
(the default 262,144-byte budget), then rounds up to machine words. That
default is word-aligned on both target widths; the source's separately rounded
`max_haystack_len` formula cannot extend its bit limit here. Invalid shapes,
unsupported word widths or arithmetic overflow refuse the projection.

`step` checks and sets `(state,offset)` before expanding it. Each union can
therefore add its alternate frames at most once per offset, and each capture
state can add one restore frame per offset. The seed is popped before any
expansion; a failed start drains the stack before the next start is seeded.
A match can leave pending frames, still within that bound. The next search
clears the stack and bitmap before reuse; UTF-8 empty-match retries perform
that same setup rather than accumulating logical lengths. Full meta captures
are passed by `Core::search_nofail`, so this derivation does not assume that
boolean validation avoids capture restoration. It does not count that shared
capture storage a second time.

All 23 generated regex helpers are projected on both word widths at the
128-byte boundary. Their maxima are 190/95 visited words (32/64-bit) and
1,161 stack frames. The actual shared hostname pattern projects 1,069/535
visited words and 17,157 stack frames. Both refuse backtracker selection at
129 bytes; the hostname's valid 255-byte case still needs other engine caches.
These are conservative logical component counts, not measured stack peaks.
An ineligible call does not clear a previously used backtracker cache. For a
reusable owner, retain the maximum prior eligible-span projection (at most
128 here); `None` is not a zero retained-state charge. Several fallback caches
can remain populated together.

Controls independently vary states, branch frames, capture states, span and
full-input lengths. They cover word-rounding boundaries, the independent
visited-bit cap, arithmetic refusal and real low-level searches with full
captures. After an unsuccessful search drains the stack, the real cache's
reported usage exactly matches the projected bitmap length. Reusing the cache
with shorter spans lowers this **reported length**, not necessarily its retained
allocation. A separate real-engine control refuses one byte below the minimum
visited budget and succeeds at that budget. Neither `memory_usage()` nor these
samples supplies a capacity, allocator, whole-meta work or production-owner
proof. Serving validation and usable C/I/V/K descriptors remain disabled.

## One-Pass Capture Scratch

For the locked one-pass DFA, a constructed fixed-owner cache has one mutable
heap vector containing `E=S-2P` optional offsets: explicit capture slots only.
Its active prefix length is separate from the vector length. The implicit
two slots per pattern remain in the existing meta captures and are not charged
again here. The checked projection rejects invalid odd slot counts, subtraction
underflow and pattern-count multiplication overflow. Above 32 explicit slots
the locked one-pass representation cannot build; this is not a new Auctioneer cap.

`dfa/onepass.rs::Cache::new` calls `reset`, which resizes the vector eagerly.
`setup_search` changes only the active prefix length and values. Search does
not resize that vector. Therefore the scratch must be counted when the engine
exists, even if a later input cannot select it. `memory_usage()` still reports
length, not retained capacity; resetting a cache for a different DFA can grow
or retain allocations. This component is a conditional fixed-owner logical
projection, not a byte bound for cross-DFA reuse.

The public builder reconstruction uses the same audited forward NFA with
`LeftmostFirst`, starts-per-pattern enabled, byte classes enabled and the meta
default 1,048,576-byte one-pass limit. That last limit is **not** the separate
65,536-byte NFA/hybrid setting. Plain one-pass builder defaults do not reproduce
these settings; the configuration control first failed on the disabled
starts-per-pattern default.

Construction, candidacy and selection are distinct. The meta wrapper's attempt
heuristic reads HIR properties: explicit captures or a Unicode word boundary
must be present. A final-NFA reconstruction is not an exact substitute for
that HIR gate. Search eligibility additionally requires an anchored input or
an always-start-anchored pattern. Outer literal strategies can bypass `Core`,
and another DFA can finish a search before this fallback runs. The audit
deliberately builds conservative candidates, not an alleged exact selector.
It does not use a failed candidate as a general zero-cost qualification rule.

Across the 23 emitted helpers, 19 reconstructed candidates build and four
refuse as non-one-pass: `consumable` helper 2, `feedad` helper 0,
`limelightDigital` helper 0 and `yandex` helper 0. Membership is literal-pinned,
not just counted. Conditional scratch across all helpers has at most four
slots; the successfully built candidates have at most two. The actual hostname
pattern has a conditional six-slot projection but fails one-pass construction;
its other engine caches remain necessary. These are source-derived component
observations, not proof of which candidates the meta heuristic constructs.

Controls compare the computed count with public NFA metadata and real eager
cache lengths. They vary total slots and pattern count independently, exercise
16/17 distinct capturing groups, empty/partial/full output slices with
full-to-partial-to-full reuse, empty and 257-byte inputs, success and failure.
An independent two-pattern public-NFA control has ten total slots and six
scratch entries, with both pattern IDs exercised; the generated corpus still
has one pattern per helper. An unanchored rejection does not
remove already-created scratch; a no-explicit-capture control has zero vector
length. The emitted full-capture `search_half_with` fallback avoids the
short-output UTF-8 workaround. Other APIs can allocate temporary implicit slots
(notably multi-pattern empty-match handling); unchanged cache length does not
prove that every regex API allocates nothing.

Immutable transition/start tables, shared NFA ownership, constructor scratch
including failed construction, retained vector capacities, allocator overhead
and search work remain separate obligations. The one-pass builder's size limit
is not a construction-peak limit. No usable C/I/V/K descriptor or serving path
is enabled by this component.

## Hybrid Directional Fixed Storage

The native `schema_hybrid_bounds` support module derives fixed logical lengths
from each candidate's **own** NFA and DFA byte-class stride. Reverse compilation
uses the same UTF-8, newline, no-shrink and 65,536-byte NFA settings as forward
compilation, but reverses the expression and disables captures, as the locked
meta strategy does. Reusing forward state counts for reverse storage is not
valid: two emitted helpers expand substantially when reversed.

For `N` states, `P` patterns, union excess `B`, DFA stride `d` and a boolean
`f` for per-pattern starts, the checked projection is:

| Component                                          | Logical length or encoding limit |
| -------------------------------------------------- | -------------------------------- |
| Two sparse sets, including dense and sparse arrays | `4N` StateID entries             |
| Start table                                        | `6*(2+f*P)` lazy StateID entries |
| Three sentinel transition rows                     | `3d` lazy StateID entries        |
| Determinizer epsilon stack                         | `max(1,B)` StateID entries       |
| One encoded state or builder payload               | At most `13+4P+5N` bytes         |

The six start-context classes come from `util/start.rs::Start::len`. The two
shared start groups are anchored and unanchored. Determinization follows
captures without pushing restore frames; pending alternate branches require
the union-excess bound, not merely the state count. The encoded payload follows
the actual writer in `util/determinize/state.rs`: a nine-byte fixed header,
up to four bytes for a pattern-list count, four per pattern and at most five
per NFA state ID. This differs from the older header comment used by
`hybrid/dfa.rs::minimum_cache_capacity`. A duplicate pattern-match state would
invalidate the `P` allowance; the audit explicitly rejects it, including a
public-NFA-builder counterexample. No SDK defect is inferred from the older
individual estimates: the complete minimum-capacity formula has other slack.

`hybrid/dfa.rs::Cache::new`, `init_cache` and `memory_usage` additionally predict
the **fresh reported** usage exactly on native:
`4*(sparse_entries+start_entries+sentinel_transitions+1) + 8*word_bytes + 27`.
This covers the sparse/start/sentinel IDs, one additional map ID, four two-word
state handles and three reported nine-byte sentinel payloads. The fresh stack
and builder capacities are zero; their later bounds are not charged in this
fresh report. This is a report-layout check, not physical bytes.
The sentinel payloads share an Arc allocation, the map report estimates entries,
and vector lengths are not retained capacities. The 32/64-bit handle-width
controls are arithmetic models, not guest execution or ABI qualification.

Public candidates retain `LeftmostFirst` forward matching, `All` reverse
matching, per-pattern starts, byte classes and Unicode-boundary support. The
65,536-byte cache setting, minimum three clears and ten bytes per state remain
unchanged. Automatic prefilters are omitted deliberately: candidate building
does not reconstruct exact private meta/HIR selection. A separate reverse
shared-start configuration covers the start-table layout used by the optional
reverse-inner engine, **not** its actual prefix NFA or selection. No absent
candidate is treated as a general zero-residency rule.

All 23 emitted helpers across the 261-schema inventory build both primary
directional candidates. Their exact helper membership is reconciled with the
existing literal-pinned candidate sets. The largest reverse NFA has 73 states
and union excess 31 (`yandex` helper 0), versus 32 states forward; the separate
maximum forward NFA has 47 states. The largest fresh pair reports 3,550 bytes
on native 64-bit (3,486 in the 32-bit model). The actual hostname pattern has
forward/reverse states 265/257 and union excess 125 each; fresh reports are
4,599/4,471 bytes on native, totaling 9,070 (9,006 in the 32-bit model).

Controls vary dimensions, start modes and word width independently; exercise
real multi-pattern starts, reverse matches, capacity admission, search growth,
clearing, failure and reset; and show eager reverse ownership before a
forward-only search. Reset retains history-dependent storage. A deliberately
minimum-sized cache can give up even on a short matching input; the test was
corrected to separate ordinary-capacity success from minimum-capacity failure
and recovery, not to raise any setting or weaken a qualification limit. Reset
recovery completes a nonmatch; it does not claim post-reset positive matching
under that minimum-capacity configuration.

This component does **not** bound whole-cache physical storage. State-builder
growth, table/map capacities, Arc/allocator overhead, old/new growth overlap and
the saved state retained across clearing still need derivation. Immutable
programs, optional prefix-reverse storage, construction peaks, search work,
production-owner multiplicity and native/guest qualification also remain open.
No serving validator or usable C/I/V/K descriptor is enabled.

## Hybrid Population And Payload Model

`tests/schema_hybrid_population/mod.rs` adds a **conditional** populated-state
and encoded-payload model. It assumes one fixed DFA per cache and normal,
nonrecursive recovery. It is not a physical-capacity bound or a usable cost
descriptor. `from_dfa` first enforces a separate recovery check, rather than
treating constructor admission or finite searches as proof. Underfunded
recovery returns `None`, which means unproved residency, **not** zero residency.
All emitted-helper and hostname projection controls require `Some`.

The recovery check uses the audited Rust 1.98.1 append paths: a stack of `u32`
IDs with logical high water `K=max(1,B)` retains at most `max(4,2K)` entries;
the byte builder retains at most `2V`; the fixed start table retains `s`
entries. With `N` NFA states, its scratch-report envelope is
`F=16N+4s+4*max(4,2K)+2V`. The largest sentinel-admission or saved-reinsertion
check needs at most `G=F+16d+12w+35+V` reported bytes. This calculation does
**not** use the population bound below, so requiring `G<=C` is non-reflexive.
It closes recovery stability for the audited fixed candidates without claiming
a whole-cache memory bound. Automatic capacity-upgrade configurations refuse,
because their nominal setting need not be the effective admission limit.

Independent literal controls vary each term and pin overflow. Real DFA builds
at `G-1` and `G` succeed while the recovery gate respectively refuses and
accepts. The hostname needs 9,725 forward and 9,477 reverse reported bytes on
native 64-bit, each below its unchanged 65,536-byte setting. The 32-bit
arithmetic needs 9,677 and 9,429 respectively; it is not a guest run.

For stride `d`, pointer width `w` in bytes, configured capacity `C` and the
previous encoded-state limit `V`, the checked model is:

| Quantity                                           | Conditional envelope        |
| -------------------------------------------------- | --------------------------- |
| Populated states `L`                               | `max(5,floor(C/(4d+2w+9)))` |
| Transition-table length                            | `L*d` entries               |
| Resident encoded-payload sum `R`                   | `max(C,27+2V)` bytes        |
| Encoded payloads including nonresident temporaries | `R+2V` bytes                |

The lower report contribution per populated state omits map/scratch costs,
but includes its transition row, two-word handle and at least nine encoded
bytes. `state_fits_in_cache` and `add_state` therefore bound ordinary admitted
population and payload sum. Normal recovery reconstructs three sentinel
states and at most a saved and a pending state, giving the separate five-state
and `27+2V` cases. Outside resident tables, a pending new payload and a detached
saved payload can each require at most `V`; a saved Arc still shared with a
resident state is not a new allocation. These envelopes intentionally do not
claim exact simultaneous maxima or count cloned handles as new payloads.
They bound encoded `State` contents, not the builder vector or Arc headers:
`to_state()` copies into an Arc while the builder remains live.

`get_state_builder` temporarily replaces the builder with an empty value;
`put_state_builder` restores its capacity after admission. The returned cache
report can consequently exceed `C`. A native regression traces an unchanged
engine with two test-only capacities: 1,129 and 9,032 bytes. It observes five
and 98 distinct returned state IDs per clearing epoch, with three clears in
each, and reports maxima of 1,117 and 9,054 bytes respectively. The latter is
an exercised counterexample to treating configuration as a hard report limit.
The initial eight-ID assertion at minimum capacity was an invalid fixture
assumption; it was replaced by cross-capacity variation and nonvacuity checks,
not by changing production capacity or clearing heuristics.

State IDs are compared as public opaque identities. Numeric IDs and complete
cache occupancy are private, so this trace is an observed subset, **not** an
exact occupancy oracle. It neither parses Debug output nor casts private
layouts. Independent literal arithmetic cases cover capacity, stride, word
width, encoded-state size, recovery thresholds and checked overflow. Zero and
tiny capacity cases test arithmetic only, not admission of actual DFAs.

Both directions of all 23 emitted helpers and the actual hostname pattern
receive the conditional projection. For the hostname's 65,536-byte setting,
the native 64-bit model yields 736 states and 11,776 transition entries per
direction; encoded-payload envelopes are 68,220 forward and 68,140 reverse.
The corresponding 32-bit arithmetic yields 809 states and 12,944 entries;
no guest execution or ABI qualification is inferred.

The matching Rust 1.98.1 `rust-src` component was installed for source audit;
the compiler version and project dependencies did not change. Source anchors
are `alloc/src/raw_vec/mod.rs::grow_amortized`, the trusted-length `Vec`
extension paths and the locked engine's `add_state`, `clear_cache`,
`get_state_builder` and `put_state_builder`. These are pinned-implementation
facts, not a promise from the generic collection APIs. Complete collection/map
capacity and requested-layout envelopes, builder/sentinel scratch, Arc and
allocator overhead, growth overlap, construction/work, actual owners and
target qualification remain open. No serving path or production charge changed.

## Requested Hybrid Heap Envelope

The next component combines the independently admitted fixed-DFA population
with requested collection layouts. `schema_heap_layout.rs` contains std-only
checked arithmetic; `schema_hybrid_heap.rs` consumes those projections. Neither
module supplies a production descriptor or allocator/RSS guarantee.

The source model is bound to Rust 1.98.1's `alloc` implementation and its
vendored hashbrown 0.17.1, not an application dependency with the same name.
For nonzero element width `e` and lifetime length high-water `H`, the inspected
append paths retain at most `e*max(minimum(e),2H)` bytes; an untouched empty
vector retains zero. The minima are eight entries for bytes, four for widths
through 1024 bytes, and one otherwise. A growing old buffer contributes at
most `e*(H-1)` extra bytes; no old allocation is needed below the initial
minimum. Clears retain capacity, so current length cannot replace high-water.

For the state-shaped map, `State` wraps a two-word `Arc<[u8]>` handle and the
ID is a four-byte value. Pair stride includes its final padding: 24 bytes in
the 64-bit model, 12 in the 32-bit model. With lifetime map occupancy bounded
by `M`, bucket selection must allow `M+1`, since insertion can reserve before
discovering that a key already exists. A table requests
`align_up(pair_stride*buckets,max(pair_alignment,group_width)) + buckets + group_width`;
there is no final padding. The pinned group implementations use at most 16
bytes; that conservative width is not an assertion that the host selects a
16-byte implementation. Growth separately allows the preceding half-sized
table. Arc backings request `align_up(2w+payload,w)`; handle clones share the
same allocation. `StateBuilderNFA::to_state` copies directly to one Arc backing,
while its ordinary byte builder remains live.

For admitted population `M`, transition-entry envelope `T`, encoded payload
envelope `E`, fixed sparse/start counts `S/J`, epsilon-stack high-water `B`,
and per-state payload bound `V`, the serial-cache live envelope sums:

```text
4*(max(S,16)+J)
+ VecEnvelope(4,T) + VecEnvelope(2w,M)
+ VecEnvelope(4,B) + VecEnvelope(1,V)
+ StateMapEnvelope(M)
+ E + (M+2)*(3w-1)
+ 9
```

`M+2` allows pending and detached saved backings, including their headers and
per-backing alignment; it does not duplicate every map/vector reference.
Sparse storage has four vectors whose initial minimum capacity is four entries;
the `max(S,16)` term covers tiny NFAs. The separate recovery admission still
uses reported sparse lengths, not this requested-capacity term.
The final nine bytes fund `State::dead`'s separate temporary sentinel builder.
Each collection growth finishes before another begins, so the peak adds the
**largest** old-growth allowance, not the sum of all such allowances.
`from_dfa` preserves the independent recovery gate and automatic-capacity-upgrade
refusal. Its unavailable result is never a zero-byte cache.

The native allocator's existing controls additionally observe real append-only
byte/ID/handle vectors (30 rows), state-shaped maps (11 rows), and Arc slice
construction, two clones and release (13 rows). Inputs, hash-seed preparation,
shared keys and JSON reports stay outside allocation windows. Five exact map
rows exercise duplicate-triggered growth; clearing/repopulation must retain the
table without another request, and releasing the final owner must debit its
entire observed request. These are independent regression controls, not the
all-path derivation. In particular, the counter replaces logical live bytes
on realloc; hidden backend old/new overlap is covered only by the conditional
source allowance, not measured as physical residency.

The primitive word/group models and candidate projections remain conditional
on the pinned layouts and fixed, serial, non-cloned DFA-cache history. They
exclude other meta strategies, optional reverse-prefix NFAs, immutable programs,
constructor work/scratch, allocator overhead/reclamation, production owner
populations and target readiness. No generator patch, schema, engine limit,
workload, ceiling or funding coefficient changes in this component.

The final 2026-10-23 runs passed 419 focused secure-default tests, 430 feature
tests and all 110 serial linked controls. The latter linked all 261 callbacks
in both modes on native and three guest targets, retained all 300 captured
outcomes per mode and 861 repeated explicit evaluations, and exercised the
30/11/13 collection rows. Activity checks reject missing insertions, missing
positive allocations, map-repopulation no-ops and Arc clone/drop no-ops.
The adjacent 32-bit map request boundary fails when only its limit guard is
removed. Independent reviews found and rechecked the sparse-vector floor and
these activity controls. Strict workspace Clippy, formatting and both fixture
gates passed without new lint allowances; see the plan's detailed verification
record. These are executed component/regression checks, not a full workspace
test run, guest execution or complete schema byte/work qualification.

## Complete Fixed-DFA State Certificates

The qualification-only `schema_hybrid_graph.rs` checker computes the complete
start/byte/terminal-EOI closure of one fixed candidate DFA. It uses one fresh
cache, public opaque ID equality, and byte-only witness replay immediately
before each edge. IDs kept in sets are comparison keys, never stale transition
handles. Seeds cover unanchored, anchored and every configured per-pattern
mode, each crossed with no look-behind and all 256 look-behind bytes.

Every byte-reached state is expanded over all 256 bytes and one terminal EOI,
including match states. Terminal-only results are counted, not expanded until
a byte witness independently reaches them. The separate discovery/enqueue sets
make that promotion explicit. Any engine error, unknown result, nonzero cache
clear count, arithmetic failure, or exhausted exploration budget refuses the
certificate. The fixed exploration budgets are 2,048 opaque IDs and 8,388,608
engine API calls, including replay; neither is a schema-work limit or runtime
work descriptor. Incomplete exploration cannot produce a partial bound.

The pinned engine interns encoded-state bytes before insertion. Within one
no-clear epoch each ordinary encoding therefore has one stable opaque key;
start specialization changes tags, not encodings. The closure includes every
encoding produced by starts, bytes and terminal EOI. Under the independent
normal-recovery premise, clearing can reinsert a saved encoding but creates no
new ordinary encoding. For the same NFA/configuration correspondence, `R`
ordinary encodings support an indexed population bound of `R+3`, including
the three artificial unknown/dead/quit slots. This source lemma is not
language-equivalent DFA minimization or a claim that runtime caches never clear.

Observed completed certificates, 2026-10-23, native Rust 1.98.1:

| Candidate Set                               | Completed | Ordinary States |       API Calls |
| ------------------------------------------- | --------: | --------------: | --------------: |
| 23 emitted helpers, both primary directions |     46/46 |      Maximum 62 | Maximum 192,236 |
| Hostname primary forward                    |       1/1 |             251 |       2,217,139 |
| Hostname primary reverse                    |       1/1 |             128 |       1,087,110 |

The candidate engine/cache limits stayed unchanged. The never-match control's
literal 1,799 calls derive from one retained Fail start encoding, dead-state
successors, 514 seeds and two/three-call replay edges; they are not copied from
the checker. A real-engine terminal-first promotion test fails when enqueueing
is incorrectly gated on first discovery. Engine-refusal, clear-epoch, zero-
budget and independently exhausted state/replay budgets remain explicit cases.

The qualification-only constructor below now consumes these counts. Actual-meta strategy composition,
optional reverse-prefix NFAs, constructor/program storage, allocator/target
proofs, work and production multiplicities remain separate obligations. This
checker changes no generated compiler code, dependencies, serving paths or
production charges.

Final runs passed 441 focused secure-default cases and 452 feature cases,
including ten colocated controls compiled in both integration owners. Strict
workspace/all-target Clippy and formatting/fixture gates passed. Source and
quality review repairs were checked with restored-source reruns and sensitive
mutations, not just reviewer reports. The unchanged linked harness was not
rerun for this checker-only follow-up; no full workspace tests or guest
execution are claimed. See the plan's complete graph verification record.

## Graph-Bound Requested Heap

`HybridHeap::from_complete_dfa` derives the coarse population, independently
checks recovery, computes a completed graph and projects storage from the
**same** fixed DFA. There is no caller-provided population/receipt argument.
Unavailable recovery remains unavailable; a failed exploration is a distinct
typed error, never a zero-byte accepted cache or an unlabelled successful
fallback. The old coarse constructor remains independently usable.

Given coarse `M/T/E`, graph ordinary count `R`, stride `d` and per-state payload
bound `V`, the additional valid bounds are:

```text
M' = min(M, max(5, R+3))
T' = M' * d
E' = min(E, (M'+2) * V)
```

All additions/products are checked before minima. The five-state floor retains
normal recovery's sentinel/saved/pending allowance. `M'` is a lifetime high-water,
not the current epoch's length. Encodings do not change with memoization history:
ordinary insertion interns before allocation, and clearing/reinsertion moves
or clones the existing saved backing. Map growth moves handles, not pointed-to
payloads. The `M'+2` backing allowance therefore survives epoch changes; its
headers/padding and `M'+1` attempted map occupancy remain separate terms.

The unchanged eight-owner heap sum consumes the tightened population. General
controls require non-increase, including valid equality rows; designated real
fixtures require strict reduction. Native conditional requested-heap results:

| Hostname Direction | Coarse Peak | Graph-Bound Live | Old-Growth Extra | Graph-Bound Peak |
| ------------------ | ----------: | ---------------: | ---------------: | ---------------: |
| Forward            |     283,675 |          135,569 |           16,252 |          151,821 |
| Reverse            |     283,387 |          106,372 |            8,380 |          114,752 |

These are independently derived conditional envelopes, not measured physical
memory, a whole-meta total or a resource-ceiling adjustment. The literal native
never-match control is 916 live plus 116 old-growth bytes, peak 1,032. Its
unavailable-recovery and cleared-graph siblings pin both refusal dispositions.
The original layout, Arc, growth, recovery and complete-graph controls remain.
No generated code, source capture, dependency, serving path, production charge,
workload/engine limit or resource ceiling changed.

Final runs passed 453 focused secure-default and 464 feature cases. All six
new controls run in both existing integration owners, including the second
positive NFA whose literal peak is 1,357 native requested bytes. Clamping a
successful graph population to one fails that control with the other five
still green. Source/compliance and quality review, strict workspace Clippy and
format/fixture gates passed. The unchanged linked probe was not rerun and no
full workspace tests or guest execution are claimed. The plan records scope
and remaining whole-meta/allocator/work/target obligations.

## Fixed-Cache Reported-Capacity Proof

`ReportedCapacityProof::from_dfa` first checks the independent coarse heap and
normal-recovery model, then completes a graph for that same fixed DFA. It
accepts neither a caller-supplied report nor a cached graph receipt. Failed
recovery, incomplete/cleared graphs, arithmetic overflow, raw-ID exhaustion and
insufficient configured capacity have distinct refusal dispositions.

For final reported cache bytes `F`, word width `w`, transition stride `d`,
epsilon-stack bound `B`, and per-state encoding bound `V`, the checked
sufficient requirement is:

```text
S = 4 * max(4, 2*B) + 2*V
D = 4*d + 4*w + 4 + V
U = F + S + D
U <= configured cache capacity
(ordinary graph states + 3) * d < 0x07ff_ffff
```

Before a hypothetical first clear, memoized states are a subset of the
completed universe. Its scratch-free reported sum is bounded by `F`; `S`
independently bounds scratch, and `D` covers the prospective row checked even
before duplicate interning. `F` already includes observed scratch, so this
deliberately overfunds it. The pre-graph checked coarse envelope separately
bounds all reported owners and prospective growth: their smaller reported
lengths/entry charges cannot wrap before the completed report is read. The
graph observation therefore does not establish its own arithmetic premise.
This is a first-overflow induction: the checked coarse transition/handle
growth and map envelopes dominate the next reported row, while its payload
allowance retains both candidate and saved encodings. Every bounded prefix
therefore makes the next unchecked report/prospective sum representable.

All 46 emitted primary directions satisfy this condition under their unchanged
65,536-byte cache setting. Exact attempted/completed/proved membership sets
must agree; the largest required report is 23,784 bytes (Yandex reverse).
The two hostname directions require 38,744 and 24,608 bytes respectively,
also below 65,536. Native never-match and terminal-match controls independently
pin final reports of 255/458 bytes and sufficient requirements of 369/622;
their separate 32-bit arithmetic expectations are 207/378 and 305/526.

This is reported accounting, not collection capacity, allocator overhead or
physical residency. The theorem is limited to one fixed, fresh cache, covered
start modes/look-behind contexts, byte transitions and terminal EOI. Inherited
or reset capacity, cache cloning/config mutation, continued transitions after
EOI and raw-ID/progress-counter changes are not inferred from it. No capacity
clear excludes the efficiency give-up branch only in that context; actual
meta strategy construction, optional reverse-prefix storage, cumulative work,
fallback and target/allocation bounds remain separate qualification work.

## Optional Reverse-Prefix Cache Superset

The native qualifier now inspects structured `regex-syntax 0.8.11` HIR instead
of guessing from pattern text. It follows the locked engine's root-concat and
capture-removal rules, including smart-constructor normalization. Every proper
normalized child-slice prefix is included, even an assertion-only prefix or a
candidate the engine later rejects for prefilter speed or match ordering.
Inspection refuses beyond 4,096 HIR nodes, 128 levels or 64 prefixes; these are
native qualification budgets, not changed serving or input limits.

All 23 emitted helpers reconcile against literal per-owner prefix counts:
43 raw alternatives total. Each raw alternative uses reverse, capture-free,
no-shrink NFA compilation and the existing shared-start reverse hybrid config.
The checked coarse heap/capacity premise precedes every graph. All retained
alternatives must pass before a conditional owner envelope is returned; one
refusal in either order refuses the entire owner, never a partial maximum.
Only one selected prefix cache can coexist with the primary pair, so its
conditional peak is the maximum of alternatives, not their sum or a sum of
independently maximized subcomponents.

There is also a separate typed strategy-admission result. On the original
single-pattern HIR, `look_set_prefix().contains(Look::Start)` exactly matches
the engine's union-property predicate. `ReverseInner::new` rejects it before
constructing any optional prefix engine. It applies uniformly, not by bidder
or format name. Of the 23 helpers, 16 are excluded by that predicate, five have
no normalized concat and two retain five possible alternatives: OpenX helper 1
and Yandex helper 0. Their native conditional requested-peak envelopes are
4,615 and 81,464 bytes; sufficient reported requirements are 1,696 and 23,071,
both below the unchanged 65,536-byte capacity. These are not physical-memory
observations or complete schema scratch descriptors.

Hostname has three raw structural alternatives. Its unpruned hypothetical
cache proof fails with `Capacity(Exploration(Cleared))`. That failure remains
explicitly tested; neither cap nor exploration budget was raised. The separate
original-HIR predicate proves this optional owner cannot be constructed for
the start-anchored hostname pattern. Its disposition carries the structural
count but no accepted zero-byte cache envelope. Primary hostname caches retain
their separate earlier proofs.

Fifteen colocated controls cover normalization/captures, assertions, Unicode,
syntax and independent inspection limits, exact native-only dependency scope,
separate maximum axes, real component binding and both-order refusal. Removing
refusal propagation yields an accepted partial 2,203-byte envelope and fails
the owning control; the mutation was restored. Actual meta availability,
fallback, shared prefix-program residency, whole-cache composition, cumulative
work and allocator/target qualification remain open.

## Explicit Quit Admission

No cache clear is not proof that a lazy DFA cannot quit. The locked engine's
effective quit set can contain configured bytes or non-ASCII bytes introduced
by heuristic Unicode word-assertion support. Ordinary Unicode character classes
or text do not themselves enable that heuristic.

The new strict capacity constructor reads the same immutable DFA's NFA look
set and all 256 configured quit-byte flags, refusing either source with a
fixed typed reason. Only then does it invoke the unchanged coarse heap/recovery
and complete-graph capacity constructor. The original capacity-only API retains
its narrower meaning; a report or caller-supplied boolean cannot substitute
for examining the qualified engine.

All 46 emitted primary directions, both hostname directions and the 43 raw
helper-prefix alternatives use that strict path. Typed keyed refusals remain
visible and optional-owner maxima cannot swallow them. The optional hostname
owner still has its separate source-proven absence; its hypothetical unpruned
cache still refuses as previously recorded.

Six additional colocated controls cover Unicode versus ASCII word assertions,
both NFA directions, every explicitly configured byte, flag removal, real
engine `Quit` outcomes with zero cache clears, constructor ordering and prefix
binding. The all-byte sweep asserts 256 iterations. A combined fixture has both
an explicit quit flag and a coarse-heap arithmetic overflow: the original API
reports overflow while the strict view reports quit refusal before projection.

Public primary builders succeed under the audited NFA/config correspondence;
this supports availability if the engine constructs Core, not an observation
of private strategy selection. Capture storage and available one-pass scratch
remain eager even when lazy fallback work can be excluded. Actual-meta
composition, checked cumulative progress over the cache lifetime, strategy
retries, work and physical/target storage still need qualification. This
component changes no serving code, compiler patch, dependency, limit, funding
coefficient or readiness verdict.

## Fresh Core Auxiliary Requested Heap

Core's auxiliary cache constructors must be read through their wrappers.
`meta/strategy.rs::Core::create_cache` constructs capture storage and available
one-pass scratch eagerly. In `meta/wrappers.rs`, both PikeVM and backtracker
`create_cache` return wrappers containing `None`; their first `get` or reset can
initialize the underlying caches later. Therefore a fresh Core cache is not
charged the already-initialized Pike logical tables from the earlier section.
Those tables still require funding if their lazy initialization is reachable.

For word width `w` and the same admitted forward NFA's `S` total slots and `P`
patterns, the native qualifier derives this conservative auxiliary bound:

```text
E = checked(S - 2P)
captures = checked(w * S)
onepass = 0                         if E == 0 or E > 32
          checked(w * max(E, 4))     otherwise
auxiliary = checked(captures + onepass)
```

The captures use `vec![None; S]` with an exact requested length; the one-pass
vector starts empty and is resized once, so pinned RawVec's four-element floor
applies only there. Above 32 explicit slots the locked one-pass engine cannot
build. Within that limit the projection funds a candidate even when meta may
skip it or its construction fails. This is an upper bound, not a claim of
private strategy observation. It rejects malformed slot counts and arithmetic
or per-allocation layout overflow before accepting a result.

Fresh constructors have no old-growth overlap. This component does not admit
reset, cloned or inherited caches, and excludes the primary and optional hybrid
owners rather than assuming they cost nothing. Same-NFA integration evaluates
all 23 emitted helpers at both word widths, with exact keyed membership; their
maximum auxiliary bounds are 40 bytes at 32-bit and 80 at 64-bit. Hostname's
56/112-byte bounds conservatively include its non-buildable one-pass candidate.

The isolated native probe separately disables hybrid only in controls to expose
the auxiliary owners. Seven cases vary capture count, pattern count, one-pass
enablement and the 32-slot limit. Their receivers pin independent literal
construction bytes/calls, metadata and complete release. A capture-only case
must allocate even though `Cache::memory_usage()` reports zero, since that API
omits meta captures. Standalone fresh PikeVM/backtracker controls separately
distinguish engine cache creation from the meta wrappers' lazy initialization.
Original emitted validators keep their unchanged engine settings.

Neither requested native allocation observations nor word-width projections
prove physical native/guest memory. Shared programs, lazy fallback reachability,
counter lifetimes, work, target owners and complete descriptors remain open;
serving validation, funding limits and readiness verdicts are unchanged.

## Retained Lazy Fallback Requested Heap

Fresh Core construction does not fund later PikeVM/backtracker initialization.
The qualification model now includes all nine of their persistent vectors for
the locked `regex-automata 0.4.18`. Its scope is one fixed NFA/cache, created
fresh, used serially through meta's `earliest(true)` path and dropped, without
public reset, clone or reuse with another engine. The declared input maximum
bounds every full haystack across that entire lifetime, not just the last span.
This is a conditional requested-layout envelope, not a physical/native/guest
memory descriptor or a claim that actual meta strategy selection was observed.

Let `N,S,P,B,C,w` denote states, total slots, patterns, branch alternatives,
capture states and word bytes. Reuse the pinned collection model: `resize`
means one fresh resize with its allocation-specific minimum; `grow` means the
append/resize high-water capacity and old-allocation overlap envelope.

```text
Pike sets   = 4 * resize(4, N)
Pike tables = 2 * resize(w, N*S + max(S, 2*P))
Pike stack  = grow(3*w, max(1, B+C))
```

The four sparse arrays and two tables are distinct allocations: applying one
minimum to their combined logical length undercharges small NFAs. The slot
tables do not resize during same-NFA searches. A closure drains its stack and
deduplicates a state before expansion, bounding pending alternatives/restores
by `max(1,B+C)` without an input-length multiplier. Meta capture storage stays
with its separate eager owner, not counted again in the Pike fallback.

For the backtracker, the wrapper rejects earliest full haystacks above 128
bytes. Default visited capacity is 262,144 bytes. The largest eligible span is
`m=min(input_max,128,floor(2,097,152/N)-1)`; if the quotient is zero, not even
an empty search is eligible. Otherwise:

```text
bitmap = grow(w, ceil(N*(m+1)/(8*w)))
stack  = grow(3*w, max(1, (B+C)*(m+1)))
```

The visited guard permits each `(state,offset)` expansion once. Both the bitmap
and frame vector can retain a previous longer search's capacity. Even when
span 128 is ineligible, smaller legal spans remain funded instead of treating
the entire owner as absent. All scalar and per-allocation layout arithmetic
is checked. Sum the five retained owner terms (with the four/two suballocations
above); add the maximum of the three grow owners' old-allocation overlaps,
since these fixed-history growth paths execute serially.

The `3*w` frame allowance is a conservative profile for the two private Rust
enums' tag, four-byte index and word-sized offset. It is not a stable Rust ABI
promise or independent guest-layout proof. Native allocator controls exercise
the actual enum allocations; target layout/residency qualification remains open.

Colocated literal controls vary states, slots, patterns, branches, captures,
input/bitmap boundaries and word widths independently. Complete inventory
application requires all 46 keyed helper/width results, plus both hostname
widths, with no discarded refusal or partial maximum. The existing isolated
probe adds three real NFA cases, each driving both fallback engines through
empty, short-match, 128-byte miss and short-after-long phases. Compilation,
inputs, caller-owned capture slots and reports are outside measurement windows.
Receivers require positive allocations, literal verdicts and byte lengths,
bounded retained/peak requested storage, full release and allocation-free short
reuse. They reconstruct totals from every construction/search/release phase,
enforce per-phase peak bounds and reconcile positive requested bytes with
allocation events; a positive summary cannot conceal missing measurements.
Six negative receiver controls cover hidden over-cap peaks, erased calls or
requests, inconsistent totals, an entirely empty history and independently
smaller retained-storage caps. The final engines'
length-based `memory_usage()` reports must be smaller
than their actual retained storage, so dropping retained capacity is observable.

This owner increment changes no serving callback, dependency, compiler patch,
limit, resource charge or readiness verdict. Composition with eager captures,
one-pass and primary/optional hybrid owners, whole-schema scratch and work,
allocator overhead and physical/target storage remain separate obligations.

## Fixed Meta Cache Requested Heap Composition

`tests/schema_meta_heap/mod.rs` now composes the mutable owners for one
fixed, native-layout explicit cache: construction, serial
`search_half_with(...earliest(true))` calls, then drop. The input bound applies
to every full haystack over that lifetime. Reset, clone, engine rebinding and
concurrent mutation are outside the contract. All earlier component layout
assumptions remain conditional, including private frame size and requested
collection growth; this is not a physical-heap or guest-ABI certificate.

Let `E` be the eager capture/one-pass total, `F` and `R` the primary directional
hybrid envelopes, `B` the combined retained fallback envelope, and `Q` the
optional reverse-prefix peak (or source-proven absence):

```text
charged_live = E + F.live + R.live + B.live + Q
growth      = max(F.overlap, R.overlap, B.overlap)
peak        = charged_live + growth
```

Both fallback caches can remain resident after different searches; neither is
maximized away. Primary forward and reverse caches are separate owners.
Only one optional prefix alternative is installed, so its maximum peak is
charged in full rather than summing alternatives. Treating that entire peak
as continuously live is deliberately conservative: it is an upper allowance,
not a claim about actual retained bytes. It also funds optional-prefix growth
while a different owner grows. Each sum and the final overlap addition is
checked separately.

The constructor derives every component from the same emitted pattern and
declared lifetime input bound. For each immutable primary candidate it proves
non-quitting capacity before deriving the graph-bound heap. Compilation,
quit, capacity, exploration, recovery and arithmetic refusals retain typed
component/direction diagnostics; no failure becomes an absent zero or a partial
total. Prefix alternatives retain their existing all-or-refuse rule.

Locked `regex-automata 0.4.18` source correspondence: `Core::new` builds the
primary NFAs from the original HIRs (reverse disables captures), and
`Core::create_cache` supplies the eager owners. `ReverseSuffix` retains the
same Core and cache; `ReverseInner` adds a distinct `revhybrid` prefix cache,
not a replacement primary NFA. Its capture-flattened prefix selection does
not change Core's original programs. Both lazy fallback wrappers keep their
initialized cache in an `Option` across calls. The public forward candidate
omits meta's prefilter and start specialization; it is not configuration-
identical. Complete graph closure covers starts/transitions, while locked
`hybrid::dfa::add_state` adds specialization tags after state encoding and
interning. The heap argument uses that coverage, not an assertion that private
strategy selection has been observed or that prefilters allocate cache state.

The inventory control walks all 261 captured schemas and reconciles exactly
23 helper results with no discarded refusals. Optional prefix charges occur
only for the independently pinned OpenX helper 1 and Yandex helper 0. On the
current 64-bit native projection, the largest helper peak is 288,639 bytes
(Yandex); the separate hostname projection is 1,530,565 bytes, including both
conservatively funded fallbacks. These are formula results, not allocator
observations, acceptance ceilings or proof that the schema scratch budget
fits. No budget or engine limit changed to accommodate them.

Shared compiled primary/prefix programs, pool bookkeeping, inline storage,
constructor peaks, whole-schema transient state and hostname overlap,
production-owner multiplicities, work/cumulative counters and physical/target
qualification remain open. In particular, this composition does not make
arbitrarily many searches operationally safe: unchecked cumulative progress
counters in the locked engine need their own lifetime/work argument. No
usable descriptor or serving integration follows from this component.

## Fresh Start-Anchored Half-Search Heap

The hostname format hook's actual cache lifetime is narrower than component
20's arbitrary serial history: construct a fresh cache, make one whole-haystack
earliest half-search on at most 255 UTF-8 bytes, then drop it. The separate
`FreshAnchoredHalfHeap` qualification constructor preserves the broader model
and its 1,530,565-byte hostname result unchanged. Neither result is a physical
allocator or guest-ABI bound.

The narrower constructor admits only a single pattern whose parsed HIR has
the mandatory `Look::Start` property and whose forward NFA cannot match empty.
Line-only anchors, escaped carets and mixed anchored/unanchored alternatives
do not establish that property. Empty-matching patterns remain outside this
proof because the library can retry UTF-8 split matches within one public call;
they are not removed from the schema inventory or rejected by serving code.

Admission also checks the current configuration and effective engine flags:
LeftmostFirst, UTF-8 empty handling, all captures, byte classes, newline
terminator, unchanged NFA/hybrid limits, pool capacity one, hybrid available
and full DFA unavailable. Both primary hybrid builders must succeed. The
forward candidate must then pass the existing complete non-quitting capacity
and checked heap proofs. Syntax, configuration, build, quit, capacity, graph,
layout and arithmetic failures remain typed refusals, not partial totals.

Locked `regex-automata 0.4.18` correspondence establishes the exclusions:

- `RegexInfo::is_always_anchored_start` uses that exact HIR property. The
  strategy builder suppresses prefilters and all three reverse optimizations
  for such a pattern, leaving Core.
- With effective full DFA disabled, Core half-search uses its available forward
  hybrid. The default ordinary input mode is supported; no quitting byte or
  capacity/ID clearing can enter its fallback arm after the complete proof.
- The nonempty NFA bypasses UTF-8 empty-match retry. A fresh progress counter
  therefore records at most one full scan; endpoint arithmetic is checked.
  This byte-progress lemma is not a CPU-work or wall-time bound.
- The reverse cache is still constructed. Its sparse sets, starts, three
  sentinel transition rows/handles, one map key and shared dead-state backing
  remain funded. Search stacks/builders stay empty; its temporary builder and
  serial construction growth overlap remain conservatively charged.

The checked composition is deliberately conservative about phase overlap:

```text
peak = eager_cache_owners + complete_forward_peak + fresh_reverse_peak
```

The source-bound native 64-bit values are **112 + 151,821 + 5,009 = 156,942
bytes**. Fresh reverse consists of 4,112 sparse, 72 start-table, 384 transition,
96 handle, 116 map, 32 shared-backing and nine temporary-builder bytes, plus
188 bytes of serial growth overlap. These independently derived literals are
pinned in the actual hostname integration control, not fitted to observed
allocation peaks. The earlier provisional 5,181-byte reverse estimate used
looser starts/map allowances; it was corrected from the source layouts, without
changing the engine or any admission/resource limit.

Source witnesses checked against the locked dependency (SHA-256):

| Member | Digest |
| --- | --- |
| `src/meta/strategy.rs` | `ae5f22a6435c0e0d99f22a506e578fc95275e98fd0f7fe424ee6474730a09a5f` |
| `src/meta/wrappers.rs` | `79b1d63d40e4e4a1a59318f06849c97776687be0ec0de924a3dac5c986004f61` |
| `src/hybrid/search.rs` | `5b2b3f2017a4b71164c7b2c42110e5503675bc741661ba2c42761921b0302938` |
| `src/hybrid/dfa.rs` | `cd0e71ad86161c9a49c6023d9dde8e07895ad03a9586723c3fb1f1c14bdb7faa` |
| `src/util/determinize/state.rs` | `a850af545b7d0bd706f0bf72fdba504b6efdeea181763657109e10fef53aa88d` |

The native callback control exercises five literal hostname shapes twice,
including valid short/255-byte values, late-invalid and Unicode input, and
the 256-byte guard refusal. Accepted callback paths allocate positively, retain
zero requested heap and stay below the narrowed envelope; guarded calls
allocate nothing. Separate forward/reverse construction controls require
positive ownership and complete release. These finite observations corroborate
the source argument; they are not its derivation.

The result cannot be applied to reused/reset/cloned/rebound caches, different
input/search APIs, concurrent mutation or unproved configurations. Shared
programs/pools, constructor scratch, complete schema/format overlap, CPU work,
physical allocator overhead and target descriptors remain open. No serving
validator, funding coefficient, schema, workload or engine setting changes.
Being below 512 KiB here does not establish that the whole schema scratch fits.

## Native Bundle And Format Overlap (2026-10-27)

The existing conditional per-pattern bounds now compose at the concrete
generated-schema wrapper boundary. A native-only probe reports each wrapper's
actual compiled size/alignment; complete inventory and strict emitted-cache
reconciliation bind these layouts to their schema and all helper owners.
No union container size is substituted for a concrete wrapper.

The component conservatively sums every pattern owner's complete lifecycle
peak, the wrapper's inline bytes, and, where declared, one fresh hostname
heap peak plus the callback-local `meta::Cache` inline size. Sibling cache
storage remains funded during growth and format calls. Repeated synchronous
hostname calls release their local cache, so the callback term is not
multiplied by occurrence count. Unknown formats and arithmetic overflow
refuse; UUID contributes no search cache, not zero total stack/work.

All 261 main-reference schemas reconcile: 23 emitted helpers, two
hostname-bearing schemas and 247 empty pattern bundles. The largest combined
native cache component is **316,705 bytes**, below the unchanged **524,288-byte**
scratch ceiling. The separate seven-schema synthetic profile checks actual
compiled zero/nonzero wrapper layouts; independently unequal arithmetic and
format controls prevent omitted owners or partial sums from passing.

This is a necessary cache-component condition, not complete schema fit.
It excludes whole call frames (including UUID's fixed decode buffer), numeric
and other scratch, shared compiled programs/pools, initialization work, cache
population/multiplicity and physical/guest layout qualification. The component
supplies no usable runtime descriptor and enables no serving validation.
Schemas, compiler patches, engines, input limits and resource ceilings remain
unchanged. Commands, review scope and feature-lane results belong to
[plan section 22](../superpowers/plans/2026-10-22-explicit-schema-search-cache.md#22-complete-native-pattern-bundle-and-format-overlap).

## Unused Native Meta-Pool Bookkeeping

The explicit-cache API bypasses the pool; it does not remove its constructor
allocations. The locked `regex-automata 0.4.18` std implementation has three
requested-heap owners even when no pool acquisition occurs:

1. A boxed private pool with a factory, shard-vector handle, atomic owner ID
   and full `UnsafeCell<Option<Cache>>` inline slot. An absent cache avoids
   nested allocations, not the slot's storage.
2. The eager shard-vector allocation. Each shard is an empty
   `Mutex<Vec<Box<Cache>>>` wrapped in `#[repr(C, align(64))] CacheLine`.
3. The boxed factory's captured `Arc<dyn Strategy>`. Its payload is a fat
   reference-counted handle, separate from the box handle stored in the pool.

The source witnesses are the same pool/regex members recorded above. The
public outer `Pool` size is only a box handle, and `Regex::memory_usage()`
delegates to strategy reporting; neither supplies this bookkeeping charge.
Use actual compiled layouts for public constituent types and conservative
field-padding arithmetic, not an assumed private Rust struct layout. Exact
shard-allocation requests additionally depend on the pinned native standard
library's `RawVec::with_capacity` implementation, not the API's weaker
at-least-capacity guarantee.

The lifetime premise is no pool acquisition: owner storage remains `None` and
all shard stacks remain empty. Generated explicit searches and the hostname
callback use caller-owned caches instead. Regex cloning is used only to isolate
these allocations in a native diagnostic: it retains existing program Arcs and
creates a new pool and factory. Every additional real clone would need another
bookkeeping charge and is outside the proposed singleton ownership.

This component is separate from mutable cache bundles. Count one bookkeeping
owner per compiled regex singleton, with the shared hostname singleton counted
once even when multiple schemas use it. Constructor scratch for building the
programs, immutable program residency, outer inline/static storage, allocator
overhead, complete work, production ownership and guest layouts remain separate
obligations. No usable runtime descriptor or full engine qualification follows
from this native component.

On the pinned macOS/aarch64 native layout, the independently derived terms are
1,448 bytes for the boxed pool, 64 bytes for its one empty shard and 16 bytes
for the captured factory: **1,528 bytes per singleton**. Strict generated-
constructor inspection discovers 23 helpers across the complete 261-schema
inventory. Together with one shared hostname regex, their conditional pool
charge is **36,672 bytes**. The two hostname-using schemas do not imply two
hostname singletons.

The six scalar/layout controls vary each field, shard count, padding and owner
count independently, with literal expectations and checked-arithmetic refusals.
They cover all 24 field orders without declaring a private-layout mirror. The
native reconciliation builds originals outside the allocation window, then
clones and releases all 24 regexes: each clone requests exactly three
allocations totaling 1,528 bytes, and its release frees the same amount without
freeing programs still retained by the original. The public pool-constructor
control proves its factory remains uncalled; a subsequent acquisition is a
positive control. None of these observations proves absence of later pooled
acquisition. The unused-pool lifetime remains a required conditional premise,
not an inferred property of arbitrary future serving code.

## Native Immutable Regex Metadata

The locked single-pattern constructor retains one `Arc<RegexI>` and one
`Arc<RegexInfoI>`. `RegexI` contains the strategy's fat Arc handle and the
metadata Arc handle. `RegexInfoI` contains the public configuration, a vector
of per-pattern `Properties` handles and one union `Properties` handle. Strategies
that reference this metadata share its Arc; those references do not create
another metadata allocation. Literal-only strategies need not retain that
reference, but `RegexI` still retains the metadata owner.

`RegexInfo::new` starts with an empty vector and pushes one cloned property
record. The pinned native `RawVec` amortized-growth rule retains four slots for
that element size, not one. The per-pattern record and union record each own a
box. In pinned `regex-syntax 0.8.11`, `PropertiesI` contains only scalar fields,
and public `Properties::memory_usage()` returns `size_of::<PropertiesI>()`;
this particular getter is an exact payload size, unlike incomplete aggregate
engine reports. The two boxes and the four-slot backing array are distinct
owners. A configured prefilter may introduce additional shared ownership and
is outside this component; unsupported configuration must refuse, not receive
a zero charge.

Derive both private payload layouts from actual compiled public constituent
types using the same conservative field-padding rule as pool bookkeeping.
Each Arc additionally includes the pinned std `ArcInner` strong/weak atomic
header, payload alignment gap and tail padding. The header uses `repr(C)`;
this is not an assumed private Rust struct mirror. The initialized
`LazyLock<Option<Regex>>` cell has separate actual compiled inline/static
storage. Its bytes are not an allocation request or another program copy.

The component equation is:

```text
heap = regex_arc + info_arc + properties_vector + two_properties_boxes
component = heap + lazy_cell_inline
```

Apply that owner once per generated singleton and once for the shared hostname
singleton. Properties construction observations corroborate vector capacity,
box requests and release; they do not derive a whole-program bound from a peak.
Strategy allocations, forward/reverse NFAs, capture metadata, one-pass tables,
literal/prefilter owners, other generated static data, executable code,
construction scratch, physical allocator cost and guest layouts remain separate
obligations. Complete immutable-program residency and runtime descriptors remain
unqualified until those owners are covered.

The pinned native 64-bit result is:

| Metadata Owner | Requested Heap Bytes Per Singleton |
| --- | ---: |
| `RegexI` Arc, including header | 40 |
| Shared `RegexInfoI` Arc, including header | 192 |
| Properties vector, four handles | 32 |
| Per-pattern property box | 80 |
| Union property box | 80 |
| Heap component | 424 |

Actual compiled lazy-cell inline storage is separately 24 bytes per singleton.
For 23 emitted helpers and one hostname regex, the component is 10,176 heap
bytes plus 576 inline/static bytes. Equal pattern strings in different emitted
owners are not deduplicated by assumption. Source-conformance observations of
the property-owner sequence reconcile three allocation requests and complete
release for every singleton, totaling 4,608 requested property bytes. Separate
public Arc controls reconcile scalar, array and 64-byte-aligned payloads,
including header and tail padding; they do not allocate private ABI mirrors.

The metadata component and unused-pool component share the checked native
field-layout helper. Pool allocation/release still reconciles 1,528 bytes per
singleton and 36,672 in aggregate; extracting that helper changes no ownership
or requested-byte result.

## Native NFA And Capture-Metadata Trees

The locked NFA owns an Arc payload, a state vector, per-pattern start IDs,
boxed transition/alternate children and a capture-metadata tree. A cloned NFA
handle in PikeVM, backtracking, one-pass or hybrid engines references the same
tree; it is not a second copy. Byte-class and look-around bookkeeping are
scalar fields in the root payload and still need their inline layout charge.
The library's `memory_extra` counter is not an independent capacity bound.

Capture metadata has its own Arc, three outer vectors and one inner name-index
vector per pattern. Its constructor shrinks the outer vectors but does not
shrink those inner vectors. In particular, a one-group inner vector starts
with one absent name and later capture groups grow it; its retained storage
cannot generally be charged from length alone. The requested-layout model
uses the pinned construction history and conservative retained-capacity
envelopes, excluding temporary growth overlap from this retained-tree component.

The capacity premise is pinned native Rust 1.98.1 `Global` requested-layout
accounting, not allocator usable bytes. Every locked `Builder::build` creates a
fresh `Inner`; compiler reuse does not transfer an earlier state's capacity.
Final-state insertion is monotonic, so final state count is its insertion
high-water mark. The conservative append envelope therefore covers it even
without relying on the final shrink. Pattern starts are copied into a fresh
vector. Sparse/union children become boxed slices. The pinned standard library
either completes the requested shrink or takes a nonreturning allocation-error
path; a successfully returned NFA does not silently keep an unbounded earlier
request. These are constructor/source premises, not deductions from a public
slice getter or the public weaker shrink guarantee.

Only positively verified unnamed groups can exclude map backing allocations
and name strings. The source creates those map entries solely for named groups;
the map headers remain in the outer vector even when empty. Public pattern,
group, slot and name-iterator counts must reconcile before applying that rule.
The qualified native `std` feature closure uses `HashMap` headers; another
feature/ABI combination needs its own qualification.
Named groups remain an explicitly unqualified case for this component, not a
zero contribution or a new serving rejection policy. A capture-free reverse
NFA still owns an allocated empty capture-metadata Arc.

The retained-owner equations are:

```text
captures = arc + slot_ranges + map_headers + inner_vector_headers + inner_buffers
nfa = arc + state_vector + pattern_starts + boxed_state_children + captures
pattern_trees = forward + reverse + max(possible_prefix_reverse_trees)
```

The optional-prefix term follows the existing bounded structured-HIR candidates
and the locked start-anchoring exclusion. Its program maximum is independent
of its search-cache maximum. Independent source enumeration checks each
pattern's disposition, candidate count and maximum; it does not let a claimed
absence skip a candidate. The current population has 24 singletons, 48 primary
trees and five eligible prefix trees. Literal permutations with an interior
maximum separately pin the reduction, rather than relying on the constructor's
current candidate order. The sum is a construction-derived upper envelope,
not public introspection of the selected private meta strategy. Literal-only
strategies may retain a separate capture owner without an NFA; their owner is
outside this tree envelope and must be accounted before whole-program closure.
Unknown constructor shape, names, compilation, candidate coverage or arithmetic
must leave the component unqualified. Strategy allocations, one-pass/literal
owners, constructor peaks, work and physical/guest bounds remain separate.

## Native Retained One-Pass Tables

The locked meta wrapper embeds an optional one-pass DFA in its strategy. Its
NFA handle shares the existing forward tree. Only the transition-table and
start-ID vectors are additional heap owners in this component; inline DFA and
strategy storage, mutable search scratch and constructor temporaries are not
part of the retained-buffer equation.

The constructor must match the wrapper: leftmost-first, per-pattern starts,
byte classes and a one-MiB size limit. The existing `candidate_onepass` helper
pins that configuration. A raw candidate's success can conservatively be
funded even when the meta wrapper's capture/word-boundary heuristic would
skip it. A rejected identical constructor is an absent candidate, whereas
unknown configuration, NFA compilation, shape or arithmetic is an error.

```text
S = finalized DFA state count, including DEAD
K = finalized stride, including the extra match-information column
P = pattern count
retained_table_bytes = 8 * S * K + 4 * (P + 1)
structural_table_envelope = 8 * (NFA_states + 1) * NFA_stride + 4 * (P + 1)
```

`Transition` contains one `u64`; `StateID` is four bytes. The locked constructor
adds an aggregate start and a start for every pattern, maps each NFA state to
at most one DFA state, and separately adds DEAD. It uses the NFA's byte-class
stride, reusing the extra column for match information. Both finalized vectors
are shrunk before successful return. Pinned native Rust 1.98.1 `Global` either
completes the requested shrink or takes a nonreturning allocation-error path.
Thus exact finalized requested bytes follow from construction, not merely
from the public length-based memory report or a generic shrink guarantee.

Independent scalar controls vary states, stride and patterns separately.
Native allocation/release controls hold the shared NFA alive to avoid charging
its tree twice. These retained requested-byte equations do not establish
constructor peaks, allocator usable size, physical/guest memory, CPU work or
complete program qualification. No serving descriptor follows from them alone.

## Native Literal DFA Buffers

The native-only qualifier uses the already-locked `aho-corasick 1.1.4` public
DFA constructor and layouts. Adding its direct qualification dependency does
not enable defaults or alter the existing `perf-literal,std` feature closure.
This component describes retained DFA buffers, not the complete automatic
prefilter or literal-only strategy selected inside a meta regex.

For nonempty literal lengths `L`, let `P = len(L)` and `B = sum(L)`. The locked
noncontiguous NFA constructor creates four initial states and at most one trie
state per literal byte. The DFA's maximum state count is `S = B + 4` for an
anchored-only constructor, or `S = 2 * (B + 4) - 4` for both starts. No regex EOI
column is added: the maximum byte stride is 256. These are structural bounds,
not the observed state count of a particular literal set.

```text
transitions     <= 4 * S * 256
match_headers   <= size_of(Vec<PatternID>) * S
pattern_lengths  = 4 * P
match_ids       <= S * retained_append_bytes(size_of(PatternID), P)
```

The DFA's transition, outer match and pattern-length vectors are created from
their final lengths and filled without growing their outer allocations.
Pinned std's repeat and slice-copy constructors request those exact lengths;
the final shrinks do not need to recover NFA capacity slack. Inner match
vectors are appended and not shrunk; charge their monotonic
capacity envelope, including the minimum allocation, rather than only their
lengths. On the scoped LeftmostFirst path each terminal gets its ID once and
nonterminals copy one failure list once; IDs may recur in different rows.
Each row contains each pattern ID at most once. All products, native
allocation limits and sums are checked. More than 500 literals refuse this
component: the regex wrapper can select a different NFA owner beyond that
threshold, which this DFA model cannot certify.

Independent count/byte/start controls and fresh construction/release checks
cover duplicate literals and overlapping prefixes. The source-driven check
uses `regex-syntax`'s HIR properties and maintained prefix extractor, not a new
parser. Across 23 generated helpers and the shared hostname there are 17
start-anchored exclusions, three unusable-prefix cases and four finite-prefix
cases. The eight separately constructed start-mode candidates have a summed
122,784-byte buffer envelope and retain 117,632 observed requested bytes.
This sum exercises alternatives; it is **not** a catalog-residency charge.

The DFA's inline value, enclosing Arc, shared packed-pattern owner, Rabin-Karp,
Teddy implementation payloads, constructor peaks, work, physical allocation
and guest layouts are outside this component. A reconstructed public candidate
does not prove the original hidden matcher choice or owner population.
In particular, Rust's default representation does not guarantee a private
struct's maximum alignment or a sum-of-fields size ceiling. Natural-layout
estimates for private Teddy payloads cannot become qualification evidence
without compiler-bound or library-owned layout facts. Whole-engine and serving
qualification remain open.

## Remaining Gate

On 2026-10-28 AG approved the library-owned layout API. The
[approved contract](../superpowers/specs/2026-10-28-regex-resource-layout-api-design.md)
uses two shallow `Layout` queries for actual private payloads, without exposing
representations or modifying matcher traits. It passed independent source/design
review. Its separate engine patch implements those queries; integration and
regression verification are recorded in the
[execution plan](../superpowers/plans/2026-10-28-regex-resource-layout-api.md).
The API closes layout observations only, not hidden child ownership, complete program
bytes, construction/work or target readiness.

### Library-Owned Native Layouts: 2026-10-28

The separate `regex-automata 0.4.18` patch adds only read-only payload queries.
The exact original 107-member archive and portable patch replay bind the full
vendored tree; standalone manifest, lockfile, metadata and exact 25-feature
controls bind the engine actually compiled. Original compiler patches, schemas,
matching options, limits and historical captures are unchanged. See
[vendor upgrade instructions](../../vendor/README.md#regex-layout-queries).

Native conformance discovers all 23 emitted pattern helpers plus one hostname
owner from the complete 261-schema inventory. It independently rebuilds one
candidate per helper with the existing constructor options, binds each to its
pattern/configuration digest, and labels every row `reconstructed_candidate`.
These are not observations of initialized generated singletons. Compiler,
actual target and ABI come from build-time Cargo metadata, not a runtime
compiler's host triple. The report consumer refuses missing/duplicate owners
and roles, changed identities, invalid layouts and checked-arithmetic overflow.
Equal layouts or patterns never establish shared ownership.

Observed Rust 1.98.1, `aarch64-apple-darwin`, default test profile layouts:

| Object | Payload Bytes | Alignment | Shallow Arc Request |
| --- | ---: | ---: | ---: |
| Public Regex value | 16 | 8 | Not charged again |
| Regex root payload | 24 | 8 | 40 |
| Metadata payload | 176 | 8 | 192 |
| Selected strategy, two candidates | 536 | 8 | 552 each |
| Selected strategy, 22 candidates | 1,984 | 16 | 2,000 each |
| Separate public prefilter value | 32 | 8 | Not charged as a separate allocation |
| That prefilter's matcher payload | 144 | 16 | 160 |

The 24 candidates' three shallow Arc requests sum to 50,672 bytes. The separate
public prefilter's 160-byte matcher request is not evidence about a hidden
automatic prefilter. The original root/metadata component bounds remain intact;
the public Regex value is already inside the lazy-cell storage and is not
added again. A valid zero-sized payload still funds its Arc header, unlike an
unproved or missing private field.

Construction occurs outside the allocation window. One hundred repetitions
over all 25 accessible objects report zero allocation calls, allocated bytes,
live peak and retained bytes. A 257-byte allocating/releasing positive control
checks the same meter. Reports, assertions and serialization occur outside the
window. Isolated no-alloc, alloc-only and minimal-meta builds check API feature
boundaries; guest compilation references both queries but does not measure a
selected guest object.

These are partial requested-layout observations, not complete immutable-program
heap, pool/cache storage, constructor overlap, work, allocator usable bytes or
physical/guest fit. No serving callback or usable complete resource descriptor
is enabled. Final regression commands and review scope are recorded in the
[implementation plan](../superpowers/plans/2026-10-28-regex-resource-layout-api.md#verification-record).

AG approved designing and qualifying explicit search-cache ownership on
2026-10-22. The [approved compiler contract](../superpowers/specs/2026-10-22-explicit-schema-search-cache-design.md)
uses a validator-specific mutable bundle and the locked public meta-search API,
with constructor/pool bookkeeping, internal growth and custom-format ownership
accounted separately. AG approved that written contract on 2026-10-22; the
[implementation plan](../superpowers/plans/2026-10-22-explicit-schema-search-cache.md)
records the patch and controls. The third portable compiler patch now supplies
the opt-in API. After the separately authorized archive refresh (`c785b454`),
the normal workspace passed 66 API/AST, constructor/cache-lifecycle, compiler
and inventory tests in each of the secure and `insecure-bidders` feature
states, plus all 96 linked-artifact/provenance controls. Strict all-target
workspace Clippy, formatting and both raw-fixture gates also passed. See the
plan's verification record for commands, counts and the earlier isolated-run
limitations. Complete byte/work/target qualification remains pending,
resource descriptors are unchanged, and serving integration stays closed.

The published candidate failed the type controls above; the patched candidate
repairs those cases but has not passed the full gate. The full
`schema_engine_qualification` target intentionally does not exist yet.
The current inventory, native compiler and linked compiler-control targets
cannot discharge that acceptance checkbox. Remaining work includes:

1. Independently sourced instance compatibility cases, including remaining
   character-class and format boundaries; native literals alone are insufficient.
2. Disjoint generated-validator code/data, initialization and lazy-cache upper
   bounds. Complete artifact byte counts are observations, not residency bounds.
3. Conservative work/allocation derivations, cold/warm cache controls and exact
   single/aggregate limits, bound to generator/helper/source/target identities.
4. Reviewed C/I/V/K descriptors, full acceptance target and later funded runtime integration.

The original predeclared ceilings remain unchanged. Native smoke tests, source
sizes or compiler success alone are not an engine or WASM memory qualification.

## Initial Preparation Verification

Executed on the native host with Rust 1.98.1 on 2026-10-17. These commands use
the reviewed workspace lock; none is an actual WASM link or live-provider test.

| Command                                                                                                                                                                          | Result                                                           |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------- |
| `cargo test --locked --offline -p auctioneer-bidder --test schema_source_inventory --test schema_compiler_controls -j 2 -- --quiet`                                                  | 24 inventory and 7 compiler-control tests passed                 |
| `cargo test --locked --offline -p auctioneer-bidder --all-targets -j 2 -- --quiet`                                                                                                   | 198 passed under secure defaults                                 |
| `cargo test --locked --offline -p auctioneer-privacy --all-targets -j 2 -- --quiet`                                                                                                  | 407 passed; its debug benchmark is not performance qualification |
| `cargo test --locked --offline -p auctioneer-bidder -p auctioneer-runtime -p auctioneer-exchange -p auctioneer-core --all-targets --features insecure-bidders,conformance-suite -j 2 -- --quiet` | 2,076 passed, zero failed, two existing ignored tests            |
| `cargo clippy --locked --offline --workspace --all-targets --features insecure-bidders,conformance-suite -j 2 -- -D warnings`                                                    | Passed                                                           |
| `cargo fmt --all -- --check`; `git diff --check`                                                                                                                                 | Passed                                                           |
| `bash scripts/check-raw-json-fixtures.sh`                                                                                                                                        | Passed: existing 169 fixtures across 50 files                    |

The two ignored tests are the existing disclosure-model child-build check and
baked-config child helper. They were not run by this regression invocation;
no new ignored test was added. Existing GVL deletion and dependency
future-incompatibility warnings remain. The production normal/build dependency
tree contains neither `jsonschema` nor `uuid-simd`; the two shared lockfile
upgrades described above remain relevant to that tree.

Separate spec-compliance and code-quality reviews approved this preparation
slice only. Source captures, deployed validation behavior, numeric ceilings
and production registry construction are unchanged.

## Native Matcher Comparison: 2026-10-27

AG approved an exploratory comparison, not a production engine change. The
existing source-conformance target discovers the actual emitted regex helpers
from all 261 pinned schemas: 23 helpers plus the separate hostname pattern.
Both candidates use locked `regex-automata 0.4.18`, UTF-8, LeftmostFirst and the
existing 65,536-byte NFA limit. Meta retains its current strategies and explicit
cache API; direct PikeVM retains the same forward NFA captures and has no meta
prefilter. No schema, compiler patch, dependency, serving path or limit changed.

The 300 captured parameter inputs yielded 287 parseable JSON inputs and 13
malformed inputs, all labeled invalid. Their 834 string/key occurrences yielded
188 distinct candidate subjects. These are not observed validator invocations.
All candidates and a fixed adversarial panel were checked for boolean agreement;
none disagreed and every helper had both positive and negative reference
subjects. Timing selects the first three admitted subjects in each class plus
the fixed panel, producing 596 cases. Guard skips remain distinct from negatives.

Measurements used Rust 1.98.1 and the workspace release profile on macOS/aarch64,
Apple M4 Pro, 14 physical/logical cores. The host was not reserved or CPU-affinity
pinned. Seven samples alternate engine order. Timing uses separate fresh caches
with identical histories: first search, then repeated matching of the same
input. Hostname additionally measures its actual fresh-cache-per-call lifetime.
Program construction includes forward-NFA compilation. Constructor measurements
are fresh API calls after process/library initialization, not cold-process startup.
Inputs, AST expansion, JSON output and allocation accounting are outside search
timing; the allocator wrapper remains installed.

The initial pilot had 15 zero-median meta warm series with its one-operation
large-input batches. Before the second run, batching was changed only to
`clamp(65536/max(input_bytes,64),64,1024)`. The same corpus, subjects, guards,
engines, resource limits and seven-sample count were retained. The refined run
has no zero-median warm series. Single-operation first-search series remain
resolution-limited: 81 engine/case series have zero medians, so no first-search
speedup ratios are inferred. Neither run supplies a p99 or a latency bound.

Representative refined medians, microseconds per operation:

| Case | Current Meta | Direct PikeVM |
| --- | ---: | ---: |
| 9-byte hostname, warm cache | 0.009 | 0.262 |
| 255-byte hostname, warm cache | 0.305 | 6.385 |
| 9-byte hostname, fresh cache per call | 2.881 | 0.733 |
| 255-byte hostname, fresh cache per call | 18.131 | 6.787 |
| ADF helper 1, 65,536-byte repeated subject, warm cache | 4.501 | 507.594 |

Observed fresh-hostname requested-heap peaks, bytes:

| Case | Current Meta | Direct PikeVM |
| --- | ---: | ---: |
| 9-byte hostname | 11,264 | 38,352 |
| 255-byte hostname | 41,056 | 38,352 |
| 256-byte guard refusal | 0 | 0 |

All measured complete program/cache lifetimes ended with zero retained requested
heap. Individual first/changed/repeated phase metrics are incremental activity,
not complete cache storage. Native requested bytes exclude allocator overhead,
stack and physical residency. Smaller finite observations cannot replace the
existing conservative proofs or establish the 512-KiB schema scratch bound.

**Disposition:** no blanket PikeVM switch. Warm search favors Meta on these
examples; cheaper fresh-cache construction can favor PikeVM, and memory is not
uniformly smaller. Keep serving and the approved engine contract unchanged.
Next qualification work must prove actual ownership/work, including the format
hook's one-call cache lifetime, and complete the catalog/target descriptors.
This experiment does not close those gates or bidder catalog.

The [comparison plan](../superpowers/plans/2026-10-27-schema-matcher-comparison.md)
records protocol revisions and reproduction commands. The frozen
[pilot](../../crates/auctioneer-bidder/tests/data/schema-matchers/native-pilot-20261027.json) and
[refined observations](../../crates/auctioneer-bidder/tests/data/schema-matchers/native-refined-20261027.json)
retain raw samples, phase metrics, provenance and source/binary hashes. The
pilot is an intermediate-prototype record, not a published source checkpoint.
The existing source-conformance target checks both records' exact digests,
scope, membership, sample counts and lifetime release. This integrity check
does not promote historical observations to current qualification evidence.

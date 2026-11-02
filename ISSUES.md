# Known issues

Tracks review findings and their disposition. Resolved entries retain the
original problem, the implemented fix, and evidence so they are not mistaken
for outstanding work.

## Local Development And Test Ownership

**Approved first phase implemented (2026-10-30).** The
[development guide](docs/testing/development.md) separates focused edits,
package checkpoints and unchanged full CI acceptance. Normal edits retain
Cargo's incremental cache and avoid the application CLI graph unless that
graph is the subject. No new runner, feature, profile or dependency is added.

- All 36 pure cases from four integration binaries now live inline in their
  endpoint/config owners, with unchanged assertions. Cargo integration targets
  decrease from 243 to 239; no new target replaces them.
- Four copies of fixed config-store/envelope/binding setup share core's
  existing opt-in `test_support::config_store`. Helper units compile once.
  The 21 existing context-helper consumers and three direct handler consumers
  retain caller-owned config, independent expectations and separate boot state.
  Custom fault/counting stores keep their behavior through the shared fallback.
- Crate-wide rules require owner/domain placement and documented isolation
  before adding targets. They prohibit sharing independent oracles just to
  reduce files. Further consolidation is not a target-count goal.

**Intentionally unchanged:** broad discovery watches still protect bidder
add/remove detection; application source identity remains complete. Narrowing
either requires separate correctness controls and measured evidence. No
end-to-end rebuild speedup or M7 readiness is claimed by this first slice.
The [verification record](docs/testing/evidence/local-development-test-ownership-20261030.md)
records 1,929 passing scoped tests, nine existing ignores, strict affected-target
Clippy, formatting and source/fixture guards.

## Schema Probe Setup And CI Contention

**Native-only setup repair implemented (2026-10-31); Linux acceptance remains
open.** Four native-only test callers now request only native probes. Both
complete native/three-WASM link tests and all original observations remain.
The catalog request census falls from 44 to 23 across the two target binaries;
this is not a clean-build count or whole-CI speedup claim. One matched local
case passed in 138.91 s before and 48.23 s after, removing 89.25 s of unused
guest builds with native compilation unchanged.

The preceding Linux run failed two cases after 20-minute waits for the shared
compiler lock. The repair reduces unnecessary work while preserving the lock
lifetime and timeout; it does not prove a worst-case admission bound. Re-run
the full Linux acceptance matrix. If acquisition failures persist, profile
holder/waiter intervals before proposing scheduling or lease-boundary changes;
do not merely raise the timeout. See the
[verification record](docs/testing/ci-prerequisite-repairs-20261031.md#native-only-setup-follow-up).

**Once-owned revision implemented and locally verified (2026-10-31); new Linux
acceptance pending.** The subsequent Linux run at `34e4ff9` again failed only the two
lease-acquisition cases (333 synthetic passed; 688 source passed / 2 failed).
Mandatory per-PR owners now isolate the four adapter artifact experiments,
compiler layouts and selected native/guest probe. Remaining hermetic compiler
checks run serially, and five explicit Cargo targets no longer rerun in broad
scopes. Source feature-view checks still execute in secure-package and both
workspace feature scopes. Sharing default/explicit catalogs reduces build
requests from 23 to 17; allocation/cold/reuse/release subprocesses stay fresh.
The duplicate default-mode reference-refusal batch and secure disclosure-codegen
invocation are removed, not their unique assertions. Artifact caching remains
unapproved. Linux acceptance and a measured whole-CI speedup remain open.
The synthetic factory/rebuild workspace also now isolates unrelated native
schema generation without changing real discovery functions; unavailable schema
hooks and a child staging-feature refusal prevent it claiming schema acceptance.
Actual source and application-artifact tests keep the real schema path.

**Standalone dependency follow-up:** the
[partitioned Linux run](https://github.com/IABTechLab/auctioneer/actions/runs/41276006971)
at `e0d72aa` passed 12 of 13 test jobs, including every separated compiler,
application-artifact and config-rejection owner. The native job stopped at
three feature-export fixture failures, not a compiler-lease timeout. Those
standalone Cargo roots omitted the workspace patches and reviewed lock, so
they fetched the stock schema compiler, which rejects `cache = explicit`.
The sealed-response compiler fixtures had the same setup defect. Both now
inherit the root patches and lock, resolve offline, verify every resulting
dependency identity against the reviewed closure, and build locked. Original
runtime API checks, positive controls and exact compiler-refusal assertions
remain. An adjacent check also exposed one overlong Triplelift prose line;
only its wrapping changes. See the
[follow-up verification](docs/testing/ci-prerequisite-repairs-20261031.md#standalone-dependency-follow-up).
Complete Linux acceptance and a whole-CI performance comparison remain open.

**Importer evidence follow-up (implemented locally):** in the
[`1b13dc3` run](https://github.com/IABTechLab/auctioneer/actions/runs/41347146065),
the repaired factory/API step passed. The next source-inventory step reached
`bidders_import`, which reported seven passes and one failure: its committed
catalog check did not recognize the newer `tests/schema/semantics.toml` files.
The read-only checker now recognizes that exact evidence format through strict
Serde DTOs, retaining schema-owner matching, path/size limits and unknown-field
refusal at every document level. Authored and source-backed fixtures have
positive controls; wrong owners, misplaced files, missing fields, wrong types
and unknown nested fields have independent negative controls. No serving path,
captured expectation, provenance pin or import-publication rule changes.
Broader CLI verification also exposed a historical patched-lock baseline being
checked as current evidence and a harness golden retaining the pre-rename path
digest. The exact byte-pinned baseline now uses the existing frozen lane while
current-audit validation still refuses it. The test-only golden was recomputed
independently for the Auctioneer path; no historical bundle was rewritten.
See the [verification follow-up](docs/testing/ci-prerequisite-repairs-20261031.md#importer-evidence-follow-up).
Complete Linux acceptance remains open.

**Bidder conformance and grouping follow-up (2026-11-01):** the
[`14a5fff` run](https://github.com/IABTechLab/auctioneer/actions/runs/41349649200)
passed the importer step, then failed 13 Triplelift protocol cases. The earlier
metadata wrap refreshed the factory fixture but missed its companion Rust
manifest checksum. That checksum now matches; no manifest or capture changes.
A complete three-package secure-default sweep then exposed four stale test
expectations and one real grouping-memory overrun. Candidate-decoder controls
now distinguish the unchanged 64-container limit from the independent node
limit and preserve valid sibling bids. The compiler guard matches the already
approved native runtime dependency and explicit guest staging. Grouping starts
each impression-index vector at one slot rather than four: the unchanged
256-KiB, 13,952-identity fixture falls from 2,189,096 to 1,854,248 bytes against
the original 2,097,152-byte allowance. No cardinality, workload or charge changes.
The repaired bidder targets pass 32 and 193 cases; the complete exchange suite
passes 648 with one existing ignore. The final complete three-package rerun
passes 3,768 test executions with zero failures and 11 existing ignores. See the
[verification follow-up](docs/testing/ci-prerequisite-repairs-20261031.md#bidder-conformance-and-grouping-follow-up).
Full Linux acceptance remains pending.

**Compiler warning-policy parity follow-up (2026-11-01):** in the
[`7a90b87` run](https://github.com/IABTechLab/auctioneer/actions/runs/41354575403),
12 of 13 test jobs passed. The native job stopped at a positive compiler
control whose unused `inspect` function violates CI's
`CARGO_BUILD_WARNINGS=deny` setting. The exact failure reproduced locally with
that setting; the previous local sweep had not enabled it. The control now
references its function, and the shared standalone-fixture runner explicitly
enforces the same warning policy even outside CI. A regression test first
failed on the missing explicit setting. Both complete fixture suites pass
10 and eight cases under secure and development features, with no filters or
ignores. Production code, compiler-refusal assertions and warning policy are
unchanged. See the
[verification follow-up](docs/testing/ci-prerequisite-repairs-20261031.md#compiler-warning-policy-parity-follow-up).
Strict Clippy passes in both lanes. A broader workspace attempt was interrupted
during compilation to avoid exhausting local disk space; it ran no tests and
does not count as acceptance. Full Linux acceptance remains pending.

**Cloudflare cached-tool installation follow-up (2026-11-01):**
[run 41381814843](https://github.com/IABTechLab/auctioneer/actions/runs/41381814843)
stopped before packaging because Cargo refused existing `worker-build` and
`worker-codegen` binaries. The cache includes executables but not Cargo's
installation records. The installer now uses `--force`, retaining worker-build
`0.8.5`, `--locked` and every existing
packaging/size check. A dependency-free, two-binary local Cargo fixture
reproduces the collision without `--force` and installs both binaries with it.
This verifies Cargo's cache-recovery behavior, not the vendor package or hosted
readiness. The next Cloudflare CI run must verify actual packaging. See the
[verification follow-up](docs/testing/ci-prerequisite-repairs-20261031.md#cloudflare-cached-tool-installation-follow-up).

**WASM compiler-fixture dependency regression (2026-11-01):** the fixture
change in `1c6af43` used `quote!` while `quote` remained a native-only test
dependency. All three guest bidder-lint jobs therefore failed with `E0433`.
The existing workspace-managed dependency now lives in common dev-dependencies;
its build dependency remains unchanged. The three original all-target WASM lint
commands pass locally, without exclusions or lint allowances. Normal dependency
graphs for the host and all three guests compare unchanged, and `Cargo.lock`
is untouched. Native fixture suites pass in both feature lanes; source-isolation
checks also pass. See the
[verification follow-up](docs/testing/ci-prerequisite-repairs-20261031.md#wasm-compiler-fixture-dependency-follow-up).
These are compilation and scoped native results, not whole-CI acceptance.

**Native accounting and remaining compiler owners (2026-11-01):**
[run 41386614042](https://github.com/IABTechLab/auctioneer/actions/runs/41386614042)
passed every separated compiler/artifact/config owner, all three guest lint
jobs and vendor packaging. Native workspace tests stopped at three Axum
accounting references that still assumed 1,584 baked-config bytes. The renamed
cookie identities produce 1,592 bytes; production accounting already funds the
extra 1,024 bytes per admitted auction. Test references now reflect that growth,
with an independent inline delta control and no changed ceiling or workload.

The audited `generated_settings` and `insecure_bidders_snapshot` suites now
have one mandatory explicit compiler-job owner instead of repeating four fresh
child build trees in each parent feature scope. All assertions, child feature
modes, workspaces and fresh measurement processes remain. Explicit strict
Clippy and complete parent/child report checks remain mandatory. Both broad
workspace invocations use `--no-fail-fast`; failures still fail the job while
later targets report their results. Cache/reuse mechanisms are unchanged.
See the [verification follow-up](docs/testing/ci-prerequisite-repairs-20261031.md#native-accounting-and-remaining-compiler-owners).
Full Linux acceptance and a matched end-to-end speedup remain unverified.

## Environment Section Separators

**Hard cutoff implemented (2026-10-30).** Current Auctioneer-owned CLI, test, browser
and CI variables use double underscores between sections and retain single
underscores inside field names. Readers, child-process callers, workflow guards
and current examples change together; no legacy-name fallback is added.
The repository-wide rule is in [AGENTS.md](AGENTS.md). External tool variables,
artifact markers and original historical evidence remain unchanged.

## Post-Merge Catalog Review

**Implemented (2026-10-30):** generated metadata, bindings and serving-registry
publication now execute in WASI as well as native tests, under secure and
development features. The source-manifest oracle discovers control populations
without a bidder-name list; removing the native-only predicate fails the actual
guest assertion. Compiled candidate factories remain distinct from publication.

- Coverage also includes the two non-routing native-only consumers: the
  existing cold-allocation census runs in WASI under both feature lanes, and
  an actual Cargo-build matrix checks baked aliases against native and guest
  definitions using synthetic owners. Removing either filter fails its own
  consumer test. Production selection rules and resource limits are unchanged.
- The original Git-object provenance walk already covered the six reported
  original-source bidders. Supplemental public-source verification and
  independent tree/blob/length/digest rejection controls strengthen that walk;
  local inventory hashes cannot stand in for the Git checks.
- Borrowed and owned Serde JSON raw values share one private marker. Removed
  the nonexistent owned spelling; both capture and flatten-boundary guards
  remain, with independent guard-removal controls.
- Substantive workflow shell bodies now live in
  `.github/workflows/scripts/<workflow-stem>/`. The 20 existing bodies retain
  their original bytes after the Bash shebang; the new guest catalog script
  requires nonempty passing results in both feature lanes. Short commands stay
  inline. At AG's direction, the Rust workflow-configuration snapshot target
  and its unused YAML dependency were removed; actual CI checks remain.
- Active developer instructions distinguish online locked acquisition from
  offline cache reuse and use the current SDK pin. Historical evidence keeps
  its original revision and bytes.
- The remaining forward-looking snapshot-test instruction in the historical
  catalog plan is retired explicitly. Checkout/cache/Rust setup action majors
  are aligned throughout the workflows; no configuration snapshot test returns.

The [scoped verification record](docs/testing/evidence/m7-review-followups-20261030.md)
does not close WASM resource fit, hosted acceptance or SDK source availability.
The latest inspected hosted Cloudflare job ran no steps and reported the GitHub
account billing/spending limit; that remains external to these repairs.

## M7 Qualification Scope And Completion

**Current delivery boundary (AG approved 2026-10-30):** the
[controlled-native design](docs/superpowers/specs/2026-10-30-m7-controlled-native-completion-design.md)
and [plan](docs/superpowers/plans/2026-10-30-m7-controlled-native-completion.md)
replace proof-before-activation with tested native reservations, enforced
schema/input/work limits, native accounting/headroom and actual 14-flow handler
acceptance. Complete worst-case source/ABI schema certification belongs to
M7-R0; guest readiness remains blocked. The native implementation now publishes
all 14 selected flows through generated discovery, with new definitions inactive
by default. Generated-App controls cover runtime activation, alias-base vetoes,
privacy before schemas/credentials, invalid parameters, missing host settings and
signed sync. Cold allocation/stress and the actual native envelope are separate
acceptance owners. The CLI requires current source conformance and all four
native producer operations before reporting native acceptance; it never reports
formal qualification, WASM readiness or full-catalog completion from these runs.
The [verification record](docs/testing/evidence/m7-controlled-native-20261030.md)
records commands, outcomes and retained limitations. Earlier checkpoint
statements below describe their recorded commits, not the new acceptance rule.

Two target-layout construction models were prepared before this correction.
Focused synthetic NFA controls pass; they do not establish real target layout,
construction work or full engine certification. The full compiler-fact join
remains R0 work, not an activation prerequisite under the new native boundary.

Independent-review follow-up (2026-10-30):

- **Historical priority-publication blocker, now implemented under the native
  amendment.** At checkpoint
  `da1d3038`, 262 bidder packages declare 17 executable factories but only four
  public definitions across all features (three with secure defaults). Only
  one of the 14 priority flows has a public definition; the other 13 remain
  unroutable. Direct protocol tests, compiler probes and approved wave-design
  receipts do not close that gap. Nonzero shipped-set bounds, funded registry
  and parameter admission, response-decoding integration and actual handler
  onboarding remain required before publishing those definitions. The fixes
  below do not count as priority-flow delivery.
- **Review corrections in native completion.** Compiled aliases retain their
  ancestry in info output and cannot use host alias replacement to bypass a
  base's activation/sync veto. Sealed candidate decoding finishes rejected
  extension tails on the original ledger, never hides a later standard-field
  error, and preserves parser-owned structural refusal provenance. Native
  evidence cache leases explicitly unlock at their existing guard boundaries,
  including when a child inherited a duplicate descriptor. Constructor failures
  release the same guard, and unproven child cleanup quarantines the cache from
  both future readers and producers. No workload,
  deployment ceiling, decode charge or frozen reservation was raised.
- **The claimed ungated fake is not present at this checkpoint.** The public
  test-support module already requires `test` or `test-support`; normal
  application dependency graphs for all four adapters leave that feature off.
  Test builds intentionally enable it. No production factory is registered for
  the fake, and this review does not add a new serving feature or exception.
- **WASM test wiring repaired.** Default guest unit builds no longer import
  native-only schema-format dependencies. Native AST/TOML/provenance controls
  have explicit target gates, while portable bidder units stay enabled. APS's
  native capture helpers and their imports follow the same native conformance
  boundary. The bidder's complete target set passes strict Clippy on all three
  guest targets. CI now selects those own-crate test modules explicitly;
  transitive application builds did not compile them. No serving dependency,
  feature, workload limit or resource charge changed. Native bidder units,
  registry and isolation controls pass 1,187 tests with nine existing ignores;
  all 124 workflow guards pass. The missing CI command first failed its guard,
  and native affected-package strict lint and formatting pass. Guest results
  are compilation/lint evidence, not guest execution or resource qualification.
- **Info alias scans repaired.** Info validation and rendering reuse one cached
  ordered metadata vector, replacing the former list cache rather than adding
  a second resident copy. Actual binary-search comparisons are counted at
  512/0, 448/64 and 384/128 compiled/effective-alias populations. The latter
  includes 64 inherited and 64 runtime aliases. These controls require two probes
  per admitted alias across validation/rendering; totals do not exceed 1,152/2,304
  and individual probes obey the independent logarithmic limits. Late and absent keys first
  failed against the old scan. Ordered alias/activation joins and rendering
  stay below 2,048 comparisons, including a fully disabled 512-identity view.
  Projection/encoding allocation controls, with an allocator positive control,
  fit the unchanged one-MiB manifest reservation. These are info-path controls,
  not complete shared auction/sync index or resource qualification.
- **Native-fit diagnostics repaired.** Valid, independently rederived native
  rows that exceed demand/headroom now fail with `bidder_resource_native_fit`;
  malformed captures still fail with `bidder_resource_input`. Refusal and
  accepted report shapes are unchanged.
- **Minor guards/documentation repaired.** The local harness bidder now asserts
  exactly one request before reading it. Vendor documentation lists the
  explicit-cache patch's six changed/one added members and all three replayed
  schema-compiler patch digests.
- **Two review claims need qualification.** The dirty-tree observation predates
  checkpoint `7a291806`, which committed those changes. Synthetic registry and
  selector tests already exercise independent public bases/schema owners sharing
  one factory; absence of a particular long-tail implementation does not remove
  that architecture coverage. Its actual protocol migration remains separate.
- **M7 completion remained open at that checkpoint.** Priority definitions could not be published
  around unqualified schemas. Production C/I/V are zero while new schema/catalog
  admission is disabled; these are not qualified costs for the planned wave.
  The existing K witness does not supply them. Nonzero bounds,
  funded serving admission and all 14 priority-flow/onboarding results remain
  M7 requirements at that checkpoint. The approved native amendment above now
  changes the certification owner without waiving actual native acceptance.

**Selected schema probe linking implemented; serving acceptance remains open
(2026-10-29).** Qualification expanded to general library internals and all
261 catalog schemas even after M7 narrowed to 14 priority flows. The
[shipped-schema amendment](docs/superpowers/specs/2026-10-29-m7-shipped-schema-qualification-design.md)
requires a generated per-adapter/feature closure of the planned wave plus
existing published bindings, aliases, references and helpers. Disabled shipped
state remains funded. Actual artifacts and bounded probes complement the
required reachable owner/work/scratch bounds; observations do not replace them.

Unused schema/backend expansion belongs to the first affected M7.7 wave or
M20, with a mandatory pre-publication qualification trigger. M7-R0 keeps target
fit/platform facts; M7.6 keeps deployed observations and publisher acceptance.
An owner actually needed by the M7 shipped set cannot be deferred. Stop adding
nested-literal introspection solely for hypothetical future reachability.
The amendment, roadmap and execution priorities record ownership; no missing
acceptance target, serving integration, onboarding producer or priority flow
is marked complete by the correction.

The source-conformance producer now derives 15 secure-lane and 16 development-
lane parameter schemas from all 14 planned flows plus current publications,
including ancestry and explicit local schema-free bindings. Both use four
generated regex helpers plus the shared hostname regex. Thirty-four focused
controls pass in each lane; source/generated membership is not linked,
resource or serving qualification. The [cohort and remaining gap table](docs/bidders/schema-qualification.md#selected-source-and-generated-membership)
keeps those acceptance obligations explicit. No full-source count or historical
resource capture was rewritten.

The subsequent standalone probe checkpoint links those 15/16-schema cohorts
on native and all three guest compiler targets, checking source-bound staged
inputs, artifact markers, actual compiler/features and native dispatch. Each
cohort executes 236 pinned cases plus mixed-input sequences from eleven
captured families, with per-family verdict counts and an explicit uncovered
schema list. Six synthetic controls cover selection/generation, witness drift,
empty/duplicate owners and staging isolation/refusal. The existing full-source
probes remain present and retain their original population.

This is not application linkage, complete semantic coverage or C/I/V resource
qualification. Probe overhead remains included. At that link-only checkpoint,
native section sizes and guest execution were unclaimed; the global Wasmtime
45.0.0 differed from the pinned 48.0.2. The later WASI observations below use
a separately verified pinned runtime without changing that global installation.
The [probe boundary](docs/bidders/schema-qualification.md#selected-schema-link-probes)
keeps application reconciliation, remaining schema coverage, owner/work bounds,
funded registry/schema admission and all-priority onboarding actionable in M7.

The selected regex retained-heap candidate now composes six existing ownership
terms for five helpers. Four start-anchored helpers exclude automatic literal
matchers; the remaining end-anchored single-byte prefix needs one scalar
matcher Arc, proved through pinned constructor/sharing paths and the existing
public type-layout API. No nested-matcher vendor patch is required for this
cohort. Native reconstructed heap is 100,792 bytes, with 120 inline bytes
separate, versus 47,944 retained bytes observed in each release probe cohort.
Constructor-only observations are phase-separated and repeats must allocate
nothing. These values do not establish initialization peak/work or target fit.

This closes a native regex ownership-composition checkpoint, not C/I/V
qualification: non-regex schema state, code/data residency, release/guest layout
correspondence, constructor/search work and funded serving admission remain
open. The [retained-heap boundary](docs/bidders/schema-qualification.md#selected-regex-retained-heap)
records the premises; no observed value is installed as a descriptor.

The selected cache checkpoint reuses existing lifecycle/bundle derivations and
compiler-reported wrapper slots, including required callback storage. Native
observations now exercise selected captured inputs and existing owner-authored
adversarial workloads across independent cache populations, carrying live
storage between cold/repeated/changed-input phases
and checking complete release. Empty-object-only observations no longer stand
in for that captured history. These remain conditional native candidates;
profile/guest correspondence, work and complete-frame bounds, other schema
state and funded serving admission are still open. See the
[cache scope](docs/bidders/schema-qualification.md#selected-validation-cache-heap).

The next checkpoint adds real selected-cohort WASIp1/P2 command observations
under the pinned Wasmtime, with artifact/runtime identity checks, independent
captured verdicts, invocation-local sequence reuse and fail-closed process/output
handling. CI supplies the checksum-pinned runtime before existing consumers.
The unknown target was still pending at that checkpoint; the later integer-ABI
result below closes its standalone invocation gap. These observations do not
provide guest memory/work bounds or serving qualification. See the
[WASI scope](docs/bidders/schema-qualification.md#selected-wasi-command-execution).

The follow-up inline-layout consumer also queries each WASI compiler's actual
cache and generated wrapper sizes, checking independent helper counts and
32-bit word identity. This is not a native-to-guest heap conversion: nested
layouts, heap/work bounds and application correspondence
remain open. See the [inline-layout boundary](docs/bidders/schema-qualification.md#selected-inline-cache-layouts).

Both selected cohorts now also execute on the unknown-unknown guest through
fixed integer exports, preserving raw and reused-cache classifications. The
exact three binding imports trap on call; a paired control proves that calls
cannot obtain default values. The 15/16-schema four-target matrix and all 243
shared compiler/protocol controls pass, with two loopback-enabled reruns for
offline-reference checks. No serving, provider or memory/work qualification is
inferred. The [integer-ABI boundary](docs/bidders/schema-qualification.md#selected-integer-abi-guest-execution)
retains application reconciliation, resource descriptors and onboarding in M7.

## Unqualified Schema Artifact Staging

**Selected semantic execution checkpoint verified (2026-10-30).** Bidder-owned
strict TOML declarations add 139/147 schema-derived rows in secure/development
lanes and separately import 14 pinned source expectations. All 153/161 rows
execute on native and all three guests with fresh and reused caches. Every
selected schema has rows; exact identities, shapes, counts, schema/data digests
and source attribution are checked. The original 236 captured inputs retain
their separate table, counts and provenance. Neither authored expectations nor
accept/reject booleans are compiled into the bare guest's input table.
This closes these missing behavioral controls, not C/I/V or serving qualification.

**Actual application retention and compiler-layout checkpoints verified
(2026-10-30); serving/resource acceptance remains open.** All four adapter
applications build in both feature lanes. Object/DWARF and WASM parsers inspect
mapped data and actual function references for every selected validator; names
in unrelated debug/custom sections are not retention evidence. Spin component
inspection includes its nested core modules. These staged applications still
refuse before serving initialization: their sizes are not final admitted-artifact
residency or a production qualification claim.

Eight compiler-layout queries also pass: the selected dependency graph supplies
the exact regex-syntax and regex-automata feature sets on native and all three
guest targets. Compiler-emitted layout facts replace guesses about private
syntax/NFA/one-pass owner slots. These queries are static inspection, not program
execution or complete heap/work/frame bounds. Final application correspondence,
nonzero C/I/V descriptors, prepaid admission and priority-flow onboarding remain
in M7. No engine, schema, workload limit or resource ceiling changed.

**Approved non-serving build path implemented; application/resource acceptance
remains open (2026-10-29).** Selected validators are generated privately from
discovery plus the frozen priority-wave union, using verified output-local
schema snapshots and the existing converter/engine/options. Snapshot edits
rerun build validation and refuse conflicting bytes; the compiler cannot reopen
different original bytes beneath the recorded identity. The exact input and
build sidecar contains an explicit unqualified state, not zero-valued costs.

Normal and staged controls separately exercise the build-state expectation,
public registry, App configuration and resolved resource-input boundary. Staged
construction refuses before initialization or gate installation. Source-bound
membership controls cover both feature cohorts without treating generated
metadata as linked-artifact proof. The [staging boundary](docs/bidders/schema-qualification.md#non-serving-application-staging)
keeps actual application/package inspection, owner/work bounds, nonzero
descriptors, funded integration and final shipping-artifact rechecks open in M7.
All normal/staged admission pairs pass in secure and development lanes, with
four admission and two exact metadata controls per combination. Build-support
and workflow suites cover that boundary. All three staged adapter guest
library checks compile. The real CLI export exits 2 with
`unqualified_schema_artifact` and zero stdout bytes; no funded profile is emitted.

Dependency review caught the unqualified runtime enabling JSON `float_roundtrip`
in normal builds. The internal Cargo feature now selects both optional
dependencies and unconditional unqualified mode. A real four-application normal
dependency-graph test failed before isolation and passes after it. There is no
second environment switch and no change to normal production parser features.

## Response Decode Receipt

**Reserved wire-key expansion closed in bounded typed decoding (2026-10-30).**
The pinned JSON dependency can interpret a private object-key protocol as an
encoded JSON tree. Both the ordinary and shared bounded decoders previously
expanded a nine-node array from a three-node wire object. Two reproducer tests
failed before the key-only guard; all 249 runtime unit tests pass afterward.
The guard checks decoded map keys before the target seed can interpret them,
including escaped and nested spellings. It preserves ordinary string values,
enum tags, numeric map keys and the existing counter algorithm. No extra
preflight walk or input-tree allocation was added.

Lexical node counting remains syntactic, not permission for an arbitrary later
Serde allocation. Preflight-plus-ordinary-Serde call sites still need a separate
private-protocol audit; this repair does not claim that broader admission proof
or complete Task 15. The exact-output guard now checks the actual source/seat
seeds' associated types, rather than aliases disconnected from those visitors.

**Post-admission limit replacement closed (2026-10-30).** The existing receipt
now also freezes its attached node limit when the first decode attempt begins.
Later ownership moves cannot widen or reduce that limit, including after syntax,
typed, structural or unfunded refusal. Before admission, exchange attachment
keeps its existing behavior. Eight new public-boundary controls first produced
one pass and seven assertion failures; all eight pass after the repair. The
bidder/response/fake/isolation regression has 1,223 passing tests and nine
existing ignored tests. No receipt field, response layout, byte/node charge,
raw-body access or decoder registration changed. Full Task 15 remains open.

The test-only source visitor now constructs the standard `BidResponse` and
`SeatBid` directly. Equal layouts of look-alike private structs were insufficient
to prove that a later vector conversion would reuse funded storage. The new
exact-type control failed before this repair and passes in that regression.
This closes that ownership mismatch without admitting the prototype to serving.

**Standard-wrapper replay closed; proprietary decoding remains open
(2026-10-29).** A response-owned atomic claim now precedes the standard bounded
parse. Failed attempts and budget replacement cannot refund it, and concurrent
borrowers cannot retain multiple funded trees. Compiler assertions require the
one-byte claim to fit the former response layout rather than borrowing parse
margin. First-attempt diagnostics, the sealed body and transport identity stay
unchanged. Fourteen synthetic response-envelope controls pass after eight
expected pre-repair failures; scoped independent review found no correctness
issue. This does not complete M7 Task 15: the aggregate source/candidate ledger,
qualified envelope DTOs, bid-local typed isolation and their owning bounds and
funding remain required before protocol promotion.

The runtime prerequisite now supports one lexical source admission plus one
borrowed source deserialization and a separate, non-cloneable aggregate typed
candidate ledger. Failed candidates keep their byte/node visits spent, raw
capture remains forbidden in typed candidates, and source depth is checked
before target allocation. This does not expose a sealed response body or admit
an arbitrary DTO. The normal parser keeps its existing counter and wrapper
layouts, node semantics and depth behavior. All 244 runtime tests and strict
library Clippy passed; independent review found no blocking issue. Three new
literal-wire exceptions test duplicate/escaped keys rather than ordinary JSON
construction. Session ownership, complete DTO allocation/work qualification,
bidder integration and original-deadline acceptance remain open M7 work.

The next test-only session prototype preserves successful bids separately from
one-byte failure records, so primitive-only bad-candidate arrays do not allocate
one full `Bid` slot per failure. Fifteen owning controls cover shared visits,
whole-source rollback, duplicate extension-key ambiguity, 63/64/65 depth and
allocator measurements with the existing 20% guard. Source/candidate scan
allowances include borrowed-raw UTF-8 validation. These scan credits are not an
instruction bound. No serving decoder or qualified descriptor is registered;
sealed-response integration and enforced prepayment remain open.

## Impression Extension Presence

**Resolved (2026-10-29).** Canonical `imp.ext.prebid: null` was discarded by
the typed ingress model, preventing a bidder from distinguishing it from an
absent field when an ASCII-case alias was also present. The existing
presence-aware `JsonField` now preserves missing, null and object values.
Unruly rejects competing spellings before admitting any outbound request;
single spellings remain accepted and the caller's request stays unchanged.
OpenX's request-presence characterization now asserts preservation; its
separate response-extension presence gap is still explicit.

Regression coverage includes typed round trips, split materialization,
canonical-null conflicts and both impression orders. The OpenRTB, exchange,
Unruly, OpenX and expired-diagnostic suites passed. Compiler assertions compare
the former and current field, optional extension and enclosing impression
layouts; native and all three guest checks passed without changing a charge,
factor, workload limit or resource ceiling.

## Bidder Evidence Run Ownership

**Source/compiler conformance added; acceptance remains open (2026-10-30).**
A second fixed producer builds and executes two exact source/compiler membership
tests. Version-2 records bind the target, environment, executable and retained
nonzero dispatch; these are Rust cases, not parameter-fixture or migration
counts. The consumer independently rechecks current reference inputs and holds
an existing-only shared cache lease throughout artifact/source/binding validation.
Production cache leases remain exclusive. Missing storage is not created by
consumption, FIFO lock files are refused without waiting for a writer, and
receipt checks precede cache access. The source-prerequisite and
consumer-lease omissions both failed their real-producer regressions before
repair. Fresh-target support and deterministic owner-manifest context were also
exercised before their fixes. Full migration, semantics, resource and onboarding
acceptance remain unavailable; no artifact is promoted by this producer.

**Preparation checkpoint implemented; migration acceptance remains open
(2026-10-28).** The CLI now begins exclusive run scopes and executes one fixed
reference-integrity producer. Its receipt binds the caller's run identity,
current binary/build/source inputs and complete inventory comparison. The
consumer independently checks requirements, membership and current inputs;
missing, failed, foreign and stale evidence cannot complete preparation.

Consistently repinned input/requirement mutations, a forged completed receipt
over source drift and same-input prior-run replay have real CLI controls.
Tracked code/lock changes and untracked additions are separately exercised in
a disposable Git fixture; that is helper coverage, not a real-workspace CLI
mutation. Claim-before-work is source-reviewed; non-overwrite is tested, but
there is no runtime inspection-count trace.

Quality review found that a bare `manifest.json` parent became an empty
directory path. The exact CLI regression failed before the repair; all four
filesystem helpers now use the validated absolute path, and all 13 evidence
tests pass. A broader pre-repair regression passed 796 tests across the CLI
library, binary, source/import/wave and help targets. Two existing ignored
child probes were driven by their passing parent tests. Final strict CLI
Clippy, maintenance compilation and formatting pass. No historical captures,
compiler patches, dependency pins, schemas, serving behavior or limits changed.

This is not full M7 Task 16. Reference preparation grants no behavioral,
schema, registry, response-decoder, resource, priority-contract or onboarding
qualification. Evidence-consuming `check` and `run` still exit one with zero
migrated identities and explicit missing acceptance requirements. See the
[command boundary](docs/bidders/evidence.md) and
[execution record](docs/superpowers/plans/2026-10-28-bidder-evidence-run-scope.md).

## Supplemental Reference Snapshot Exclusion

**Resolved (2026-10-28).** CI declares three pinned reference checkouts, but
the source-snapshot test still expected two and `.gitignore` omitted the
supplemental checkout. Exercising all three reproduced
`source_identity_nonfile` rather than a stable application-source snapshot.

The exact supplemental path is now ignored. The guard derives every declared
reference path, rejects an empty or duplicate set, and still verifies tracked
and untracked vendor code plus refusal of an unknown nested repository. Its
red/green execution passed after the exact-path fix; snapshot production logic,
source pins and historical evidence are unchanged.

## Shared Native CLI Controls

**Implemented prerequisite; evidence tooling remains open (2026-10-28).**
The existing bounded process and I/O controls now have one explicit library
owner. Qualification consumers retain their caps, cleanup, fixed errors and
wire formats; maintenance builds remain independent of embedded config. All
31 original unit controls remain inline, and the existing test-support observer
preserves the HTTP assertion without a new feature.

Native harness, provider harness and provider fixture digests include the moved
source namespace. Each independent omission probe fails only its intended
guard; byte-identical restoration and final green controls are recorded. The
complete CLI run passed 848 tests, including historical evidence and real boot
parity. This does not implement or certify missing migration producers. See
the [execution and verification record](docs/superpowers/plans/2026-10-28-native-cli-control-sharing.md).

## Priority Bidder Shared Services

**Implemented; priority acceptance and provider readiness remain open
(2026-10-27).** AG approved bringing floor-only conversion and coherent
credentials forward into M7. Full response FX and external rate feeds remain M9.

Floor conversion borrows an operator-owned, expiring, strictly validated map
through the existing configuration/context path. The table admits at most 256
rates, rejects decoded duplicate keys and USD overrides, and normalizes lookup
codes on the stack. Missing/stale rates, invalid floors, overflow and positive
underflow refuse rather than inventing a rate. Independent amount/rate controls
exercise both context constructors and aliases; allocation controls include an
owned-map positive control.

Credentials use one bounded versioned username/password/tracker bundle from the
existing default secret binding. Generated `optional_secret_reference` fields
reject invalid references at config admission with fixed diagnostics. Pure
bidder eligibility, privacy, construction funding, call capacity and the original
deadline precede I/O. Late results cannot restart that deadline. The exchange
retains one sequential bundle; cancellation and permit refunds are exercised
through an actual suspended auction, not only the reader in isolation.

The independently derived parse-heap envelope is 86,528 bytes; cold observations
peak at 49,823 bytes. The logical 128-KiB construction bank also covers the two
bounded read/value buffers, encoding scratch and fixed layout/binding reserves.
It enters the complete exchange descriptor once per admitted auction, and core
adds its existing ownership costs. Removing this bank from runtime admission
makes the focused zero-poll funding control fail. Exact-funding helpers must use
this complete descriptor, not the scrub-only component. This is not SDK/provider
RSS qualification; workload limits, ceilings and historical evidence stay intact.

The current default shared-library run passed config/core/exchange/runtime
65/277/532/235 tests. Focused exchange admission, construction and provenance
targets passed 9/17/10/6; the real compiled secret-reference consumer also passed.
The rate owner also survives the actual core HTTP preparation path: removing
that handoff makes its nonempty fake-bidder observer fail with `MissingSnapshot`.
The initial four-test HTTP checkpoint passed. The default and insecure shared-library
runs, all nine explicitly driven source checks, allocation/response suites,
three WASM compilation checks and real CLI-versus-boot comparison are recorded
in the linked checkpoint. A subsequent paid HTTP conformance increment joins
the request secret registry, SDK bounded loader, exchange and fake constructor
using genuinely prepaid phase receipts. It covers independent requested keys,
alias/base rotations, missing/malformed values, privacy-denied identities with an
allowed sibling, and unfunded zero-read refusal. Its explicitly synthetic row
and existing test-only entry point do not claim generated-App/config-extraction
or hosted-provider qualification. Generated-schema survivor admission remains
open. The expanded HTTP target passes all eight tests in both feature lanes;
receipt removal and delayed conversion preflight each fail their independent
control, and the restored paths pass again.
These results do not complete protocol-candidate promotion, schema/work/cache
qualification, all 14 composed priority flows, absolute R0 fit or live delivery.
Sharethrough and Rubicon public definitions remain empty while those gates are
open. Rubicon's consumed-field conversion admission now follows AG's approved
canonical-ingress boundary: shared typed decoding and legacy privacy rejection
remain authoritative, while borrowed checks reject malformed consumed supply
chains, legacy EIDs, DSA, first-entry multibid maxbids, rewarded signals, GPID and
slot data before credential selection or sink work. Shadowed values and otherwise
filtered impressions are covered; parameters, targets, media and floor failures
retain their distinct impression-local scope. No unused framework DTO decoder,
new required-field rule or enum restriction was added. The source differences
are recorded in the owning bidder manifest. This scoped repair does not close
generated-schema admission, resource qualification, destination approval or all
14 composed flows. Verification and remaining readiness gates are recorded in the
[shared-services plan](docs/superpowers/plans/2026-10-26-priority-bidder-services.md).

The subsequent Rubicon routing decision is now implemented: AG approved exactly
four explicit HTTPS regions, with no default, geo selection or failover. Invalid
requested settings cannot fall back to a valid base or read credentials/reserve
calls. All four hosts and requested/base resolution have local controls. Public
definitions stay empty and prospective activation stays opt-in; this resolves
the routing decision, not schema/resource acceptance or live authorization.

## Exploratory Schema Matcher Comparison

**Comparison complete; engine qualification remains open (2026-10-27).**
The existing native source-conformance target compares all 23 emitted schema
helpers plus hostname using the locked library's current Meta and direct
PikeVM APIs. The two optimized runs preserve the same 188 captured candidate
subjects and 596 selected cases; larger second-run batches address clock
resolution, not a changed resource threshold. Matching agrees on the finite
corpus and stress panel. Warm matching and fresh-cache setup show different
tradeoffs, and PikeVM's requested peak heap is not uniformly smaller.

No production engine switch, schema/limit change, funding descriptor or platform
qualification follows. Complete source-bound work/cache/ownership and target
proofs remain required. Exact historical records are protected by the existing
source-conformance target and formatting exclusions; see the
[results and limitations](docs/bidders/schema-qualification.md#native-matcher-comparison-2026-10-27)
and [implementation plan](docs/superpowers/plans/2026-10-27-schema-matcher-comparison.md).

## Hostname Cache Lifetime Qualification

**Conditional native component implemented; complete qualification remains
open (2026-10-27).** The format hook uses one fresh explicit cache per call,
not an arbitrary retained search history. A separate start-anchored, nonempty
half-search proof now requires exact configuration, both primary constructions
and complete non-quitting forward capacity before excluding unreachable
fallback/search-grown reverse storage. Fresh reverse construction remains paid.

The native 64-bit requested-heap envelope is 156,942 bytes; the broader
1,530,565-byte serial-history result is preserved for its different lifetime.
Independent arithmetic/refusal controls and actual callback/construction
allocation/release checks cover the narrower result. This changes no production
engine, schema, limit or budget and is not a physical/guest, CPU-work or
whole-schema scratch proof. Shared programs, initialization and complete
catalog/target descriptors remain required. See the
[derivation and scope](docs/bidders/schema-qualification.md#fresh-start-anchored-half-search-heap)
and [component 21 plan](docs/superpowers/plans/2026-10-22-explicit-schema-search-cache.md#21-fresh-start-anchored-half-search-qualification).

## Unused Schema Pool Bookkeeping

**Conditional native component implemented; complete engine qualification
remains open (2026-10-28).** Explicit search caches bypass, but do not remove,
the locked matcher's eager pool bookkeeping. The new source-derived model
includes the full absent-cache slot, one aligned empty shard and the factory's
fat Arc capture. Native clone/release controls reconcile 1,528 requested bytes
per regex, or 36,672 across 23 generated helpers and one shared hostname regex.

This is fixed singleton storage, not per-auction mutable cache storage. It is
conditional on no pool acquisition or additional real clones and is not a
physical/guest, complete constructor, immutable-program or CPU-work bound.
No usable descriptor or serving admission was enabled. Complete program,
initialization, work, production ownership and target qualification remain M7
work. See the [derivation](docs/bidders/schema-qualification.md#unused-native-meta-pool-bookkeeping)
and [component plan](docs/superpowers/plans/2026-10-22-explicit-schema-search-cache.md#23-unused-native-meta-pool-bookkeeping).

## Library-Owned Regex Layout API

**Approved API and native consumer verified; complete qualification open (2026-10-28).**
AG approved a library-owned layout API rather than further private-ABI guesses.
The [written contract](docs/superpowers/specs/2026-10-28-regex-resource-layout-api-design.md)
defines two shallow read-only queries using Rust's `Layout`: actual regex and
prefilter payloads, without new engine features, trait-vtable methods or stored
bookkeeping. A separately replayable patch to locked `regex-automata 0.4.18`
implements the queries without another workspace crate. The
[execution plan](docs/superpowers/plans/2026-10-28-regex-resource-layout-api.md)
records missing-API red, owner-local controls and source/consumer verification.
The full source-conformance target passes 406 checks, including default/explicit
261-schema native and three-guest builds, all 300 captured parameter outcomes,
source replay and a nonallocating 24-regex/one-public-prefilter candidate census.
Guest checks establish compilation, not observed guest layouts. Build-time
compiler/actual-target metadata replaces the runtime-host attribution found in
review. No private ABI mirror or matching-trait extension was introduced.

This is not a complete heap report. Arc allocation headers,
hidden prefilters/literal children, shared ownership, initialization/work and
guest/physical qualification remain separate obligations. Only the engine's
source/checksum pair changes in the root lockfile; versions and features stay
unchanged. No schema, workload limit, historical artifact, accounting coefficient
or serving descriptor changes.

## Published Registry Failure Caching

**Current-roster fallibility verified; complete funded join remains open
(2026-10-29).** Generated serving construction returns a typed error after
preflighting borrowed published rows, before any factory call. Explicit
duplicate registration preserves the incumbent. Core caches successful and
failed construction and propagates immutable failures distinctly through
configuration, auction/info/usersync and CLI validation. Runtime-default and
cached-status paths cannot turn an established registry failure into success.

Non-initializing cache inspection may precede funding; cold construction may
not. The existing lazy boundary is retained, with no eager `configure_app`
construction, new serving indexes or candidate activation. Qualified resource
descriptors, precharged startup, full-catalog alias/activation integration and
process-isolated failure lifecycle acceptance remain [Task 7](docs/superpowers/plans/2026-10-16-m7-bidder-catalog.md#task-7-validate-and-cache-the-complete-shipped-join).

The affected configuration/core, bidder/registry and exchange sweeps pass
1,430, 1,160 and 642 tests respectively; existing ignores remain explicit in
the linked checkpoint. CLI push validation preserves `registry_build_failed`
separately from repairable `invalid_effective_config`. Fixtures now construct
their intended fake bidder directly instead of depending on replacement.

## Immutable Schema Metadata Storage

**Native metadata component implemented; complete program qualification remains
open (2026-10-28).** The qualifier now separately accounts for the two shared
metadata Arcs, four retained property-vector slots, two scalar property boxes
and compiled lazy-cell storage. Its source-derived native result is 424 heap
bytes plus 24 inline/static bytes per singleton, or 10,176 heap bytes and 576
static bytes across 23 generated helpers and one shared hostname regex.

Public property construction and aligned Arc allocation/release checks
corroborate the derivation. The common checked layout helper also preserves the
existing unused-pool result. No engine, workload, ceiling, compiler patch,
funding coefficient or serving behavior changed. Strategy allocations,
inline engine storage, literal/prefilter owners, initialization/work
and physical/guest qualification remain M7 work. No complete program descriptor
is enabled. See the [derivation](docs/bidders/schema-qualification.md#native-immutable-regex-metadata)
and [component plan](docs/superpowers/plans/2026-10-22-explicit-schema-search-cache.md#24-native-immutable-regex-metadata).

## Immutable Schema NFA And Capture Storage

**Native retained-tree component implemented; complete program qualification
remains open (2026-10-28).** The new
source-derived qualifier includes the NFA Arc, state/start vectors, boxed
transition children and every unnamed capture-metadata owner. Empty reverse
capture metadata still has a nonzero Arc. Named captures and unknown shapes
refuse qualification rather than contributing zero.

Forward/reverse trees and possible prefix-reverse trees are covered separately;
the program-prefix maximum is not its search-cache maximum. The 24-singleton,
53-tree cohort yields a 113,960-byte construction envelope, with independent
candidate coverage and order-sensitive maximum controls. This remains a
conditional native requested-byte proof, not full program, constructor/work,
physical/guest or serving qualification. See the
[derivation](docs/bidders/schema-qualification.md#native-nfa-and-capture-metadata-trees)
and [component plan](docs/superpowers/plans/2026-10-22-explicit-schema-search-cache.md#25-native-nfa-and-capture-metadata-trees).

## Retained Schema One-Pass Tables

**Native retained-buffer component implemented; complete program qualification
remains open (2026-10-28).** The existing qualifier now covers transition and
start-ID buffers from the identical configured constructor, including DEAD,
the match-information stride column and aggregate/per-pattern starts. The
shared NFA is not charged again. Per-buffer native allocation limits and all
arithmetic are checked through the existing layout helper.

All 24 independently discovered singletons reconcile: 19 raw candidates retain
14,520 requested bytes, bounded by a 22,360-byte structural candidate envelope;
five identical constructor rejections contribute no one-pass buffers. Actual
construction/release checks keep the original NFA alive. This is not a complete
strategy, constructor/work, physical/guest or serving descriptor. See the
[derivation](docs/bidders/schema-qualification.md#native-retained-one-pass-tables)
and [component plan](docs/superpowers/plans/2026-10-22-explicit-schema-search-cache.md#26-native-retained-one-pass-tables).

## Retained Literal DFA Buffers

**Native DFA buffer component verified; whole-engine qualification remains
open (2026-10-29).** The source-derived qualifier now bounds fresh Aho-Corasick
DFA transitions, outer match rows, pattern lengths and every inner match row's
append capacity. Counts, total literal bytes and start modes have independent
controls, with checked arithmetic and native allocation-limit refusals. A new
native-only dependency edge references the already-locked library without
changing its features or adding a package/version/source.

Every emitted schema pattern plus hostname is inspected through the maintained
HIR/prefix APIs. The eight separately constructed finite-prefix candidates
retain 117,632 requested bytes within a 122,784-byte buffer envelope. These
alternative-constructor sums are not a catalog charge or proof of the original
hidden matcher choice. Packed-pattern, Rabin-Karp and Teddy owners remain
unqualified; private `repr(Rust)` natural-layout estimates cannot establish
their payload bounds. Initialization/work, guest/physical proofs and serving
admission remain open. Seven focused controls and the source-driven constructor
check pass. Under the shipped-schema amendment, derive the actual closure
before extending future-only matcher proofs. See the [component derivation](docs/bidders/schema-qualification.md#native-literal-dfa-buffers).

## Historical qualification evidence formatting

**Resolved (2026-10-26).** Formatting changed byte-pinned payloads/profiles and
checksum-table layout, causing three CLI evidence integration failures. AG
subsequently chose to restore the affected records instead of adding a migration.
The 36 affected files were restored to their exact bytes at
`040e3b8d5090cff13cc1ddeb435c2a1a2aa7d515`; unrelated formatting remains.

All five existing evidence integration tests now pass, including the seven-packet
census, 25 indexed payloads, and original profile bindings. No digests, historical
claims, or integrity checks changed. `.prettierignore` excludes frozen evidence,
profiles, and other byte-pinned source assets. The unimplemented record/view spec
was removed; no duplicate store, migration command, or new verifier is needed.

The resumed CLI run also exposed a formatter-changed historical harness ownership
table. Its plan was restored to the same source revision and excluded from
Prettier. The previously unreached CLI harness, ownership, reference and boot
parity targets passed 8, 5, 2 and 5 tests respectively. These are distinct passing
targets, not a claim of a new whole-workspace or provider qualification run.
The same restoration/exclusion was applied to the historical M3b plan after its
roster boundary failed the documentation gate. The gate and its planted rejection
controls remain unchanged.

## Current status

The table distinguishes completed repairs from open investigations. Repairs
landed in successive commits, not one commit.
`a1cbeb2` published the initial user-sync repair on 2026-10-13;
`afe569a` added GDPR macro guards, `6a70ebf` added key-selection guards,
and `b49c823` closed further review gaps. `72bae69` added the consent macro
case on 2026-10-14; the October 14 follow-up below completes its direct
endpoint coverage and closes the request-root defect. The JSON-fixture
cleanup predates these repairs.

AG approved continuing M7 catalog implementation on 2026-10-19 with R0
readiness blockers explicit. This changes sequencing, not their disposition:
absolute fit, SDK facts and local allocation/stack proofs remain open.
Catalog development still requires its own schema, descriptor and resource
acceptance; no relative result qualifies a target.

The October 27 [field-metadata derive checkpoint](docs/superpowers/plans/2026-10-27-openrtb-field-metadata-derive.md#compiler-qualification-checkpoint-2026-10-27)
qualifies compiler-derived request descriptors and native/guest structural
agreement. The disclosure compiler uses compiled metadata; the source scanner
is removed after guard migration and privacy/resource acceptance. The
[final execution record](docs/superpowers/plans/2026-10-27-openrtb-field-metadata-derive.md#scanner-removal-and-final-acceptance-2026-10-27)
distinguishes this completed refactor from the still-open bidder-schema,
catalog-join and M7-R0 readiness gates.

| Finding                                                                                        | Status                                                                                                                           | Resolution                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| ---------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Cross-crate source imports bypass normal test/build API ownership | Resolved; compiled ownership and layout guards | The October 27 refactor replaces handwritten module paths and sibling implementation imports with ordinary module trees and opt-in compiled test-support APIs in existing crates. Configuration DTOs and native build contracts belong to auctioneer-config-schema; serving edges select config-model without native generators. Core owns handler-private wire controls and conformance interfaces; the canonical fake, TCF encoders and allocator counters have one compiled owner, with allocator installation retained per executable. Generated OUT_DIR includes and automatic production discovery remain intentional. Rust emission uses quote/syn. Structured source guards, all 239 target/feature identities, native/guest probes, the ignored model guard, secure/development privacy suites, 104 child-build cases and real CLI/boot parity are verified in the [execution record](docs/superpowers/plans/2026-10-27-standard-test-modules.md). No workload limit, budget coefficient, historical capture or SDK pin changed; this is not complete M7 delivery or hosted qualification. |
| Supplemental immutable-source check is not selected by CI | Wired and locally verified; hosted execution not claimed | The workflow now checks out the immutable supplemental revision separately, binds AUCTIONEER__BIDDER__SUPPLEMENTAL_SOURCE_DIR and executes its owner-local source probe. Shared CI controls derive the repository/revision from discovered provenance, without a bidder-name switch, and reject missing, suppressed, failed, zero-test or ambiguous runs. All 118 local workflow controls pass; the actual supplemental/main checks pass separately (1/8) and together under the development feature (9). The new generic environment name replaces AUCTIONEER_APS_SOURCE without a fallback; dated execution records retain their original commands. Captures, source pins, serving eligibility and workload limits are unchanged. Hosted CI execution and complete M7 promotion are not claimed. |
| Source HTTP routing still required separate vendor TLS-mapping evidence after the HTTPS-default decision | Native transport selection and source checking implemented; serving cutover open | The importer records typed scheme-only selections beside immutable metadata witnesses for 122 identities across 77 owners: 78 bases, 27 own-endpoint aliases and 17 inherited aliases. Every field and record presence is checked against fresh source reconstruction; non-metadata records refuse even in supplemental-only manifests. Existing definitions, factories, activation, settings, original witnesses and captures are unchanged. Four authored current-manifest input bindings and the pending wave's three routing labels are renewed without changing protocol cases or historical evidence. This is not URI-safety approval, a new HTTP exception, serving promotion or full M7 acceptance. See the [transport preparation and verification](docs/superpowers/plans/2026-10-16-m7-bidder-catalog.md#transport-selection-preparation). |
| Shared privacy wire fixtures assume every published protocol is passthrough | Resolved; independent full-context contracts | The October 27 repair replaces extrapolation from unrelated factory examples with authored bidder-owned fixtures/privacy.json contracts for the exact full privacy input. Pinned source review verifies publisher substitution, impression FPD conversion and absent/present root-extension rules; canonical ingress app defaults are separate. Core selects the whole published cohort without provider-name branches or skips. Identifier/geo denials and before-state presence assertions remain unconditional; marker scans cover bodies, URLs and headers, with a leak-injection control. All six privacy_outbound tests and seven response/GDPR goldens pass under both secure defaults and the explicit development feature. Response expectations add only independently authored protocol enrichment; they cannot overwrite shared type, targeting or metadata assertions. Production decoders, workload limits and SDK pins are unchanged. This targeted result does not claim a green complete workspace, M7 completion or hosted readiness. |
| Shared workspace code, configuration and tests knew concrete bidders                           | Strict component ownership implemented                                                                                           | Other crates use fake roles or generic metadata discovery, not provider names, constructors, settings fields or capture selectors. The bidder component owns all name-bearing catalog/wave/usersync/OpenRTB captures, with 19 original assets byte-identical. Shared handler and normalized-response proofs use the canonical fake; real decoder/allocation and generated-field evidence stays beside each implementation. Cargo test targets cannot point outside their crate. The cookie codec calls discovered owner-local UID policy without changing sentinel behavior or promoting a factory. Guards cover crate code, structured fixtures, manifests, root configuration and workflows, including grouped imports and embedded JSON/TOML negative controls. GVL JSON remains exempt; authoritative standards attribution is unchanged. Synthetic generated declarations exercise 14 real compiled-API child tests without serving fake variants. Browser and provider harnesses select metadata by properties. The shortest punctuation aliases remain AG-approved adversarial inputs, never real names. Cross-crate private-source imports are resolved by the separately verified ownership refactor above. The [isolation plan](docs/superpowers/plans/2026-10-25-workspace-test-bidder-isolation.md#strict-component-boundary) records fresh verification and the unexecuted full browser check; this does not claim complete M7 delivery or hosted readiness. |
| Shared bidder integration tests still depended on production identities, protocols and schemas | Isolated through one capability fake                                                                                             | Shared boundary tests and scratch registries use `src/test_support/fake_bidder/`, including bounded construction, host/query selection, correlation, diagnostics, debug projection, all media, partial responses and browser-auction output. A declaration-derived source guard rejects production names and constructors in shared tests. Real allocation and sealed-decoder cases remain beside their owners and are automatically discovered into separate native conformance targets; complete source/schema evidence remains distinct from synthetic controls. Original aggregate captures retain their byte pins under `crates/auctioneer-bidder/tests/conformance/fixtures/reference/`, and supplemental exclusions require an independent frozen source witness. No serving factory, workload limit, charge coefficient or engine pin changed. Validation and scope are recorded in the [isolation plan](docs/superpowers/plans/2026-10-25-bidder-test-isolation.md).                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| Shared bidder helpers retained concrete implementation coupling                                | Owner-local caller evidence and complete-source guard implemented                                                                | Media helper tests now exercise the pure shared function directly; original caller assertions and five factory-fixture negative controls remain inline with their real owners. The framework no longer reexports concrete bidder types. Real allocation controls resolve discovered factories before measurement, retaining their fixtures and bounds. The source guard scans complete shared files outside manifest-owned directories and checks grouped/renamed imports and factory paths, not inferred test contexts. Public-API conformance builds and runs both feature selections against discovered declarations. See the [follow-up](docs/superpowers/plans/2026-10-25-workspace-test-bidder-isolation.md#shared-helper-follow-up) for actual verification; no protocol, serving roster or budget changed.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| Optional browser-auction configurations had no bounded bidder-to-response output channel       | Shared channel and Criteo consumer implemented; complete bidder delivery remains open                                            | Bidder results move already node-budgeted JSON without provider-controlled outer identities. The exchange checks scoped and exact sent-call impression coverage, keeps valid siblings and currency-independent configs, and supplies authoritative requested/canonical names. Config-only output is supported; empty output is omitted. Escaped repeated identities are charged before wrapper allocation without borrowing debug capacity or changing limits/coefficient values. Native integration/allocation controls cover runtime-derived funding, fixed owners, multiple calls per alias, and final-byte refusal. Criteo's 13 original protocol captures and 35 parameter literals retain adjacent provenance; seven missing response-ID envelopes remain explicit shared-decoder boundaries, not original-envelope parity. Public definitions, schema/resource acceptance, endpoint approval and hosted delivery remain separate. See the [output contract](docs/superpowers/specs/2026-10-25-browser-auction-output-design.md) and [implementation record](docs/superpowers/plans/2026-10-25-browser-auction-output.md).                                                                                                                                                                                                                                                                                                      |
| OpenX had no Rust request/response protocol | Native protocol candidate implemented; selected-flow acceptance open | The discovered owner streams grouped banner/native and split video calls without caller-tree copies, retains exact sent coverage, maps reserved metadata and moves node-budgeted browser configurations independently of ordinary bids. AG's October 26 OpenRTB-first decision rejects unsupported/invalid explicit media codes per bid; fallback applies only when mtype is absent. Ten original captures retain all 37,709 bytes, with 27 source witnesses, three trees and 22 parameter literals checked independently. Source unsent-config keys, duplicate coverage and inherited typed-envelope limitations remain named boundaries. Four native allocation controls use unchanged charges; an injected video clone fails the repaired larger-cap control and exact restoration passes. Definitions remain empty. Endpoint, schema/resource, composed-flow and delivery gates remain open in the [execution record](docs/superpowers/plans/2026-10-23-priority-bidder-parallel-execution.md#openx-protocol-candidate-2026-10-26). |
| Each bidder conversion required shared test rosters, schema pins and protocol branches         | Replaced with discovered, bidder-owned conformance                                                                               | Adjacent `fixtures/factory.json` files hold independent wire expectations and frozen manifest digests. Existing discovery generates native conformance tests under each factory's feature guard. Independent manifest/source inventories replace growing factory, schema and provenance-count tables; incremental build probes snapshot pristine source inputs before applying synthetic changes. No runtime protocol, resource limit, external capture, CI gate, dependency or crate changed. The [test pipeline plan](docs/superpowers/plans/2026-10-24-bidder-conversion-test-pipeline.md) separates affected development checks from the unchanged full publication gate. This removes conversion bookkeeping, not protocol-specific testing or promotion requirements.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| Endpoint templates depended on a central placeholder allowlist                                 | Bidder-owned typed contracts implemented and adopted by TTD                                                                      | Adjacent `[endpoint.fields]` declarations generate borrowed named bindings and fixed stack pairs for the existing bounded builder. Template names must belong to the resolved implementation, including inactive aliases; definitions cannot grant themselves names. Authority and sync/privacy placeholders remain separate. Boundary controls measure zero heap allocation, and a real importer mutation proves unreviewed reference packages cannot acquire declarations silently. TTD now consumes its generated binding, with public definitions still empty. No endpoint approval, workload limit or funding coefficient changed. See the [implementation record](docs/superpowers/plans/2026-10-24-bidder-endpoint-bindings.md).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| TTD had no Rust protocol implementation                                                        | Protocol candidate implemented; selected-flow acceptance open                                                                    | The discovered factory uses generated borrowed settings and endpoint bindings for safe nonempty SupplyId routing, streams one aggregate request, preserves valid bid siblings and rewrites exact price macros without networking. All 20 original protocol captures total 69,721 bytes; required-ID and sent-identity boundaries are explicit, not source success claims. Native request and response allocation controls use unchanged limits and charges. Public definitions remain empty; metadata/alias promotion, schema/resource, composed-flow and delivery acceptance remain open in the [execution record](docs/superpowers/plans/2026-10-23-priority-bidder-parallel-execution.md#ttd-protocol-candidate-2026-10-24).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| MSFT query replacement had no bounded shared path                                              | Shared prerequisite implemented and adopted by the MSFT candidate                                                                | The existing endpoint builder can replace every decoded matching key after macro expansion, using only its transactional metadata pool. Independent original/output caps, post-edit rollback, encoded keys, discarded invalid macros and zero-heap controls cover the method. Unrelated query bytes are preserved, not source-normalized; quadratic work stays bounded by the fixed metadata region. MSFT now consumes it; public definitions remain empty. See the [shared implementation record](docs/superpowers/plans/2026-10-23-priority-bidder-parallel-execution.md#shared-query-replacement-2026-10-24).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| MSFT had no Rust protocol implementation                                                       | Protocol candidate implemented; selected-flow acceptance open                                                                    | The discovered stateless factory streams ten-impression chunks, consumes generated requested/base settings and bounded member routing, and preserves valid response siblings using sealed decode and exact sent coverage. All 31 original captures remain unchanged (117,400 bytes); 33 parameter literals and 95 category pairs have independent source witnesses. Native construction and final-response allocation controls preserve existing limits and charges. Lazy SDK projection repairs observed eager-copy regressions; malformed SDK diagnostic/work and target-level proofs remain open, not covered by successful buffer bounds. Public definitions remain empty. Endpoint/alias promotion, shared schema/resource, composed-flow and delivery gates remain required in the [execution record](docs/superpowers/plans/2026-10-23-priority-bidder-parallel-execution.md#msft-protocol-candidate-2026-10-24).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| MSFT/TTD host settings had no shared typed runtime path                                        | Generated non-secret foundation adopted by both candidates; credential work open                                                 | Adjacent declarations generate strict object-only types and redacting borrowed views without a central name list. Core moves/prunes the original config map; exchange checks requested and base entries against the independently resolved implementation before construction. Empty wrong-tag payloads fail with fixed code 5, while valid aliases borrow fields independently before bidder normalization. Typed limits use the predeclared 16-KiB/1,024-node/depth-16 ceilings. Both candidates consume this path; it does not activate imported identities, fund copied context or qualify WASM residency.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| Duplicate source metadata YAML alongside bidder-owned TOML provenance                          | Removed; source verification retained                                                                                            | Metadata records keep their repository, immutable revision, path, blob, byte length, SHA-256 and review notes in `bidder.toml`. The importer reads pinned Git objects without publishing YAML; 359 duplicate captures and their local retention records are removed. Parameter schemas and protocol/parameter captures remain unchanged. Fresh main-source staging is 521 files; source-backed import and provenance checks remain required.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Bidder test driving committed calls after a semantic failure unlike production                 | Resolved                                                                                                                         | The shared driver now discards accepted calls and refunds reservations on the failure path. A control failed with two accepted calls before the repair, then verified complete refund with diagnostics/errors/counts preserved; the success row commits both calls. Factory response controls now consume metadata from actual request construction.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| Yahoo Ads had no Rust protocol implementation                                                  | Protocol candidate implemented; selected-flow acceptance open                                                                    | The automatically discovered factory preserves one call per surviving impression, independent inventory/tag rewrites, banner-first call-local response context, GPP relocation and scoped input. All 13 untouched captures total 25,589 bytes; their empty endpoints are not production defaults. Native construction controls vary input size, placement, impression count, call slots and body caps with actual clone controls. Public definitions remain empty; endpoint, schema/resource, integration and delivery acceptance remain required. See the [execution record](docs/superpowers/plans/2026-10-23-priority-bidder-parallel-execution.md#shared-construction-and-yahoo-2026-10-24).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| Sovrn assembled a UID-sized temporary cookie header outside the fixed metadata pool            | Resolved                                                                                                                         | A real constructor allocation control failed at 6,909 peak / 7,016 allocated bytes against the unchanged 6,152-byte allowance. Shared `header_with` now streams Cookie and DNT values into the existing pool, with whole-component rollback and preflight checks. Eleven new header controls, two URL compatibility controls, and small/near-capacity/oversized UID construction cases pass without raising any limit or coefficient.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| Broader suites retained obsolete catalog and response-parser fixture assumptions               | Resolved in the fixtures                                                                                                         | Runtime alias tests now independently pin the approved 512-name catalog ceiling and exercise an exact-boundary merge plus one extra name; caller-work limits remain unchanged. The debug parser control now supplies an invalid typed `nbr`, rather than depending on an absent optional seat or an opaque media hint. The owning targets pass all nine and sixteen tests respectively.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| Scoped bidder null and video MIME null were erased by optional typed fields                    | Resolved for these request fields                                                                                                | Reused `JsonField<T>` for `ImpExt.bidder` and `Video.mimes`, preserving missing/null/value without raw request storage. Kargo's two affected captures now compare unchanged; Triplelift's explicit-null parameters use source defaults and remain null on wire. All three grouping locations have typed materialization controls. Sixteen independent parse fixtures remain within the unchanged 30x tight bound and 36x floor.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| Rise, Sovrn and Unruly had no Rust protocol implementation                                     | Protocol candidates implemented; selected-flow completion remains open                                                           | Automatically discovered factories, borrowed capped transforms, shared integer-field checks and adjacent immutable captures now cover the three families. Their 55 protocol captures total 165,386 bytes. Sovrn's typed protocol arrays and response preflight, and Unruly's canonical duration/video metadata, were repaired after observed regressions. Public definitions remain empty. AG's 2026-10-24 OpenRTB-first direction intentionally excludes legacy hexadecimal/underscore floor-string emulation; no additional parser dependency is needed. Decoded invalid UTF-8 has an approved fixed bid-local refusal with valid siblings preserved. These compatibility decisions are closed; shared decoder, schema and delivery work remains in the [parallel execution plan](docs/superpowers/plans/2026-10-23-priority-bidder-parallel-execution.md).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| Exact projection-boundary fixtures omitted existing diagnostic context funding                 | Resolved in the fixtures                                                                                                         | Restored independent 4,096-byte root and 98,304-byte per-survivor context terms, separately from the retained-growth/scratch boundary being tested. All 17 controls pass, including one-byte refusal and zero-survivor cases. No production charge, coefficient or limit changed.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| Priority Triplelift lacked its source-specific request/response protocol                       | Protocol candidate implemented; delivery and response-shape parity open                                                          | The discovered stateless factory streams ordered surviving impressions without caller-tree copies, preserving source tag/floor overrides and zero omission. Fixed capped diagnostics retain rejected siblings, and response classification checks actual sent IDs, nested format codes and source USD/status rules. Twelve verbatim captures total 29,226 bytes; ten end-to-end cases are distinct from two shared typed-boundary controls. AG separately approved single case-insensitive parameter/media keys and affected-object rejection of ambiguous variants on 2026-10-23. The request null/absence gap is resolved; response and envelope gaps remain unqualified. Empty public definitions prevent activation. The [candidate plan](docs/superpowers/plans/2026-10-23-triplelift-protocol-candidate.md) retains schema/resource, gzip, sync and composed delivery gates; no complete migration or wave approval is claimed.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| Priority Kargo had metadata/schema captures but no Rust protocol                               | Protocol candidate implemented; delivery and response-shape parity open                                                          | The automatically discovered, stateless factory streams one scoped request through the bounded sink and uses sealed decode plus shared candidate-local media classification. All five original protocol captures remain byte-identical, totaling 21,276 bytes, with adjacent TOML provenance verified before filtered dispatch and against real pinned Git objects. AG approved single case-insensitive `mediaType` keys and rejection of ambiguous variants on 2026-10-23. Presence-aware typed ingress now preserves `video.mimes: null`; both affected captures compare without removing that field. Empty `[definitions]` prevents promotion. Response-shape, endpoint/site eligibility, shared schema/resource and composed delivery gates remain open in the [candidate plan](docs/superpowers/plans/2026-10-23-kargo-protocol-candidate.md).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Typed request normalization erased original extension null/absence provenance                  | Shared prerequisite and APS candidate guard implemented; delivery open                                                           | Shared bounded ingress retains a heap-free shape marker before account discovery and typed parsing, then supplies it to aliases independently of later transforms. Typed-only callers explicitly use unavailable provenance. All adapters consume the fixed per-auction reserve, including zero-call paths. The [shared control record](docs/testing/evidence/request-extension-provenance-20261023.md) is complemented by the [APS candidate controls](docs/testing/evidence/aps-protocol-candidate-20261023.md): raw admission tests distinguish absent, null and object extensions across all three regions. Factory discovery is not metadata promotion, schema/resource qualification or complete delivery.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| Bidder implementation units and captures followed inconsistent layouts                         | Inline-unit convention documented and guarded                                                                                    | Every discovered bidder keeps units inline with its implementation, without routine request/test file splits. APS's 21 capture artifacts moved beside its implementation unchanged; its separate source/protocol conformance validates supplied capture bytes before dispatch. The shared harness owns fixture registration and bounded sink driving. One process-level bidder allocation target replaces a bidder-named executable; registry/build and handler/transport controls retain their boundary owners. `registry_join` checks all declarations, including disabled features, without a second name list. See the [canonical rules](crates/auctioneer-bidder/src/bidders/README.md).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| Fresh meta reports omit capture storage and hide wrapper initialization differences            | Native auxiliary and fixed-cache composition implemented; full qualification open                                                | The [fresh Core auxiliary model](docs/bidders/schema-qualification.md#fresh-core-auxiliary-requested-heap) binds capture slots and conditional one-pass scratch to the same NFA with checked 32/64-bit requested layouts. PikeVM/backtracker wrappers remain lazy; initialized fallback buffers are not relabeled eager. Independent literal allocation controls distinguish capture-only zero reports, one-pass enablement and its representation boundary. The [fixed native cache composition](docs/bidders/schema-qualification.md#fixed-meta-cache-requested-heap-composition) now joins these owners with both primary directions, optional prefix and retained fallbacks. Shared programs, work/lifetimes, production multiplicities and physical/target accounting still gate serving; no production charge or limit changed.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| Cache-clear proofs did not exclude engine quit conditions                                      | Strict native admission implemented; complete engine qualification open                                                          | The [strict constructor](docs/bidders/schema-qualification.md#explicit-quit-admission) checks Unicode word assertions and all 256 configured quit bytes on the same immutable DFA before coarse projection or graph exploration. Primitive, constructor-ordering and prefix-binding controls cover both directions, real Quit with zero clears, every byte and combined failures. All primary/hostname directions and retained helper-prefix alternatives use it, including the fixed native cache composition. This closes quit-set admission only; cumulative counter lifetimes, shared programs, whole-schema/format overlap, work and physical/target descriptors still gate serving integration.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| Optional reverse-prefix search storage lacked a bounded owner distinction                      | Native conditional coverage implemented; whole-engine qualification open                                                         | Structured HIR inspection preserves 43 raw prefixes across all 23 emitted helpers. A source-equivalent single-pattern start-anchor gate separately yields 16 exclusions, five no-concat cases and two eligible owners with five alternatives. Every eligible alternative must qualify before its maximum is accepted; keyed refusals and a mutation-sensitive both-order test prevent partial maxima. Hostname's raw cache-clear refusal remains explicit while its optional-owner absence is independently proven. The [record](docs/bidders/schema-qualification.md#optional-reverse-prefix-cache-superset) keeps conditional requested heap separate from shared-program residency, actual meta availability/fallback, cumulative work and physical/target descriptors.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| Disabled automatic sync candidates suppressed enabled family siblings                          | Resolved in automatic selection; broader catalog integration open                                                                | Cooperative priority groups and catalog fallback now filter the cached effective sync-enable bit before shuffling and family deduplication. Explicit requests retain first-attempt diagnostics, denial precedence and shared-family suppression; disabled entries remain lookupable and participate in eviction views. Independent priority/fallback removal controls fail the RNG-count assertion. Five colocated unit controls and a separately declared three-scenario design fixture cover runtime/baked/base/alias vetoes without rewriting external captures. Existing scratch capacities and all funding limits remain unchanged; this closes only the [Task 11 selection slice](docs/superpowers/plans/2026-10-16-m7-bidder-catalog.md#task-11-indexed-info-and-sync-views), not registry/schema qualification or bidder migration.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| Schema search-cache ownership is not resource-qualified | Explicit API and conditional native cache composition implemented; complete qualification open | AG approved the [explicit-cache contract](docs/superpowers/specs/2026-10-22-explicit-schema-search-cache-design.md) and callback-local hostname ownership on 2026-10-22. Source-derived owners feed the fixed native cache composition, including primary directions, optional-prefix storage and retained fallbacks. The [native bundle component](docs/bidders/schema-qualification.md#native-bundle-and-format-overlap-2026-10-27) additionally reconciles actual compiled wrappers for all 261 schemas: all sibling pattern lifecycle peaks, wrapper inline bytes and one fresh hostname cache remain funded together. Its maximum is 316,705 bytes against the unchanged 524,288-byte scratch ceiling; independent unequal terms, overflow refusals and a separate synthetic profile guard the join. These are conditional requested-storage projections, not complete frames, physical residency or usable byte/work descriptors. M7 Tasks 3/12 retain shared programs/pools, constructor storage, other scratch, cumulative lifetime/work, production multiplicities and guest/allocator qualification. Serving, schemas, compiler patches, workload/engine limits, resource ceilings and funding coefficients remain unchanged. |
| Fixed-cache capacity clearing lacked a sufficient proof                                        | Conditional fixed-DFA proof implemented; whole-engine qualification open                                                         | The [capacity constructor](docs/bidders/schema-qualification.md#fixed-cache-reported-capacity-proof) checks an independent coarse arithmetic/recovery envelope before graph exploration, then binds the complete report, scratch/prospective-row allowance and raw-ID guard to that same DFA. All 46 emitted primary directions and both hostname directions pass without larger capacities or exploration budgets. Keyed typed refusals, independent scalar axes and real exact-capacity/refusal controls retain unproved outcomes. Reported accounting is not physical heap, actual meta availability, optional-prefix coverage, cumulative work or a usable resource descriptor.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Priority display migration lacked source-specific operation contracts                          | Design preparation implemented; review, prerequisites and bidder delivery open                                                   | The [13-identity source-backed wave](crates/auctioneer-bidder/tests/conformance/fixtures/waves/priority.toml) names 113 requirements and 16 planned Rust driver labels with 38 exact source-owner blobs, canonical schemas, sync provenance and routing blockers. APS (`aps`) is an additional fourteenth priority integration absent from the pinned catalog; its independent contract and executable evidence remain required alongside this wave. The CLI checks bounded design shape and pinned source mappings only; success remains `wave_design_only`, pending review, zero promoted and zero migrated. Full completion still refuses. The [family plan](docs/superpowers/plans/2026-10-22-priority-display-bidders.md) retains HTTPS/credential/version/error-policy review, fixed producer/fixture counts, schema/resource qualification, shared primitives and actual implementation as prerequisites; no driver execution or Task 18A completion is claimed.                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| Missing planned schema-work admission boundary                                                 | Scalar guard and borrowed input measurement implemented; cost qualification and serving integration open                         | A non-Clone auction budget checks original expiry and fallible projected work/scratch before invoking a validator, debits admitted work before the callback, and does not refund false results or panics. Borrowed final-Value admission separately measures exact compact bytes, nodes including keys and bounded container depth, without cloning or retaining serialized output. Independent limits, native allocator controls and actual generated callbacks guard these boundaries without claiming engine bounds. Qualified cost projections, full cache multiplicity and allocator/target descriptors still gate live integration; see the [work admission](docs/bidders/schema-qualification.md#schema-work-admission) and [input measurement](docs/bidders/schema-qualification.md#borrowed-input-measurement) contracts.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| Native allocator-control child exceeded its watchdog during concurrent compilation             | Diagnostics implemented; original cause open                                                                                     | Fixed phase markers now stay outside measurement windows; deadline refusals retain a bounded 4-KiB UTF-8 stderr tail after process cleanup. Forced-deadline and output-limit controls guard the diagnostics. A native replay passed 64 fresh control processes while a two-job workspace Clippy process remained active, but did not reproduce or explain the original timeout. On recurrence, test-harness maintainers must use the retained phase to investigate; do not raise the ten-second deadline, retry failed measurements or claim parallel-run stability from this finite replay. See the [watchdog record](docs/bidders/schema-qualification.md#allocator-control-watchdog-diagnostics).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| Generated schema regexes had no explicit complete-constructor operation                        | Constructor API and native allocation controls implemented; full qualification open                                              | A separate checksum-pinned compiler patch exposes every generated pattern constructor through shared initialize-once storage, without changing schemas, engines or limits. Generated-code controls observe cached errors, repeat/concurrent use and both engine builders. A fresh native process measures all 261 callbacks and the shared format separately, requires zero constructor-repeat allocation, and measures later validation/cache growth without conflating the phases. Search-cache/work/allocation bounds, target descriptors and production integration remain open. See the [constructor contract](docs/bidders/schema-qualification.md#explicit-pattern-constructor-initialization) and [allocation observations](docs/bidders/schema-qualification.md#constructor-allocation-observations).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| Pure resource checks lacked independent owner and all-adapter parity coverage                  | Controls implemented; catalog qualification remains open                                                                         | Complete isolate/auction vectors use independent literal deltas across 12 owner cases and 72 config/catalog population cases. Eighteen budget mutations fail their owning controls. The existing isolated CLI harness now exercises all four adapter selectors against 40 fresh boot installations; fixed traces distinguish actual scratch corruption from an unknown-hook refusal. Duplicate Cargo units are accepted only with identical source/features; conflicts and missing owners still fail. These are native accounting controls, not qualified storage bounds, actual provider observations or full Task 4 acceptance. See the [owner/parity record](docs/testing/evidence/resource-owner-parity-controls-20261021.md).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| Catalog resource export had no matched arithmetic comparison                                   | Numeric comparison and matched capture implemented; qualified bounds open                                                        | The explicit CLI baseline mode rejects mismatched build/workload inputs and rederives both rows before checking C/I/V/K, auxiliary and aggregate growth. A separate clean-source patched-lock capture retains the original pin, whole-audit continuity and the 65,536-byte per-permit chooser increase, with its population-scaled component shown in output. Both complete auxiliary sums fit the unchanged reservation; no cost is erased or charged twice. Native local headroom remains mandatory and relative WASM results do not promote readiness. Physical increments still require qualified records against the original source basis, not this arithmetic recapture. No live cutover or full Task 4 acceptance is claimed; see the [continuity record](docs/testing/evidence/catalog-baseline-continuity-20261021.md).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| Linked guest schema evidence recorded only whole-file artifact sizes                           | Section observations implemented; residency and full qualification open                                                          | The existing linked control records validated core-module/component code bodies, active/passive data payloads and declared memory minima without enclosing-section double counting. Exact binary fixtures and actual four-artifact wiring guard the observations; native code/data, caches, allocation/work bounds and qualified C/I/V/K records remain pending. See the [measurement record](docs/bidders/schema-qualification.md#guest-section-measurements). No production validation or target readiness is enabled.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| Schema compatibility controls lacked independently captured source parameter expectations      | Complete source tables covered for fourteen families; full qualification open                                                    | The 300 pinned literals retain their source booleans, duplicates and ordering, and exercise actual generated native callbacks with separate per-family JSON/schema refusal counts. The original 236-row capture is reconstructible byte-for-byte; both the whole-capture cap and earlier 90-row guard remain unchanged. The owning reader guards complete shapes/provenance/dispatch; an explicit source checkout verifies immutable blob and byte-table witnesses. Git checkout filters preserve the capture bytes. The [qualification contract](docs/bidders/schema-qualification.md#format-and-pattern-source-cases) retains the remaining 247-schema, work/cache/resource and live-integration gaps; absent priority-family tables are explicit and no production behavior is enabled.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| Catalog owner costs had no single checked aggregation boundary                                 | Scalar boundary implemented; production qualification and integration pending                                                    | The bidder crate checks complete owner membership, independently expected source/qualification records and common build identity before an allocation-free merge. Shared storage counts once only when every lifetime charge agrees; missing/stale records, ambiguous sharing and overflow refuse. This validates scalar consistency, not physical bounds or evidence authenticity. No live owner record or new runtime allocation is enabled; see the [aggregation record](docs/testing/evidence/catalog-cost-aggregation-20261020.md).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| Metadata had no explicit implementation binding or retained feature declaration                | Data-only reconciliation installed; published serving dispatch generated                                                         | Required implementation keys and optional predicates generate deterministic active bindings plus an all-declarations view. Bidder discovery consumes that view before filtering, rejecting missing factories, mismatched predicates and missing explicit config-feature forwarding. The approved follow-up generates current-published-roster constructor dispatch from those same declarations; core no longer chooses implementations by bidder name or substitutes a generic fallback. Borrowed metadata, feature ownership and runtime enable/disable remain. The full-catalog resource/index cutover is still pending, and no candidate is promoted. See the [binding controls](docs/testing/evidence/metadata-binding-controls-20261019.md), [factory reconciliation](docs/testing/evidence/factory-binding-controls-20261019.md) and current isolation plan.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Inactive metadata skipped renderer checks and error strings could echo values                  | Repaired at the metadata build boundary                                                                                          | Full render validation precedes filtering. TOML diagnostics omit source/parser text, and canonical macro/URL/allowlist/enum diagnostics retain field/reason without rejected strings. Child regressions require failure and absent whole/prefix/suffix credential markers; unrelated host/account diagnostics are not reclassified. Verification is recorded with the [binding controls](docs/testing/evidence/metadata-binding-controls-20261019.md).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| Catalog capacity shared its owner with caller/debug limits                                     | Resolved in M7 count foundations                                                                                                 | Independent runtime constants allow 512 catalog identities while retaining 256 requested, priority, per-filter and debug entries, 32 priority groups and 64 runtime aliases. Build/default/runtime boundary controls and independent widening/summand mutations pin the owners; see the [count/storage record](docs/testing/evidence/catalog-count-controls-20261019.md). This does not complete the strict registry join, full migration or R0 readiness.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| Full-size request-local sync catalog and family copies exceeded their charges                  | Resolved for the measured native slice                                                                                           | Borrowed compact catalog entries, borrowed candidate ordering and indexed family projection replace duplicated names/maps without raising coefficients. The existing allocator target requires 72 independent inputs and all 216 construction/choose/projection stages under their own charges. These are native requested-heap controls, not complete guest/physical-memory qualification.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| Bidder factories require central module registration                                           | Discovery installed; current-roster handwritten serving switch removed                                                           | Adjacent declarations generate sorted factory/declaration views and published constructor dispatch. Only currently published bindings are referenced by serving construction; no all-candidate table or new heap index is installed. Core keeps its existing lazy registry/cache and borrowed metadata. Executable synthetic controls distinguish public names from implementation names, shared factories, unused candidates and feature off/on/off; the existing [incremental controls](docs/testing/evidence/registry-join-controls-20261019.md#incremental-discovery-preparation-october-21) retain add/remove/rename and feature coverage. Full-catalog resource qualification, indexes/descriptors and lifecycle acceptance remain separate M7 work; no bidder was promoted.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| Resource inspection introduced a separate target vocabulary                                    | Resolved in M7                                                                                                                   | The CLI selects `--adapter axum/cloudflare/fastly/spin`, deriving compiler ABI from adapter-owned profiles. Reports declare single-adapter or explicit `--all-adapters` scope. Canonical audits/reviews still require all four adapters. Command and typed-record tests prevent scope confusion; no workload or accounting policy changed.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| Raw valid-JSON fixtures                                                                        | Resolved                                                                                                                         | Structured fixtures; counted exemptions and rejection gate                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| Missing cookie-signing mechanism                                                               | Resolved                                                                                                                         | Mandatory versioned MAC, secret-store key rotation and bounded writes                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| Untested sync-URL encoding and endpoint checks                                                 | Resolved                                                                                                                         | Five independently varied macros at both splice points, plus six endpoint-identity guards; staged repairs described below                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| Unpinned cookie minimum-size pre-check                                                         | Resolved                                                                                                                         | Measured early rejection excludes formatter allocations, including a branch-removal mutation; complete-field check remains authoritative                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| Uncovered account GDPR opt-in                                                                  | Resolved                                                                                                                         | Independent account/web permission tests and branch mutations                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| Host-name exception lists masking each other                                                   | Resolved                                                                                                                         | Independently varied lists and per-disjunct mutation checks                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| Privacy reference corpus not exercising Auctioneer                                                 | Resolved                                                                                                                         | All 71 rows drive real extraction; agreements and differences explicitly asserted                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| Unused `ccpa_enforce` field                                                                    | Resolved                                                                                                                         | Dead field and ladder removed without changing the sync-denial contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| Positional cookie-sync request arrays accepted                                                 | Resolved                                                                                                                         | Object-only deserialization; parser and routed 400 regressions before signing-key reads                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| Repository-owned reference tooling required Go                                                 | Resolved                                                                                                                         | Native Rust verifier; ten externally captured artifacts remain unchanged, with read-only staged checks and no circular expected-output generation                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| Reference revision/schema/attribution guards lacked direct negative controls                   | Resolved                                                                                                                         | Retained revision/provenance, manifest and attribution controls remain directly exercised. Approved slim-checker boundary removes generic schemas and Rust syntax parsing; owning consumer tests pin counts and typed GPP vectors. Round-five mutation evidence below records the previous implementation.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| Optional Spin secrets prevented application loading                                            | Resolved                                                                                                                         | Empty unusable defaults for both secrets; pinned vendor-loader test exercises absent/supplied values and rejects default removal, with required CI execution                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Signed-cookie boundary fixture used a moving wall clock                                        | Resolved                                                                                                                         | Fixed nanosecond test clock and installation guard; production precision and complete-field ceiling unchanged                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| Reported intermittent `usersync_routes` failure                                                | Open, not reproduced                                                                                                             | The fixed clock is not a proved cause/fix. Routed setup now checks idle process gates; twenty fresh full-binary sweeps with eight test threads passed, but do not close this investigation.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| Duplicate-UID regression failed on missing bidder binding                                      | Resolved                                                                                                                         | Valid bidder binding isolates duplicate-UID rejection; removing only the config UID-count guard now fails the assertion                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| Chooser capture fields had no complete consumer owner                                          | Resolved                                                                                                                         | All 21 `choose` inputs and outputs require their exact key sets and output types; four ordering rows also have exact key sets. Missing fields and unexpected fields fail direct consumer probes.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| Cookie reference loops could silently skip renamed operations                                  | Resolved                                                                                                                         | Real decode/read drivers require 36/7 executions; the reference owner requires the 36/7/5 split and rejects unknown operations.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| Callback test named a later guard than it exercised                                            | Resolved                                                                                                                         | Renamed to config-layer rejection and requires the precise validator diagnostic; existing private callback unit tests separately exercise encoded UID spelling.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| Source-selection capture evidence label lost its assertion                                     | Resolved                                                                                                                         | All 36 selection rows require `source-selection-only` in the real source-selection driver.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| Utility boundary test claimed consumer ownership indirectly                                    | Intentional boundary                                                                                                             | Renamed to state only that provenance validation does not inspect operation fields/counts. Separate consumer probes prove the repaired owner checks.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| Flattened-map negative coverage absent from merged M4                                          | Covered in M5                                                                                                                    | Guard and exact negative test are present on M5 and pass. Local `origin/main` at `5839dae` contains the guard but not that named test; merging M5 carries the coverage, with no ancestry-only cherry-pick.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| Callback authority and bidder macro validators lacked independent controls                     | Resolved                                                                                                                         | Exact-error tests separate templated authority suffixes, redirect/base userinfo, fragments, macro length/syntax and syncer-key length/token grammar. The host-presence check is redundant after strict HTTPS parsing, not a separately reachable guard.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| Encoding-label sanitizer tested only with benign labels                                        | Resolved                                                                                                                         | The real body reader rejects hostile, oversized, blank and non-ASCII labels before polling; response bytes and redacted Debug are asserted exactly.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Fixed application sync response header count was not enforced                                  | Resolved                                                                                                                         | Shared decorators refuse more than 16 application-owned field values, including duplicates/CORS/Set-Cookie. This fixed ceiling does not consume the profile's full accounting allowance. Adapter/provider-added fields are outside the count/byte checks.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| Provider-added response headers fall outside application checks                                | Scope clarified                                                                                                                  | Application count/byte limits exclude provider-added framing fields. Whole-wire bounds require separate target qualification; no fixed overhead or completed provider guarantee is claimed.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| Opaque numeric overflow lost the auction's publisher identity                                  | Resolved in M6                                                                                                                   | Bounded structured discovery skips unrelated numeric conversion; a named disabled publisher beside banner.ext 1e400 retains its 503 refusal before typed preparation. Discovery failure is not absent identity.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| Response metadata preflight charged unrelated request FPD                                      | Resolved in M6                                                                                                                   | Checked used metadata bytes/nodes now consume the independently funded four-representation bookkeeping term; response ID/global warnings are reserved before copying. The original 512-node runtime-derived receipt regression accepts full FPD and all sixteen responses without borrowing debug or changing measured factors.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| Native compiled envelope and local measured acceptance                                         | Rust local acceptance passed; production qualification unsupported                                                               | The reviewed 4 GiB, 16-call/two-auction, 2,048-node candidate fits at 2,533,631,672 bytes. Digest-bound Rust source d68ba17 passed all 45 local rows and the separate frozen control, with ordinary-only dispatch/decode geometry and contemporaneous GVL overlap. The [report and limitations](docs/testing/evidence/native-axum-rust-d68ba17-local-pass/README.md) retain overall qualification unsupported and publisher readiness false. Historical JavaScript captures and the first Rust capture's failed control remain unchanged and are not relabeled.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| Shipped WASM worst-case envelopes exceed declared memory ceilings                              | Open, M7-R0 readiness prerequisite                                                                                               | The four-target inspector reproduces native-layout controls of 138,048,496,272 bytes on Cloudflare and 23,589,646,360 on Fastly/Spin, against 128,000,000/128,000,000/134,217,728-byte declarations. These include actual baked bytes and native construction descriptors, not guest measurements. The [audit notes](docs/testing/evidence/resource-envelope-audit-20261018.md) trace all eleven lifetimes and show that even a severe two-bidder/4-KiB counterexample does not fit under the existing equation. AG authorized verified unchanged-workload improvements and then proof-backed accounting corrections on 2026-10-18; preserve workload limits and the shared additive total. At bc300ace all four SDK targets still lack host-ingress accounting; Axum lacks ceiling/population facts and Cloudflare lacks an inbound bound. EdgeZero maintainers own those published contracts; AG owns operator declarations. Auctioneer's Spin dispatch metadata now matches the selected hosted contract, but actual hosted provenance, Fastly's application stack bound and WASM proofs remain pending. The [contract acceptance table](docs/superpowers/specs/2026-10-17-m7-r0-wasm-resource-envelope-design.md#54-external-contract-acceptance) separates their requirements. `--require-fit` refuses; R0 is not complete or ready. Credentialed deployment qualification remains M7.6.                                             |
| M7-R0 planning lacked complete cost and review-scope controls                                  | Adapter-owned profiles, audit capture/render and actual boot/CLI parity implemented; remediation and complete acceptance pending | Runtime owns the checked equation and headroom policy; core owns normalization. All four adapters publish typed profiles consumed by bootstrap and the inspector, with actual metadata comparison rather than provider-name exceptions. Source-bound audit capture and generated tables preserve pending proofs. [Checkpoint controls](docs/testing/evidence/resource-review-controls-20261018.md) retain strict package validation and frozen scope enforcement for specifically approved limit/model packages. The [ownership controls](docs/testing/evidence/resource-ownership-controls-20261018.md) compare ten fresh-process boot profiles with twenty actual CLI runs under matched source/features, including independent defaults, the complete baked tuple, all terms/receipts and warn/install versus strict refusal. Two scratch-only CLI corruptions refuse. AG's later authorization permits verified unchanged-workload implementation improvements without a frozen approval window. Production memory remediation, any needed model approval/CI binding, matched-baseline allowances and physical qualification remain open. Workload limits, coefficients, schema-2 wire shape and historical captures are unchanged.                                                                                                                                                                                               |
| Preboot/stale-App config extraction could use unfunded SDK defaults                            | Extraction boundary resolved; complete allocation proof remains open                                                             | Under AG's approved contract, the resolver now refuses before any store read without installed gates or when captured backend/blob/value caps exceed normalized funding. The fixed `unfunded_extraction` code preserves bounded liveness; a new configured App recovers, and cancellation releases work. Missing bindings retain their existing policy. The [verification record](docs/testing/evidence/config-funding-boundary-20261018.md) records 1,139 core and 323 adapter tests plus both feature-gated Mocktioneer cases; fault tests retain positive SDK-read evidence. Raw SDK lifecycle controls remain intact. Core maintainers still need per-phase/guest allocation and all-route response-storage bounds before handoff savings or fit may be claimed.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| Source resource commands lacked an explicit CI execution contract                              | Controls implemented; absolute fit remains open                                                                                  | CI captures and validates a fresh four-adapter audit in temporary storage, then reconciles exactly one strict JSON report with status zero exactly when all rows fit and status one otherwise; other failures refuse. Semantic guards protect ordering, prerequisites and failure propagation; executable shell cases reject empty/multiple reports. A real nested-Git control isolates only the two external fixture checkouts while preserving ordinary source detection. Pure non-installing profiles and independent term/report corruptions retain the runtime-owned equation, mandatory zero catalog terms and source binding. The [verification record](docs/testing/evidence/resource-command-controls-20261018.md) distinguishes these controls from pending matched-catalog allowances, physical allocation proofs and target qualification. No workload, coefficient, numeric equation or historical capture changes.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| Auction retained the entire runtime overlay during fanout                                      | Resolved, unchanged-workload R0 improvement                                                                                      | The handler now drops the extracted overlay after producing owned host/account/plan state. The generated-App allocation test varies 0/256/4096 unselected vendor entries through completion and cancellation: prior vendor-dependent held growth of 7,936/126,976 bytes disappears, while config parsing work still grows, selected privacy/alias/output behavior stays identical and each request reads config once. All gates/receipts and dispatch-owned allocations release. This is a native lifetime result, not a general allocation upper bound, lower accounting charge or WASM fit. See the [verification record](docs/testing/evidence/resource-ownership-controls-20261018.md#auction-overlay-release).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Auxiliary config storage used blob bytes as its node population                                | Corrected; funded-context guard and native consumer checks verified                                                              | Explicit SDK JSON-value counts use the existing 623-byte config bound, independently of 384-byte body/cookie syntactic nodes and unchanged byte/baked conversion charges. At shipped WASM caps the receipt component falls by 397,549,568 bytes; declared slots and isolate totals do not change. Parent reruns reproduce 191 runtime and 27 extraction/handler/resource tests. The [unit verification](docs/testing/evidence/config-value-accounting-20261018.md) and [funding-boundary verification](docs/testing/evidence/config-funding-boundary-20261018.md) distinguish the node correction, initialization guard and native golden repair from still-open guest and handoff proofs.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| Auxiliary handoff multiplies config/privacy work as if it were response storage                | Open; info/health output and CORS ownership prerequisites repaired                                                               | The [route audit](docs/testing/evidence/auxiliary-handoff-audit-20261018.md) derives current content bounds and terminal owners across all routes. Info/health JSON now reserves counted output once without growing; reflected CORS values no longer retain arbitrary ingress backing. The [storage controls](docs/testing/evidence/auxiliary-storage-controls-20261018.md) distinguish those verified changes from still-open sync final backing, header-map storage and construction/terminal overlap proofs. Core maintainers must close those bounds before runtime maintainers may substitute response storage for work in the handoff charge. Preserve auxiliary, probe-config, probe-output/cache and static populations separately. No accounting savings or guest/provider guarantee is claimed.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| SDK decoder reservation alone prevents the declared WASM wave from fitting                     | Open, SDK-owned bound/remediation prerequisite                                                                                   | At bc300ace the unchanged 32-stream wave reserves 536,870,912 fixed decoder bytes before window/buffer storage. The SDK's 16-MiB fixed policy allowance is not a measured retained lower bound: its source audit identifies 3,442,530 payload bytes and reserves the remaining 13,334,686 for unproved wrapper/allocator terms. Auctioneer must not reduce that charge or replace the codec with a panic-on-exhaustion preallocator. SDK compression maintainers must establish a complete scope/ABI-bound allocation contract or equivalent bounded implementation, retaining window/framing/error/cancellation semantics and supported workload, before Auctioneer can consume any smaller proved bound. The [feasibility record](docs/testing/evidence/auxiliary-storage-controls-20261018.md#decoder-dependency) separates accounting non-fit from physical impossibility and records the source owners and acceptance controls.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Candidate schema macro bypasses declared numeric types                                         | Bounded compiler repair applied; full schema qualification still open                                                            | Published jsonschema 0.58.6 accepted strings in Yandex integer properties and Consumable's numeric networkId. AG approved the checksum-pinned, one-file compiler patch on 2026-10-19; literal rejection/positive controls and integer/union keyword-dispatch controls now exercise the repair. The original package and portable patch preserve provenance and upgradeability, and standalone qualification checks actual Cargo source resolution. No schema or production validation behavior changes. Complete instance/work/cache/target-resource qualification still blocks runtime integration; see the [patch and remaining gate](docs/bidders/schema-qualification.md#approved-compiler-patch).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| Version-one resource reports cannot encode catalog growth                                      | Resolved in M7; catalog materialization still gated                                                                              | Native export and CLI qualification now hard-cut to schema 2 with an explicit object-valued four-field descriptor, identity-bound costs and inspectable accounting terms. The harness independently rederives total/catalog terms and checks the resource digest; missing/duplicate/unknown fields, positional arrays, obsolete versions and mismatched evidence refuse. Historical captures remain byte-identical and integrity-checked, not requalified or defaulted. The shipped context increment funds construction diagnostics (`4,096 + 98,304 * max_bidders_per_auction` bytes, zero when that limit is zero); qualified schema/catalog aggregation and the full catalog cutover remain unfinished in the [accounting plan](docs/superpowers/plans/2026-10-16-m7-bidder-catalog.md#task-4-catalog-accounting-and-matched-resource-reports).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| Bidder reference inputs stored in documentation                                                | Resolved in M7                                                                                                                   | Bidder-owned schemas and captures live beside their implementation. Shared inventory/binding witnesses and the byte-pinned aggregate parameter capture live under `crates/auctioneer-bidder/tests/conformance/fixtures/reference/`, not in shared tests or documentation. Original source/license digests and raw-schema line endings are preserved; CLI defaults, import checks, schema controls and Git attributes follow those owners. `docs/bidders/` retains documentation and reports. Relocation does not promote or newly qualify a bidder.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| Compiled bidder join lacks one funded, cached serving registry                                 | Current serving roster discovered; full-catalog funded join open                                                                 | The non-installed `ValidatedRegistryPlan` checks complete declarations, actual feature membership, explicit bindings and alias ancestry before any constructor. It borrows metadata and exposes sorted public/base/alias/factory views. The existing lazy serving registry now uses generated published-binding dispatch instead of the old handwritten match/fallback, retaining its representation and original metadata references. This does not install the allocating full-catalog validation plan, activate candidates or establish positive catalog resource qualification. Generated resource descriptors/precharging and the full-catalog lifecycle cutover remain Task 7 work. The [verification record](docs/testing/evidence/registry-join-controls-20261019.md) scopes the earlier controls; the isolation plan records the current-roster follow-up.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Generated metadata lacks a complete qualified residency contribution                           | Requested-heap and target-layout census implemented; qualification open                                                          | Config derives its census from actual emitted constructor syntax, before lazy materialization. Checked target-size arithmetic and independent cold/warm allocator controls cover both feature modes; unsupported expression work refuses generation. The [verification record](docs/testing/evidence/metadata-storage-census-20261020.md) distinguishes requested storage from allocator overhead, literal pooling/alignment, executable residency and full qualification identities. These plus schema/registry contributions and aggregate precharging remain open; the source digest alone is not qualification and no production cost/profile/limit changes.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| Multi-call protocols lack call-local correlation and partial construction diagnostics          | Shared channels implemented and funded; protocol adoption open                                                                   | Call-local typed metadata remains on the existing Arc through finalization and positional results. The independent 32-entry nonfatal channel now retains fixed safe rows through accepted siblings, zero-call aliases, fatal refunds and both deadline exits, without sharing the 16-entry sink-error window. All adapters bind its context charge to their supplied bidder limit; one combined Projection receipt must fund request work and diagnostic owners before construction. See the [correlation controls](docs/testing/evidence/call-correlation-controls-20261023.md) and [diagnostic controls](docs/testing/evidence/nonfatal-construction-diagnostics-20261023.md). Protocol migration, schema qualification and target readiness remain separate open gates.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| Selected deployment SDK attestations and publisher execution                                   | Open, M7.6 plus SDK owners                                                                                                       | The local report retains all six SDK capability attestations pending and overall qualification unsupported. The provider integration maintainer and selected deployment operator must execute the [M7.6 cases](docs/roadmap.md#8-limited-bidder-publisher-pilot-and-hosted-acceptance), resolve target-specific SDK prerequisites, bind actual Q2-D evidence and observe rollout/rollback after M7.5/selected bidder setup. No credentialed live provider test has been run.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Admission witnesses masked ordinary workload underload                                         | Resolved                                                                                                                         | Ordinary successful request IDs must independently reach the expected fixture call count and return decoded bids. Ordinary decode/byte/node/call evidence excludes witnesses; empty/fake replies without dispatch and dispatch without decoding fail independent controls.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| GVL overlap lacked a contemporaneous witness                                                   | Resolved                                                                                                                         | A real ordinary auction remains parked with active bidder calls across each credited lookup; serialized work fails, and held work is released and drained on every exit.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| Successful helper exit lost descendant cleanup ownership                                       | Resolved                                                                                                                         | Managed process groups remain registered after wrapper exit and receive bounded signal escalation until termination is observed, including signal-resistant descendants. Persistent permission/termination failures fail cleanup.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| Contradictory process-lifetime memory peaks qualified                                          | Resolved                                                                                                                         | The evaluator rejects decreasing RSS lifetime high-water across a startup's cold/warm and declared scenario schedule. Window-scoped meters and independently restarted variants retain their separate semantics.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| Qualification verdict and memory refusals lacked isolated boundary guards                      | Resolved                                                                                                                         | Current source already maps fail to nonqualified, and its existing CLI regression catches the proposed promotion mutation. Added an explicit status matrix, real CLI below/at/above/zero-peak verdict/exit/readiness cases, and independent meter/source/limitation/window, growth, overflow and unequal-ceiling tests. Seventeen individually applied guard mutations fail the intended tests.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| Effective debug clamps were tested only below their caps                                       | Resolved                                                                                                                         | All four resource-dependent clamps have independently varied below/at/above tests with literal expected caps. Removing each clamp fails its corresponding test.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| Qualification latency, extra-evidence and capability refusals lacked direct negative controls  | Resolved                                                                                                                         | Independently consistent below/at/above-threshold histograms, incorrect reported percentiles, unexpected scenario/variant identities, failed or digest-free capabilities, and over-limit achieved calls assert exact reasons and overall verdicts. The actual CLI also refuses altered copies of the committed 45-row Rust capture. Single-guard removals fail behaviorally in an isolated source copy.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| Historical JavaScript exact-call mutation reported against the Rust replacement                | Covered and strengthened                                                                                                         | The deleted harness is not shipped. The Rust fixture already requires exact per-ID counts, and ordinary-only reconciliation requires the declared width. Successful and timed-out populations now explicitly reject both 2-of-16 and 17-of-16; weakening either equality fails its owner tests. No measurement or runtime behavior changed.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| Empty successful auctions could borrow sibling decode credit in a regression fixture           | Resolved                                                                                                                         | Three successful HTTP outcomes now have all 48 expected calls funded; the empty auction alone must refuse, while its one-bid counterpart passes. Removing either the zero-bid flag or its refusal fails the named test. Production refusal behavior is unchanged.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| Individual native measurement abort codes had no direct regression guards                      | Resolved                                                                                                                         | Interrupted, run-deadline and cleanup-failed errors are individually fatal with and without stage metadata; ordinary recoverable errors remain nonfatal. Removing each fatal code fails the test. No new signal execution or cleanup guarantee is inferred.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| Planned and achieved call boundaries used the same one-call fixture value                      | Resolved                                                                                                                         | A fully funded four-call synthetic candidate varies planned calls from zero through five. The evaluator independently holds planned work at two and varies achieved calls across both boundaries. Removing planned nonzero/ceiling or achieved floor/ceiling guards fails the corresponding test; real profiles and thresholds are unchanged.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| Qualification identity permits three development features rather than two                      | Intentional and covered                                                                                                          | Rust tooling records insecure_bidders, conformance_suite and test_harness_override. Profiles accept all eight unique subsets and reject repeated or unknown modes; observations reject a fourth entry. The profile length check is also protected by the finite enum plus uniqueness, not a separately reachable fourth unique mode. Development identities remain production-unsupported.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| Configuration override indicator was tested against its own cfg expression                     | Resolved                                                                                                                         | Separate cfg-selected tests require literal disabled/enabled states. Forcing either false-positive or false-negative reporting now fails without relying on the exported constant as the expectation. Existing dependency-only export CI controls remain required.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| Retained qualification README tables and checksum manifests were unchecked                     | Resolved                                                                                                                         | Rust integration tests require all seven captures and 25 indexed raw-byte digests, exact members, regular files and safe names. Negative controls vary every payload/index and independently reindex incorrect observation/report profile bindings and snapshot bytes. This checks retained-index consistency, not authenticity against coordinated rewriting or a new resource qualification.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| Historical failed capture has a runner/report digest discrepancy                               | Retained historical limitation                                                                                                   | In native-axum-7106af6-failed, the introduced runner records a different report digest than the retained report and README. Both original files remain unchanged; the new checker validates the README against retained bytes and does not invent historical chain equality or relabel the failed attempt.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| Broad native peak ceilings could be mistaken for tight regression budgets                      | Scope clarified                                                                                                                  | The current 3 GiB criterion is approximately 85 times the latest Rust capture's maximum process-lifetime high-water. It is an envelope ceiling, not a minimum safe deployment size or tight regression budget. No criteria or historical captures changed; tighter candidates need new predeclaration and measurement.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| Non-browser qualification harnesses require JavaScript                                         | Resolved                                                                                                                         | Both harnesses use the existing auctioneer-cli, including bounded child protocol, native orchestration, source/build/artifact handoffs and the explicit frozen-artifact control. The five replaced scripts/tests are removed; browser JavaScript and SDK-owned external tooling remain. The repaired committed-source/predeclared release passed all 45 rows and its separate frozen control without relaxing criteria. Independent review, workspace/secure/feature tests, strict lint, all nine WASM checks and local vendor controls passed; the [final verification record](docs/testing/evidence/m6-verification-20261015.md#rust-harness-migration) retains exact scopes, failures and M7.6 limits.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| Rust harness cleanup and final publication could outlive or lose failure fences                | Resolved                                                                                                                         | Abort acknowledgement has a second deadline and retains unjoined handles; meter state is retained until writers are proven stopped. HTTP requires an external supervisor. Earlier cleanup failures outrank later meter errors and retain scratch; cancellation is checked after teardown before summary publication. A changed persisted profile is rejected before evaluated files are emitted. Each reproduced failure has a direct regression.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| Rust port changed stress-row and settings-probe semantics                                      | Resolved                                                                                                                         | Independently tracked ordinary offers/admissions allow real timeout-with-bids rows with zero ordinary successes, while empty/all-rejected/no-dispatch/no-decode work remains invalid. Settings checks retain their 1,000 ms budget and corrupt-config refusal uses the candidate timeout. Delayed real HTTP controls exercise both accepted and refused boundaries; the catalog and thresholds are unchanged.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| Native slow-response disconnect test reset once in the workspace sweep                         | Open, not reproduced                                                                                                             | The first Rust-cutover workspace run stopped in real_binary_recovers_all_aux_slots_after_slow_response_disconnects on a zero-byte connection reset. The exact case passed in standalone and original-workspace feature graphs; six concurrent-sibling runs passed 72 tests. No production defect or repair is established, no assertion/retry changed, and the failed log is retained. Native test maintainers should preserve caller stage, child exit/log and port/PID ownership on the next occurrence; the fixture currently accepts TCP-only readiness after releasing its reserved port, a possible ownership race, not a proven cause. This is separate from the older usersync_routes report.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| Full workspace lint rejected the configuration override test name                              | Resolved                                                                                                                         | Removed only the redundant test_ prefix inside cfg(test), preserving every assertion. The renamed test executes once in each feature state. Default, insecure-feature and conformance workspace Clippy pass with -D warnings; no suppression or production/harness behavior changed.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| Frozen invalid-consent control credited an unrelated valid proof                               | Resolved                                                                                                                         | Credit requires the exact supplied consent, and negative controls independently distinguish warning-free missing-consent denial from malformed/core-only refusal. Direct red/green tests, 156 native controls and strict CLI lint passed; the newly source-bound/predeclared release passed all 45 local rows and the actual frozen control with three invalid inputs and zero invalid credit. The [first capture and failed control](docs/testing/evidence/native-axum-rust-aefcf47-local-pass/README.md) remain unchanged.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Repeated debug JSON fields had no explicit wire contract                                       | Scope clarified and tested                                                                                                       | The streaming projector preserves repeated permitted occurrences, independently validates or masks each, and omits a body with any malformed allowed occurrence. Structured duplicate-field tests pin both valid occurrences, both masks and both malformed/valid orders; the alias gate also uses a literal permission truth table.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| Test metadata overrides disappeared from artifact identity                                     | Resolved                                                                                                                         | Native export includes the linked config dependency's test_harness_override feature. The bounded schema preserves it and production qualification remains unsupported; a dependency-only feature CI lane checks the actual export against an explicit expectation.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| Negative debug prices survived floating-point underflow                                        | Resolved                                                                                                                         | Negative nonzero significands cannot evade the price domain via -0.0. Exact-token tests retain valid signed-zero and positive controls under a documented lexical JSON exemption.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| Non-MIME debug headers bypassed the input cap                                                  | Resolved                                                                                                                         | Every header value is byte-capped before trimming or MIME parsing, with independent exact-boundary encoding/GPC/MIME controls.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| Suffix-bearing MIME types authorized exact-JSON debug capture                                  | Resolved                                                                                                                         | The shared MIME predicate now requires no suffix as well as application/json. Request projection, sealed response projection and debug header normalization reject application/json+secret and application/json+json; case-insensitive JSON, valid parameters containing + and empty-body precedence remain covered. Tests failed on the old predicate and pass after the repair; no raw-data leak was demonstrated.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| CI feature-mode probes selected their own expectation                                          | Resolved                                                                                                                         | Separate CI invocations declare literal expected states independently of cfg; mismatch tests and workflow mutations reject accidental default conformance or a missing dependency feature. Ordinary unselected-mode tests remain usable.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| SDK upgrade left tool versions and CI installers inconsistent                                  | Resolved in M6; source updated for M7                                                                                            | M6 aligned revision 596f9f4e, Rust 1.98.1, public dependencies and all six SDK tool declarations; its checks remain in the [SDK/toolchain verification](docs/testing/evidence/m6-verification-20261015.md#sdk-and-toolchain-alignment). The historical PR #275 checkpoint selected bc300ace; the current integration source and its limitations are recorded in the following adoption row. The f6c708cc-to-bc300ace update is one documentation-only commit with identical SDK crate tree, Cargo manifest and tools. The [scoped update verification](docs/testing/evidence/resource-sdk-update-20261018.md) records 97 passing pin/CI/artifact-identity tests. Historical captures retain their pins; this is not memory qualification or recertification of old measurements.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| PR #275 adoption changes outbound semantics and SDK bookkeeping | Adopted; resource correspondence open | Auctioneer pins 9fe6c56c648f0a601b1c6c051ce71c09d3733ea9 and uses prepared dispatch, paired time, immutable receipts and SDK-owned lifecycle helpers. Required admission/cache-bypass declarations, absolute deadlines and independent payload caps remain. Earlier-own-deadline timeout receipts use logical expiry; Spin advertises BestEffort header fidelity. Surviving recognized coding on partial responses fails closed before parsing. The [original adoption](docs/superpowers/plans/2026-10-30-edgezero-outbound-adoption.md) and [current SDK update](docs/testing/evidence/edgezero-pr275-update-20261030.md) distinguish source audit from executed verification. Complete opaque-host allocation bounds remain Unsupported; scope/task/ledger owners and the new Fastly request-scoped registration recorder still need independent correspondence proofs to funded Auctioneer charges. No workload limit, coefficient, ceiling or frozen evidence was changed. |
| Existing exchange tests do not compile in broader guest CI checks | Resolved locally, 2026-10-30 | Four filesystem-backed provider-conformance modules are explicitly native-only; their portable fake-based controls remain compiled for guests, and the real provider tests still execute natively. The construction fixture uses JsonField::Value rather than a removed Option API. All 641 exchange tests pass across 36 native targets, and the unchanged all-target adapter/exchange/privacy strict-Clippy scopes pass for Cloudflare, Fastly and Spin. No ignored test, CI narrowing or lint exemption was added. See the [repair plan and verification](docs/superpowers/plans/2026-10-30-bidder-maintainability.md). This does not qualify hosted execution or close SDK resource-accounting gaps. |
| Bidder roots and duplicated protocol infrastructure do not scale to later waves | Resolved for all 17 implemented bidders | Every executable owner now has compact factory/trait wiring plus ordinary request.rs and response.rs modules; larger protocols have cohesive parameter/projection/native helpers. Unit tests remain inline, conformance stays owner-local, and all 716 explicit owner test declarations are retained. A discovery-driven source guard pins the layout, the shared status/header helpers preserve intentional protocol differences, and an inert template lives outside catalog discovery. The [17-path audit](docs/testing/bidder-shared-contract-audit.md) records reuse and nonreuse. Captures and schemas are unchanged; two authored manifest notes were rewrapped with parsed-value equality and only their local checksum witnesses updated. No bidder activation, workload/budget change or new resource qualification is claimed. |
| Shared fake retained the old monolithic layout and scratch builds copied only its root | Resolved in the layout follow-up | The feature-gated fake now uses compact trait/factory wiring plus controls.rs, request.rs and response.rs, with unchanged public controls and inline helper units. The layout guard covers it without serving registration. Scratch installation copies the complete module tree and verifies every path and byte; its stale registry reexport is removed while the real fail-closed wrapper is preserved. A comparison fixture now owns one sealed decode receipt per invocation, with unchanged payload, cap and assertions. The [follow-up record](docs/superpowers/plans/2026-10-30-bidder-maintainability.md#shared-fake-follow-up) separates these repairs from resource readiness. |
| Transitive derive dependency triggers a future Rust incompatibility                            | Open, dependency maintenance                                                                                                     | Rust 1.98.1 reports proc-macro-error2 2.0.1 re-exporting a private proc_macro extern crate at src/lib.rs:494, reached through validator_derive 0.20.0. Current builds and strict lint pass. Before the next compiler upgrade, dependency maintainers must resolve the reported E0365 through a verified compatible upstream update; no local dependency patch or warning suppression was added.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| Native retained-growth endpoint preceded work/window completion                                | Resolved in M6                                                                                                                   | Final RSS starts after both completion boundaries and consumes the unchanged 128-sample cap. Deterministic late-growth, minimum-window, final-read failure/cancellation and cap controls supplement the real 32-MiB service reproducer; stale growth is not reused.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Duplicate provider evidence fields were discarded before validation                            | Resolved in M6                                                                                                                   | The bounded file reader rejects duplicate decoded keys recursively before Value conversion, including escaped/nested/ignored fields. Real CLI authorization and artifact-identity controls distinguish single-value refusal from duplicate-input rejection; valid offline evidence still grants no deployment readiness.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| Fastly archive expansion preceded Auctioneer's resource checks                                     | Resolved in M6                                                                                                                   | A CLI-only Rust tar/gzip preflight bounds raw records, file count, individual/aggregate payload and the entire expanded stream before SDK lookup or extraction. GNU names remain bounded; structural PAX path/size/link/sparse overrides are refused to avoid parser disagreements. The pinned SDK still verifies the complete portable archive.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| Archive tail and member limits were masked by later validators                                 | Resolved in M6                                                                                                                   | Direct raw/extracted-stage tests pin hidden post-terminator members in both gzip layouts, cumulative entry/file ceilings and oversized zero-only expansion. Two redundant raw/semantic file-count checks were removed: raw admission owns upper bounds; shared metadata validation owns completeness. All fourteen isolated guard-removal probes compile and fail their intended tests; see the [archive guard record](docs/testing/evidence/m6-verification-20261015.md#archive-guard-ownership-october-16).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| Archive and extracted-bundle metadata policy could drift independently                         | Resolved in M6                                                                                                                   | Both callers now use ReleaseMetadata::validate for format, lifecycle, adapter, source revision, safe unique paths/digests and exact member-set policy. Each identity-term removal fails both caller tests; direct member-policy controls are independent of filesystem/SDK backstops.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| Evidence header described a superseded SDK pin as current                                      | Resolved in M6                                                                                                                   | Header now distinguishes the original 65312643 matrix from later scoped checks on 596f9f4e/Rust 1.98.1. Historical counts and captures are unchanged.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| Raw JSON count could be mistaken for a literal-count allowance                                 | Documented, unchanged                                                                                                            | io.rs has thirteen br# literals but twelve object-leading matching source lines. The array-leading literal is outside the existing object-fixture gate. Allowance remains twelve; the repository gate still reports 169 fixtures across 50 files and all rejection controls pass.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| Functional Clippy exceptions were allowed workspace-wide                                       | Resolved in M6                                                                                                                   | unused_async_trait_impl and missing_trait_methods are again inherited restrictions; justified exceptions are limited to affected impls, including stable Drop's unavailable pin_drop method and deliberately retained external defaults. inline_modules and inline_trait_bounds remain explicit repository-wide style policies, preserving colocated unit tests and existing generic syntax. Native workspace/default/insecure/conformance and all three actual CI WASM lint scopes pass. These lint-only edits change no runtime behavior, dependency pin or handling of the transitive compiler warning.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| Evidence writer sync failure lacked an isolated regression                                     | Resolved in M6                                                                                                                   | Fault-injected writer controls require completed body/newline writes before sync, propagate sync failure with a fixed error, and reject a mutation removing the sync call. This is writer-boundary evidence, not a real disk-full filesystem test.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| Qualification file newline and override-feature controls were inconsistent                     | Resolved in M6                                                                                                                   | Standalone evaluation writes one newline-terminated JSON record like the harness; the CLI test uses test_harness_override alone. Historical artifact bytes/digests remain unchanged.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| Runner/evaluator fail-closed ownership was implicit                                            | Documented in M6                                                                                                                 | The runner records observations; the evaluator enumerates the declared schedule and missing rows cannot be promoted by successful siblings. Cleanup/cancellation publication fences and evaluator acceptance remain separate required boundaries.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| Fixture count/ID checks duplicated the shipped child implementation                            | Intentional defense, covered                                                                                                     | Keep runtime checks at the deserialized child-protocol boundary. Independent controls reject mismatched auction counts, mismatched complete counts and repeated success IDs; a future fixture implementation must not silently weaken reconciliation.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |

The [closure plan](docs/superpowers/plans/2026-10-13-usersync-review-closure.md)
and [verification record](docs/superpowers/plans/2026-09-30-m5-verification.md#cookie-integrity-and-review-closure-october-13)
record implementation scope, executed suites, mutation evidence and
qualification limits. These are recorded implementation results, not a claim
that every deployment surface has been qualified.

### Auction Configuration Repair: October 15

The original Value-based account discovery could lose a named publisher when an
unrelated opaque numeric token exceeded f64. The bounded structured resolver now
preserves that identity and the disabled-account 503 before typed preparation.
This is an identity/refusal repair, not evidence of a successful outbound privacy
bypass. Malformed/trailing discovery input is BadInput rather than absent identity;
the public handler already checks syntax before discovery.

Exact-wire resolver and generated-App regressions cover the overflow, snapshot
replacement during suspension, refusal precedence and recovery. The expanded
configuration acceptance command passed 464 library and 93 integration tests;
independent spec/quality review approved after reconciling two old resolver tests.
See the [M6 execution record](docs/superpowers/plans/2026-10-07-m6-parity-hardening.md#6-execution-record).
The separate intermittent usersync_routes investigation remains open.

### Self-review repairs: October 14

The self-review at `99c7ee1` reproduced three additional findings. Spin 4.1.0
rejected each optional secret without a default, even when values were supplied;
the previous generic-TOML tests asserted that invalid declaration. Both secrets
now default to `""`, not a credential. Existing health/keyring logic still fails
closed. The CLI-dependent manifest test reaches the deliberately missing local
component only for valid declarations; removing either default must instead
fail variable validation. CI explicitly runs this ignored test and guards the
command against removal, skipping, and ignored failures. It is a loader check,
not guest execution or proof of secret-provider delivery.

The self-review observed an intermittent routed boundary failure and measured
that fractional timestamp precision can change signed payload lengths. The
fixture now installs the existing `FakeClock` at a fixed nine-digit nanosecond
timestamp. This controls an input; it does not establish the cause of the
separately reported route flake. A regression checks the installed wall clock,
and the original acceptance/refusal and no-partial-write assertions remain.
Production clocks, timestamp serialization and the 4096-byte cap are unchanged.

The encoded duplicate-UID test now includes one correctly bound bidder macro.
A scratch-only removal of the config UID-count check compiles and changes its
result from `InvalidConfig` to `InvalidCallback`, failing the repaired assertion
instead of passing on a missing bidder. A valid single-UID control still passes.

### Provenance guard follow-up: round five

The round-five report targeted `99c7ee1`; this follow-up starts at `03bb984`.
The verifier already enforced the source revision, but no test supplied a
wrong revision while retaining valid source fingerprints. Exact-error tests
now vary that revision and each of the six privacy row counts independently.
Additional controls exercise undeclared typed fields, zero/nine/eleven capture
manifests, wrong manifest versions, license and pixel digest mismatches on
either side, and GPP item/identity/tuple/string/duplicate/ID requirements.
Private manifest parsing and attribution helpers expose these existing checks
to tests; the actual validator still calls them using compiled-in pins.

All 19 temporary single-guard mutations compiled and failed the targeted
assertions. The normal Rust utility passed 26 tests and its real-tree CLI
verified all ten original captures without writing files. No expected output,
source fingerprint, revision constant, or artifact pin changed. These checks
protect declared provenance; they do not re-execute the external reference or
qualify an upstream source archive that was not supplied.

The carried-over cookie-preflight and flattened-map findings were rechecked
from current source, not commit ancestry. The allocation guard in
`usersync_handler_alloc.rs` and exact-error negative guard in
`disclosure_codegen.rs` both execute and pass on the current branch. Their
existing fixes remain in place; no server-code changes were needed here.

### Approved slim-checker boundary: October 14

AG approved simplifying the reference utility to artifact integrity and
provenance. The sole command is `validate --root <checkout-or-staged-tree>`
with optional `--source`; the old staging command is deliberately removed.
Version-2 compiled-in pins retain all ten paths, digests and byte counts.
No captured answer, source fingerprint, revision or attribution changes.

Operation input/expected schemas now belong only to their existing consumers,
not a second generic engine in the utility. The core reference tests pin every
corpus count. Privacy tests pin all six groups, unique scrub names and ordered
valid fold pairs. Compiler-typed GPP vectors and parser tests own tuple shapes,
count, unique headers, ordered IDs and decoded content, replacing `syn` parsing.
The historical round-five controls above describe the previous implementation;
they are not a claim that removed CLI schema checks still exist.

Bounded reads, unsafe-path/symlink refusal, unchanged hashes and sizes, fixed
revision, capture metadata, source-call provenance, unique case IDs, attribution,
Rust-only repository tooling and optional source fingerprints remain in the
utility, with direct negative controls. No production crate dependency or
runtime behavior is added. Fresh execution results and limits are recorded in
the [verification record](docs/superpowers/plans/2026-09-30-m5-verification.md#slim-reference-integrity-checker-october-14).

### Consumer ownership follow-up: October 14

Feedback against `0418dae` exposed three real coverage losses after slimming
the utility: chooser fields, operation dispatch/cardinality and selection
evidence labels. Temporary consumer copies reproduced the gaps without
depending on capture hashes. The repaired owners reject all eight probes:
missing chooser `evaluated`/`privacy_calls`, an extra expected field, a missing
input field, two renamed cookie filters, a renamed cookie operation and a
changed selection label. All 21 choice rows are checked, not only named samples.
No capture or pin changed, and no generic schema engine was restored.

The duplicate-UID integration test now names its actual config-layer boundary
and requires `user_sync.redirect_url must end in one uid macro field`.
`rendered_callback_requires_one_selected_bidder_and_terminal_uid` already
directly exercises encoded UID keys with `InvalidCallback`; the two layers
are not conflated. The utility's permissive operation-field test is only an
architectural boundary marker, not evidence of behavioral consumer coverage.

The cookie preflight is already allocation-pinned. Its current measured test
passed, and a freshly compiled temporary `minimum > hard_cap && false` mutant
failed with five allocations instead of the permitted one. The flattened-map
negative guard also passed on current M5; a missing historical commit does
not imply missing current code. The local merged-main snapshot still lacks
that named negative test, so PR #18 carries that coverage to main.

The route flake remains open and unreproduced. Twenty complete fresh-process
sweeps of all 20 routed tests, with eight test threads and separate logs,
passed. Setup now asserts idle aux, aux-work, auction-egress and static-egress
gates, providing a specific diagnostic for inherited reservations. It does
not reset or replace process-global gates. All current routed boot/dispatch
tests use their shared mutex, including the two tests whose helper acquires it.
Do not treat these observations or the fixed clock as a root-cause repair.

Next evidence needed: preserve the first failing full `usersync_routes` log
with `--nocapture --test-threads=8`, including the test name, status/body,
clock input, discovered UID boundary and idle-gate diagnostic. Determine whether
that failure is inherited gate state, an in-test retained response or a different
condition before changing production gate ownership. This is an investigation,
not an established production permit leak.

---

### Callback and response guard follow-up: October 14

The follow-up to `308f270` adds source-local exact-error controls for each
reachable redirect userinfo/fragment check, the non-slash suffix after
`{{.ExternalURL}}`, user-macro bounds/reserved syntax and syncer-key bounds/token
grammar. Base external URLs and allowed origins now independently exercise
username, password, query and fragment refusal too. A hostless HTTPS URL is
already refused by URL parsing/strict spelling; the later host-presence check
is defensive redundancy, not a separately mutation-proved decision.

The encoding-label controls use the real body reader, an unpollable stream,
literal expected labels and exact error bodies. They vary hostile syntax,
blank values, non-ASCII bytes and 128/129-byte boundaries rather than relying
on benign `gzip` alone. The response Debug remains redacted.

Shared browser decorators now enforce the fixed 16-field application ceiling
after policy application, counting duplicate values and Set-Cookie as fields.
The 15/16/17-field cases keep serialized bytes below their independent cap.
All three handlers derive byte caps from runtime accounting constants, rather
than repeating numeric literals. Overflow still refuses the whole response.
Getuids serialization is independently checked at its exact escaped-body
boundary, including the newline. This is a serializer test, not a claim that
the stricter signed-cookie input ceilings permit a 256-KiB route response.

The current flattened-map negative test is already on M5. No historical
cherry-pick is required. The separately reported route flake remains open;
these changes do not claim to repair process-global gate ownership.

### Application Limits Versus Accounting: October 14

The wording in `34ff0a4` overstated the relationship between the fixed header
ceiling and funding. Accounting reserves the larger of the profile's ingress
header allowance and the sync constants: a 128-field profile reserves 128,
but application enforcement remains 16. The byte allowance can likewise be
larger than the fixed application cap. Neither application check measures
framing fields added later by an adapter/provider; no final wire-level count
or byte ceiling, or universal number of extra fields, is claimed.

The count test now derives below/at/above boundaries from the constant and
measures the browser-policy field contribution. A separate shipped-value
assertion still pins 16, so changing both implementation and boundary inputs
cannot silently alter that contract. The budget test independently covers
small, equal and larger profiles for count and bytes.

The carried duplicate-UID and cookie-preflight claims were rechecked against
current source. The old `callback_rejects_percent_encoded_duplicate_uid_key`
name is absent; `config_rejects_encoded_duplicate_uid_before_callback_rendering`
requires the exact config diagnostic, and the private rendered-callback test
separately requires `InvalidCallback`. The allocation preflight test remains
present and is re-executed with a freshly compiled branch-disabled control.
These are existing repairs, not new fixes in this round.
The control compiled and failed at five allocations versus one; both updated
application-count guard removal controls also compiled and failed their exact
targeted test. Fresh suites passed 447 core-library, 161 runtime and 31 focused
cookie/template assertions, with strict Clippy and format/fixture gates clean.

---

## Build valid JSON fixtures with `serde_json::json!`, not raw strings

**Status:** resolved. The original sweep converted valid fixtures; every
remaining raw site was annotated with its exemption and budgeted in the
gate's allowlist.
Kept here because the exemption categories and the gate's design are
what a future reviewer needs in order to judge a new raw fixture.

### The problem

Test fixtures that need a valid JSON object were written as raw string
literals:

```rust
let req = parse(r#"{"id":"r","imp":[{"id":"i","banner":{}}],"app":{}}"#);
```

`serde_json::json!` rejects a missing brace or comma at compile time,
carries no escaping, and diffs one field at a time. A raw string gets
none of that, and the codebase had drifted into both conventions at
once — `json!` in 31 files, raw strings in 30 more.

### Original sweep results

The counts and crate names in this subsection describe the original sweep,
not today's suite or allowlist. The transport tests subsequently moved into
`crates/auctioneer-adapter-axum/tests`.

40 sites converted, 74 left raw with a comment at each site naming the
exemption that applies:

| crate                       | converted | exempt |
| --------------------------- | --------- | ------ |
| auctioneer-openrtb              | 30        | 0      |
| auctioneer-gvl-schema           | 8         | 5      |
| auctioneer-core                 | 1         | 20     |
| auctioneer-exchange             | 1         | 16     |
| auctioneer-runtime              | 0         | 21     |
| auctioneer-privacy              | 0         | 4      |
| auctioneer-axum-transport-tests | 0         | 5      |
| auctioneer-config               | 0         | 2      |

`auctioneer-openrtb` swept clean: `test_support::round_trip` and
`prepare_integration`'s `parse` helper now take a `serde_json::Value`
rather than a `&str`. Both already compared as `Value`, so no
assertion changed.

Test count before and after: 97 binaries, 1160 passed, 0 failed, 2
ignored — identical, including the per-binary distribution.

### Why 74 sites stay raw

Each has a comment at its own site, so this list is for orientation
rather than re-derivation. One module-level note covers all 21 sites in
`bounded_json.rs` and one builder-level note covers all 8 in
`response_factors.rs`: they share a reason, and repeating it per site
would be noise.

- **Deliberately malformed input** (`br#"{"known": }"#`). `json!`
  cannot express input that does not parse — it would not compile.
- **Byte-string and `&'static str` constants.** `json!` yields a
  `Value`, which no `const` can hold. Converting means a function
  returning a `String`, re-rendered at every use.
- **Fixtures whose exact text is the measurement.** `bounded_json.rs`
  hand-counts each literal's syntactic node count in the assertion
  beside it (14 for `FIXTURE`, 3 for `{"New":1}`, 5 for
  `{"Pair":[1,2]}`). `response_factors.rs` reads each body's own
  `len()` as the denominator of every memory-charge ratio. Three sites
  read a fixture's `len()` for a Content-Length or a gzip size, and the
  `auctioneer-config` snapshots are compared with `assert_eq!` against
  `to_string`. This is the category that bites: convert one and the
  guard stays green while measuring something else.
- **`format!` templates splicing JSON TEXT, not `Value`s.** The
  `auctioneer-gvl-schema` manifest templates splice bare fragments
  (`"gcl": {…}`, a trailing `, "field": value`) and their callers
  rewrite the rendered output with `replacen` on its literal spelling.
- **Original dependency-boundary exemption.** At the time of the sweep,
  converting the four privacy fixtures would have introduced a new
  dependency solely for formatting. The production GVL/GCL parse path
  remains delegated to `auctioneer-gvl-schema`; privacy tests now have a
  `serde_json` dev-dependency, so absence of that dependency is not a reason
  to introduce new raw valid-JSON fixtures.

One more site is not an exemption but a genuine judgement call:
`node_charge.rs` parses `{"b":0,"a":0}` and asserts the keys come back
sorted, which is what proves `serde_json::Map` is `BTreeMap`-backed.
`json!` would construct the map directly and so could not observe the
parser at all.

### The guard that keeps it swept

`scripts/check-raw-json-fixtures.sh` fails on a raw JSON-object fixture
that is not budgeted in `scripts/raw-json-fixtures.allow`. The
allowlist records a per-file COUNT, not just a path: a new raw fixture
dropped into an already-exempt file still fails, which a path-only
allowlist would have waved through. A budget with slack in it fails
too, since slack silently re-admits the next raw fixture.

`scripts/check-raw-json-fixtures-fixtures.sh` is its negative case —
eight planted defects the gate must reject, plus the unplanted tree it
must accept. Three of them caught a real bug in the gate on first run
(it resolved paths against the checkout rather than the tree it was
handed, so every verdict missed when run over a copy).

### The last 6 sites, and why they stay raw

These were the final sites examined. All six are `br#"..."#` byte
strings fed to APIs typed `&[u8]`, so they are exemption category 2;
the two in `sink.rs` are also category 3, since the assertion compares
the accepted body against those exact bytes. Each carries a per-site
comment naming its exemption.

```
crates/auctioneer-bidder/src/bidders/generic/mod.rs   1
crates/auctioneer-bidder/src/bidders/pubmatic/mod.rs  3
crates/auctioneer-bidder/src/sink.rs                  2
```

Converting them would mean `.to_string().into_bytes()` at each call
site to turn a `Value` back into the bytes the API wants — strictly
worse than the literal. They are budgeted in
`scripts/raw-json-fixtures.allow` as exempt, not as pending work;
there is no `NOT YET SWEPT` heading to drain.

---

## "Optional cookie signing and encryption remain disabled" describes a switch that does not exist

**Status:** resolved in the 2026-10-13 repair. AG approved the
[integrity design](docs/superpowers/specs/2026-10-13-usersync-cookie-integrity-design.md)
and implementation: versioned HMAC envelopes, one bounded atomic EdgeZero
secret read, rotation, unsigned-state rejection, 4-KiB complete writes and
explicit browser deletion. Encryption and callback UID ownership are not
claimed. The old verification sentence is corrected. The diagnosis below
records the pre-repair state, not the current contract.

### Original finding (before the repair)

The verification note previously read:

> Optional cookie signing and encryption remain disabled.

That implied an operator-enableable mechanism, but the cookie was only
base64url-encoded JSON. Signing had been a non-goal with no milestone owner.
AG subsequently approved implementing integrity in M5, not merely correcting
the sentence. Config signing is a separate trust boundary and was not changed.

### Implemented resolution and evidence

- [The signed codec](crates/auctioneer-core/src/usersync/signed.rs) authenticates
  the version, key ID and payload in a domain-separated HMAC-SHA256 envelope.
  All three production UID readers reject unsigned or tampered state; there
  is no unsigned fallback.
- [Key loading](crates/auctioneer-core/src/handlers/usersync_keys.rs) uses one
  bounded atomic default EdgeZero secret-store snapshot per request, with
  explicit rotation and redacted failures. Unavailable keys refuse processing
  instead of producing unsigned writes.
- Complete `Set-Cookie` output is capped at 4 KiB, including envelope and
  attributes. Deletion expires the same Path/Domain cookie.
- [HTTP integration tests](crates/auctioneer-core/tests/usersync_integrity.rs)
  exercise rejection, secret failures, rotation, tampering and deletion;
  codec mutations also detect first-byte-only MAC verification and swallowed
  authenticated payload-limit errors.
- [Provisioning documentation](docs/deployment/usersync-cookie-keys.md),
  the M5 design, master spec, roadmap and verification note now assign signing
  to M5 and describe the actual mechanism.

Encryption, callback UID ownership, account authorization and replay
protection are not provided by this MAC and are not claimed as fixes.

---

## The sync-URL encoding and endpoint-identity guards have no tests

**Status:** resolved through the 2026-10-14 follow-up. The initial repair
covered GPP/USP/SID and endpoint identity; `afe569a` added GDPR cases and
`72bae69` added the named consent case. That consent case initially omitted
the direct bidder endpoint slot. The shared assertion now includes all five
macros in both the callback and direct endpoint, and ten isolated encoding
mutants are rejected. Immutable source fixtures are retained, not edited
into artificial reference observations. The diagnosis below is historical.

### Original finding (before the repair)

The template path spliced privacy values into an outbound sync URL and then
validated the result. Both steps were implemented correctly, but their
individual guards were untested. Line references and mutation counts below
describe that pre-repair version.

**Encoding (`template.rs:251-255`).** Three of the five callback splice
points can drop `encoded(...)` with no test failing. Measured against
the whole core suite, not a subset:

| line   | macro          | failures | reachable today |
| ------ | -------------- | -------- | --------------- |
| `:252` | `.GDPRConsent` | 1        | killed          |
| `:253` | `.USPrivacy`   | 0        | latent          |
| `:254` | `.GPP`         | 0        | **yes**         |
| `:255` | `.GPPSID`      | 0        | latent          |

`.GPP` is the live one. `gpp::parse` validates the header and the
`~`-delimited structure only; each section payload is stored as a
verbatim `&str` slice with no charset check, and `Envelope::raw()`
returns the original string. An attacker therefore controls those bytes.
With the encoding dropped, `gpp = "DBABMA~&forged_param=1&uid=STOLEN"`
emits `forged_param` and `uid` as real query parameters the bidder
reads. `.USPrivacy` and `.GPPSID` survive only because `Usp::parse`
admits `[1][YN-]{3}` and `GppSid::parse` admits digits and commas — they
are protected by a neighbour's charset, not by this code.

**Endpoint identity (`template.rs:200-206`).** `validate_emitted_endpoint`
is the last line of defence on an emitted URL. Each of its six
sub-checks — scheme, host, port, username, password, fragment — can be
deleted individually with no test failing. Only the existence of the
call is pinned, and only by byte-length tests that notice incidentally.

The two interact. With `.GPP` encoding removed but the validator intact,
a hostile `gpp` carrying `#` is still refused — caught solely by
`parsed.fragment().is_some()` at `:205`, the check with zero tests.
Remove both and the fragment truncates the URL, destroying the `back=`
callback so the sync never returns, and delivering the fragment into the
bidder's page context.

### Why the corpus cannot see it

`crates/auctioneer-bidder/tests/conformance/fixtures/usersync/templates.json` records strings
in all 12 cases: GDPR is `""` or `"1"`, consent is `"consent-test"`, USP is
`"1NYN"`, and GPP/SID are `""`. These are all form-safe, so the fixture is
structurally unable to distinguish `encoded(x)` from `x`. The test requires
strings; it does not turn nulls into empty values.
This is the recorded unvaried-parameter shape: a term proportional to a
constant test input is indistinguishable from its absence. The earlier
consent mutation count came from a separate behavioral case, not these
captured fixtures; it was not proof that both splice points stayed guarded.

### Implemented resolution and evidence

[Rendering tests](crates/auctioneer-core/tests/usersync_template.rs) have named
hostile-value cases for GDPR, GDPRConsent, USP, GPP and SID. The shared
assertion checks every macro in the callback and directly in the bidder URL,
including full URL bytes, decoded values, exact parameter counts and no
injected parameters or fragments. The six endpoint checks have
separate [colocated unit tests](crates/auctioneer-core/src/usersync/template.rs)
for scheme, host, port, username, password and fragment.

The October 13 record covers sixteen selected template mutations. On
October 14, the direct consent-splice mutation was first shown to survive
the named consent test. After strengthening the assertion, all ten encoding
mutants (five macros times two splice points) fail their named test after
successful compilation. The immutable captured `templates.json` was not
rewritten with synthetic observations; adversarial inputs live in the
behavioral tests instead.

---

## A redundant cookie pre-check is indistinguishable from a bypass

**Status:** resolved in the 2026-10-13 M5 repair. The site now explains that
the full-field check is authoritative and the minimum-size check only avoids
formatting a value already known to exceed its cap.

### Original finding and implemented resolution

The minimum-size pre-check could be disabled without failing tests because
the complete-field check already subsumed it. Historical line references
were `cookie.rs:495` and `:515`.

It is genuinely redundant rather than a hole: `minimum` counts name plus
domain plus encoded value, while the check at `:515` measures the
complete serialized field, which always includes those plus the
attributes — so `:515` subsumes it. The early-out is a defensive
optimisation.

The missing explanation is now present at the
[preflight site](crates/auctioneer-core/src/usersync/cookie.rs): it avoids
formatting a value already known to exceed the cap. The complete serialized
field check remains authoritative. This was a diagnostic clarity fix, not
the discovery or removal of an authorization bypass.

---

## Account-level GDPR opt-in is enforced by production and by no test

**Status:** resolved in the 2026-10-13 repair. Real permission tests vary
account and web opt-in independently with host TCF disabled, malformed TC,
valid denial and grant. Both account branches are mutation-guarded; the
production enablement semantics were retained.

### Original finding (before the repair)

The permission-enable ladder was correct in production, but its account
branches had no independent regression coverage. The original code excerpt,
line references and mutation count below describe the pre-repair tests:

```rust
let gdpr_permissions_enabled = input.host.gdpr.enabled
    && match input.account.gdpr.channel_enabled.web {
        TriState::Enabled => true,
        TriState::Disabled => false,
        TriState::Unset => match input.account.gdpr.enabled {
            TriState::Enabled => true,      // no test covers this arm
            TriState::Disabled => false,    // nor this one
            TriState::Unset => input.host.gdpr.tcf2.enabled,
        },
    };
```

Changing `TriState::Enabled => true` to `false` fails **0 of 399**
privacy tests. Measured with `--no-fail-fast`, so the count is complete
rather than truncated, and with zero compile errors.

### Why it matters

Host sets `gdpr.enabled = true` but `gdpr.tcf2.enabled = false`. An
account opts in explicitly with `account.gdpr.enabled = Enabled`. A
request arrives with `gdpr=1` and a malformed consent string. The
correct outcome is `MalformedTcf` — fail closed.

With that arm returning `false`, `gdpr_permissions_enabled` is `false`,
`cookie_sync_host_permission()` returns `Allowed` at
`permission.rs:192`, taken because the guard at `:191` tests
`!self.gdpr_permissions_enabled`, and every TCF, vendor-consent, purpose-1 and
publisher-restriction check is skipped for the whole request. An account
that explicitly asked for GDPR enforcement silently receives none, and
no test fails.

The sibling `TriState::Disabled => false` arm is equally uncovered,
though it mutates toward more enforcement rather than less.

### Why the suite misses it

`malformed_tcf_is_ignored_when_gdpr_permission_path_is_inactive`
(`crates/auctioneer-privacy/tests/sync_permissions.rs:796`) enumerates only
`"host off"`, `"web off"` and `"signal no"`. It never exercises the
`account.gdpr.enabled` arm, nor the `host.gdpr.tcf2.enabled` fallback
beneath it.

### Implemented resolution and evidence

`account_opt_in_permission_ladder_includes_disabled_host_tcf` in
[sync permission tests](crates/auctioneer-privacy/tests/sync_permissions.rs)
varies account-global and web enablement independently with host TCF off.
It asserts the actual permission result for malformed consent, valid denial
and grant, including both account-enabled and account-disabled branches.
Removing account opt-in enforcement or changing its sibling branch is
detected by the recorded mutation checks. Production enablement semantics
were retained; this fix closes the missing guard rather than introducing
a new GDPR policy.

---

## `host_policy_name`'s two disjuncts hide each other

**Status:** resolved in the 2026-10-13 repair. Separate basic-vendor and
vendor-exception inputs pin each host-name exclusion independently, including
empty/NUL candidates. Removing either disjunct now fails its tests.

### Original finding (before the repair)

The following diagnosis and line references describe the old fixtures,
which populated both exception lists identically.

`crates/auctioneer-privacy/src/sync/permission.rs:279-292` derives a
synthetic host "bidder name" matching neither `basic_enforcement_vendors`
nor purpose-1 `vendor_exceptions`, so the host can never inherit a
bidder-only exception. Deleting the whole function is caught. Deleting
**either disjunct alone is caught by nothing**.

### Why the fixture cannot see it

`crates/auctioneer-privacy/tests/sync_permissions.rs:498-505` sets both
lists to the same value:

```rust
account.gdpr.basic_enforcement_vendors              = vec![String::new(), "\0".into()];
host.gdpr.tcf2.purposes[&1].vendor_exceptions       = vec![String::new(), "\0".into()];
```

Whichever disjunct survives still advances the name past both entries,
so the loop terminates identically and the test stays green. This is the
recorded unvaried-parameter shape — two inputs held equal make each
term indistinguishable from its absence. It is the third instance of
that shape found in M5.

### Why it matters

Account sets `basic_enforcement_vendors = [""]`; host sets purpose-1
`vendor_exceptions = ["\0"]`; `enforce_purpose` and `enforce_vendors`
are both `Enabled`; the TC string grants neither purpose-1 nor vendor
consent. With the `vendor_exceptions` disjunct dropped,
`host_policy_name` returns `"\0"`, which _is_ in `vendor_exceptions`, so
`is_vendor_exception` returns true and `purpose_allowed` grants.
Measured outcome: `Allowed` where `BlockedByPrivacy` is correct — the
host obtains Purpose-1 device-storage permission with no consent, which
is exactly what this guard exists to prevent.

### Implemented resolution and evidence

`host_permission_never_uses_configured_bidder_exceptions` in
[sync permission tests](crates/auctioneer-privacy/tests/sync_permissions.rs)
now varies the two lists independently, including empty and NUL candidates.
Each exclusion is the sole reason for advancing in at least one case.
Removing either disjunct is detected by its own mutation check; production
host-name selection remains unchanged.

---

## The 1843-line sync reference corpus asserts nothing about Auctioneer

**Status:** resolved in the 2026-10-13 repair. `sync_reference.rs` drives
all 71 immutable rows through real extraction: 49 agreements and 22 explicitly
named differences under the existing Auctioneer contract. Each difference pins
the Auctioneer result and asserts it differs from the frozen reference; its reason
is documented in the corpus README. No captured reference output was changed.

### Original finding (before the repair)

`crates/auctioneer-privacy/tests/data/sync/reference.json` holds 71 rows
(40 `cookie_privacy`, 13 `setuid_privacy`, 18 `parse_sid`) of real
captured output from the pinned prebid-server revision, obtained through
a Go overlay calling the upstream private functions, with hashed
provenance. The data is genuine and worth having.

No test drives a single row through Auctioneer. `usersync_reference.rs`
asserts integrity only: schema version, revision/license/generator pins,
non-empty and unique ids, `provenance.kind`, hash presence, and
`cases.len() == 71`. `sync_sources.rs` never opens the file; it
hand-writes its own 12 cases.

The three apparently behavioural assertions in
`privacy_reference_keeps_upstream_results_separate_from_auctioneer_policy`
compare a row's `expected` value against the same fixture that was just
loaded. That is the file agreeing with itself: it would pass if Auctioneer's
behaviour were arbitrarily wrong.

So 68 of 71 rows are read by no assertion, and the remaining 3 are
self-referential. The file's own header states the missing step:
"Production cookie/chooser/handler tests must also consume these
fixtures."

### What is already right

`divergences.json` is the opposite and needs no change: 7 cases, each
with `auctioneer_design` provenance pointing at a spec path the test checks
exists, each with a stated rationale, and `reference_case` cross-links
that resolve into the reference corpus. One row carries
`"permission_grant_claimed": false`, explicitly declining to overclaim.
These are documented deliberate differences, not known failures being
waved through.

### Implemented resolution and evidence

`every_reference_row_drives_real_sync_extraction` in
[sync_reference.rs](crates/auctioneer-privacy/tests/sync_reference.rs) executes
all 71 rows through real Auctioneer extraction: 40 cookie, 13 setuid and 18 SID
rows. It asserts 49 agreements and 22 explicitly named differences under
the existing Auctioneer contract. Each difference pins the Auctioneer result and
asserts inequality with the frozen reference; an unexpected difference is
not accepted.

The [extraction-divergence artifact](crates/auctioneer-privacy/tests/data/sync/extraction-divergences.json)
records the exact Auctioneer result, named reason, design references and rationale
for all 22 extraction differences; the test no longer keeps an inline allowlist.
The [corpus README](crates/auctioneer-privacy/tests/data/sync/README.md) explains
the separation. The seven separately
captured design-divergence rows remain additional evidence, not a complete
allowlist for these 71 comparisons. No captured reference output or its
provenance was changed to make the test pass.

---

## `ccpa_enforce` is a dead `pub` field

**Status:** resolved in the 2026-10-13 M5 repair. Removed the unused field and
ladder. Existing USP propagation remains; no new CCPA/COPPA sync-denial stage
is implied. Auction privacy enforcement is unchanged.

### Original finding (before the repair)

The sync context computed `ccpa_enforce` through a three-level `TriState`
ladder, exposed it as `pub`, and printed it in `Debug`. Nothing consumed it;
the historical line references were `sync/mod.rs:102,255-263`.

Relatedly, `selected_usp` never reaches a deny decision. Its single
production use passes the string into `SyncPrivacyMacros` for bidder-URL
substitution (`cookie_sync.rs:359`), so a `us_privacy=1YYN` sale
opt-out does not block a sync.

**The absent denial is correct per spec** — the M5 design preserves the
pinned master's candidate ladder, which defines a CCPA callback but does
not invoke it. That is not the issue. The issue is that a `pub` field
named `ccpa_enforce`, computed through a real ladder, reads as a wired-up
CCPA gate to anyone scanning the struct. This repo already learned that
shape once, in M3b: helpers built while the live path bypassed them.

For the same reason, worth recording explicitly: **M5 sync has no COPPA
decision path at all.** COPPA appears only on the auction path
(`resolve.rs:176`, `decision.rs`). The spec never assigns it to sync, so
this is consistent — but a reader checking which regimes gate a sync
should not have to grep to find out.

### Implemented resolution and evidence

The unused declaration, assignment and debug field are gone from
[the sync context](crates/auctioneer-privacy/src/sync/mod.rs).
`usp_propagation_has_no_ccpa_sync_denial_stage` in
[sync permission tests](crates/auctioneer-privacy/tests/sync_permissions.rs)
pins existing USP propagation without inventing a new denial stage. The
cookie-integrity repair does not change auction privacy enforcement or add
a COPPA sync-denial path.

---

## Additional closures in the same repair

These fixes were found while closing the original review and are also
implemented in `a1cbeb2`:

| Finding                                                                                          | Implemented fix                                                                                                                                              | Evidence                                                                                                                                                                                           |
| ------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Callback account injection and receiver-contract drift                                           | Reject query-key macros and duplicate/forged account routing; reserve the inserted account within the parameter cap; share strict encoding and lexical rules | [Template unit tests](crates/auctioneer-core/src/usersync/template.rs), [rendering tests](crates/auctioneer-core/tests/usersync_template.rs), [config URL tests](crates/auctioneer-config/tests/url_safety.rs) |
| Valid alias names rejected in sync priority                                                      | Priority validation accepts the same bounded alias names as runtime selection                                                                                | [Config validation and colocated tests](crates/auctioneer-config/src/usersync.rs)                                                                                                                      |
| Resource exhaustion reported as malformed privacy input                                          | Distinct fixed resource-limit diagnostics for GPP/SID limits                                                                                                 | [Handler unit tests](crates/auctioneer-core/src/handlers/cookie_sync.rs)                                                                                                                               |
| Flattened JSON-map regression absent from the merged ancestry                                    | Restored the storage-shape regression test without rewriting historical commits                                                                              | [Disclosure codegen tests](crates/auctioneer-config/tests/disclosure_codegen.rs)                                                                                                                       |
| Fastly guest checks or the secure probe could give a false green | Keep explicit guest build/execution and exact selected secure-probe results. The earlier Rust configuration snapshot guard was retired at AG's direction on 2026-10-30; executable checks remain. | [Fastly CI](.github/workflows/ci-fastly.yml), [secure probe script](scripts/check-secure-usersync.sh) |
| Handler/signing allocation coverage incomplete                                                   | Measure real generated-App paths with independent dimensions and cross-size deltas; fund keyring/signing work and coexisting cookie encodings                | [Handler allocation tests](crates/auctioneer-core/tests/usersync_handler_alloc.rs), [auxiliary charges](crates/auctioneer-runtime/src/aux_work.rs)                                                         |
| Partial MAC comparison or swallowed authenticated payload-limit errors could evade the new tests | Add exact regression tests and confirm both isolated mutations fail                                                                                          | [Signed codec unit tests](crates/auctioneer-core/src/usersync/signed.rs)                                                                                                                               |

## Qualification boundaries

These are explicit limits of the approved design and recorded evidence, not
unresolved versions of the findings above:

- A signed cookie authenticates issued state, not ownership of a callback UID,
  consent, account authorization, confidentiality or replay protection.
- Native/browser qualification covers signed storage, credentialed readback
  and deletion. The loopback browser test is cross-origin but same-site;
  cross-site cookie policy and hosted-provider behavior are not qualified.
- Native allocator-request measurements and accepted secret-value caps are
  not provider materialization, physical RSS or universal interruption proofs.
- WASM compile/lint and manifest checks do not establish fresh release artifact
  size or live-provider execution of the new signing implementation.

See the [integrity design](docs/superpowers/specs/2026-10-13-usersync-cookie-integrity-design.md)
and [verification record](docs/superpowers/plans/2026-09-30-m5-verification.md#cookie-integrity-and-review-closure-october-13)
for the precise contracts and evidence.

## Final User-Sync Review Repairs

**Status:** resolved in the October 13 follow-up. Integrated verification
is recorded in the
[repair plan](docs/superpowers/plans/2026-10-13-usersync-final-review-repairs.md).
The ten peer-verified closures, including GDPR escaping and active-key/weak-key
guards from `afe569a` and `6a70ebf`, remain intact.

| Finding                                                                  | Resolution and regression evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| ------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| DNS root dots bypassed localhost/alias denial                            | Shared classification removes root dots for denial, without widening the exact development allowlist. Config and rendering tests independently vary callback and bidder authorities.                                                                                                                                                                                                                                                                                                                                                                                                        |
| Generation accepted URL spellings that rendering rejected                | Both use the shared lexical check; extra authority slashes, backslashes and whitespace fail with valid public/path controls.                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| Fastly health/logging subprocesses inherited signing keys                | All three spawn paths remove the keyring; synthetic actual-child environment tests prove removal while retaining a control value.                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| Keyrings accepted positional JSON arrays                                 | Object-only borrowed deserialization at both root and key-record levels; codec and generated-App tests require fixed signing-unavailable refusal. Duplicate and unknown fields remain rejected.                                                                                                                                                                                                                                                                                                                                                                                             |
| A syntax-only shell could silence CI execution                           | Effective step/job/workflow shells are checked with precedence and positive controls; isolated mutations of both Fastly execution steps fail.                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| Account algorithm and TCF segment-resource classification were unguarded | Distinguishing Full/Basic host/account/vendor cases assert exact decisions. Valid, below-byte-cap consent with excessive segments requires resource refusal and terminal HTTP 400 without a cookie write.                                                                                                                                                                                                                                                                                                                                                                                   |
| Build-failure harness exceeded a ten-minute budget                       | Dependency targets are exclusively leased within the default parallel run, with distinct fixtures and real good/bad/good/bad build-script transitions. Children have a five-minute bound, file-backed output and Unix group teardown/reaping, even after leader exit; compiler wrappers are removed so a cache daemon cannot escape that ownership. Infrastructure failure cannot count as config rejection. Cargo reports the generated output directory, preventing stale-file selection. The final wrapper-free run passed all 84 cases in 472.10 seconds with fresh child target trees. |
| Four USP differences lacked an audited artifact                          | All 22 extraction differences now have exact expectations and existing design rationale in separate fixture data. Captured reference observations and the seven original divergence cases are unchanged.                                                                                                                                                                                                                                                                                                                                                                                    |
| Unsupported-version test depended on a broken MAC                        | Independent HMAC constructions authenticate both v1 and v2; only the unsupported version is rejected, including through the generated App.                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| Auxiliary signing test varied no actual cap                              | Different complete-cookie caps require exact output/peak deltas, while fixed signing scratch is pinned independently.                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| Empty GPP/SID captured templates masked encoding defects                 | Captured rows stay immutable; later consent guards and the October 14 Rust-authored five-macro controls complete the adversarial coverage at both splice points.                                                                                                                                                                                                                                                                                                                                                                                                                            |
| The complete-cookie cutoff is deployment-visible                         | The approved 4-KiB hard cutoff is explicitly documented and tested below/at/above the requested cap. Higher requested caps do not restore the former 16-KiB allowance.                                                                                                                                                                                                                                                                                                                                                                                                                      |
| Minimum-size cookie precheck is redundant                                | Retained as preflight only; boundary cases exercise the authoritative complete-field limit. The October 14 follow-up separately measures and mutation-proves the preflight's allocation benefit.                                                                                                                                                                                                                                                                                                                                                                                            |

The harness does not promise dependency reuse with `--test-threads=1`:
when the last active/waiting owner leaves, it cleans up immediately. This
keeps serial runs correct and bounded without leaving a process-static cache
behind. Default parallel-run completion is the performance qualification;
compiler-cache state is reported separately from fresh target directories.
The final run disables compiler wrappers for every child build. It is not
a claim that every slower machine or fully serialized run completes within
ten minutes; hung children instead produce explicit infrastructure failures.

## Cookie-Sync JSON Root And October 14 Follow-Up

**Status:** resolved on 2026-10-14. `CookieSyncWire`'s derived deserializer
accepted positional arrays such as `[]`, `[[]]` and `[[],0]`. Through the real
server these produced 200 responses with sync candidates instead of the
required JSON-shape refusal.

[The parser](crates/auctioneer-core/src/usersync/wire.rs) now uses the same
object-only Serde wrapper as keyring parsing. Byte/node limits still run
first; defaults, unknown fields and deferred raw privacy values are unchanged.
Colocated tests reject arrays, scalars and null, retain limit precedence and
exercise in-place replacement without changing existing state on rejection.
[Routed tests](crates/auctioneer-core/tests/usersync_http.rs) require the exact
400 body, CORS/no-store headers, no cookie write and zero signing-key reads,
with a successful object request as the control. Both parser and routed
regressions failed on the original implementation before the fix.

The shared wrapper retains one narrowly scoped visitor lint exception:
Serde's provided methods reject non-map inputs, including library-owned
private hooks. The redundant `Deserialize` exception was removed; its
in-place method is explicit and tested. No general warning suppression was
added.

Carried findings were checked against source, not commit ancestry alone:
the `is_json_map` guard and its negative tests are present even though
`b299cea` is not an ancestor. The separate 22-row extraction-divergence
artifact is consumed by all 71 reference comparisons. Version tests sign
both v1 and v2 prefixes independently; auxiliary tests vary complete-cookie
caps and pin exact charge deltas. The approved 4-KiB cutoff remains a
documented deployment-visible decision, not an outstanding compatibility fix.

## Preflight And Template Evidence Follow-Up: October 14

The latest five findings were checked against the actual source:

| Finding                                                       | Disposition                                                                                                                                                                                                                                                                                                                                                                                                                           |
| ------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Cookie preflight's allocation benefit was unpinned            | Fixed with a measured integration test over 4,097-byte and 64-KiB inputs, with and without cookie attributes. Its comparison measures HTTP cookie-name validation independently: that bounded validation allocates, while rejected values must not reach formatting. An accepted-value control proves the allocator is observing work. Disabling only the preflight compiles and fails the new test: five allocations instead of one. |
| Captured GPP/SID template inputs are empty at generation      | Confirmed evidence limitation, not rewritten reference output. New `template_controls.rs` is explicitly Rust-authored, varies every privacy macro independently and pins literal escaped values at both splice points. All ten independent encoder-removal mutations compile and fail its driver. The historical twelve-row corpus remains unchanged; it proves captured metadata/callback behavior, not hostile escaping.            |
| Handler allocation guard repeats a literal signing charge     | Removed the redundant `530_432` literal and compare the funded term against the independent 4-KiB/eight-key wire accounting equation already used by the measurements.                                                                                                                                                                                                                                                                |
| Callback config tests accept any error                        | All six test functions now require the `RenderError::UserSync` variant and its exact diagnostic, independently on external URL, literal callback and CORS origin surfaces.                                                                                                                                                                                                                                                            |
| Flattened-map fix absent because `b299cea` is not an ancestor | Not a current source defect. `is_json_map` and `only_a_serde_json_map_catch_all_proves_storage_for_unnamed_wire_members` are present; the selected negative test executed and passed against five invalid storage shapes. No ancestry-only repair or cherry-pick was applied.                                                                                                                                                         |

The preflight protects temporary formatting work; it is not the complete-cookie
acceptance authority. The final field-length check and its below/at/above-bound
tests are unchanged. Allocation observations are native allocator requests,
not physical RSS or provider-wide memory guarantees.

AG also approved a Rust-only tooling boundary: captured evidence remains
independent, while repository-owned generators and validators must not require
Go. The [tooling design](docs/superpowers/specs/2026-10-14-rust-reference-fixtures-design.md)
explicitly retires in-repository recapture. External capture metadata and source
attribution remain truthful; Auctioneer never generates its own reference answers.

The initial `auctioneer-reference-fixtures` implementation verified all ten frozen
captures, their schemas, capture metadata and source-call provenance. The approved
slim-checker boundary above supersedes its generic schema and two-command design.
Its unit and real-CLI negative
controls reject changed/missing artifacts, unsafe paths and symlink escapes;
staged checks never bless or overwrite results. Every original capture is
byte-for-byte unchanged. The four Go programs, three embedded harness fragments
and two module files are removed. Current fixture documentation uses Cargo
commands and distinguishes reference validation from external recapture.

Final independent review found one unsafe-path test rejecting its extra
fingerprint on the count guard instead. It now replaces an existing key,
preserves all 35 entries and requires the path-specific diagnostic. Removing
only the source-lock path check compiles and fails the corrected test. Hash
format and missing-entry controls also require their own exact diagnostics.

No real source archive was available for this repair. Optional source checking
was exercised with synthetic matching/missing/changed inputs and an actual CLI
missing-archive refusal; this is not fresh upstream execution or independent
Git-commit authentication. See the verification record for executed commands.

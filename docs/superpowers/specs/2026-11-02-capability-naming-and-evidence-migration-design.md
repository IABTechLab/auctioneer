# Capability Naming And Evidence Migration

Status: independently reviewed and approved by AG on 2026-11-02 for local implementation. Commits and publication require separate approval.

## Purpose

Replace legacy numbered delivery labels throughout Auctioneer-owned source,
tests, configuration, tooling, filenames and documentation with descriptive
capability names. This includes historical plans, review records and evidence
annotations. GitHub tracking is separate: keep the approved `202611` assignment
and real GitHub field/API names unchanged. Do not modify Git history.

The user requested a reviewed migration for pinned evidence, not an exception
that leaves old labels in it. The audit found no delivery labels in the ten
reference artifacts listed in the CLI's `captures.json`, or in the retained
qualification JSON/profile payloads. Those bytes and their pins can remain
unchanged; their surrounding documentation and project-owned annotations need
the migration described below.

## Approach

Use direct file moves and owner-local edits in one coordinated cleanup. Keep
historical documents, their dates and their substantive decisions; do not
delete design history merely to eliminate a label. No compatibility aliases,
parallel evidence views, new evidence format, crate or production feature.

The alternatives are incomplete: excluding historical annotations leaves the
requested cleanup unfinished; blindly replacing text and regenerating every
hash can falsely bless altered evidence. Change only identified names and
annotations, then verify unchanged payloads independently.

## Naming Contract

- Name capabilities directly: foundations, auction core, GDPR fanout, bounded
  production fanout, privacy enforcement, user sync, diagnostic qualification,
  bidder catalog, resource qualification, Trusted Server embedding, publisher
  pilot and subsequent roadmap capabilities. Do not introduce replacement
  numbered phases.
- Retain date prefixes on specs, plans and evidence documents. Remove legacy
  delivery components from paths and use a descriptive capability when the
  remaining suffix would be ambiguous or collide. Move the fanout progress
  directory with its contents.
- Update all consumers together: Markdown links and heading anchors, inline
  paths, Rust fixture imports, generated documentation, shell defaults and
  parsers, workflow references, ignore files and fixture allowlists. Preserve
  task/subtask identity through descriptive document-scoped names and update
  every cross-reference; do not collapse distinct tasks into one label.
- Rename tests for observable behavior. Preserve signatures, callers, test
  attributes, features, assertions, case ownership and dispatch counts.
- Use descriptive synthetic request/account IDs. Keep their UTF-8 byte lengths
  unchanged when allocation or boundary fixtures depend on them. Do not
  remeasure or widen a bound to compensate for a cosmetic rename.
- Remove generic schedule wording from source comments. Correct verified stale
  claims about implemented privacy/user-sync behavior instead of merely
  replacing a label in a claim that is no longer true. Preserve historical
  statuses as dated history, not as newly asserted current readiness.
- Preserve unrelated lookalikes, including CPU model names, shell options,
  regex data, source revisions and protocol values. Vendor code, external
  source captures and license text are not project scheduling terminology.

Create a one-time old-to-new path/symbol inventory in the review workspace
before editing; require unique destinations and complete consumer updates.
Do not check in a permanent list of the names being removed.

## Evidence Migration

The implementation records a pre-edit manifest of affected files and their
SHA-256 digests outside the source tree, then applies these distinct rules:

| Artifact | Permitted change | Required invariant |
| --- | --- | --- |
| Ten CLI-pinned external reference artifacts | None, unless an independently reviewed scope extension is needed | Original bytes, digests, source lock and source-call metadata remain unchanged |
| Seven retained qualification bundles and their declared profiles | Edit human-authored README annotations only; no payload or profile rename is needed by the audit | All 25 indexed payloads and declared profiles remain byte-identical; existing indexes and profile bindings still verify |
| Other frozen catalog/schema/bidder captures, baselines and vendor patches | No content edits, apart from the explicit Rubicon manifest-binding exception below | Existing captured bytes, source identities, integrity checks and license attribution remain unchanged |
| Three project-owned user-sync divergence JSON documents | Change design citation paths and delivery-label wording in provenance notes only | Case IDs, inputs, expected values, reasons, operation names, reference metadata and provenance kinds remain unchanged |
| Rubicon's project-owned `bidder.toml` provenance commentary and its two whole-manifest bindings | Replace the delivery-label citation in the metadata artifact's `port_differences`; update only the two manifest digests described below | Source paths, blobs, sizes, hashes, retained artifacts, routing declarations, policy decisions and factory cases remain unchanged |
| Verification Markdown and certification record annotations | Rename files and update project-owned labels, cross-links and commentary | Keep source/artifact/SDK identities, timestamps, commands' substantive arguments, results, counts, limits, thresholds, verdicts and readiness limitations |

The three editable divergence documents are privacy `tests/data/sync/`'s
`divergences.json` and `extraction-divergences.json`, and bidder
`tests/conformance/fixtures/usersync/divergences.json`. They are not among the
CLI's ten pinned external captures. Update the approved-design path check in
`auctioneer-privacy/tests/sync_reference.rs` with the citations, without
broadening its allowed provenance.

Rubicon's whole-manifest digest is bound by `MANIFEST_SHA256` in
`src/bidders/rubicon/tests/conformance.rs` and `manifest_sha256` in
`src/bidders/rubicon/fixtures/factory.json`. After the structured TOML diff
proves that only the identified commentary changed, compute the new manifest
digest and update exactly those two bindings. Preserve every factory case and
all upstream/retained-artifact fingerprints. This is a local annotation
migration, not a new protocol capture or permission to recalculate source
hashes. Run the existing Rubicon protocol conformance and
`registry_source_conformance` checks without bypassing either digest guard.

Historical branch labels should be described by capability while retaining
the recorded commit identity. Do not invent a replacement branch that existed
at execution time. When removing labels from historical temporary paths or
displayed invocations, mark the document as naming-normalized: displayed
locations are illustrative and are not an exact transcript or a new run.
Keep commands' non-naming arguments and all recorded outcomes unchanged. Raw
runner output in the two certification text records stays untouched; their
project-authored headers and filenames may be updated with that same notice.
Place notices in qualification READMEs before their digest tables: the existing
index parser consumes every subsequent line as a table row.

No changed numeric result, workload, expected output, revision, source digest,
capability attestation or qualification status is authorized. Do not refresh
archives, recapture upstream outputs, rerun qualification to replace historical
evidence, or remove a failed record. Only the two named local manifest-binding
updates are authorized; the audited external/measurement pins stay unchanged.
An unexpected label inside another byte-pinned payload
requires a specific reviewed extension; never regenerate pins until tests pass.

## Coupled Guards

The fanout roster remains the same named test contract. Update its document
path, section-heading parser, landing-split labels, local variables and negative
fixtures together. Keep total arithmetic, unique ownership, per-target expansion
and missing-member detection intact. The batch-rebase guard must retain its
existing frozen-history exclusions and active API checks after the path moves.
Update `.prettierignore` without reformatting protected roster declarations or
frozen artifacts.

Keep frozen scrub comparator logic and independent privacy oracles unchanged.
Update only commentary and descriptive labels where needed; never replace an
oracle with generated/current output. No resource equation, release limit,
runtime setting, dependency pin or feature default changes are in scope.

## Acceptance

1. Audit tracked and intended-to-be-added files and path components, including
   hidden project configuration. Check plain, hyphenated, dotted, underscore
   and compact script-variable forms. Review matches semantically; a raw zero
   regex count is not sufficient, and hardware/protocol lookalikes are not
   failures. Git metadata, generated build output and external data retain
   their proper meanings.
2. Confirm every planned move has exactly one destination and no dangling
   local links, moved heading anchors, script paths, fixture citations or
   imports remain. Preserve the two existing untracked planning drafts.
3. Compare immutable artifact digests with the pre-edit manifest. Compare the
   three divergence JSON documents structurally, allowing changes only to the
   reviewed citation/note fields. Reject missing, extra or otherwise changed
   cases. Verify Rubicon's commentary-only TOML change and exact two binding
   updates; require identical factory cases. Review normalized historical
   annotations against the original diff.
4. Execute the existing CLI `reference check`, retained `qualification_evidence`
   integrity target, privacy `sync_reference`, core user-sync reference/chooser
   and handler divergence consumers, and exchange `disclosure_policy_diff`.
   Include Rubicon protocol conformance and `registry_source_conformance`.
   Enable each target's required features; report executed counts and blockers.
5. Run the fanout roster and its negative fixtures, batch-rebase guard, and
   feature-enabled fanout conformance cases affected by the test rename. Check
   before/after case lists and every caller. Do not weaken gates or silently
   skip required-feature tests.
6. Run scoped formatting, compilation and existing relevant source/fixture
   checks. Run the full acceptance matrix in CI only after separate publication
   approval; a local documentation migration does not prove hosted readiness.

Report renamed paths, annotation changes, unchanged evidence/pin checks and
tests actually run. Obtain maintainer approval of this reviewed design before
implementation, and separate approval before any commit, push or GitHub edit.

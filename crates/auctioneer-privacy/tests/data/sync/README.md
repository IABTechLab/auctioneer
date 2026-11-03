# Sync Privacy Reference Fixtures

`reference.json` contains 71 actual-source cases from
[Prebid Server](https://github.com/prebid/prebid-server), by Prebid.org and its
contributors, at commit
`d7f8515b86258688304b0d9b6668c6a0e258bc9e` (Apache-2.0): 40 cookie-sync
extraction cases, 13 setuid extraction cases, and 18 signed-int8 SID cases.
`divergences.json` contains seven **Auctioneer design** cases, not source parity.
`extraction-divergences.json` is the separate reviewed 22-row extraction
allowlist, keyed by exact case IDs in the immutable `reference.json`.

The repository import renamed project references in provenance notes and
updated local integrity pins. Captured inputs and expected outputs, source
revisions and source fingerprints are unchanged; see `docs/repository-import.md`.

The original license, upstream source hashes, complete schema, historical
capture toolchain and Rust validation instructions live in
`crates/auctioneer-bidder/tests/conformance/fixtures/usersync/`. Both directories use
`schema_version`, `reference`, and `cases`; each case has `id`, `operation`,
`provenance`, `input`, and `expected`.

- `cookie_privacy` calls the private source `extractPrivacyPolicies` through
  a temporary Go overlay after original JSON decoding. `stage` distinguishes
  JSON rejection from extraction. This helper is **before** parseRequest's
  additional legacy USP validation; an invalid USP macro here is not a claim
  that the final endpoint will forward it.
- `setuid_privacy` calls source `extractGDPRInfo`. `error_kind` distinguishes
  warnings from errors. `signal`: -1 ambiguous, 0 no, 1 yes. Exact error
  strings are reference observations, not safe Auctioneer diagnostic requirements.
- `parse_sid` calls `stringutil.StrToInt8Slice`, preserving permissive signed
  inputs and its upper bound instead of substituting Auctioneer's domain.

Source consent strings are either upstream test strings or explicitly opaque
extraction inputs. None of these cases claims a valid authorization grant.
The core wire corpus separately captures endpoint permission error mapping
with test permission stubs, and chooser fixtures record privacy callback
ordering, including the unused CCPA callback. GVL/GCL freshness, purpose-one
treatment, publisher restrictions and exhaustive permission policy are not
qualified by this corpus.

The seven divergences cover wider positive SIDs; mixed sentinel rejection;
strict positive/sentinel spellings; active-section TCF selection; setuid
missing-section fallback; deferred malformed legacy JSON under SID precedence;
and extraction before activity. Their provenance points to the Auctioneer design,
not to the upstream fingerprint map. Reference source outputs are never changed
to match these rules.

No fixture consumer needs Go, the source archive, network access, or `/tmp`.
`tests/sync_reference.rs` drives **all 71 reference rows** through the real
`resolve_sync_privacy` API. It parses the original cookie JSON and carries
the literal ASCII setuid queries with first-duplicate-wins semantics (the
corpus has no percent/form encoding). Query decoding and HTTP response
qualification remain in the core integration tests. SID helper rows also
enter the normalizer, with nonempty opaque consent to isolate SID extraction.
They check every numeric membership value, the standalone sentinel, and exact
raw text; they do not pretend that the upstream int8 helper is Auctioneer's parser.

The test asserts the exact row-ID set, uniqueness, operation counts, and
divergence counts. Every expected result is compared with actual extraction,
not another fixture. Successful rows check the parsed signal and selected
consent, and cookie rows also check USP/GPP/SID outputs. Normalized defaults
and retained raw GPP/SID are checked separately. Error rows compare closed
error categories, not upstream diagnostic bytes or partial outputs after
failure. Upstream warnings are nonfatal, not permission grants.

| Operation | Rows Executed | Reference Agreement | Deliberate Difference |
| --- | ---: | ---: | ---: |
| `cookie_privacy` | 40 | 30 | 10 |
| `setuid_privacy` | 13 | 9 | 4 |
| `parse_sid` | 18 | 10 | 8 |
| Total | 71 | 49 | 22 |

The 22 row-level differences live in `extraction-divergences.json`, not an
inline switch. Each entry names its exact reference case and operation,
named reason, complete Auctioneer extraction expectation, rationale, and existing
approved master/user sync/GPP design paths. Its `reference` metadata identifies the
frozen observations, not an upstream capture of the Auctioneer expectation.
The consumer rejects duplicate/unknown case membership, missing reasons or
rationales, missing/non-approved design paths, and expectations equal to the
reference projection. All entries must be exercised exactly once, and actual
extraction must differ from the frozen reference and equal the reviewed
expectation. Exact per-operation and per-reason counts retain 49 agreements
and 22 differences; the recorded reference outputs remain immutable:

| Auctioneer Rule (Design Section 5.2) | Reference Row IDs | Count |
| --- | --- | ---: |
| Defer malformed legacy JSON under SID precedence | `cookie_sentinel_cannot_override_json_string_default_{0,1}` | 2 |
| Select envelope TCF only for active SID 2 | `cookie_inactive_tcf_in_envelope_default_{0,1}`, `cookie_envelope_without_sid_default_{0,1}`, `setuid_inactive_tcf_in_envelope`, `setuid_envelope_without_sid` | 6 |
| Clear invalid selected USP | `cookie_usp_invalid_default_{0,1}` | 2 |
| Selected active GPP USP wins a legacy conflict | `cookie_usp_conflict_default_{0,1}` | 2 |
| Retain legacy consent when active section 2 is missing | `setuid_sid_missing_tcf_no_fallback` | 1 |
| Reject mixed sentinel lists | `setuid_mixed_sentinel`, `sid_08` | 2 |
| Reject nonpositive/noncanonical sentinel and plus spellings | `sid_07`, `sid_09`, `sid_10`, `sid_11`, `sid_12` | 5 |
| Accept wider positive SIDs | `sid_15`, `sid_17` | 2 |

The four invalid/conflicting USP source rows follow the approved user sync section
5.2 selection policy: invalid selected USP clears without a hard error or
fallback, and active GPP USP wins a legacy conflict. The upstream observations
remain unchanged. Invalid legacy USP is observed before the reference's later
endpoint validation; conflict errors are extraction-helper observations, not
a Auctioneer privacy-policy requirement. Independent permission cases also vary
invalid active GPP USP with valid legacy USP to prove no fallback, and inactive
section 6 to prove that it cannot override legacy USP.

The seven design cases in `divergences.json` describe rules, not seven of
the 71 source rows. The USP clearing/conflict rules above are also explicit
in design section 5.2. `extraction_before_activity` concerns HTTP wire-stage
ordering and is guarded by `sync_permissions.rs`'s extraction/activity
matrix; it is not an extra reference row or a privacy-level Set-Cookie claim.
No reference rows are skipped, and no unexplained source differences are
accepted. This harness qualifies extraction, not TCF authorization or HTTP
byte parity. Real permission tests separately cover account/global and web
opt-in with host GDPR enabled and host TCF disabled, and independent basic
vendor/exception lists. USP propagation does not add CCPA/COPPA sync denial
or require the removed, unused `ccpa_enforce` context field.
Permission tests distinguish account Basic/Full overrides from host policy
using vendor 8 declaring Purpose 2 but not Purpose 1 with Purpose 1/vendor 8
consent. Valid below-byte-cap consent plus excess trailing segments must stay
a resource decision, including candidate evaluation with a zero host vendor.
Colocated handler tests separately pin terminal HTTP 400, fixed resource
diagnostics, and no Set-Cookie, using the existing baked runtime fixtures.

Behavioral verification (the first command emits one coverage line per row):

```sh
cargo test -p auctioneer-privacy --test sync_reference --locked --offline -- --nocapture
cargo test -p auctioneer-privacy --all-targets --locked --offline
```

From the Auctioneer worktree, standalone integrity validation is:

```sh
cargo run --locked --offline -p auctioneer-cli --no-default-features -- reference check --root .
cargo run --locked --offline -p auctioneer-cli --no-default-features -- reference check --root /path/to/staged-worktree
```

These commands verify immutable captured evidence and provenance, not upstream
re-execution or operation schemas. The existing Rust consumers own row shapes,
counts and behavioral comparisons.
Recapture happens externally; historical Go tools are not included in this
fresh-history repository.
Staged comparison is read-only and never updates or accepts expectations.

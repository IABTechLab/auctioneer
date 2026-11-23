# Hosted Schema Growth Capture

## Post-Repair Feature Lanes

These unmodified frames were emitted by proof-code checkpoint `4a863c6c796e55e0b17e1b3252b46060396d3c5a`, tested as merge `604cf03104f0d6581c0a75b49ea43a6467e02c04` over `05f2a56e9555be9c46a789c868100e37162abe5b`, in [Run Tests 45021426271 / compiler-schema job 137204983421](https://github.com/IABTechLab/auctioneer/actions/runs/45021426271/job/137204983421). All 25 hosted checks passed at that checkpoint.

| Lane / Raw File | Bytes Including Final Newline | SHA-256 |
| --- | ---: | --- |
| [default](hosted-growth-4a863c6-default.json) | 1,624,271 | `71d1be5f96ef8ac6cb3f756cb13ff26efd06b57dd3f6a44a7e23c9beddba2a51` |
| [development](hosted-growth-4a863c6-development.json) | 1,624,548 | `87da24f9e0a9644384ffe1f1c9bcb2eb2df5d3e52401d9937eeba897430d1f13` |

Each file is the unique `AUCTIONEER_SCHEMA_GROWTH_BEGIN/<lane>` through `AUCTIONEER_SCHEMA_GROWTH_END/<lane>` JSON body, preserving its final newline. Only GitHub line prefixes/timestamps, transport BOMs and ANSI controls were removed. JSON was parsed for completeness, not reserialized, normalized or used to recompute observations. Capture publication and its pure-core test consumer follow the proof checkpoint; they do not change the measured implementation or model sources. Their own full CI remains recorded on PR #262 rather than being attributed to this earlier run.

The reference snapshot is commit `d26fcf7f566db6d1dce2e1847d79c9f44581edb8`, tree `2d54c6f53ef3d062ca9292449a4546bb76e5ddd8`, with provenance lock SHA-256 `78118dcb53fd3f486a6032c47124ba249bd5fbf361a4a1973824e56d4f55001c`. The candidate and current measurement lock SHA-256 are `ae849e777f8216f983112f631168bf585c4604076381d265459249d62f88df38`. These are deliberately different locks, not historical execution of the current dependency graph.

Here, "current measurement" means the build that emitted these frozen records. Their pure-core consumer checks recorded internal consistency and these pins, not freshness against a later checkout. An unrelated dependency change must not trigger replacement of these files. The compiler/schema live-growth owner separately measures the current workspace lock and enforces current source/model compatibility; preserving historical data does not waive that gate.

Both measurements use Rust 1.98.1, `x86_64-unknown-linux-gnu`, debug, empty encoded rustflags and `test-support`; development also enables `insecure-bidders`. Full source, seed, expanded-graph, generator, dependency, ABI and feature identities remain inside each frame.

### Per-Run Digest Scope

`seed_sha256`, `expanded_sha256`, graph `captured_manifest_sha256` and `captured_rust_sha256`, declaration `expansion_sha256`, `source_recipe_identity`, and linked/inline-group `source_sha256` values correlate and protect the serialized inputs of one capture. They are not cross-run reproducibility or qualification keys. Reconstruction uses a fresh random private temporary root; its absolute snapshot paths enter generated schema attributes, and `OUT_DIR` enters the recorded generator inputs. Thus identical source and proof code can produce different digests on every rerun, including the pinned reference side. The same limitation applies to the historical 081 record below.

The existing stable content/semantic witnesses are workspace-relative source paths with byte lengths/SHA-256 and directory membership; pinned reference commit/tree and Git file path/mode/blob/byte-length/SHA-256; actual schema bytes and schema-owner digests; helper owner/marker/symbol identities, patterns and initialization order; accepted semantic attributes; and the compiler/library/generator/model sources, measurement lock, options, exact features, build/ABI and workload. `growth::match_views` checks the pinned reference, declared schema-owner delta, unchanged-owner helper patterns and exact within-run measurement equality. Its producers verify schema bytes, actual attributes and each run's snapshot paths; `attach_source_recipes` compares program/formula and linked-group topology rather than path-bearing serialized digests. The [dated evidence](../../../../../../../docs/testing/evidence/schema-residency-initialization-20261121.md#per-run-digests-and-source-comparison) names those existing fields and the retained rerun that demonstrates this distinction; no normalized identity field or new comparison API is supplied.

Downstream consumers must use one immutable capture bundle and its exact handoff/binding, or establish source continuity under those content/semantic witnesses and consume a fresh current-run bundle. Independently reconstructed raw digests are not an equality oracle. Do not combine bindings from different runs, strip arbitrary hashes to force owner agreement, or treat equal measurements as source equivalence. All exact within-run/frozen-byte/snapshot checks and semantic source/attribute/owner-drift refusals remain required. Preserve the raw-file lengths and SHA-256 values above unchanged; those identify these exact published bytes.

`metadata_registry_scope.reference` is `equal-input-proxy-of-candidate`, and its candidate role is `actual-current-constructor`. The closed comparison requires exact manifest/constructor/binding/count and measurement equality before measuring the current constructors twice in fresh processes. `supports_nonzero_metadata_registry_delta` is false: unequal metadata/registry inputs refuse, even when part of a schema change. The reference slot does not execute historical constructors. Schema/program reconstruction and its source-level growth controls retain their separate scope. Neither a zero delta nor `matched_reconstruction_complete` is common C/I, target physical or serving qualification.

The job reported 337 compiler controls, 840 default source tests and 127 development ownership cases passed, zero failed/ignored. Six heavy source cases retain their separately executed owners; 719 development cases were outside its exact prefix selection. Strict Clippy passed. At publication, a read-only inventory matched all 112 captured measurement/handoff/metadata source-witness records to 69 unchanged current files in each lane. This is source continuity, not execution of another resource proof. Preserve both raw records byte-for-byte; a later proof-code capture gets another filename and provenance.

## Historical 081 Record

`hosted-growth-081b521.json` is the unmodified default-feature JSON record from
[Run Tests 44966255810, compiler schema job 137035850296](https://github.com/IABTechLab/auctioneer/actions/runs/44966255810/job/137035850296),
at source commit `081b52116fcc6e690ad164121c5677ffd1441065`.

- Bytes: 1,620,650, including the final newline.
- SHA-256: `6fff751282b3ea39ce70f4e5d7cb64acc65c9be594301035d22a2e63fb6e61e4`.
- Extraction: the unique `AUCTIONEER_SCHEMA_GROWTH_BEGIN/default` and
  `AUCTIONEER_SCHEMA_GROWTH_END/default` frame, with only GitHub's line
  timestamps, transport BOMs and ANSI controls removed. JSON was parsed to
  verify completeness, not rewritten.
- The job executed 335 compiler controls and 806 source tests successfully,
  then failed strict Clippy. That failure is not a passing full CI result.

This compares the pinned reference tree with current source data using one
current compiler, library, feature and measurement build. It does not execute
historical source. The full input, source and measurement identities remain
inside the record. Constructor and metadata/registry samples are requested
allocation observations; the complete C/I recipes still require target facts.
The numeric C/I example is explicitly hypothetical, not a physical cost or
serving qualification. The core arithmetic consumer must remain non-installing.

Keep this record byte-identical. A later capture receives a new filename and
provenance rather than replacing or repinning this observation.

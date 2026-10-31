# Auxiliary Storage Controls

Date: 2026-10-18
Starting code: `6d26d259`; SDK `bc300ace`.

This is an unchanged-workload ownership repair, not an accounting reduction,
all-route response bound, guest-memory measurement or R0 completion.

## Implemented

Info and GVL-health JSON output now counts a borrowed payload's serialized size,
converts it fallibly and uses the existing fixed-reservation capped writer.
There is no new product ceiling. A changing second serialization pass cannot
grow beyond its reservation or trigger an unbounded retry. Encoding errors
remain terminal, with the existing info/health response dispositions.

CORS decoration copies the exact matched configured origin into response-owned
header storage. It no longer clones a request header that may be a tiny slice
of a much larger inbound buffer. Normal, setuid and preflight responses retain
their successful wire values; invalid, duplicate and denied origins remain
denied. No cookie, privacy, admission or workload policy changed.

Both changes have executed failing-before/passing-after controls. Info tests
observe capacity rather than bytes alone; a real health handler has a positive
unbounded-serializer allocation control; the CORS test first proves the input
shares each 16-KiB/1-MiB backing, then requires all six output cases to detach.
Unit tests stay beside their owners, and allocation instrumentation remains in
the existing `serialize_allocation_bound` integration target.

## Verification

| Check                                                                       | Result                                                                                              |
| --------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| Implementer unit/allocation sweep                                           | 488 unit and 7 allocation tests passed                                                              |
| Parent secure-default core all-targets                                      | 1,129 passed, 0 failed, 1 existing ignored, 95 targets                                              |
| Parent focused insecure-bidder info/allocation checks                       | 23 unit and 7 allocation tests passed; not a full insecure-feature sweep                            |
| Independent spec and code-quality reviews                                   | Approved after inspecting the frozen implementation                                                 |
| Core all-target Clippy with conformance/insecure features and `-D warnings` | Passed                                                                                              |
| CLI resource/source-identity unit prefix                                    | 77 passed; 530 unrelated tests filtered                                                             |
| Formatting, diff and raw-JSON gate                                          | Passed; 168 raw fixtures across 50 files                                                            |
| Core and three adapters' feature-gated native conformance                   | 77 passed across four targets; no failures                                                          |
| Three real WASM conformance targets                                         | Linked with `--no-run`; not executed                                                                |
| Three WASM Clippy rows, including exchange/privacy and all targets          | Passed with `--features conformance-suite -- -D warnings`                                           |
| Fastly actual CLI package build                                             | Passed; 8,876,119 bytes against 100,000,000                                                         |
| Spin actual CLI component build                                             | Passed; 26,510,240 bytes against 50,000,000                                                         |
| Cloudflare actual Wrangler dry-run/build                                    | Passed; reported 25,291.53 KiB uncompressed / 4,015.17 KiB gzip, below 60,000,000 / 9,000,000 bytes |

The build tools were Rust 1.98.1, Fastly CLI 16.0.0, Spin 4.1.0,
worker-build 0.8.5, Node 24.20.0 and Wrangler 4.151.1 (the existing CI `@4`
selector). Cloudflare's displayed KiB figures convert using 1,024 bytes/KiB;
the existing gate's truncation yields 25,898,526 / 4,111,534 bytes. No budget,
profile, target or build command was weakened to obtain these results.
Installed Wasmtime 45.0.0 does not match the declared 48.0.2; neither version
was used to execute these test guests. No guest-runtime observation is implied.

Native tests and package builds are separate from guest heap/stack proofs and
hosted qualification. The default core command does not silently claim the
feature-gated conformance target. Existing ignored probes keep their CI owners;
no new ignore, dependency, suppression or wire-format change was added.

The two exact-size/ownership fixes were reviewed against the starting code;
only known Cloudflare generated-output directories were added to `.gitignore`
to keep local packaging products outside source snapshots. The unrelated dirty
roadmap remains untouched. No deployed Auctioneer service was contacted or mutated;
the vendor commands only built packages or performed a dry-run.

Logs: `/private/tmp/r0-storage-{core-all,insecure-info,insecure-allocation,
clippy-core,cli-resources,conformance,adapter-conformance,
fastly-build,spin-build,cloudflare-build,clippy-cloudflare,clippy-fastly,
clippy-spin,conformance-link-cloudflare,conformance-link-fastly,
conformance-link-spin}.log`. The standalone core conformance run is repeated
inside the four-target total, not extra unique coverage.

## Clean-Source Execution

After code commit `94cce777d83e839b3442d65d6961bd5f4ccf98b4`, the three
resource commands from `test.yml` were executed sequentially with locked,
offline dependencies and `-j2`. Capture and audit validation exited zero.
The strict CLI exited one; the actual Bash/jq reconciliation accepted exactly
one truthful nonfitting report. Independent literal checks also required all
four rows to bind that code checkpoint and SDK `bc300ace`, with
`dirty = false`, `source_tree_matches_commit = true`, application-only
features, all eleven additive terms and both zero catalog terms.

The totals remain 2,533,631,672 bytes (Axum), 138,048,496,272 (Cloudflare)
and 23,589,646,360 (Fastly/Spin). Every `required_fit` is false. Axum's
configured arithmetic fits, but its unpublished facts prevent qualification;
the WASM rows also exceed their ceilings and retain
`host_native_substitute` layouts. No native result was promoted to guest
evidence. Source identity excludes the unchanged unrelated roadmap edit.

Generated files are in `/private/tmp/auctioneer-storage-commands.7ZBCcq/`:

| File                  | SHA-256                                                            |
| --------------------- | ------------------------------------------------------------------ |
| `resource-audit.toml` | `13446afe2073eb8eea4e38470d5155cc8b6990dd67ec152e1413d194a74a554b` |
| `resource-audit.md`   | `b5b25811574cefc5147afaa1a61cada0ddccceb06b45356f6138784aca24f090` |
| `resource-fit.json`   | `4c27305495a90311f0f6f02cb236e15b2b043f4d8a5ddabe89372a1fdd65cdf2` |

The same directory holds capture/validation/strict stderr logs. No canonical
audit or historical capture was overwritten. This documentation-only
follow-up records execution at the code checkpoint, not a qualification or
recapture against a later documentation commit.

## Decoder Dependency

The current head of SDK PR #275 was rechecked and remains `bc300ace`.
Read-only source inspection traces the actual codec to EdgeZero's
`compression::decode_brotli_payload`, using async-compression's pinned codec
and `BrotliState<StandardAlloc, StandardAlloc, StandardAlloc>`.

The SDK's [pinned source audit](https://github.com/IABTechLab/edgezero/blob/bc300ace5ca1ffe7060594a7bdb01c1da80de305/docs/audits/2026-09-27-brotli-decoder-memory-accounting.md)
derives 3,442,530 allocation-payload bytes. Its remaining 13,334,686 bytes form
a conservative wrapper/allocator policy reserve, not an exact heap proof.
Neither number establishes a physically necessary retained lower bound.

Nevertheless, the unchanged accounting contract reserves
`32 * 16,777,216 = 536,870,912` fixed decoder bytes before window/buffer storage,
already above every WASM ceiling. Other Auctioneer savings alone cannot make that
contract fit. Replacing the fixed allowance with only the payload subtotal is
not justified. A 256-KiB decoded limit does not force a smaller ring: the pinned
decoder starts at the declared window for nonfinal metablocks. The available
preallocation helper can panic on exhaustion and lacks an equivalent consumed-
input/trailing-data contract, so it is not a qualified drop-in replacement.

SDK compression maintainers own the next evidence or implementation: bound the
complete decoder/wrapper/allocator lifetime at the selected ABI and scope, or
supply an equivalent bounded implementation. Preserve supported window/layer
limits, full fanout, encoded/decoded limits, framing/trailing-data handling,
errors and cancellation. Exercise concurrent retained states, multiblock and
malformed input, cancellation and terminal release on applicable WASM targets.
Only then may Auctioneer consume a reviewed smaller bound through its shared owner.
The existing codec pins and arithmetic tests do not supply this proof.

Unpublished SDK ingress/population facts, Fastly's application stack proof,
Auctioneer's remaining allocation bounds and absolute fit remain open separately.
No failed or unknown result is promoted by these storage repairs or builds.

# Unicode 16 Simple-Fold Qualification Data

`CaseFolding-16.0.0.txt` is the unmodified official Unicode 16.0.0 data file:
https://www.unicode.org/Public/16.0.0/ucd/CaseFolding.txt

`ReadMe-16.0.0.txt` is the unmodified release directory's provenance:
https://www.unicode.org/Public/16.0.0/ucd/ReadMe.txt

The data header points to https://www.unicode.org/terms_of_use.html (redirects
to https://www.unicode.org/copyright.html). Those terms identify files under
`https://www.unicode.org/Public/` as Unicode Data Files and apply Unicode
License v3. `LICENSE-UNICODE.txt` preserves the complete notice downloaded
from https://www.unicode.org/license.txt alongside the original file's
copyright notice. These external files are retained byte-for-byte.

The independent test oracle uses only common (`C`) and simple (`S`) records.
Full (`F`) and Turkic (`T`) records are validated but grant no equivalence.
Unlisted scalars are singleton classes. No production library supplies expected
classes. The Go 1.23.12 / Unicode 15 JSON capture remains unchanged at its
original privacy-test path; it is historical evidence, not a Unicode 16 oracle.

## Acquisition And Identity

Retrieved directly over HTTPS on 2026-11-12. SHA-256 values are literal
checksums of the downloaded bytes, not recomputed expected values in the tests:

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `CaseFolding-16.0.0.txt` | 86092 | `6f1f9c588eb4a5c718d9e8f93b782685e5c7fec872cf05e8e6878053599e09bb` |
| `ReadMe-16.0.0.txt` | 693 | `14cafa23788d3a20dd21d6b0cdcb8d6dab520781fcd9ad9392f3b88ea607e633` |
| `LICENSE-UNICODE.txt` | 1995 | `e7a93b009565cfce55919a381437ac4db883e9da2126fa28b91d12732bc53d96` |

The license URL is unversioned: the retained notice says Unicode License v3,
Copyright 1991-2026 Unicode, Inc. It is the notice served on acquisition, not
a claim that the notice's bytes were published with Unicode 16 in 2024.

The unchanged `crates/auctioneer-privacy/tests/data/privacy/reference.json`
SHA-256 is `da9414231de7ad7e71abfb3f79e2fa84a5353bb19b40ae391ca1121c20bc9d8f`.
Its embedded versions remain `go1.23.12` and `15.0.0`.

## Inspected Relation Delta

These counts were derived by parsing the official file and frozen JSON as
data, without using the Rust case-mapping library or recapturing Go output.
They are fixture observations, not executed Rust-test results.

The official file contains 1590 records: C=1453, S=31, F=104, T=2.
Its C/S relation has 1454 nontrivial classes containing 2938 scalars. The Go
capture has 1454 noncanonical member/representative records, describing 1424
nontrivial classes containing 2878 scalars when representatives are included.
All 1424 old nontrivial classes are unchanged. There are no removed nontrivial
classes, no extensions/merges of old nontrivial classes, and no splits.

There are 30 added two-member classes. Counting singleton classes too, these
are exactly 30 merges of two old singletons, affecting 60 scalars; the other
1112004 scalar class memberships are unchanged. The complete domain has 1112064
scalars, excluding the 2048 surrogate code points. The total number of classes
including singletons decreases from 1110610 to 1110580.

The complete added-class inventory (hexadecimal, ascending members) is:

| First Member | Second Member |
| --- | --- |
| `019B` | `A7DC` |
| `0264` | `A7CB` |
| `0390` | `1FD3` |
| `03B0` | `1FE3` |
| `1C89` | `1C8A` |
| `A7CC` | `A7CD` |
| `A7DA` | `A7DB` |
| `FB05` | `FB06` |
| `10D50` through `10D65` inclusive (22 Garay members) | corresponding first member plus `0020`, `10D70` through `10D85` |

This is the official-16-versus-Go-15 delta, not a claim that all 30 pairs
originated in Unicode 16. In particular, the Greek and ligature S mappings
are included explicitly rather than silently omitted as unchanged history.

## Controller Verification

The named behavioral-red filter is
`tests::unicode16_new_cyrillic_tje_class_is_recognized` with `--exact` in the
existing `simple_fold` integration target. It first proves the pair is absent
from the frozen relation and present in the official relation, then exercises
`simple_fold_eq` in both directions. It intentionally has no Rust-library
version assertion that could substitute for the behavioral failure.

The author-side helper does not run Cargo, builds or tests. The controller
owns the old-dependency red, subsequent dependency change and serialized
qualification. The existing target retains exhaustive anti-merge/anti-split
checks and asserts exactly 1112064 visited scalars per full-domain comparison.

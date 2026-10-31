# Parameter Literal Reference Capture

`parameters.json` captures the raw literal `validParams` and `invalidParams`
tables from the fourteen sources listed below at the pinned reference revision
`d7f8515b86258688304b0d9b6668c6a0e258bc9e`. Each source path is
`adapters/<schema>/params_test.go`, including the case-sensitive `yahooAds`.
Its source witnesses describe complete pinned Git blobs, not working-tree files.

| Schema | Valid | Invalid | Total | Source bytes |
| --- | ---: | ---: | ---: | ---: |
| appnexus | 16 | 26 | 42 | 3121 |
| bidstack | 4 | 3 | 7 | 1368 |
| criteo | 16 | 19 | 35 | 2502 |
| feedad | 19 | 20 | 39 | 4247 |
| kargo | 4 | 9 | 13 | 1273 |
| msft | 12 | 21 | 33 | 2859 |
| openx | 7 | 15 | 22 | 2542 |
| pubmatic | 6 | 20 | 26 | 2915 |
| sharethrough | 7 | 14 | 21 | 1588 |
| sovrn | 5 | 6 | 11 | 1160 |
| thetradedesk | 3 | 9 | 12 | 1489 |
| unruly | 2 | 8 | 10 | 1492 |
| visx | 4 | 14 | 18 | 1330 |
| yahooAds | 1 | 10 | 11 | 1546 |
| Total | 106 | 194 | 300 | 29432 |

The capture is 62,945 bytes, within the unchanged 65,536-byte whole-capture cap.
Its SHA-256 is
`6efc0eaea62063f86d391df46265e1df2fb06789cfe2e746f53c3c9e4c41863e`.
This cap bounds the capture document, not any instance workload limit.

All captured members are raw source literals; decoded inputs retain their
literal contents, whitespace, empty strings, duplicates, and source array order.
Each case's `source_offset` is the zero-based byte offset immediately after its
opening backtick in the complete pinned source blob. All fourteen blobs are ASCII
and contain no carriage returns; offsets were located in source order, retaining
duplicates. Reading `input`'s UTF-8 byte length at that offset reproduces its bytes.

The original appnexus/openx/pubmatic 90 rows and all their provenance remain
unchanged, with their original preservation test retained. The preceding
eleven-source capture added 146 rows (50 valid, 96 invalid) to those original 90.
This increment adds 64 rows (27 valid, 37 invalid) from bidstack, feedad, and visx;
source dispatch remains alphabetically ordered. Filtering out these three
families and reconstructing the typed pretty JSON, including its final newline,
reproduces the previous 48,194-byte capture byte-for-byte, with SHA-256
`d504554491c1c0f4fcd0f0ebf45da38fb0cc1f8e5ecfb8b4200e3cfdc144f8b0`.
All previous 236 raw inputs, offsets, metadata, and field order are preserved;
the explicit reconstruction test was green before expansion and remains in place.
The fourteen sources were independently read with
`git --no-replace-objects show <revision>:<path>`; no Go files are imported or
executed. Complete blob IDs, SHA-256 digests, byte lengths, and group counts are
pinned in both the capture and the corresponding bidder-owned
`tests/schema/parameters.pin.toml` declaration. Shared synthetic controls
do not consume this production capture; the separate source-conformance target does.

The source tests directly iterate these arrays. Expected booleans come only from
table membership, never from Auctioneer or executing a validator. Nine invalid rows
are empty strings (appnexus, criteo, msft, pubmatic, sharethrough, sovrn,
thetradedesk, visx, yahooAds); three more contain malformed JSON (kargo), and
Visx also includes the non-JSON literal `nil`. All thirteen retain their exact
raw input bytes and false booleans. FeedAd's repeated valid `some-` entry and
invalid `advertising_id` entry remain distinct rows with distinct source offsets.
JSON syntax classification is not a replacement for source expectations.

The witness supports only the inspected raw-literal tables and their separators:
ASCII whitespace and newline-terminated `//` comments. Kargo has one trailing
comment and Unruly has eight. Complete blob digests bind those comments; table
witnesses account for every literal and reject hidden members after a comment's
newline, code, quoted-string members, and block comments. No general Go parser,
nonliteral expression evaluation, or source execution is supported.

This is source-only reference capture, not full engine/resource qualification,
production runtime behavior, or bidder migration. It makes no claim of complete
bidder/reference coverage. In particular, rise, rubicon, and triplelift have no
`params_test.go` at this revision and contribute no invented cases or counts.

## Array Content Spans

These zero-based half-open byte spans start immediately after the declaration's
opening `{` and end immediately before its terminal `}`. They include the leading
and trailing newlines inside the braces, but neither brace.

| Schema | Group | Start | End (exclusive) | Bytes |
| --- | --- | ---: | ---: | ---: |
| appnexus | valid | 1371 | 2158 | 787 |
| appnexus | invalid | 2190 | 3119 | 929 |
| bidstack | valid | 136 | 476 | 340 |
| bidstack | invalid | 508 | 589 | 81 |
| criteo | valid | 1355 | 2026 | 671 |
| criteo | invalid | 2058 | 2500 | 442 |
| feedad | valid | 907 | 2720 | 1813 |
| feedad | invalid | 2752 | 4245 | 1493 |
| kargo | valid | 904 | 1005 | 101 |
| kargo | invalid | 1037 | 1271 | 234 |
| msft | valid | 1325 | 2088 | 763 |
| msft | invalid | 2120 | 2857 | 737 |
| openx | valid | 1347 | 1767 | 420 |
| openx | invalid | 1799 | 2540 | 741 |
| pubmatic | valid | 1389 | 1852 | 463 |
| pubmatic | invalid | 1884 | 2913 | 1029 |
| sharethrough | valid | 925 | 1229 | 304 |
| sharethrough | invalid | 1261 | 1586 | 325 |
| sovrn | valid | 922 | 1082 | 160 |
| sovrn | invalid | 1114 | 1158 | 44 |
| thetradedesk | valid | 1107 | 1308 | 201 |
| thetradedesk | invalid | 1340 | 1487 | 147 |
| unruly | valid | 973 | 1014 | 41 |
| unruly | invalid | 1046 | 1490 | 444 |
| visx | valid | 965 | 1080 | 115 |
| visx | invalid | 1112 | 1328 | 216 |
| yahooAds | valid | 1357 | 1391 | 34 |
| yahooAds | invalid | 1423 | 1544 | 121 |

## Added Source Pins

These complete immutable Git blobs supply the additional six raw-literal tables.
Each member has raw backticks and an immediate comma; only ASCII whitespace and
newline-terminated `//` comments may occupy gaps. The existing strict source
owner and table witnesses also apply to all three additions.

| Schema | Git Blob ID | SHA-256 |
| --- | --- | --- |
| bidstack | `cc240292807de5458775c2b9bce3dbc7f702055c` | `758aeca79079736f92ee43aee731e8eae84a278bb3c888c09e1025e1a0a1b286` |
| feedad | `279b6c6c6d815107876961e36745ce0b5d15baab` | `edc6e30d96eb6b3a6c5bb7509d22ed672b9abaf34b70169500fb87a4cfb72d39` |
| visx | `de8925866e786afc6bf8153e11d994cf9096c428` | `883ad7bd04b83931ab01b2b8465c483b3ad93f33dd2d6719f4b5cf58e844e593` |

## Attribution

Derived from Prebid Server, copyright 2017 PREBID.ORG, INC, under Apache-2.0;
see the shared [source license](../../../../../src/bidders/LICENSE.reference).

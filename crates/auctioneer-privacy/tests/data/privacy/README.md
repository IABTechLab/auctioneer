# Privacy Reference Fixtures

These fixtures record actual outputs from
[Prebid Server](https://github.com/prebid/prebid-server), by Prebid.org and its
contributors, at commit
`d7f8515b86258688304b0d9b6668c6a0e258bc9e`, its `go.mod`/`go.sum`, and
[Prebid's go-gpp v0.2.0](https://github.com/prebid/go-gpp/tree/v0.2.0).
The local upstream checkout HEAD is not the
reference. The historical `reference/main.go` called the pinned packages
directly and is retained in Git history, not as executable repository tooling.
Toolchain and Unicode versions are embedded
in `reference.json`.

`fold_orbits` contains every nonidentity mapping to the least scalar in each
Go `unicode.SimpleFold` orbit. Identity rows are implicit. This permits an
exhaustive equivalence comparison across all 1,112,064 Unicode scalars without
storing a million rows. Qualification selected `unicode-case-mapping =0.5.0`,
whose tables are Unicode 15.0.0, matching Go 1.23.12; locale/full folding and
normalization are not used. The candidate has no dependencies and passed an
isolated native exhaustive comparison and compile checks for the three
existing WASM targets before workspace adoption.

Validate from the Auctioneer worktree root:

```sh
cargo run --locked --offline -p auctioneer-cli --no-default-features -- reference check --root .
cargo run --locked --offline -p auctioneer-cli --no-default-features -- reference check --root /path/to/staged-worktree
```

The Rust utility verifies frozen bytes and original metadata; it does not
regenerate expectations from Auctioneer or execute upstream. The existing Rust
consumers own row shapes and behavior; `privacy_reference.rs` pins all six
group counts, unique scrub names and ordered valid Unicode fold pairs.
Future recapture must happen externally using the pinned reference. Staged
checks are read-only import review against committed pins, not automatic
acceptance. Retired Go programs and harnesses are not included in this
fresh-history repository.

The reference repository is Apache-2.0 licensed. The historical generator
imported public APIs rather than embedding their implementation. Envelope-only
parser fixtures live separately under `src/gpp/tests`, so the future parser
crate does not depend on these Auctioneer policy fixtures.

## Interpretation

`usp` records value validation and sale opt-out. `selection` uses opaque
placeholder TCF values and records source selection only, not valid TCF
authorization. `nosale` records the reference parser's behavior; error
precedence with malformed USP intentionally differs from Auctioneer. `activities`
records all eight activities and explicit ordered rule/component inputs.
`scrubs` stores input and actual serialized reference output; rows with
intentional differences are named separately from parity cases.

The thirteen approved M4 decisions remain in section 12 of the M4 design:
inactive section 2 handling; opaque unenforced sections; account CCPA
fallback; hard parsing/configuration limits; no implicit DV grant; independent
no-sale validation; consistent native/legacy EID removal; USP conflicts do not
disable enforcement; empty USPV1 fallback; Unicode simple folding; and
negative-coordinate rounding/idempotence; unconditional COPPA user-FPD removal;
and enabled CCPA ID/geo scrubbing for malformed selected nonempty USP. The last
two were approved on 2026-10-12 as Auctioneer policy, not IAB-mandated transforms.
They do not alter syntax validation, source precedence, no-sale exceptions, or
the validated legacy projection. Do not overwrite reference outputs
to match Auctioneer. Auctioneer-specific expectations and integration assertions must
name the relevant deliberate difference.
The `coppa` reference row is labeled `intentional-coppa-user-fpd-difference`;
its recorded reference output is unchanged and is not a Auctioneer parity oracle.

Fixture generation is evidence acquisition, not full M4 acceptance. Rust
consumers, outbound captures, resource tests, and portable builds are tracked
independently by the implementation plan.

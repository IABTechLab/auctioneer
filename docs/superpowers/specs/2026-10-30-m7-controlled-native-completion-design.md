# M7 Controlled Native Delivery

Status: approved by AG on 2026-10-30. This explicitly amends M7's earlier
proof-before-activation requirement; it does not claim those proofs exist.

## Delivery Boundary

M7 delivers the existing frozen 14-flow priority set for controlled native
use. Complete source/ABI worst-case schema memory, initialization, work and
stack certification moves to M7-R0. Guest production readiness remains blocked.
These are tested reservations, not qualified upper bounds or physical-OOM
guarantees. Historical evidence retains its original meaning and bytes.

Keep imported schemas unchanged, compile them with the existing reviewed
converter, and use the existing regex engine and explicit caches. Retain
byte/node/depth admission, positive work charges, the original deadline,
aggregate work accounting, privacy-first survivor ordering and fixed errors.
No runtime schema compilation, network schema resolution or allow-all fallback.

Use the existing predeclared component ceilings as native reservations:
16 MiB catalog residency, 8 MiB initialization and 512 KiB sequential
validation scratch per auction. They enter the existing additive accounting
with their existing lifetimes/populations, not baked-config parsing. Existing
context/diagnostic charges remain. Freeze these before new measurements.
Measured/stress allocation failures refuse completion; do not raise these
reservations or the deployment ceiling after seeing a failure.

Freeze native work policy before those runs: 4,194,304 tokens per evaluation,
67,108,864 per auction, with a positive precharge of
`1024 + 16 * compact_json_bytes + 128 * syntactic_nodes` per evaluation.
These tokens are admission reservations, not a source-derived instruction or
CPU-time upper bound. Input admission remains 65,536 bytes, 4,096 syntactic
nodes (including keys), and container depth 32. Recheck the original deadline
before and after evaluation; no timer, cache or work allowance is renewed for
another impression, alias or bidder.

## Publication And Activation

Definitions remain bidder-owned and automatically discovered. Mark new
controlled-native publications declaratively; exclude them from guest public
metadata and serving dispatch together. Do not add a central bidder-name
switch, plugin loader or operator rebuild requirement.

New publications default inactive and require existing runtime enablement.
This is separate from immutable source safety vetoes. Compiled alias ancestry
must retain its canonical-base runtime and baked vetoes; enabling only an
alias cannot enable its base.
Existing publication activation and guest behavior are preserved. A funded
native boot also validates selected legacy public schemas, so all 14 native
priority flows receive admission. Unfunded legacy-only contexts retain their
existing protocol path; this is never a fallback for new native-only definitions.
Settings, regional routing, credentials and sync remain independently required.

Generate schema dispatch and cache wrappers from the same declared bindings,
not another manual list. Initialize shared patterns only after the native
reservation is installed, before admitted native-schema evaluation. Cache
initialization failure, preserve bounded liveness and never retry an invalid
installation through a request. Each evaluation uses caller-owned cache
storage and returns a fixed refusal if the installed reservation is absent.
Do not label inspection-only staging as qualified or enable it for serving.

## Acceptance

- All 14 frozen flows produce real calls and bids through generated-App
  dispatch, using actual implementations and source-derived fixtures.
- Invalid parameters, missing settings/secrets, privacy denial, disabled
  aliases and expired work preserve valid siblings with exact attribution.
- Fresh configuration snapshots enable/disable/re-enable without rebuilding
  or reconstructing the process registry.
- Applicable sync uses the existing signed, browser-bounded cookie path.
- Native accounting plus existing headroom fits without reducing workload,
  raising ceilings or changing the equation. Reservation/allocation/stress
  tests are mandatory, explicitly distinct from static worst-case proof.
- Secure-default, alternate-feature, guest nonactivation/build and source
  integrity regressions pass. Evidence producers execute real current tests
  and cannot certify missing operations or replay old success.

M7 closure records controlled-native acceptance only. M7-R0 owns complete
worst-case resource certification and affected-target readiness; M7.6 owns
hosted-provider/publisher execution. Additional bidders stay in M7.7/M20.
No frozen priority protocol branch is waived by this resource amendment.

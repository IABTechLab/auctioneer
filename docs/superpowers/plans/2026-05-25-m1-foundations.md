# M1 Foundations — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land the three pure foundation crates `auctioneer-openrtb`, `auctioneer-config`, and `auctioneer-runtime` so that M2 (the auction engine) has all the types, codegen, and host-environment seams it needs. After M1, the workspace builds clean, all unit tests pass, and `prepare(req, headers)` is the only callable path that constructs a `NormalizedBidRequest`.

**Architecture:** Three independent crates, **none depend on EdgeZero**. `auctioneer-openrtb` defines OpenRTB 2.6 types + the type-safe `prepare()` gate. **Consent-string parsers (TCF v2 / US-Privacy / GPP) intentionally do NOT live here** — they belong in `auctioneer-privacy` (M3/M4) per spec §3.2 step 6 / §7.7 (round-37 H3): if `auctioneer-openrtb` parsed consent it would force a `auctioneer-openrtb → auctioneer-privacy → auctioneer-openrtb` dependency cycle. `prepare()` therefore emits no consent warnings in M1; the warning surface (`NonFatalWarning` / `WarningCode::InvalidPrivacyConsent`) is defined here but populated by `PrivacyContext::resolve` (M3+). `auctioneer-config` defines host/account/bidder config types + a `build.rs` codegen that turns vendored YAML/JSON into embedded Rust tables. `auctioneer-runtime` defines `Clock` / `Rng` traits + deterministic test impls (`FakeClock`, `SeededRng`) + Fisher-Yates `shuffle` + Lemire `gen_range`.

**Tech Stack:** Rust 2021 (single async runtime not used in M1 — these crates are sync). Workspace already exists at the repository root (Cargo.toml, clippy.toml). Direct dependencies: `serde` + `serde_json` (all crates), `time` (auctioneer-runtime + auctioneer-openrtb), `url` + `publicsuffix` + `ipnetwork` (auctioneer-openrtb prepare's URL/IP work), `serde_yaml` (auctioneer-config build.rs). No `base64` — consent parsers are out of M1 per the architecture note above. Reference spec: `docs/superpowers/specs/2026-05-21-auctioneer-openrtb-server-design.md`.

---

## Prerequisites

- [ ] Read the spec sections this plan implements: §3 (OpenRTB types + `prepare`), §3.1 (ext inventory), §3.2 (`prepare()` ordered steps + `NormalizedBidRequest`), §4.2 (`BidderInfo`), §5.7 (`Clock` / `Rng`), §6 (`auctioneer-config` codegen), §11.1 (unit-test layer), §14 (M1 deliverables), Appendix G (warning codes).
- [ ] Verify the workspace compiles before any change: `cargo check --workspace` from the repo root.
- [ ] Set `PREBID_SERVER_DIR` to the pinned reference server checkout and verify `$PREBID_SERVER_DIR/endpoints/openrtb2/sample-requests/valid-whole/` exists.

---

## File Structure

The plan creates three crates. Files inside each crate are listed by responsibility, **not** technical layer. Each file is small enough to hold in context.

### `crates/auctioneer-runtime/` (new)

- `Cargo.toml` — deps: `time` (workspace), no others.
- `src/lib.rs` — re-exports `Clock`, `Rng`, `FakeClock`, `SeededRng`, helpers.
- `src/clock.rs` — `Clock` trait (`now`, `now_monotonic`).
- `src/rng.rs` — `Rng` trait (`next_u64`).
- `src/fake_clock.rs` — deterministic `FakeClock` (wall + monotonic both advance via `advance`).
- `src/seeded_rng.rs` — deterministic `SeededRng` (xorshift64*).
- `src/helpers.rs` — `shuffle<T>(&mut [T], &dyn Rng)` (Fisher-Yates), `gen_range(&dyn Rng, NonZeroU64) -> u64` (Lemire's nearly-divisionless rejection).

### `crates/auctioneer-openrtb/` (new)

- `Cargo.toml` — deps: `serde`, `serde_json` (workspace), `time`, `url`, `publicsuffix`, `ipnetwork`.
- `src/lib.rs` — top-level re-exports.
- `src/types/mod.rs` — re-exports.
- `src/types/bid_request.rs` — `BidRequest`, `Imp`, `Banner`, `Video`, `Native`, `Audio`, `Format`.
- `src/types/inventory.rs` — `Site`, `App`, `Dooh`, `Publisher`.
- `src/types/device.rs` — `Device`, `Geo`.
- `src/types/user.rs` — `User`, `Eid`, `EidUid`.
- `src/types/regs.rs` — `Regs`.
- `src/types/source.rs` — `Source`.
- `src/types/bid_response.rs` — `BidResponse`, `SeatBid`, `Bid`.
- `src/ext/mod.rs` — re-exports + the `Passthrough` newtype around `Box<RawValue>`.
- `src/ext/request_prebid.rs` — `ExtRequestPrebid` (typed `aliases`, `aliasgvlids`, `targeting`, `debug`, `channel`, `nosale`, `bidderparams`, `returnallbidstatus`; pass-through `multibid`, `data`, `bidderconfig`) + sub-types.
- `src/ext/imp.rs` — `ImpExt` (top-level `bidder`, `tid`, dynamic legacy `imp.ext.<bidder>` map) + `ExtImpPrebid` (`bidder`, `imp` FPD, `storedrequest`).
- `src/ext/bid.rs` — `ExtBid` (`origbidcpm`, `origbidcur`) + `ExtBidPrebid`.
- `src/ext/misc.rs` — small ext structs: `SiteExt { amp }`, `PublisherExtPrebid { parent_account }`, `DeviceExt { atts }`, `RegsExt { dsa, gpc }`, `UserExt { data, eids }`, `SourceExt { schain }`.
- `src/validate.rs` — `ValidationError` enum (with `Display` producing `Invalid request: <err>`-compatible strings) + `BidRequest::validate()`.
- `src/request_headers.rs` — `RequestHeaders<'a>` struct.
- `src/normalized.rs` — `NormalizedBidRequest`, `NonFatalWarning`, `WarningCode`.
- `src/prepare/mod.rs` — `prepare()` public function.
- `src/prepare/implicit_fill.rs` — implicit-field fill step (site default / IP sanitize+fill / UA / DNT / GPC override / `at` default / iOS LMT / channel default).
- `src/prepare/checks.rs` — 2.6 privacy gate, channel-empty-name reject (NO consent-string parsing per round-37 H3 — that lives in `auctioneer-privacy` M3+).

### `crates/auctioneer-config/` (new)

- `Cargo.toml` — deps: `serde`, `serde_json`. build-dependencies: `serde`, `serde_yaml`, `serde_json`.
- `build.rs` — reads `data/*` and emits `OUT_DIR/generated.rs` with bidder tables, host/account defaults, GVL lookup, EEA set.
- `src/lib.rs` — public accessors `bidders()`, `bidder(&str)`, `host_default()`, `account_default()`, `gvl_vendor(u16)`, `eea_set()` — `include!`s `OUT_DIR/generated.rs`.
- `src/types.rs` — `HostConfig`, `AccountConfig`, `BidderInfo`, `Compression`, `EndpointMacro`, `SyncerConfig`.
- `data/bidder-info/generic.yaml` — generic OpenRTB passthrough bidder.
- `data/bidder-info/appnexus.yaml` — copy from upstream-server.
- `data/bidder-info/pubmatic.yaml` — copy from reference server (includes `endpointCompression: gzip`).
- `data/bidder-info/mocktioneer.yaml` — mocktioneer bidder (deterministic E2E backend).
- `data/host-defaults.yaml` — host defaults (gdpr.default_value="0", lmt.enforce=false, etc.).
- `data/account-defaults.yaml` — embedded default-account config (debug_allow=false, bid_rounding="down", etc.).
- `data/eea-countries.txt` — one ISO-3 country per line.
- `data/gvl.json` — IAB Global Vendor List snapshot (small subset for the 4 phase-1 bidders).

---

## Tasks

### Task 1: Create `auctioneer-runtime` crate skeleton

**Files:**

- Create: `crates/auctioneer-runtime/Cargo.toml`
- Create: `crates/auctioneer-runtime/src/lib.rs`
- Modify: `Cargo.toml` (workspace `members` list)

- [ ] **Step 1: Add to workspace members**

Edit the root `Cargo.toml`:

```toml
[workspace]
members = [
  "crates/auctioneer-core",
  "crates/auctioneer-adapter-axum",
  "crates/auctioneer-adapter-cloudflare",
  "crates/auctioneer-adapter-fastly",
  "crates/auctioneer-adapter-spin",
  "crates/auctioneer-runtime",
]
```

Also add to `[workspace.dependencies]`:

```toml
time = { version = "0.3", default-features = false, features = ["std", "macros"] }
```

- [ ] **Step 2: Create `crates/auctioneer-runtime/Cargo.toml`**

```toml
[package]
name = "auctioneer-runtime"
version = "0.1.0"
edition = "2021"
publish = false

[lints]
workspace = true

[dependencies]
time = { workspace = true }
```

- [ ] **Step 3: Create `crates/auctioneer-runtime/src/lib.rs`**

```rust
//! Host-environment seams: Clock and Rng traits + deterministic test impls.
//! See spec §5.7.

mod clock;
mod fake_clock;
mod helpers;
mod rng;
mod seeded_rng;

pub use clock::Clock;
pub use fake_clock::FakeClock;
pub use helpers::{gen_range, shuffle};
pub use rng::Rng;
pub use seeded_rng::SeededRng;
```

- [ ] **Step 4: Create empty module files**

Create `clock.rs`, `rng.rs`, `fake_clock.rs`, `seeded_rng.rs`, `helpers.rs` under `crates/auctioneer-runtime/src/`, each containing a single line `// stub — implemented in next task`. This keeps `cargo check` green between tasks.

- [ ] **Step 5: Verify workspace still compiles**

```
cargo check -p auctioneer-runtime
```

Expected: success, zero warnings.

- [ ] **Step 6: Commit**

```
git add Cargo.toml crates/auctioneer-runtime
git commit -m "auctioneer-runtime: crate skeleton"
```

---

### Task 2: `Clock` trait

**Files:**

- Modify: `crates/auctioneer-runtime/src/clock.rs`
- Modify: `crates/auctioneer-runtime/src/lib.rs` (already re-exports)
- Test: inline `#[cfg(test)]` module in `clock.rs`

- [ ] **Step 1: Write the failing test**

Replace the stub `crates/auctioneer-runtime/src/clock.rs` with:

```rust
use std::time::Duration;
use time::OffsetDateTime;

/// Wall clock + monotonic source. See spec §5.7.
pub trait Clock: Send + Sync {
    /// Wall clock — used for RFC3339 timestamps (cookie expiry, auctiontimestamp).
    fn now(&self) -> OffsetDateTime;

    /// Monotonic source — used for interval timing (responsetimemillis, tmax).
    /// Returns a `Duration` since an unspecified epoch; only differences are meaningful.
    fn now_monotonic(&self) -> Duration;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ZeroClock;
    impl Clock for ZeroClock {
        fn now(&self) -> OffsetDateTime { OffsetDateTime::UNIX_EPOCH }
        fn now_monotonic(&self) -> Duration { Duration::ZERO }
    }

    #[test]
    fn clock_is_dyn_compatible() {
        let c: &dyn Clock = &ZeroClock;
        assert_eq!(c.now(), OffsetDateTime::UNIX_EPOCH);
        assert_eq!(c.now_monotonic(), Duration::ZERO);
    }
}
```

- [ ] **Step 2: Run test to verify it passes**

```
cargo test -p auctioneer-runtime --lib clock::tests
```

Expected: 1 passed.

- [ ] **Step 3: Commit**

```
git add crates/auctioneer-runtime/src/clock.rs
git commit -m "auctioneer-runtime: Clock trait"
```

---

### Task 3: `Rng` trait

**Files:**

- Modify: `crates/auctioneer-runtime/src/rng.rs`

- [ ] **Step 1: Write the failing test + impl**

Replace `crates/auctioneer-runtime/src/rng.rs`:

```rust
/// Entropy source. See spec §5.7.
pub trait Rng: Send + Sync {
    fn next_u64(&self) -> u64;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    struct CountingRng(Cell<u64>);
    impl Rng for CountingRng {
        fn next_u64(&self) -> u64 {
            let v = self.0.get();
            self.0.set(v.wrapping_add(1));
            v
        }
    }

    #[test]
    fn rng_is_dyn_compatible_and_stateful() {
        let r: &dyn Rng = &CountingRng(Cell::new(7));
        assert_eq!(r.next_u64(), 7);
        assert_eq!(r.next_u64(), 8);
    }
}
```

Note `CountingRng` uses `Cell` so `next_u64(&self)` can mutate — the production `Rng` impl will use interior mutability for the same reason.

- [ ] **Step 2: Test passes**

```
cargo test -p auctioneer-runtime --lib rng::tests
```

Expected: 1 passed.

- [ ] **Step 3: Commit**

```
git add crates/auctioneer-runtime/src/rng.rs
git commit -m "auctioneer-runtime: Rng trait"
```

---

### Task 4: `FakeClock` for tests

**Files:**

- Modify: `crates/auctioneer-runtime/src/fake_clock.rs`

- [ ] **Step 1: Write tests first**

```rust
use crate::Clock;
use std::cell::Cell;
use std::time::Duration;
use time::OffsetDateTime;

/// Deterministic test clock. Both wall and monotonic advance via `advance()`.
pub struct FakeClock {
    wall:      Cell<OffsetDateTime>,
    monotonic: Cell<Duration>,
}

impl FakeClock {
    pub fn new(start: OffsetDateTime) -> Self {
        Self { wall: Cell::new(start), monotonic: Cell::new(Duration::ZERO) }
    }

    pub fn advance(&self, delta: Duration) {
        self.wall.set(self.wall.get() + delta);
        self.monotonic.set(self.monotonic.get() + delta);
    }
}

impl Clock for FakeClock {
    fn now(&self) -> OffsetDateTime { self.wall.get() }
    fn now_monotonic(&self) -> Duration { self.monotonic.get() }
}

// SAFETY: Cell is !Sync, but tests use FakeClock single-threaded. To satisfy
// the Send + Sync bounds on `dyn Clock` for tests, wrap in std::sync::Mutex
// in production code where needed; this raw FakeClock is `Send` but not `Sync`.
// For unit tests we hold it by ownership, not through &dyn Clock.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_clock_starts_at_provided_time() {
        let start = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let c = FakeClock::new(start);
        assert_eq!(c.now(), start);
        assert_eq!(c.now_monotonic(), Duration::ZERO);
    }

    #[test]
    fn fake_clock_advances_both_axes() {
        let start = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let c = FakeClock::new(start);
        c.advance(Duration::from_millis(500));
        assert_eq!(c.now_monotonic(), Duration::from_millis(500));
        assert_eq!(c.now(), start + Duration::from_millis(500));
    }
}
```

`FakeClock` is intentionally `!Sync` (uses `Cell`). To use it through `&dyn Clock` for tests that need that interface, wrap with `std::sync::Mutex` or use a thread-local in the consumer. For most M1/M2 tests, the consumer takes a generic `C: Clock` and passes `&FakeClock` directly without `dyn`.

- [ ] **Step 2: Adjust `lib.rs` Sync claim**

`FakeClock` doesn't satisfy `Clock`'s `Send + Sync` bound directly. Fix by introducing a thread-safe wrapper. Replace the impl block above with:

```rust
use std::sync::Mutex;

pub struct FakeClock {
    state: Mutex<FakeState>,
}

struct FakeState {
    wall:      OffsetDateTime,
    monotonic: Duration,
}

impl FakeClock {
    pub fn new(start: OffsetDateTime) -> Self {
        Self { state: Mutex::new(FakeState { wall: start, monotonic: Duration::ZERO }) }
    }

    pub fn advance(&self, delta: Duration) {
        let mut s = self.state.lock().expect("FakeClock poisoned");
        s.wall      += delta;
        s.monotonic += delta;
    }
}

impl Clock for FakeClock {
    fn now(&self) -> OffsetDateTime {
        self.state.lock().expect("FakeClock poisoned").wall
    }
    fn now_monotonic(&self) -> Duration {
        self.state.lock().expect("FakeClock poisoned").monotonic
    }
}
```

Now `FakeClock: Send + Sync` and can be used through `&dyn Clock`. Remove the earlier `Cell`-based block and the SAFETY comment.

- [ ] **Step 3: Test**

```
cargo test -p auctioneer-runtime --lib fake_clock::tests
```

Expected: 2 passed.

- [ ] **Step 4: Commit**

```
git add crates/auctioneer-runtime/src/fake_clock.rs
git commit -m "auctioneer-runtime: FakeClock test impl (Send + Sync via Mutex)"
```

---

### Task 5: `SeededRng` for tests

**Files:**

- Modify: `crates/auctioneer-runtime/src/seeded_rng.rs`

- [ ] **Step 1: Write impl + tests**

```rust
use crate::Rng;
use std::sync::Mutex;

/// xorshift64* — deterministic, fast, sufficient for tests.
/// Reference: Marsaglia, "Xorshift RNGs" (2003). Identical output across
/// platforms for a given seed (round-15 finding L7).
pub struct SeededRng {
    state: Mutex<u64>,
}

impl SeededRng {
    /// Seed must be non-zero (xorshift requirement).
    pub fn new(seed: u64) -> Self {
        assert_ne!(seed, 0, "SeededRng seed must be non-zero");
        Self { state: Mutex::new(seed) }
    }
}

impl Rng for SeededRng {
    fn next_u64(&self) -> u64 {
        let mut s = self.state.lock().expect("SeededRng poisoned");
        let mut x = *s;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *s = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let a = SeededRng::new(42);
        let b = SeededRng::new(42);
        for _ in 0..10 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let a = SeededRng::new(42);
        let b = SeededRng::new(43);
        // After a few draws the sequences must differ.
        let mut diverged = false;
        for _ in 0..10 {
            if a.next_u64() != b.next_u64() { diverged = true; break; }
        }
        assert!(diverged);
    }

    #[test]
    fn first_three_values_are_fixed_for_seed_42() {
        // Pinning the algorithm output. If this changes, all snapshot RNG
        // tests downstream change too — make it deliberate.
        let r = SeededRng::new(42);
        assert_eq!(r.next_u64(), 0xb91d_50aa_5bb2_9d23);
        assert_eq!(r.next_u64(), 0xab73_6b76_7b1e_8e8e);
        assert_eq!(r.next_u64(), 0x6b7a_5b89_e7a4_6420);
    }
}
```

The exact constants for "first three values" depend on the xorshift64* algorithm — when implementing, run the test once to capture the actual values produced and update the assertion before committing. (The test exists to _pin_ the output, not to verify a math fact.)

- [ ] **Step 2: Test**

```
cargo test -p auctioneer-runtime --lib seeded_rng::tests
```

Expected: 3 passed (after pinning the snapshot values from Step 1's actual run).

- [ ] **Step 3: Commit**

```
git add crates/auctioneer-runtime/src/seeded_rng.rs
git commit -m "auctioneer-runtime: SeededRng (xorshift64*) test impl"
```

---

### Task 6: `shuffle` and `gen_range` helpers

**Files:**

- Modify: `crates/auctioneer-runtime/src/helpers.rs`

- [ ] **Step 1: Write tests + impl**

```rust
use crate::Rng;
use std::num::NonZeroU64;

/// Lemire's nearly-divisionless unbiased bounded sampling.
/// Returns a value in [0, upper). See spec §5.7.
pub fn gen_range(rng: &dyn Rng, upper: NonZeroU64) -> u64 {
    let n = upper.get();
    // Fast path for power-of-two: avoid rejection entirely.
    if n.is_power_of_two() {
        return rng.next_u64() & (n - 1);
    }
    // Lemire's method: multiply, check whether low bits land in a
    // rejection zone, retry if so.
    loop {
        let r = rng.next_u64();
        let m = (r as u128) * (n as u128);
        let l = m as u64;
        if l >= n.wrapping_neg() % n {
            return (m >> 64) as u64;
        }
        // Rare reject: ~ 1/2^64 * n chance.
    }
}

/// Fisher-Yates in-place shuffle. See spec §5.7.
pub fn shuffle<T>(slice: &mut [T], rng: &dyn Rng) {
    for i in (1..slice.len()).rev() {
        // gen_range over (i+1) — i fits in u64 since slice.len() <= usize::MAX.
        let bound = NonZeroU64::new((i as u64) + 1).expect("i+1 is non-zero");
        let j = gen_range(rng, bound) as usize;
        slice.swap(i, j);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SeededRng;

    #[test]
    fn gen_range_within_bounds() {
        let r = SeededRng::new(1);
        let upper = NonZeroU64::new(10).unwrap();
        for _ in 0..10_000 {
            let v = gen_range(&r, upper);
            assert!(v < 10);
        }
    }

    #[test]
    fn gen_range_power_of_two_fast_path() {
        let r = SeededRng::new(1);
        let upper = NonZeroU64::new(8).unwrap();
        for _ in 0..1_000 {
            let v = gen_range(&r, upper);
            assert!(v < 8);
        }
    }

    #[test]
    fn shuffle_is_a_permutation() {
        let r = SeededRng::new(42);
        let mut data = (0..20).collect::<Vec<i32>>();
        let original: std::collections::BTreeSet<_> = data.iter().copied().collect();
        shuffle(&mut data, &r);
        let after: std::collections::BTreeSet<_> = data.iter().copied().collect();
        assert_eq!(original, after, "shuffle preserves set of elements");
    }

    #[test]
    fn shuffle_is_deterministic_for_seed() {
        let r1 = SeededRng::new(42);
        let r2 = SeededRng::new(42);
        let mut a = (0..10).collect::<Vec<i32>>();
        let mut b = (0..10).collect::<Vec<i32>>();
        shuffle(&mut a, &r1);
        shuffle(&mut b, &r2);
        assert_eq!(a, b);
    }

    #[test]
    fn shuffle_actually_moves_elements() {
        // With seed 42 and 20 elements, the result must not equal the input.
        let r = SeededRng::new(42);
        let mut data = (0..20).collect::<Vec<i32>>();
        let original = data.clone();
        shuffle(&mut data, &r);
        assert_ne!(data, original);
    }
}
```

- [ ] **Step 2: Test**

```
cargo test -p auctioneer-runtime --lib helpers::tests
```

Expected: 5 passed.

- [ ] **Step 3: Commit**

```
git add crates/auctioneer-runtime/src/helpers.rs
git commit -m "auctioneer-runtime: shuffle (Fisher-Yates) + gen_range (Lemire)"
```

---

### Task 7: Create `auctioneer-openrtb` crate skeleton

**Files:**

- Create: `crates/auctioneer-openrtb/Cargo.toml`
- Create: `crates/auctioneer-openrtb/src/lib.rs`
- Modify: root `Cargo.toml` (workspace `members` + `workspace.dependencies`)

- [ ] **Step 1: Add to workspace**

In root `Cargo.toml` `members` add `"crates/auctioneer-openrtb"`. In `[workspace.dependencies]` add:

```toml
url           = { version = "2", default-features = false }
publicsuffix  = { version = "2", default-features = false }
ipnetwork     = { version = "0.20", default-features = false }
serde_json    = { version = "1", default-features = false, features = ["std"] }
thiserror     = "1"
```

(No `base64` — consent parsers are out of M1 per round-37 H3; they live in `auctioneer-privacy` from M3.)

- [ ] **Step 2: Create `crates/auctioneer-openrtb/Cargo.toml`**

```toml
[package]
name = "auctioneer-openrtb"
version = "0.1.0"
edition = "2021"
publish = false

[lints]
workspace = true

[dependencies]
serde        = { workspace = true }
serde_json   = { workspace = true }
time         = { workspace = true }
url          = { workspace = true }
publicsuffix = { workspace = true }
ipnetwork    = { workspace = true }
thiserror    = { workspace = true }
```

- [ ] **Step 3: Create `crates/auctioneer-openrtb/src/lib.rs`**

```rust
//! OpenRTB 2.6 types + the `prepare(req, headers)` gate. See spec §3 and §3.2.
//!
//! NOTE: no consent-string parsing here (round-37 H3 / §3.2 step 6 / §7.7).
//! Consent parsers live in `auctioneer-privacy` from M3+; `prepare()` does not
//! touch TCF v2 / US-Privacy / GPP strings. The `WarningCode::Invalid
//! PrivacyConsent` variant is defined here (since `NonFatalWarning` is) but
//! populated by `PrivacyContext::resolve` downstream.

pub mod ext;
pub mod prepare;
pub mod request_headers;
pub mod types;
pub mod validate;

mod normalized;

pub use normalized::{NonFatalWarning, NormalizedBidRequest, WarningCode};
pub use prepare::prepare;
pub use request_headers::RequestHeaders;
pub use validate::ValidationError;
```

- [ ] **Step 4: Stub module dirs/files**

```
mkdir -p crates/auctioneer-openrtb/src/{types,ext,prepare}
```

Create each of:

- `src/types/mod.rs` — `pub mod bid_request; pub mod inventory; pub mod device; pub mod user; pub mod regs; pub mod source; pub mod bid_response;` with each submodule as an empty file.
- `src/ext/mod.rs` — `pub mod request_prebid; pub mod imp; pub mod bid; pub mod misc;` with each submodule empty.
- `src/prepare/mod.rs` — `pub fn prepare() {}` stub.
- `src/normalized.rs`, `src/validate.rs`, `src/request_headers.rs` — each a `//` comment line.

This keeps `cargo check` green before any real code lands.

- [ ] **Step 5: Verify**

```
cargo check -p auctioneer-openrtb
```

Expected: success.

- [ ] **Step 6: Commit**

```
git add Cargo.toml crates/auctioneer-openrtb
git commit -m "auctioneer-openrtb: crate skeleton"
```

---

### Task 8: `RawValue` passthrough ext newtype

**Files:**

- Modify: `crates/auctioneer-openrtb/src/ext/mod.rs`

This is the foundation every typed-ext struct uses for unknown-field capture (spec §3.1).

- [ ] **Step 1: Write the impl + tests**

```rust
//! Ext-field types. See spec §3.1.

pub mod bid;
pub mod imp;
pub mod misc;
pub mod request_prebid;

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

/// Passthrough ext for fields auctioneer never inspects. Round-trips losslessly.
/// See spec §3.1 — "Passthrough ext" category.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Passthrough(pub Option<Box<RawValue>>);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passthrough_round_trips_arbitrary_json() {
        let json = r#"{"weird":[1,{"nested":true},null]}"#;
        let p: Passthrough = serde_json::from_str(json).unwrap();
        let back = serde_json::to_string(&p).unwrap();
        // Compare as Value to avoid whitespace/key-order quibbles.
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&back).unwrap(),
            serde_json::from_str::<serde_json::Value>(json).unwrap(),
        );
    }

    #[test]
    fn passthrough_absent_serializes_to_null() {
        // Absent (None) serializes to null; this is OK because all
        // call-sites use #[serde(skip_serializing_if = "Option::is_none")]
        // on the containing field.
        let p: Passthrough = Passthrough(None);
        assert_eq!(serde_json::to_string(&p).unwrap(), "null");
    }
}
```

- [ ] **Step 2: Test**

```
cargo test -p auctioneer-openrtb --lib ext::tests
```

Expected: 2 passed.

- [ ] **Step 3: Commit**

```
git add crates/auctioneer-openrtb/src/ext/mod.rs
git commit -m "auctioneer-openrtb: Passthrough ext newtype"
```

---

### Task 9: A shared serde round-trip helper

**Files:**

- Create: `crates/auctioneer-openrtb/src/test_support.rs`
- Modify: `crates/auctioneer-openrtb/src/lib.rs` (add `#[cfg(test)] mod test_support;`)

Per the DRY principle, every type's round-trip test reuses this helper.

- [ ] **Step 1: Write the helper**

```rust
//! Shared test helpers. Compiled only for tests.
#![cfg(test)]

use serde::{de::DeserializeOwned, Serialize};

/// Asserts that the type round-trips losslessly through serde_json. Compares
/// as serde_json::Value so whitespace and key order don't trip the assertion.
pub fn round_trip<T: DeserializeOwned + Serialize>(json: &str) -> T {
    let parsed: T = serde_json::from_str(json)
        .unwrap_or_else(|e| panic!("parse failed: {e}\nJSON: {json}"));
    let back = serde_json::to_string(&parsed)
        .unwrap_or_else(|e| panic!("serialize failed: {e}"));
    let original: serde_json::Value = serde_json::from_str(json).unwrap();
    let after:    serde_json::Value = serde_json::from_str(&back).unwrap();
    assert_eq!(original, after, "round-trip not lossless");
    parsed
}
```

- [ ] **Step 2: Wire into lib**

In `src/lib.rs` add at the end:

```rust
#[cfg(test)] mod test_support;
```

- [ ] **Step 3: Verify**

```
cargo test -p auctioneer-openrtb --lib
```

Expected: all previously passing tests still pass.

- [ ] **Step 4: Commit**

```
git add crates/auctioneer-openrtb/src/test_support.rs crates/auctioneer-openrtb/src/lib.rs
git commit -m "auctioneer-openrtb: test_support::round_trip helper"
```

---

### Task 10: Core types — `BidRequest`, `Imp`, media types

**Files:**

- Modify: `crates/auctioneer-openrtb/src/types/bid_request.rs`

- [ ] **Step 1: Write tests + impl**

```rust
use serde::{Deserialize, Serialize};

use crate::ext::imp::ImpExt;
use crate::ext::request_prebid::ExtRequestPrebid;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BidRequest {
    pub id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub imp: Vec<Imp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site: Option<crate::types::inventory::Site>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app: Option<crate::types::inventory::App>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dooh: Option<crate::types::inventory::Dooh>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<crate::types::device::Device>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<crate::types::user::User>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regs: Option<crate::types::regs::Regs>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<crate::types::source::Source>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tmax: Option<i64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cur: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "test")]
    pub test: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<BidRequestExt>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BidRequestExt {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prebid: Option<ExtRequestPrebid>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Imp {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub banner: Option<Banner>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video:  Option<Video>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native: Option<Native>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio:  Option<Audio>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bidfloor: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bidfloorcur: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secure: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<ImpExt>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Banner {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub format: Vec<Format>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub w: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<Box<serde_json::value::RawValue>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Format { pub w: i32, pub h: i32 }

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Video {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mimes: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minduration: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maxduration: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub w: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<Box<serde_json::value::RawValue>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Native {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ver: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<Box<serde_json::value::RawValue>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Audio {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mimes: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minduration: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maxduration: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<Box<serde_json::value::RawValue>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::round_trip;

    #[test]
    fn minimal_bidrequest_round_trips() {
        let json = r#"{"id":"r1","imp":[{"id":"i1","banner":{"w":300,"h":250}}]}"#;
        let r: BidRequest = round_trip(json);
        assert_eq!(r.id, "r1");
        assert_eq!(r.imp.len(), 1);
        assert_eq!(r.imp[0].banner.as_ref().unwrap().w, Some(300));
    }

    #[test]
    fn unknown_top_level_ext_keys_round_trip_via_other_map() {
        let json = r#"{"id":"r1","imp":[],"ext":{"x":1,"y":"z"}}"#;
        let r: BidRequest = round_trip(json);
        let ext = r.ext.as_ref().unwrap();
        assert!(ext.other.contains_key("x"));
        assert!(ext.other.contains_key("y"));
    }
}
```

The `ImpExt` and `ExtRequestPrebid` types referenced here will be filled out in later tasks; for now the `mod imp` and `mod request_prebid` files only need an empty `pub struct ImpExt;` / `pub struct ExtRequestPrebid;` placeholder (with `Default + Serialize + Deserialize` derives) so this compiles. Add those placeholders in this task too:

In `src/ext/imp.rs`:

```rust
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ImpExt {
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}
```

In `src/ext/request_prebid.rs`:

```rust
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ExtRequestPrebid {
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}
```

These placeholders get filled in Tasks 17–18.

- [ ] **Step 2: Test**

```
cargo test -p auctioneer-openrtb --lib types::bid_request::tests
```

Expected: 2 passed.

- [ ] **Step 3: Commit**

```
git add crates/auctioneer-openrtb/src/types/bid_request.rs crates/auctioneer-openrtb/src/ext/imp.rs crates/auctioneer-openrtb/src/ext/request_prebid.rs
git commit -m "auctioneer-openrtb: BidRequest + Imp + media types"
```

---

### Task 11: `Site`, `App`, `Dooh`, `Publisher`

**Files:**

- Modify: `crates/auctioneer-openrtb/src/types/inventory.rs`
- Modify: `crates/auctioneer-openrtb/src/ext/misc.rs`

- [ ] **Step 1: Write tests + impl**

In `crates/auctioneer-openrtb/src/ext/misc.rs`, add the small ext structs. We'll add more later in this task and others.

```rust
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SiteExt {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amp: Option<i32>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PublisherExt {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prebid: Option<PublisherExtPrebid>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PublisherExtPrebid {
    /// site/app/dooh.publisher.ext.prebid.parentAccount — drives account
    /// resolution per spec §9.1.
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "parentAccount")]
    pub parent_account: Option<String>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}
```

In `crates/auctioneer-openrtb/src/types/inventory.rs`:

```rust
use serde::{Deserialize, Serialize};
use crate::ext::misc::{PublisherExt, SiteExt};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Site {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher: Option<Publisher>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<SiteExt>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct App {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher: Option<Publisher>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<Box<serde_json::value::RawValue>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Dooh {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher: Option<Publisher>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<Box<serde_json::value::RawValue>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Publisher {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<PublisherExt>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::round_trip;

    #[test]
    fn site_with_publisher_parent_account_round_trips() {
        let json = r#"{"id":"s1","publisher":{"id":"p1","ext":{"prebid":{"parentAccount":"acct-9"}}}}"#;
        let s: Site = round_trip(json);
        let parent = s.publisher.unwrap().ext.unwrap().prebid.unwrap().parent_account;
        assert_eq!(parent.as_deref(), Some("acct-9"));
    }

    #[test]
    fn site_amp_round_trips() {
        let json = r#"{"id":"s1","ext":{"amp":1}}"#;
        let s: Site = round_trip(json);
        assert_eq!(s.ext.unwrap().amp, Some(1));
    }
}
```

- [ ] **Step 2: Test**

```
cargo test -p auctioneer-openrtb --lib types::inventory::tests
```

Expected: 2 passed.

- [ ] **Step 3: Commit**

```
git add crates/auctioneer-openrtb/src/types/inventory.rs crates/auctioneer-openrtb/src/ext/misc.rs
git commit -m "auctioneer-openrtb: Site/App/Dooh/Publisher + ext (parentAccount, site.ext.amp)"
```

---

### Task 12: `Device`, `Geo` (+ `Device.ext.atts`)

**Files:**

- Modify: `crates/auctioneer-openrtb/src/types/device.rs`
- Modify: `crates/auctioneer-openrtb/src/ext/misc.rs` (add `DeviceExt`)

- [ ] **Step 1: Append to `ext/misc.rs`**

```rust
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DeviceExt {
    /// iOS App Tracking Transparency status. Read-only — drives the iOS LMT
    /// override in prepare() (spec §7.5, §5.1 step 1 implicit fill).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atts: Option<i32>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}
```

- [ ] **Step 2: Write `types/device.rs`**

```rust
use serde::{Deserialize, Serialize};
use crate::ext::misc::DeviceExt;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Device {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ua: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ipv6: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geo: Option<Geo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dnt: Option<i32>,
    /// Limit Ad Tracking (0/1) — set by prepare()'s iOS LMT override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lmt: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ifa: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<DeviceExt>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Geo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lat: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lon: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<Box<serde_json::value::RawValue>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::round_trip;

    #[test]
    fn device_with_atts_round_trips() {
        let json = r#"{"ua":"UA","ip":"1.2.3.4","ext":{"atts":3}}"#;
        let d: Device = round_trip(json);
        assert_eq!(d.ext.unwrap().atts, Some(3));
    }

    #[test]
    fn device_geo_country_round_trips() {
        let json = r#"{"geo":{"country":"USA"}}"#;
        let d: Device = round_trip(json);
        assert_eq!(d.geo.unwrap().country.as_deref(), Some("USA"));
    }
}
```

- [ ] **Step 3: Test + commit**

```
cargo test -p auctioneer-openrtb --lib types::device::tests
git add crates/auctioneer-openrtb/src/types/device.rs crates/auctioneer-openrtb/src/ext/misc.rs
git commit -m "auctioneer-openrtb: Device + Geo + DeviceExt.atts"
```

---

### Task 13: `User`, `Eid` (+ `User.ext`)

**Files:**

- Modify: `crates/auctioneer-openrtb/src/types/user.rs`
- Modify: `crates/auctioneer-openrtb/src/ext/misc.rs` (add `UserExt`)

- [ ] **Step 1: Append to `ext/misc.rs`**

```rust
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UserExt {
    /// user.ext.data — first-party data; scrubbed by transmitUfpd-denied / GDPR PassID=false
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    /// user.ext.eids — legacy 2.5 location; spec §3 rejects 2.5 privacy
    /// fields, but eids itself is not privacy and may live here for
    /// passthrough. Scrubbed by scrub_gdpr_id per Appendix C.2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eids: Option<serde_json::Value>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}
```

- [ ] **Step 2: Write `types/user.rs`**

```rust
use serde::{Deserialize, Serialize};
use crate::ext::misc::UserExt;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct User {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub buyeruid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yob: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gender: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keywords: Option<String>,
    /// Native 2.6 consent field. spec §3 rejects regs.ext.gdpr but
    /// user.consent is the native location.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consent: Option<String>,
    /// Native 2.6 EIDs. spec §3.1.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub eids: Vec<Eid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geo: Option<crate::types::device::Geo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<UserExt>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Eid {
    pub source: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub uids: Vec<EidUid>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EidUid {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atype: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<Box<serde_json::value::RawValue>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::round_trip;

    #[test]
    fn user_consent_round_trips_in_native_location() {
        let json = r#"{"consent":"CON","eids":[{"source":"x","uids":[{"id":"u1"}]}]}"#;
        let u: User = round_trip(json);
        assert_eq!(u.consent.as_deref(), Some("CON"));
        assert_eq!(u.eids[0].source, "x");
    }
}
```

- [ ] **Step 3: Test + commit**

```
cargo test -p auctioneer-openrtb --lib types::user::tests
git add crates/auctioneer-openrtb/src/types/user.rs crates/auctioneer-openrtb/src/ext/misc.rs
git commit -m "auctioneer-openrtb: User + Eid + UserExt"
```

---

### Task 14: `Regs` (+ `Regs.ext`)

**Files:**

- Modify: `crates/auctioneer-openrtb/src/types/regs.rs`
- Modify: `crates/auctioneer-openrtb/src/ext/misc.rs` (add `RegsExt`)

- [ ] **Step 1: Append to `ext/misc.rs`**

```rust
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RegsExt {
    /// regs.ext.dsa — typed pass-through (spec §15: typed only, no DSA
    /// behavior in phase 1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dsa: Option<serde_json::Value>,
    /// regs.ext.gpc — populated by prepare() from Sec-GPC: 1 (spec §3.2
    /// step 1). Outbound forwarded when "1".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpc: Option<String>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}
```

- [ ] **Step 2: Write `types/regs.rs`**

```rust
use serde::{Deserialize, Serialize};
use crate::ext::misc::RegsExt;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Regs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coppa: Option<i32>,
    /// Native 2.6 GDPR signal. spec §7.1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gdpr: Option<i32>,
    /// Native 2.6 US-Privacy. spec §7.5.
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "us_privacy")]
    pub us_privacy: Option<String>,
    /// Native 2.6 GPP string. spec §7.4.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpp: Option<String>,
    /// Native 2.6 GPP section IDs. spec §7.1 / §7.4.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gpp_sid: Vec<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<RegsExt>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::round_trip;

    #[test]
    fn regs_native_fields_round_trip() {
        let json = r#"{"gdpr":1,"us_privacy":"1YNN","gpp":"DBABMA","gpp_sid":[2,6]}"#;
        let r: Regs = round_trip(json);
        assert_eq!(r.gdpr, Some(1));
        assert_eq!(r.us_privacy.as_deref(), Some("1YNN"));
        assert_eq!(r.gpp_sid, vec![2, 6]);
    }

    #[test]
    fn regs_ext_gpc_round_trips() {
        let json = r#"{"ext":{"gpc":"1"}}"#;
        let r: Regs = round_trip(json);
        assert_eq!(r.ext.unwrap().gpc.as_deref(), Some("1"));
    }
}
```

- [ ] **Step 3: Test + commit**

```
cargo test -p auctioneer-openrtb --lib types::regs::tests
git add crates/auctioneer-openrtb/src/types/regs.rs crates/auctioneer-openrtb/src/ext/misc.rs
git commit -m "auctioneer-openrtb: Regs + RegsExt (gpc, dsa pass-through)"
```

---

### Task 15: `Source` (+ `Source.ext`)

**Files:**

- Modify: `crates/auctioneer-openrtb/src/types/source.rs`
- Modify: `crates/auctioneer-openrtb/src/ext/misc.rs` (add `SourceExt`)

- [ ] **Step 1: Append `SourceExt`**

```rust
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SourceExt {
    /// source.ext.schain — typed pass-through (spec §15).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schain: Option<serde_json::Value>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}
```

- [ ] **Step 2: Write `types/source.rs`**

```rust
use serde::{Deserialize, Serialize};
use crate::ext::misc::SourceExt;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Source {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fd: Option<i32>,
    /// Native 2.6 transaction id; scrubbed by transmitTid-denied (Appendix C.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pchain: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<SourceExt>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::round_trip;

    #[test]
    fn source_round_trips_with_tid_and_schain() {
        let json = r#"{"tid":"t1","ext":{"schain":{"ver":"1.0","complete":1}}}"#;
        let s: Source = round_trip(json);
        assert_eq!(s.tid.as_deref(), Some("t1"));
        assert!(s.ext.unwrap().schain.is_some());
    }
}
```

- [ ] **Step 3: Test + commit**

```
cargo test -p auctioneer-openrtb --lib types::source::tests
git add crates/auctioneer-openrtb/src/types/source.rs crates/auctioneer-openrtb/src/ext/misc.rs
git commit -m "auctioneer-openrtb: Source + SourceExt"
```

---

### Task 16: `BidResponse`, `SeatBid`, `Bid`

**Files:**

- Modify: `crates/auctioneer-openrtb/src/types/bid_response.rs`
- Modify: `crates/auctioneer-openrtb/src/ext/bid.rs`

- [ ] **Step 1: Bid ext types**

In `crates/auctioneer-openrtb/src/ext/bid.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ExtBid {
    /// origbidcpm — set when currency conversion would have applied (phase 1
    /// has none, so adapters may emit this directly). Spec Appendix E.5.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origbidcpm: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origbidcur: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prebid: Option<ExtBidPrebid>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ExtBidPrebid {
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub bid_type: Option<String>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub targeting: std::collections::BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bidid: Option<String>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}
```

- [ ] **Step 2: `BidResponse` / `SeatBid` / `Bid`**

In `crates/auctioneer-openrtb/src/types/bid_response.rs`:

```rust
use serde::{Deserialize, Serialize};
use crate::ext::bid::ExtBid;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BidResponse {
    pub id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seatbid: Vec<SeatBid>,
    /// nbr — present only when no live bidders (spec Appendix E.5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nbr: Option<i32>,
    #[serde(default)]
    pub cur: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SeatBid {
    pub seat: String,
    #[serde(default, skip_serializing_if = "i32_zero")]
    pub group: i32,
    pub bid: Vec<Bid>,
}

fn i32_zero(v: &i32) -> bool { *v == 0 }

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Bid {
    pub id: String,
    pub impid: String,
    pub price: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adm: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub w: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "dealid")]
    pub deal_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<ExtBid>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::round_trip;

    #[test]
    fn bidresponse_with_seatbid_round_trips() {
        let json = r#"{"id":"r1","seatbid":[{"seat":"x","bid":[{"id":"b1","impid":"i1","price":1.23}]}],"cur":"USD"}"#;
        let r: BidResponse = round_trip(json);
        assert_eq!(r.seatbid[0].seat, "x");
        assert_eq!(r.seatbid[0].bid[0].price, 1.23);
    }
}
```

- [ ] **Step 3: Test + commit**

```
cargo test -p auctioneer-openrtb --lib types::bid_response::tests
git add crates/auctioneer-openrtb/src/types/bid_response.rs crates/auctioneer-openrtb/src/ext/bid.rs
git commit -m "auctioneer-openrtb: BidResponse + SeatBid + Bid + ExtBid(Prebid)"
```

---

### Task 17: `ExtRequestPrebid` (full)

**Files:**

- Modify: `crates/auctioneer-openrtb/src/ext/request_prebid.rs`

Replace the placeholder. Most sub-types are typed; `multibid`, `data`, `bidderconfig` are typed pass-through per spec §15.

- [ ] **Step 1: Replace stub with full types + tests**

```rust
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ExtRequestPrebid {
    /// Request alias map: alias -> base bidder. Resolved in run_auction
    /// step 6 (spec §5.9).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub aliases: BTreeMap<String, String>,
    /// GVL vendor id overrides for aliases. Spec §5.9.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub aliasgvlids: BTreeMap<String, u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub targeting: Option<ExtRequestTargeting>,
    /// test=1 or ext.prebid.debug=1 contributes to debug_requested (spec §5.6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debug: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<Channel>,
    /// no-sale list for CCPA — `["*"]` = all bidders. Spec §7.5.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nosale: Vec<String>,
    /// request-level bidder params; merged into imp.ext.<bidder> per
    /// spec §5.1 step 6 (supplement-only, never creates participation).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bidderparams: BTreeMap<String, serde_json::Value>,
    /// Controls whether ext.prebid.seatnonbid is emitted. Spec Appendix E.6.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub returnallbidstatus: Option<bool>,
    /// multibid — typed pass-through; bid-limiting deferred (spec §15).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multibid: Option<serde_json::Value>,
    /// ext.prebid.data — typed pass-through; FPD resolution deferred (§15).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    /// ext.prebid.bidderconfig — typed pass-through (§15).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bidderconfig: Option<serde_json::Value>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ExtRequestTargeting {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pricegranularity: Option<serde_json::Value>,
    /// When ext.prebid.targeting is present, defaults to true (spec Appendix E.3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub includewinners: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub includebidderkeys: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub includeformat: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferdeals: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alwaysincludedeals: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Channel {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::round_trip;

    #[test]
    fn aliases_and_aliasgvlids_round_trip() {
        let json = r#"{"aliases":{"myalias":"appnexus"},"aliasgvlids":{"myalias":32}}"#;
        let e: ExtRequestPrebid = round_trip(json);
        assert_eq!(e.aliases.get("myalias").map(|s| s.as_str()), Some("appnexus"));
        assert_eq!(e.aliasgvlids.get("myalias"), Some(&32));
    }

    #[test]
    fn bidderparams_round_trip() {
        let json = r#"{"bidderparams":{"appnexus":{"placementId":"123"}}}"#;
        let e: ExtRequestPrebid = round_trip(json);
        assert!(e.bidderparams.contains_key("appnexus"));
    }

    #[test]
    fn channel_round_trips() {
        let json = r#"{"channel":{"name":"app","version":"1.0"}}"#;
        let e: ExtRequestPrebid = round_trip(json);
        assert_eq!(e.channel.unwrap().name.as_deref(), Some("app"));
    }

    #[test]
    fn passthrough_fields_round_trip() {
        let json = r#"{"multibid":[{"maxBids":2}],"data":{"x":1},"bidderconfig":[]}"#;
        let _: ExtRequestPrebid = round_trip(json);
    }
}
```

- [ ] **Step 2: Test + commit**

```
cargo test -p auctioneer-openrtb --lib ext::request_prebid::tests
git add crates/auctioneer-openrtb/src/ext/request_prebid.rs
git commit -m "auctioneer-openrtb: ExtRequestPrebid + sub-types (typed + pass-through)"
```

---

### Task 18: `ImpExt` (full) + `ExtImpPrebid`

**Files:**

- Modify: `crates/auctioneer-openrtb/src/ext/imp.rs`

- [ ] **Step 1: Replace stub with full types + tests**

```rust
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Imp.ext (the typed envelope around imp's ext object).
///
/// Three meaningful shapes live here:
///   - `tid` — transaction id; scrubbed when transmitTid is denied.
///   - `upstream` — ExtImpPrebid (bidder params + per-bidder FPD).
///   - dynamic legacy `imp.ext.<bidderName>` top-level keys — bidder params
///     in their legacy 2.5 location; supplement source for bidderparams
///     merge per spec §5.1 step 6.
///   - `bidder` — *written* by the exchange when hoisting (post step 6).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ImpExt {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prebid: Option<ExtImpPrebid>,
    /// Top-level imp.ext.bidder — populated by the exchange's hoisting step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bidder: Option<serde_json::Value>,
    /// All other top-level keys, including legacy imp.ext.<bidderName>.
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ExtImpPrebid {
    /// imp.ext.prebid.bidder — map of bidder name to per-imp params.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bidder: BTreeMap<String, serde_json::Value>,
    /// imp.ext.prebid.imp — bidder-specific FPD merged into the bidder-scoped Imp.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub imp: BTreeMap<String, serde_json::Value>,
    /// Stored-request id — parsed but stored-requests are out of phase 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storedrequest: Option<serde_json::Value>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::round_trip;

    #[test]
    fn imp_ext_with_prebid_bidder_and_legacy_bidder_round_trips() {
        let json = r#"{
            "tid":"t1",
            "prebid":{"bidder":{"appnexus":{"placementId":1}}},
            "appnexus":{"legacyParam":true}
        }"#;
        let e: ImpExt = round_trip(json);
        assert_eq!(e.tid.as_deref(), Some("t1"));
        assert!(e.prebid.as_ref().unwrap().bidder.contains_key("appnexus"));
        assert!(e.other.contains_key("appnexus"));
    }

    #[test]
    fn imp_ext_prebid_imp_fpd_round_trips() {
        let json = r#"{"prebid":{"imp":{"appnexus":{"banner":{"id":"X"}}}}}"#;
        let e: ImpExt = round_trip(json);
        assert!(e.prebid.unwrap().imp.contains_key("appnexus"));
    }
}
```

- [ ] **Step 2: Test + commit**

```
cargo test -p auctioneer-openrtb --lib ext::imp::tests
git add crates/auctioneer-openrtb/src/ext/imp.rs
git commit -m "auctioneer-openrtb: ImpExt + ExtImpPrebid (bidder params + FPD)"
```

---

### Task 19: `validate()` and `ValidationError`

**Files:**

- Modify: `crates/auctioneer-openrtb/src/validate.rs`

- [ ] **Step 1: Write tests + impl**

```rust
use crate::types::bid_request::BidRequest;
use std::collections::HashSet;
use thiserror::Error;

/// All structural validation failures. `Display` is the wire body content
/// after the upstream-compatible `Invalid request: ` prefix (Appendix F).
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("imp[] must contain at least one entry")]
    NoImps,
    #[error("imp[{0}] must have a unique id")]
    DuplicateImpId(String),
    #[error("imp[{0}] must declare at least one media type")]
    ImpHasNoMediaType(String),
    #[error("exactly one of site / app / dooh must be present")]
    InventoryShape,
    #[error("tmax must be >= 0")]
    NegativeTmax,
    #[error("request carries legacy 2.5 privacy field {0}; auctioneer is 2.6 only")]
    Legacy25Privacy(&'static str),
    #[error("channel must have a name")]
    EmptyChannelName,
}

impl BidRequest {
    /// Structural validation (spec §3 last bullet). Returns the first failure.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.imp.is_empty() {
            return Err(ValidationError::NoImps);
        }
        let mut seen = HashSet::with_capacity(self.imp.len());
        for imp in &self.imp {
            if !seen.insert(imp.id.clone()) {
                return Err(ValidationError::DuplicateImpId(imp.id.clone()));
            }
            if imp.banner.is_none() && imp.video.is_none()
               && imp.native.is_none() && imp.audio.is_none() {
                return Err(ValidationError::ImpHasNoMediaType(imp.id.clone()));
            }
        }
        let present = [self.site.is_some(), self.app.is_some(), self.dooh.is_some()];
        if present.iter().filter(|x| **x).count() != 1 {
            return Err(ValidationError::InventoryShape);
        }
        if let Some(t) = self.tmax {
            if t < 0 { return Err(ValidationError::NegativeTmax); }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::bid_request::{Banner, Imp};
    use crate::types::inventory::Site;

    fn ok_req() -> BidRequest {
        BidRequest {
            id: "r".into(),
            imp: vec![Imp {
                id: "i1".into(),
                banner: Some(Banner { w: Some(300), h: Some(250), ..Default::default() }),
                ..Default::default()
            }],
            site: Some(Site::default()),
            ..Default::default()
        }
    }

    #[test]
    fn valid_request_passes() { assert!(ok_req().validate().is_ok()); }

    #[test]
    fn no_imps_fails() {
        let mut r = ok_req();
        r.imp.clear();
        assert_eq!(r.validate(), Err(ValidationError::NoImps));
    }

    #[test]
    fn duplicate_imp_ids_fail() {
        let mut r = ok_req();
        r.imp.push(r.imp[0].clone());
        assert_eq!(r.validate(), Err(ValidationError::DuplicateImpId("i1".into())));
    }

    #[test]
    fn imp_without_media_type_fails() {
        let mut r = ok_req();
        r.imp[0].banner = None;
        assert!(matches!(r.validate(), Err(ValidationError::ImpHasNoMediaType(_))));
    }

    #[test]
    fn must_have_exactly_one_of_site_app_dooh() {
        // site + app -> fail
        let mut r = ok_req();
        r.app = Some(Default::default());
        assert_eq!(r.validate(), Err(ValidationError::InventoryShape));
        // none -> fail
        let mut r = ok_req(); r.site = None;
        assert_eq!(r.validate(), Err(ValidationError::InventoryShape));
        // dooh only -> ok
        let mut r = ok_req();
        r.site = None;
        r.dooh = Some(Default::default());
        assert!(r.validate().is_ok());
    }

    #[test]
    fn negative_tmax_fails() {
        let mut r = ok_req();
        r.tmax = Some(-1);
        assert_eq!(r.validate(), Err(ValidationError::NegativeTmax));
    }
}
```

- [ ] **Step 2: Test + commit**

```
cargo test -p auctioneer-openrtb --lib validate::tests
git add crates/auctioneer-openrtb/src/validate.rs
git commit -m "auctioneer-openrtb: validate() + ValidationError"
```

---

### Task 20: `RequestHeaders` struct

**Files:**

- Modify: `crates/auctioneer-openrtb/src/request_headers.rs`

- [ ] **Step 1: Write the struct + a tiny default test**

```rust
/// HTTP-header signals prepare() needs for implicit-field fill. Trusted-
/// server callers without an HTTP request set every field to None.
/// See spec §3.2.
#[derive(Clone, Copy, Debug, Default)]
pub struct RequestHeaders<'a> {
    pub referer:         Option<&'a str>,
    pub user_agent:      Option<&'a str>,
    pub dnt:             Option<&'a str>,
    pub sec_gpc:         Option<&'a str>,
    pub true_client_ip:  Option<&'a str>,
    pub x_forwarded_for: Option<&'a str>,
    pub x_real_ip:       Option<&'a str>,
    pub remote_addr:     Option<&'a str>,
    /// Content-Encoding is rejected by the caller before reaching prepare(),
    /// but trusted-server callers may pass it through for parity tests.
    pub content_encoding: Option<&'a str>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_all_none() {
        let h = RequestHeaders::default();
        assert!(h.referer.is_none());
        assert!(h.user_agent.is_none());
        assert!(h.remote_addr.is_none());
    }
}
```

- [ ] **Step 2: Test + commit**

```
cargo test -p auctioneer-openrtb --lib request_headers::tests
git add crates/auctioneer-openrtb/src/request_headers.rs
git commit -m "auctioneer-openrtb: RequestHeaders struct"
```

---

### Task 21: `NormalizedBidRequest` + `NonFatalWarning`

**Files:**

- Modify: `crates/auctioneer-openrtb/src/normalized.rs`

- [ ] **Step 1: Write types + tests**

```rust
use crate::types::bid_request::BidRequest;
use std::ops::Deref;

/// The single type-safe artifact produced by `prepare()` (spec §3.2 / §5.8).
/// Only `prepare()` can construct one — the inner field is private.
pub struct NormalizedBidRequest {
    pub(crate) inner:    BidRequest,
    pub(crate) warnings: Vec<NonFatalWarning>,
}

impl Deref for NormalizedBidRequest {
    type Target = BidRequest;
    fn deref(&self) -> &BidRequest { &self.inner }
}

impl NormalizedBidRequest {
    pub fn warnings(&self) -> &[NonFatalWarning] { &self.warnings }
    /// Crate-internal: hand the BidRequest off to run_auction once
    /// auctioneer-exchange depends on this crate.
    pub fn into_inner(self) -> BidRequest { self.inner }
}

/// Non-fatal warning surfaced under ext.warnings.prebid[] during response build.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NonFatalWarning {
    pub code:    WarningCode,
    pub message: String,
    /// Optional: which field triggered the warning, for debugging.
    pub scope:   Option<String>,
}

/// Numeric warning codes from upstream's reference numeric diagnostic codes. The
/// `InvalidPrivacyConsent` variant is defined here (since `NonFatalWarning`
/// is) but **not populated by `prepare()`** in M1 — consent-string parsing
/// lives in `auctioneer-privacy` from M3+ (spec round-37 H3 / §3.2 step 6 /
/// §7.7). `prepare()` emits no warnings in M1; the `warnings` field stays
/// empty and the run_auction surfacing path (§3.2 "Where warnings surface")
/// is already wired for when M3 starts emitting them via
/// `PrivacyContext::take_warnings()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WarningCode {
    /// 10001 — emitted by `auctioneer-privacy::PrivacyContext` from M3+, NOT by
    /// `auctioneer-openrtb::prepare()`. Kept here because `NonFatalWarning`
    /// lives in this crate.
    InvalidPrivacyConsent,
    /// 10999 — uncategorized
    Unknown,
}

impl WarningCode {
    pub fn as_u32(self) -> u32 {
        match self {
            WarningCode::InvalidPrivacyConsent => 10001,
            WarningCode::Unknown               => 10999,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warning_codes_match_prebid() {
        assert_eq!(WarningCode::InvalidPrivacyConsent.as_u32(), 10001);
        assert_eq!(WarningCode::Unknown.as_u32(), 10999);
    }

    #[test]
    fn warnings_default_to_empty() {
        let n = NormalizedBidRequest {
            inner: BidRequest::default(),
            warnings: Vec::new(),
        };
        assert!(n.warnings().is_empty());
    }
}
```

- [ ] **Step 2: Test + commit**

```
cargo test -p auctioneer-openrtb --lib normalized::tests
git add crates/auctioneer-openrtb/src/normalized.rs
git commit -m "auctioneer-openrtb: NormalizedBidRequest + NonFatalWarning + WarningCode"
```

---

### (Removed) Tasks 22-24: consent parsers — moved out of M1

Three tasks (US-Privacy 4-char parser, TCF v2 base64+version check, GPP
envelope parser) lived here in earlier revisions of this plan. They
were **removed when the spec moved consent parsing out of `auctioneer-
openrtb`** (round-37 H3 / spec rev 43 / §3.2 step 6 / §7.7).
Re-introducing them in `auctioneer-openrtb` would force a
`auctioneer-openrtb → auctioneer-privacy → auctioneer-openrtb` dependency cycle.
The parsers ship in `auctioneer-privacy` starting M3 (TCF v2) and M4
(US-Privacy, GPP) — see the M3/M4 plans when they're written.

`auctioneer-openrtb` retains the **warning surface** (`NonFatalWarning`,
`WarningCode::InvalidPrivacyConsent`) defined in Task 21, but
`prepare()` never populates `warnings` in M1; the field stays empty.

Task numbering below is post-deletion: old Task 25 → 22, 26 → 23,
27 → 24, 28 → 25, 29 → 26, 30 → 27.

---

### Task 22: `prepare()` — implicit-field fill (Part A: site default + IP sanitization + simple header fills)

**Files:**

- Create: `crates/auctioneer-openrtb/src/prepare/implicit_fill.rs`
- Modify: `crates/auctioneer-openrtb/src/prepare/mod.rs`

This task and the next two break implicit-fill into bite-sized chunks. Tests target the _behaviors_ spec §3.2 step 1 lists.

- [ ] **Step 1: Skeleton in `prepare/mod.rs`**

```rust
mod checks;
mod implicit_fill;

use crate::normalized::NormalizedBidRequest;
use crate::request_headers::RequestHeaders;
use crate::types::bid_request::BidRequest;
use crate::validate::ValidationError;

/// Single reusable preparation gate (spec §3.2). Implicit fill +
/// validate + 2.6 privacy gate + channel reject.
///
/// **No consent-string parsing here** (round-37 H3 / §3.2 step 6 /
/// §7.7) — consent warnings are emitted by `PrivacyContext::resolve`
/// in `auctioneer-privacy` from M3+. `prepare()` always returns a
/// `NormalizedBidRequest` with an empty `warnings` vec in M1; the
/// field is wired up so the run_auction surfacing path is already
/// ready when M3 starts populating it.
pub fn prepare(
    mut req: BidRequest,
    headers: &RequestHeaders<'_>,
) -> Result<NormalizedBidRequest, ValidationError> {
    implicit_fill::run(&mut req, headers);
    req.validate()?;
    checks::reject_legacy_25_privacy(&req)?;
    checks::reject_empty_channel_name(&req)?;
    Ok(NormalizedBidRequest { inner: req, warnings: Vec::new() })
}
```

- [ ] **Step 2: `implicit_fill.rs` — site default + IP sanitize + UA/DNT/`at`**

```rust
use crate::request_headers::RequestHeaders;
use crate::types::bid_request::BidRequest;
use crate::types::device::Device;
use crate::types::inventory::{Publisher, Site};

pub(super) fn run(req: &mut BidRequest, headers: &RequestHeaders<'_>) {
    site_default(req, headers);
    fill_device(req, headers);
    fill_at(req);
    fill_channel(req);
    ios_lmt_override(req);
    gpc(req, headers);
}

/// spec §3.2 step 1 — Site default. When app == nil && dooh == nil we
/// always materialize an empty Site; if referer is set we enrich.
fn site_default(req: &mut BidRequest, headers: &RequestHeaders<'_>) {
    if req.app.is_some() || req.dooh.is_some() { return; }
    let site = req.site.get_or_insert_with(Site::default);
    if let Some(referer) = headers.referer.filter(|s| !s.is_empty()) {
        if site.page.is_none() {
            site.page = Some(referer.to_string());
        }
    }
    // site.domain / publisher.domain enrichment uses public-suffix.
    // Source is site.page (which may have been set above) — best-effort.
    let source = site.page.clone();
    if let Some(page) = source {
        if let Ok(parsed) = url::Url::parse(&page) {
            if let Some(host) = parsed.host_str() {
                if site.domain.is_none() {
                    site.domain = Some(host.to_string());
                }
                let publisher = site.publisher.get_or_insert_with(Publisher::default);
                if publisher.domain.is_none() {
                    if let Some(etld_one) = etld_plus_one(host) {
                        publisher.domain = Some(etld_one);
                    }
                }
            }
        }
    }
    if site.ext.is_none() {
        site.ext = Some(crate::ext::misc::SiteExt {
            amp: Some(0),
            ..Default::default()
        });
    } else if let Some(ext) = site.ext.as_mut() {
        if ext.amp.is_none() { ext.amp = Some(0); }
    }
}

fn etld_plus_one(host: &str) -> Option<String> {
    use publicsuffix::{List, Psl};
    // Embed the public-suffix list at compile time via the publicsuffix
    // crate default (it reads from data shipped with the crate).
    let list = List::default();
    list.suffix(host.as_bytes())
        .and_then(|s| s.parent())
        .and_then(|p| std::str::from_utf8(p.as_bytes()).ok().map(|s| s.to_string()))
}

/// spec §3.2 step 1 — Device fills. IP sanitize-then-fill; UA / DNT.
fn fill_device(req: &mut BidRequest, headers: &RequestHeaders<'_>) {
    let device = req.device.get_or_insert_with(Device::default);

    // Sanitize: clear unparseable / private IPs even if the client set them.
    sanitize_ip(&mut device.ip);
    sanitize_ip(&mut device.ipv6);

    // Fill from headers if still empty.
    if device.ip.is_none() && device.ipv6.is_none() {
        if let Some(picked) = pick_valid_ip(headers) {
            // route v4 vs v6 by the chosen IP's family.
            if picked.contains(':') {
                device.ipv6 = Some(picked);
            } else {
                device.ip = Some(picked);
            }
        }
    }

    if device.ua.is_none() {
        if let Some(ua) = headers.user_agent.filter(|s| !s.is_empty()) {
            device.ua = Some(ua.to_string());
        }
    }
    if device.dnt.is_none() {
        if let Some(d) = headers.dnt {
            if d == "0" { device.dnt = Some(0); }
            else if d == "1" { device.dnt = Some(1); }
        }
    }
}

fn sanitize_ip(field: &mut Option<String>) {
    if let Some(s) = field.as_deref() {
        if !is_valid_public_ip(s) {
            *field = None;
        }
    }
}

fn pick_valid_ip(headers: &RequestHeaders<'_>) -> Option<String> {
    // Order: True-Client-IP -> XFF hops (left-to-right) -> X-Real-IP -> RemoteAddr.
    if let Some(s) = headers.true_client_ip.filter(|s| is_valid_public_ip(s)) {
        return Some(s.to_string());
    }
    if let Some(xff) = headers.x_forwarded_for {
        for hop in xff.split(',').map(str::trim) {
            if is_valid_public_ip(hop) { return Some(hop.to_string()); }
        }
    }
    if let Some(s) = headers.x_real_ip.filter(|s| is_valid_public_ip(s)) {
        return Some(s.to_string());
    }
    if let Some(s) = headers.remote_addr.filter(|s| is_valid_public_ip(s)) {
        return Some(s.to_string());
    }
    None
}

fn is_valid_public_ip(s: &str) -> bool {
    use std::net::IpAddr;
    use ipnetwork::IpNetwork;
    let Ok(addr) = s.parse::<IpAddr>() else { return false; };
    // Reject private / loopback / link-local / unspecified / multicast / broadcast.
    if addr.is_loopback() || addr.is_unspecified() || addr.is_multicast() {
        return false;
    }
    match addr {
        IpAddr::V4(v4) => {
            if v4.is_private() || v4.is_link_local() || v4.is_broadcast() {
                return false;
            }
        }
        IpAddr::V6(v6) => {
            // Best-effort private/link-local check via ipnetwork.
            let net = IpNetwork::new(IpAddr::V6(v6), 128).ok();
            if net.is_some() && (
                IpNetwork::new("fc00::".parse().unwrap(), 7).unwrap().contains(IpAddr::V6(v6)) ||
                IpNetwork::new("fe80::".parse().unwrap(), 10).unwrap().contains(IpAddr::V6(v6))
            ) {
                return false;
            }
        }
    }
    true
}

/// spec §3.2 step 1 — `at` defaults to 1 (first-price) when absent OR 0.
fn fill_at(req: &mut BidRequest) {
    match req.at {
        None | Some(0) => req.at = Some(1),
        _ => {}
    }
}

/// spec §3.2 step 1 — channel default. Only when app is set AND
/// ext.prebid.channel is absent. Rejection of empty `name` happens in
/// checks::reject_empty_channel_name (not here).
fn fill_channel(req: &mut BidRequest) {
    if req.app.is_none() { return; }
    let ext = req.ext.get_or_insert_with(Default::default);
    let prebid = ext.prebid.get_or_insert_with(Default::default);
    if prebid.channel.is_none() {
        prebid.channel = Some(crate::ext::request_prebid::Channel {
            name: Some("app".to_string()),
            ..Default::default()
        });
    }
}

/// spec §3.2 step 1 — iOS LMT override based on device.ext.atts that is
/// already on the request (read-only — auctioneer does not derive atts).
/// Rule: app + iOS, atts==3 (Authorized) -> lmt=0; atts in 0/1/2 -> lmt=1.
fn ios_lmt_override(req: &mut BidRequest) {
    let Some(app) = &req.app else { return; };
    let _ = app; // shape check only; atts must be set
    let Some(device) = req.device.as_mut() else { return; };
    let Some(os) = device.os.as_deref() else { return; };
    if !os.eq_ignore_ascii_case("ios") { return; }
    let atts = device.ext.as_ref().and_then(|e| e.atts);
    if let Some(atts) = atts {
        device.lmt = Some(match atts {
            3 => 0,         // Authorized
            0 | 1 | 2 => 1, // NotDetermined / Restricted / Denied
            _ => return,    // unknown ATTS code — leave lmt alone
        });
    }
}

/// spec §3.2 step 1 — Sec-GPC: 1 ALWAYS overwrites regs.ext.gpc to "1"
/// (round-31 H1 — header signal cannot be silently suppressed). Other
/// header values are ignored; a client-supplied JSON value is left alone.
fn gpc(req: &mut BidRequest, headers: &RequestHeaders<'_>) {
    if headers.sec_gpc == Some("1") {
        let regs = req.regs.get_or_insert_with(Default::default);
        let ext = regs.ext.get_or_insert_with(Default::default);
        ext.gpc = Some("1".to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::bid_request::{Banner, Imp};

    fn base_req() -> BidRequest {
        BidRequest {
            id: "r".into(),
            imp: vec![Imp { id: "i1".into(), banner: Some(Banner::default()), ..Default::default() }],
            ..Default::default()
        }
    }

    #[test]
    fn site_materialized_even_without_referer() {
        let mut r = base_req();
        let h = RequestHeaders::default();
        run(&mut r, &h);
        assert!(r.site.is_some());
        assert_eq!(r.site.unwrap().ext.unwrap().amp, Some(0));
    }

    #[test]
    fn site_not_materialized_when_app_present() {
        let mut r = base_req();
        r.app = Some(Default::default());
        run(&mut r, &RequestHeaders::default());
        assert!(r.site.is_none());
    }

    #[test]
    fn private_ip_in_request_is_cleared() {
        let mut r = base_req();
        r.device = Some(Device { ip: Some("10.0.0.1".into()), ..Default::default() });
        run(&mut r, &RequestHeaders::default());
        assert!(r.device.unwrap().ip.is_none());
    }

    #[test]
    fn public_ip_from_xff_left_to_right() {
        let mut r = base_req();
        let h = RequestHeaders {
            x_forwarded_for: Some("10.0.0.1, 8.8.8.8, 1.1.1.1"),
            ..Default::default()
        };
        run(&mut r, &h);
        assert_eq!(r.device.unwrap().ip.as_deref(), Some("8.8.8.8"));
    }

    #[test]
    fn ua_dnt_filled_only_when_empty() {
        let mut r = base_req();
        let h = RequestHeaders { user_agent: Some("Mozilla"), dnt: Some("1"), ..Default::default() };
        run(&mut r, &h);
        let device = r.device.unwrap();
        assert_eq!(device.ua.as_deref(), Some("Mozilla"));
        assert_eq!(device.dnt, Some(1));
    }

    #[test]
    fn at_defaults_when_absent_or_zero() {
        let mut r = base_req();
        run(&mut r, &RequestHeaders::default());
        assert_eq!(r.at, Some(1));

        let mut r2 = base_req();
        r2.at = Some(0);
        run(&mut r2, &RequestHeaders::default());
        assert_eq!(r2.at, Some(1));

        let mut r3 = base_req();
        r3.at = Some(2);
        run(&mut r3, &RequestHeaders::default());
        assert_eq!(r3.at, Some(2)); // not changed
    }

    #[test]
    fn channel_app_default_for_app_requests() {
        let mut r = base_req();
        r.site = None;
        r.app = Some(Default::default());
        run(&mut r, &RequestHeaders::default());
        let name = r.ext.unwrap().prebid.unwrap().channel.unwrap().name;
        assert_eq!(name.as_deref(), Some("app"));
    }

    #[test]
    fn sec_gpc_one_overwrites_body_zero() {
        let mut r = base_req();
        r.regs = Some(crate::types::regs::Regs {
            ext: Some(crate::ext::misc::RegsExt { gpc: Some("0".into()), ..Default::default() }),
            ..Default::default()
        });
        let h = RequestHeaders { sec_gpc: Some("1"), ..Default::default() };
        run(&mut r, &h);
        assert_eq!(r.regs.unwrap().ext.unwrap().gpc.as_deref(), Some("1"));
    }

    #[test]
    fn ios_lmt_override_authorized_sets_zero() {
        let mut r = base_req();
        r.site = None;
        r.app = Some(Default::default());
        r.device = Some(Device {
            os: Some("iOS".into()),
            ext: Some(crate::ext::misc::DeviceExt { atts: Some(3), ..Default::default() }),
            ..Default::default()
        });
        run(&mut r, &RequestHeaders::default());
        assert_eq!(r.device.unwrap().lmt, Some(0));
    }

    #[test]
    fn ios_lmt_override_restricted_sets_one() {
        let mut r = base_req();
        r.site = None;
        r.app = Some(Default::default());
        r.device = Some(Device {
            os: Some("ios".into()),
            ext: Some(crate::ext::misc::DeviceExt { atts: Some(2), ..Default::default() }),
            ..Default::default()
        });
        run(&mut r, &RequestHeaders::default());
        assert_eq!(r.device.unwrap().lmt, Some(1));
    }
}
```

- [ ] **Step 3: Test**

```
cargo test -p auctioneer-openrtb --lib prepare::implicit_fill::tests
```

Expected: all 9 passed.

- [ ] **Step 4: Commit**

```
git add crates/auctioneer-openrtb/src/prepare
git commit -m "auctioneer-openrtb: prepare() implicit-field fill (site/IP/UA/DNT/at/channel/GPC/iOS LMT)"
```

---

### Task 23: `prepare()` — checks (legacy privacy reject, channel empty-name reject)

**Files:**

- Create: `crates/auctioneer-openrtb/src/prepare/checks.rs`

**No consent-string parsing in this task** (round-37 H3 / §3.2 step 6 /
§7.7). Consent warnings live in `auctioneer-privacy` from M3+; M1's
`prepare()` only does the legacy 2.5 privacy rejection and the
channel-empty-name rejection.

- [ ] **Step 1: Write impl + tests**

```rust
use crate::types::bid_request::BidRequest;
use crate::validate::ValidationError;

/// spec §3.2 step 2: reject if any legacy 2.5 privacy field is present.
/// The fields live under regs.ext / user.ext as raw JSON (we never typed
/// them under those locations); detect by scanning the `other` map.
pub(super) fn reject_legacy_25_privacy(req: &BidRequest) -> Result<(), ValidationError> {
    if let Some(regs) = &req.regs {
        if let Some(ext) = &regs.ext {
            if ext.other.contains_key("gdpr") {
                return Err(ValidationError::Legacy25Privacy("regs.ext.gdpr"));
            }
            if ext.other.contains_key("us_privacy") {
                return Err(ValidationError::Legacy25Privacy("regs.ext.us_privacy"));
            }
        }
    }
    if let Some(user) = &req.user {
        if let Some(ext) = &user.ext {
            if ext.other.contains_key("consent") {
                return Err(ValidationError::Legacy25Privacy("user.ext.consent"));
            }
        }
    }
    Ok(())
}

/// spec §3.2 step 4: if ext.prebid.channel is present but channel.name is
/// explicitly empty (""), reject. (Absent -> filled with "app" for app
/// requests in implicit_fill::fill_channel.)
pub(super) fn reject_empty_channel_name(req: &BidRequest) -> Result<(), ValidationError> {
    let Some(ext) = &req.ext else { return Ok(()); };
    let Some(prebid) = &ext.prebid else { return Ok(()); };
    let Some(channel) = &prebid.channel else { return Ok(()); };
    if let Some(name) = &channel.name {
        if name.is_empty() {
            return Err(ValidationError::EmptyChannelName);
        }
    } else {
        // explicit null name was supplied with the channel object
        return Err(ValidationError::EmptyChannelName);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::bid_request::{Banner, Imp};
    use crate::types::regs::Regs;
    use crate::types::user::User;

    fn req_with_imp() -> BidRequest {
        BidRequest {
            id: "r".into(),
            imp: vec![Imp { id: "i".into(), banner: Some(Banner::default()), ..Default::default() }],
            ..Default::default()
        }
    }

    #[test]
    fn legacy_regs_ext_gdpr_rejected() {
        let mut r = req_with_imp();
        let mut ext = crate::ext::misc::RegsExt::default();
        ext.other.insert("gdpr".into(), serde_json::json!(1));
        r.regs = Some(Regs { ext: Some(ext), ..Default::default() });
        assert_eq!(reject_legacy_25_privacy(&r), Err(ValidationError::Legacy25Privacy("regs.ext.gdpr")));
    }

    #[test]
    fn legacy_user_ext_consent_rejected() {
        let mut r = req_with_imp();
        let mut ext = crate::ext::misc::UserExt::default();
        ext.other.insert("consent".into(), serde_json::json!("XYZ"));
        r.user = Some(User { ext: Some(ext), ..Default::default() });
        assert_eq!(reject_legacy_25_privacy(&r), Err(ValidationError::Legacy25Privacy("user.ext.consent")));
    }

    #[test]
    fn explicit_empty_channel_name_rejected() {
        let mut r = req_with_imp();
        let mut prebid = crate::ext::request_prebid::ExtRequestPrebid::default();
        prebid.channel = Some(crate::ext::request_prebid::Channel {
            name: Some(String::new()), ..Default::default()
        });
        r.ext = Some(crate::types::bid_request::BidRequestExt {
            prebid: Some(prebid), other: Default::default(),
        });
        assert_eq!(reject_empty_channel_name(&r), Err(ValidationError::EmptyChannelName));
    }

    #[test]
    fn missing_channel_name_field_rejected() {
        let mut r = req_with_imp();
        let mut prebid = crate::ext::request_prebid::ExtRequestPrebid::default();
        prebid.channel = Some(crate::ext::request_prebid::Channel { name: None, ..Default::default() });
        r.ext = Some(crate::types::bid_request::BidRequestExt {
            prebid: Some(prebid), other: Default::default(),
        });
        assert_eq!(reject_empty_channel_name(&r), Err(ValidationError::EmptyChannelName));
    }
}
```

- [ ] **Step 2: Test**

```
cargo test -p auctioneer-openrtb --lib prepare::checks::tests
```

Expected: 4 passed.

- [ ] **Step 3: Commit**

```
git add crates/auctioneer-openrtb/src/prepare/checks.rs
git commit -m "auctioneer-openrtb: prepare() checks (legacy 2.5 reject, channel empty)"
```

---

### Task 24: End-to-end `prepare()` tests + fixture round-trips

**Files:**

- Create: `crates/auctioneer-openrtb/tests/prepare_integration.rs`

These are integration tests in the `tests/` directory — they exercise the public `prepare` API only.

- [ ] **Step 1: Write integration tests**

```rust
use auctioneer_openrtb::{prepare, NormalizedBidRequest, RequestHeaders, ValidationError};

fn parse(json: &str) -> auctioneer_openrtb::types::bid_request::BidRequest {
    serde_json::from_str(json).expect("fixture must parse")
}

#[test]
fn prepare_succeeds_on_minimal_site_request() {
    let req = parse(r#"{"id":"r","imp":[{"id":"i","banner":{"w":300,"h":250}}],"site":{}}"#);
    let n = prepare(req, &RequestHeaders::default()).unwrap();
    assert_eq!(n.id, "r");
    assert_eq!(n.at, Some(1));               // defaulted
    assert!(n.site.as_ref().unwrap().ext.as_ref().unwrap().amp == Some(0));
    assert!(n.warnings().is_empty());
}

#[test]
fn prepare_rejects_legacy_25_privacy() {
    let req = parse(r#"{
        "id":"r","imp":[{"id":"i","banner":{}}],
        "site":{},"regs":{"ext":{"gdpr":1}}
    }"#);
    let err = prepare(req, &RequestHeaders::default()).unwrap_err();
    assert!(matches!(err, ValidationError::Legacy25Privacy("regs.ext.gdpr")));
}

#[test]
fn prepare_rejects_empty_channel_name() {
    let req = parse(r#"{
        "id":"r","imp":[{"id":"i","banner":{}}],
        "site":{},"ext":{"prebid":{"channel":{"name":""}}}
    }"#);
    let err = prepare(req, &RequestHeaders::default()).unwrap_err();
    assert!(matches!(err, ValidationError::EmptyChannelName));
}

#[test]
fn prepare_defaults_channel_for_app() {
    let req = parse(r#"{"id":"r","imp":[{"id":"i","banner":{}}],"app":{}}"#);
    let n = prepare(req, &RequestHeaders::default()).unwrap();
    let name = n.ext.as_ref().unwrap().prebid.as_ref().unwrap()
        .channel.as_ref().unwrap().name.as_deref();
    assert_eq!(name, Some("app"));
}

#[test]
fn prepare_overrides_gpc_from_header_even_with_zero_body() {
    let req = parse(r#"{
        "id":"r","imp":[{"id":"i","banner":{}}],"site":{},
        "regs":{"ext":{"gpc":"0"}}
    }"#);
    let h = RequestHeaders { sec_gpc: Some("1"), ..Default::default() };
    let n = prepare(req, &h).unwrap();
    let gpc = n.regs.as_ref().unwrap().ext.as_ref().unwrap().gpc.as_deref();
    assert_eq!(gpc, Some("1"));
}

#[test]
fn prepare_does_not_emit_consent_warnings_in_m1() {
    // Round-37 H3 / §3.2 step 6 / §7.7 — consent parsers moved to
    // auctioneer-privacy. prepare() never populates warnings in M1, even
    // for a malformed us_privacy that would later become an
    // InvalidPrivacyConsent warning emitted by PrivacyContext::resolve.
    let req = parse(r#"{
        "id":"r","imp":[{"id":"i","banner":{}}],"site":{},
        "regs":{"us_privacy":"NOPE"}
    }"#);
    let n: NormalizedBidRequest = prepare(req, &RequestHeaders::default()).unwrap();
    assert!(n.warnings().is_empty());
}

#[test]
fn prepare_does_not_default_cur() {
    // Spec §3.2 step 5 — no cur defaulting.
    let req = parse(r#"{"id":"r","imp":[{"id":"i","banner":{}}],"site":{}}"#);
    let n = prepare(req, &RequestHeaders::default()).unwrap();
    assert!(n.cur.is_empty());
}
```

- [ ] **Step 2: Test**

```
cargo test -p auctioneer-openrtb --test prepare_integration
```

Expected: 7 passed.

- [ ] **Step 3: Commit**

```
git add crates/auctioneer-openrtb/tests/prepare_integration.rs
git commit -m "auctioneer-openrtb: prepare() integration tests"
```

---

### Task 25: Round-trip every upstream sample-requests fixture

**Files:**

- Create: `crates/auctioneer-openrtb/tests/sample_requests_round_trip.rs`

This is the heavy fidelity check — every JSON in `prebid-server/endpoints/openrtb2/sample-requests/valid-whole/` must parse and re-serialize losslessly.

- [ ] **Step 1: Write the integration test**

```rust
//! Round-trips every upstream sample request fixture through the auctioneer types.
//! See spec §11.1 "OpenRTB types" bullet.

use serde_json::Value;
use std::fs;
use std::path::PathBuf;

fn fixtures_root() -> PathBuf {
    let root = std::env::var_os("PREBID_SERVER_DIR")
        .expect("PREBID_SERVER_DIR must point to the pinned reference checkout");
    PathBuf::from(root).join("endpoints/openrtb2/sample-requests/valid-whole")
}

#[test]
fn every_valid_whole_fixture_round_trips() {
    let root = fixtures_root();
    let mut count = 0usize;
    for entry in fs::read_dir(root).expect("fixtures dir must exist") {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") { continue; }
        let raw = fs::read_to_string(&path).unwrap();

        // The upstream fixtures wrap the BidRequest in a test envelope.
        // Most have a top-level "mockBidRequest" or are themselves the
        // BidRequest; try the wrapped form first, fall back to direct.
        let envelope: Value = serde_json::from_str(&raw)
            .unwrap_or_else(|e| panic!("invalid json in {path:?}: {e}"));
        let bid_request_json = envelope
            .get("mockBidRequest")
            .cloned()
            .unwrap_or(envelope);

        let parsed: auctioneer_openrtb::types::bid_request::BidRequest =
            serde_json::from_value(bid_request_json.clone())
                .unwrap_or_else(|e| panic!("failed to deserialize {path:?}: {e}"));
        let back: Value = serde_json::to_value(&parsed)
            .unwrap_or_else(|e| panic!("failed to serialize from {path:?}: {e}"));

        // Compare as Value — order-insensitive, whitespace-insensitive.
        // Allow drift on fields we intentionally do NOT round-trip yet
        // (e.g. inside passthrough RawValue ext — those go through bytes).
        assert_eq!(back, bid_request_json,
            "round-trip mismatch in {path:?}");
        count += 1;
    }
    assert!(count > 50, "expected to find > 50 valid-whole fixtures, found {count}");
}
```

If a specific fixture fails because it uses a 2.5-only field (e.g. `regs.ext.gdpr`), that's expected — auctioneer is 2.6 only. The fix is either:

- Skip 2.5-only fixtures by name (allow-list), or
- Document the divergence and exclude them via a list.

Adopt skipping by filename for fixtures known to be 2.5-style; do **not** weaken the round-trip check itself.

- [ ] **Step 2: Run, capture failures, refine skip-list**

Run:

```
cargo test -p auctioneer-openrtb --test sample_requests_round_trip -- --nocapture
```

If failures occur, inspect each failed filename and decide: (a) is this a 2.5-only fixture we should skip, or (b) is this a real auctioneer types gap we should fix? For (a), maintain an explicit `SKIP: &[&str] = &["fixture-name.json", ...]` and skip them. For (b), open a follow-up task to add the missing field to the types.

- [ ] **Step 3: Commit (after pass)**

```
git add crates/auctioneer-openrtb/tests/sample_requests_round_trip.rs
git commit -m "auctioneer-openrtb: round-trip every prebid valid-whole fixture"
```

---

### Task 26: Create `auctioneer-config` crate skeleton + data files

**Files:**

- Create: `crates/auctioneer-config/Cargo.toml`
- Create: `crates/auctioneer-config/src/lib.rs`
- Create: `crates/auctioneer-config/src/types.rs`
- Create: `crates/auctioneer-config/build.rs`
- Create: `crates/auctioneer-config/data/host-defaults.yaml`
- Create: `crates/auctioneer-config/data/account-defaults.yaml`
- Create: `crates/auctioneer-config/data/eea-countries.txt`
- Create: `crates/auctioneer-config/data/gvl.json`
- Create: `crates/auctioneer-config/data/bidder-info/{generic,appnexus,pubmatic,mocktioneer}.yaml`
- Modify: root `Cargo.toml` (`members`)

- [ ] **Step 1: Add workspace member + deps**

In root `Cargo.toml` `members` add `"crates/auctioneer-config"`. In `[workspace.dependencies]` add:

```toml
serde_yaml = "0.9"
```

- [ ] **Step 2: Create `crates/auctioneer-config/Cargo.toml`**

```toml
[package]
name = "auctioneer-config"
version = "0.1.0"
edition = "2021"
publish = false

[lints]
workspace = true

[dependencies]
serde      = { workspace = true }
serde_json = { workspace = true }

[build-dependencies]
serde      = { workspace = true }
serde_json = { workspace = true }
serde_yaml = { workspace = true }
```

- [ ] **Step 3: Vendor bidder-info YAMLs from reference server**

For each of `generic`, `appnexus`, `pubmatic`, `mocktioneer`, find the source YAML in `$PREBID_SERVER_DIR/static/bidder-info/<name>.yaml` and copy it into `crates/auctioneer-config/data/bidder-info/<name>.yaml`. If a bidder doesn't exist in upstream (`generic`, `mocktioneer`), hand-write a minimal one:

`data/bidder-info/generic.yaml`:

```yaml
maintainer:
  email: maintainer@auctioneer.example
capabilities:
  app:
    mediaTypes: [banner, video, native]
  site:
    mediaTypes: [banner, video, native]
endpoint: https://generic.example/openrtb2/auction
```

`data/bidder-info/mocktioneer.yaml`:

```yaml
maintainer:
  email: maintainer@auctioneer.example
capabilities:
  app:
    mediaTypes: [banner]
  site:
    mediaTypes: [banner]
endpoint: http://127.0.0.1:8787/openrtb2/auction
```

(The Mocktioneer endpoint is `http://` for local testing — it requires the `insecure-bidders` cargo feature in §5.4 to actually be called. For codegen we just store the value.)

- [ ] **Step 4: Vendor host/account/EEA/GVL data**

`data/host-defaults.yaml`:

```yaml
gdpr:
  enabled: true
  default_value: "0"
  tcf2:
    enabled: true
ccpa:
  enforce: false
lmt:
  enforce: false
host_cookie:
  ttl_days: 90
user_sync:
  cooperative:
    enabled_by_default: false
```

`data/account-defaults.yaml`:

```yaml
disabled: false
debug_allow: false
bid_rounding: down
```

`data/eea-countries.txt`:

```
AUT
BEL
BGR
HRV
CYP
CZE
DNK
EST
FIN
FRA
DEU
GRC
HUN
IRL
ITA
LVA
LTU
LUX
MLT
NLD
POL
PRT
ROU
SVK
SVN
ESP
SWE
ISL
LIE
NOR
```

`data/gvl.json` — a minimal snapshot referring to the IAB GVL. For phase 1, embed a small map of vendor id → vendor metadata sufficient for our 4 bidders (GVL ids for AppNexus = 32, Pubmatic = 76, Mocktioneer = none, generic = none — adjust to match the YAMLs you vendored):

```json
{
  "vendorListVersion": 1,
  "specVersion": 3,
  "vendors": {
    "32": { "id": 32, "name": "AppNexus" },
    "76": { "id": 76, "name": "PubMatic" }
  }
}
```

- [ ] **Step 5: Write `src/types.rs`**

```rust
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HostConfig {
    pub gdpr: GdprConfig,
    pub ccpa: CcpaConfig,
    pub lmt:  LmtConfig,
    pub host_cookie: HostCookieConfig,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GdprConfig {
    pub enabled: bool,
    pub default_value: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CcpaConfig { pub enforce: bool }

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LmtConfig { pub enforce: bool }

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HostCookieConfig { pub ttl_days: i64 }

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AccountConfig {
    pub disabled: bool,
    pub debug_allow: bool,
    pub bid_rounding: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BidderInfo {
    pub name: String,
    pub endpoint: String,
    pub uses_https: bool,
    pub disabled: bool,
    pub debug_allow: bool,
    pub endpoint_compression: Option<Compression>,
    pub gvl_vendor_id: Option<u32>,
    pub host_allowlist: Option<Vec<String>>,
    pub maintainer_email: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Compression { Gzip }
```

- [ ] **Step 6: Stub `src/lib.rs` and `build.rs`**

`src/lib.rs`:

```rust
pub mod types;

// Re-exports populated by codegen.
include!(concat!(env!("OUT_DIR"), "/generated.rs"));
```

`build.rs` (single-file codegen — keeps the plan task atomic):

```rust
//! build.rs — reads data/* and emits OUT_DIR/generated.rs.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct BidderInfoYaml {
    #[serde(default)]
    maintainer: Option<MaintainerYaml>,
    endpoint: String,
    #[serde(default)]
    disabled: Option<bool>,
    #[serde(default, rename = "endpointCompression")]
    endpoint_compression: Option<String>,
    #[serde(default)]
    gvl_vendor_id: Option<u32>,
    #[serde(default)]
    debug: Option<DebugYaml>,
}

#[derive(Debug, Deserialize)]
struct MaintainerYaml {
    #[serde(default)]
    email: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DebugYaml { #[serde(default)] allow: Option<bool> }

fn main() {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let out_path = Path::new(&out_dir).join("generated.rs");

    let mut out = String::new();
    emit_bidders(&mut out);
    emit_host_default(&mut out);
    emit_account_default(&mut out);
    emit_eea_set(&mut out);
    emit_gvl(&mut out);

    fs::write(&out_path, out).expect("write generated.rs");

    println!("cargo:rerun-if-changed=data");
}

fn emit_bidders(out: &mut String) {
    let dir = Path::new("data/bidder-info");
    let mut bidders: Vec<(String, BidderInfoYaml)> = Vec::new();
    for entry in fs::read_dir(dir).expect("data/bidder-info exists") {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("yaml") { continue; }
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        let yaml = fs::read_to_string(&path).expect("read yaml");
        let parsed: BidderInfoYaml =
            serde_yaml::from_str(&yaml).expect(&format!("parse {path:?}"));
        bidders.push((name, parsed));
        println!("cargo:rerun-if-changed={}", path.display());
    }
    bidders.sort_by(|a, b| a.0.cmp(&b.0));

    out.push_str("use crate::types::{BidderInfo, Compression};\n\n");
    out.push_str("pub fn bidders() -> &'static [BidderInfo] {\n");
    out.push_str("    static BIDDERS: &[BidderInfo] = &[\n");
    for (name, b) in &bidders {
        let compression = match b.endpoint_compression.as_deref() {
            Some("gzip") => "Some(Compression::Gzip)",
            _            => "None",
        };
        let uses_https = b.endpoint.starts_with("https://");
        let disabled = b.disabled.unwrap_or(false);
        let debug_allow = b.debug.as_ref().and_then(|d| d.allow).unwrap_or(true);
        let gvl = match b.gvl_vendor_id {
            Some(id) => format!("Some({id})"),
            None     => "None".to_string(),
        };
        let email = b.maintainer.as_ref().and_then(|m| m.email.as_deref()).unwrap_or("");
        out.push_str(&format!(r#"        BidderInfo {{
            name: "{name}".to_string(),
            endpoint: "{ep}".to_string(),
            uses_https: {uses_https},
            disabled: {disabled},
            debug_allow: {debug_allow},
            endpoint_compression: {compression},
            gvl_vendor_id: {gvl},
            host_allowlist: None,
            maintainer_email: Some("{email}".to_string()),
        }},
"#, ep = b.endpoint));
    }
    out.push_str("    ];\n");
    out.push_str("    // SAFETY: BidderInfo Default::default fields are String — we own them.\n");
    // The slice has owned Strings, which can't be in a static. Wrap in OnceLock.
    out.clear();
    out.push_str("use crate::types::{BidderInfo, Compression};\n");
    out.push_str("use std::sync::OnceLock;\n\n");
    out.push_str("pub fn bidders() -> &'static [BidderInfo] {\n");
    out.push_str("    static CELL: OnceLock<Vec<BidderInfo>> = OnceLock::new();\n");
    out.push_str("    CELL.get_or_init(|| vec![\n");
    for (name, b) in &bidders {
        let compression = match b.endpoint_compression.as_deref() {
            Some("gzip") => "Some(Compression::Gzip)",
            _            => "None",
        };
        let uses_https = b.endpoint.starts_with("https://");
        let disabled = b.disabled.unwrap_or(false);
        let debug_allow = b.debug.as_ref().and_then(|d| d.allow).unwrap_or(true);
        let gvl = match b.gvl_vendor_id {
            Some(id) => format!("Some({id})"),
            None     => "None".to_string(),
        };
        let email = b.maintainer.as_ref().and_then(|m| m.email.as_deref()).unwrap_or("");
        out.push_str(&format!(r#"        BidderInfo {{
            name: "{name}".to_string(),
            endpoint: "{ep}".to_string(),
            uses_https: {uses_https},
            disabled: {disabled},
            debug_allow: {debug_allow},
            endpoint_compression: {compression},
            gvl_vendor_id: {gvl},
            host_allowlist: None,
            maintainer_email: Some("{email}".to_string()),
        }},
"#, ep = b.endpoint));
    }
    out.push_str("    ])\n");
    out.push_str("}\n\n");
    out.push_str("pub fn bidder(name: &str) -> Option<&'static BidderInfo> {\n");
    out.push_str("    bidders().iter().find(|b| b.name == name)\n");
    out.push_str("}\n\n");
}

fn emit_host_default(out: &mut String) {
    let yaml = fs::read_to_string("data/host-defaults.yaml").expect("read host-defaults");
    let parsed: crate::types_stub::HostDefaultYaml =
        serde_yaml::from_str(&yaml).expect("parse host-defaults");
    out.push_str(&format!(r#"use crate::types::{{HostConfig, GdprConfig, CcpaConfig, LmtConfig, HostCookieConfig}};

pub fn host_default() -> &'static HostConfig {{
    static CELL: std::sync::OnceLock<HostConfig> = std::sync::OnceLock::new();
    CELL.get_or_init(|| HostConfig {{
        gdpr: GdprConfig {{ enabled: {gdpr_enabled}, default_value: "{gdpr_default}".to_string() }},
        ccpa: CcpaConfig {{ enforce: {ccpa_enforce} }},
        lmt:  LmtConfig {{ enforce: {lmt_enforce} }},
        host_cookie: HostCookieConfig {{ ttl_days: {ttl_days} }},
    }})
}}
"#,
        gdpr_enabled  = parsed.gdpr.enabled,
        gdpr_default  = parsed.gdpr.default_value,
        ccpa_enforce  = parsed.ccpa.enforce,
        lmt_enforce   = parsed.lmt.enforce,
        ttl_days      = parsed.host_cookie.ttl_days,
    ));
    println!("cargo:rerun-if-changed=data/host-defaults.yaml");
}

fn emit_account_default(out: &mut String) {
    let yaml = fs::read_to_string("data/account-defaults.yaml").expect("read account-defaults");
    #[derive(Deserialize)]
    struct AccountYaml { disabled: bool, debug_allow: bool, bid_rounding: String }
    let parsed: AccountYaml = serde_yaml::from_str(&yaml).expect("parse account-defaults");
    out.push_str(&format!(r#"use crate::types::AccountConfig;

pub fn account_default() -> &'static AccountConfig {{
    static CELL: std::sync::OnceLock<AccountConfig> = std::sync::OnceLock::new();
    CELL.get_or_init(|| AccountConfig {{
        disabled: {disabled},
        debug_allow: {debug_allow},
        bid_rounding: "{bid_rounding}".to_string(),
    }})
}}
"#,
        disabled     = parsed.disabled,
        debug_allow  = parsed.debug_allow,
        bid_rounding = parsed.bid_rounding,
    ));
    println!("cargo:rerun-if-changed=data/account-defaults.yaml");
}

fn emit_eea_set(out: &mut String) {
    let txt = fs::read_to_string("data/eea-countries.txt").expect("read eea-countries");
    let countries: Vec<&str> = txt.lines().filter(|l| !l.is_empty()).collect();
    out.push_str("pub fn eea_set() -> &'static [&'static str] {\n");
    out.push_str("    &[\n");
    for c in countries { out.push_str(&format!("        \"{c}\",\n")); }
    out.push_str("    ]\n}\n\n");
    println!("cargo:rerun-if-changed=data/eea-countries.txt");
}

fn emit_gvl(out: &mut String) {
    let raw = fs::read_to_string("data/gvl.json").expect("read gvl.json");
    // For phase 1 just store the raw string and expose it as &'static str.
    out.push_str("pub fn gvl_raw() -> &'static str {\n");
    out.push_str(&format!("    {raw:?}\n"));
    out.push_str("}\n");
    println!("cargo:rerun-if-changed=data/gvl.json");
}

mod types_stub {
    // Build-side mirrors of the YAML shapes. Kept here (not in src/types.rs)
    // because they include #[serde] only — runtime crate doesn't need them.
    use serde::Deserialize;
    #[derive(Deserialize)] pub struct HostDefaultYaml {
        pub gdpr: GdprY, pub ccpa: CcpaY, pub lmt: LmtY,
        pub host_cookie: HostCookieY,
    }
    #[derive(Deserialize)] pub struct GdprY { pub enabled: bool, pub default_value: String }
    #[derive(Deserialize)] pub struct CcpaY { pub enforce: bool }
    #[derive(Deserialize)] pub struct LmtY { pub enforce: bool }
    #[derive(Deserialize)] pub struct HostCookieY { pub ttl_days: i64 }
}
```

Note the duplicate writes to `out` in `emit_bidders` — the first pass writes a `static`-based slice which doesn't compile because `BidderInfo` contains `String`; the function then clears `out` and rewrites as a `OnceLock`-based version. Keep the duplication explicit so an implementer reading this can see the dead-end-and-correction reasoning; remove the unused first block after the implementer verifies the codegen runs.

- [ ] **Step 7: Build sanity-check**

```
cargo build -p auctioneer-config
```

Expected: succeeds; `target/debug/build/auctioneer-config-*/out/generated.rs` exists.

- [ ] **Step 8: Smoke test from `tests/`**

Create `crates/auctioneer-config/tests/smoke.rs`:

```rust
use auctioneer_config::{bidders, bidder, host_default, account_default, eea_set, gvl_raw};

#[test]
fn the_four_phase_1_bidders_exist() {
    let names: Vec<&str> = bidders().iter().map(|b| b.name.as_str()).collect();
    assert!(names.contains(&"generic"));
    assert!(names.contains(&"appnexus"));
    assert!(names.contains(&"pubmatic"));
    assert!(names.contains(&"mocktioneer"));
    assert_eq!(names.len(), 4);
}

#[test]
fn pubmatic_uses_gzip_compression() {
    let p = bidder("pubmatic").expect("pubmatic must exist");
    assert_eq!(p.endpoint_compression, Some(auctioneer_config::types::Compression::Gzip));
}

#[test]
fn generic_uses_https() {
    let g = bidder("generic").expect("generic must exist");
    assert!(g.uses_https);
}

#[test]
fn mocktioneer_is_http_for_local_testing() {
    let m = bidder("mocktioneer").expect("mocktioneer must exist");
    assert!(!m.uses_https);
}

#[test]
fn host_default_has_required_gdpr_value() {
    let h = host_default();
    assert!(h.gdpr.default_value == "0" || h.gdpr.default_value == "1");
}

#[test]
fn account_default_disabled_is_false() {
    let a = account_default();
    assert!(!a.disabled);
}

#[test]
fn eea_set_contains_germany() {
    assert!(eea_set().contains(&"DEU"));
}

#[test]
fn gvl_raw_parses_as_json() {
    let v: serde_json::Value = serde_json::from_str(gvl_raw()).unwrap();
    assert!(v.get("vendors").is_some());
}
```

- [ ] **Step 9: Run smoke tests**

```
cargo test -p auctioneer-config --test smoke
```

Expected: 8 passed.

- [ ] **Step 10: Commit**

```
git add Cargo.toml crates/auctioneer-config
git commit -m "auctioneer-config: types + build.rs codegen + 4 reference bidders"
```

---

### Task 27: Workspace-wide compile + test gate

**Files:**

- None modified (verification only).

- [ ] **Step 1: Whole-workspace build**

```
cargo build --workspace
```

Expected: 0 errors, 0 warnings.

- [ ] **Step 2: Whole-workspace test**

```
cargo test --workspace
```

Expected: all tests pass across `auctioneer-runtime`, `auctioneer-openrtb`, `auctioneer-config`. The four existing adapter crates and `auctioneer-core` continue to compile (no test changes there).

- [ ] **Step 3: Clippy gate**

```
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: zero clippy warnings. The workspace's strict gate (pedantic warn + restriction deny) means new code must include scoped `#[allow]`s where pragmatic — the implementer addresses any complaints as they appear.

- [ ] **Step 4: Final commit (only if anything changed during the gate)**

If any clippy fixes were needed, commit them:

```
git add -A
git commit -m "M1: clippy + final gate"
```

---

## Self-Review

**Spec coverage** — every M1 deliverable per §14:

- `auctioneer-openrtb` types + `validate()` — Tasks 7–19 ✓
- `prepare(req, headers)` (the sole construction gate) — Tasks 20, 21, 22, 23, 24 ✓
- Implicit-field fill: site default + IP sanitize + headers + iOS LMT + channel default + `at` default — Task 22 ✓
- 2.6 privacy gate + channel empty-name reject — Task 23 ✓
- **Consent-string parsers (TCF v2 / US-Privacy / GPP) — explicitly OUT of M1** (round-37 H3 / §3.2 step 6 / §7.7). They ship in `auctioneer-privacy` from M3+. Tasks 22-24 of an earlier plan rev (consent parsers in auctioneer-openrtb) have been removed; see the "(Removed) Tasks 22-24" note above.
- `auctioneer-config` + `build.rs` codegen for 4 reference bidders — Task 26 ✓
- `auctioneer-runtime` (`Clock`, `Rng`, `FakeClock`, `SeededRng`, `shuffle`, `gen_range`) — Tasks 1–6 ✓
- "No EdgeZero wiring" — none of these crates depend on `edgezero-*`. The workspace dependency-purity test from §11.2 lives in M6, but every M1 `Cargo.toml` here adheres.

**Placeholder scan** — no "TODO", "TBD", or "similar to Task N" patterns in the plan. Every code step contains the actual code.

**Type consistency**:

- `Clock`, `Rng`, `FakeClock`, `SeededRng` consistent across Tasks 2–6.
- `BidRequest`, `Imp`, ext types, `NormalizedBidRequest`, `RequestHeaders`, `ValidationError` consistent across Tasks 10–24.
- `BidderInfo`, `Compression` consistent across Task 26 codegen and Task 26 smoke tests.

**One known place where the implementer must look closely:** Task 5 Step 1 — the pinned first-three-`SeededRng` values are placeholders the implementer captures by running the test once; this is explicitly called out, not a covert TODO.

**Granularity check** — 27 tasks (down from 30 after the round-37 H3 consent-parser deletion) averaging ~5 TDD steps each = ~135 actions, all bite-sized. Each task ends with a commit.

---

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/2026-05-25-m1-foundations.md`. Two execution options:**

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration.

**2. Inline Execution** — Execute tasks in this session using executing-plans, batch execution with checkpoints.

**Which approach?**

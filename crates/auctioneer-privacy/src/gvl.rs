//! Runtime Global Vendor List: plain lowering targets plus the
//! lazy-parse [`GvlStore`] over the baked archive.
//!
//! One parse path: [`Gvl::load_from_json`] / [`Gvl::load_from_json_bytes`]
//! delegate to `auctioneer_gvl_schema::validate_gvl_json` and lower the
//! returned `ValidatedGvl` into these ordinary Rust structs — no serde
//! derives here and no independent `serde_json` call, so every schema
//! and hostile-input invariant holds identically at build time and at
//! runtime.

use crate::tcf::PubRestrictionType;
use auctioneer_gvl_schema::{GvlSchemaError, ValidatedGvl, validate_gvl_json};
use lru::LruCache;
use once_cell::sync::OnceCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::num::NonZeroUsize;
use std::str;
#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use time::OffsetDateTime;

/// The cache bound: at most this many parsed `Gvl` handles are held BY
/// THE CACHE — a CACHE-OWNED-ENTRY bound, not a total-resident-memory
/// bound. `Gvl` is `Arc<GvlInner>`; an auction clones the handle out of
/// the cache and holds it for the auction's lifetime, so an evicted
/// entry's memory stays resident until the last clone drops. Cache-owned
/// entries are bounded by this count times the worst-case parsed size; a
/// total-resident reservation must also fund entries retained by outstanding
/// auction handles, independently of the cache's entry count.
///
/// Sized to 4: a ~4 MiB worst-case `Gvl` × 4 = ~16 MiB, comfortably
/// inside the 128 MB isolate. The hot working set is 1-2 versions
/// (current, plus the previous during a rollover); 4 leaves headroom
/// without letting the cache approach the ceiling. Raise it only with a
/// real whole-isolate budget.
pub const GVL_PARSE_CACHE_MAX_ENTRIES: usize = 4;

/// Raw-bundle ceiling the baked archive must satisfy — the wasm hard
/// maximum (24 MiB), the most-constrained target's figure. `build.rs`
/// enforces it per-target at build time; the resource-bound regression
/// re-asserts it against the runtime slice.
pub const GVL_BUNDLE_MAX_RAW_BYTES: u64 = 0x0180_0000;

/// Boot-time allocation ceiling for `GvlStore` initialization (64 KiB):
/// boot builds only the raw index over `&'static [u8]` pointers — zero
/// JSON parses.
pub const GVL_STARTUP_PEAK_MAX_BYTES: u64 = 0x0001_0000;

/// GVL spec version for a TC string's `tcfPolicyVersion`: policy
/// versions below 4 encode against spec-2 lists; 4 and above (TCF
/// v2.2+, including the v2.4-era policy 5) against spec-3.
#[must_use]
pub fn gvl_spec_for_policy(tcf_policy_version: u32) -> u32 {
    if tcf_policy_version < 4 { 2 } else { 3 }
}

#[derive(Debug, PartialEq, Eq)]
pub enum GvlParseError {
    InvalidJson(String),
    MissingRequiredField(&'static str),
}

fn map_schema_error(err: GvlSchemaError) -> GvlParseError {
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "every schema rejection beyond the two mapped variants \
                  deliberately collapses to InvalidJson with the validator's \
                  message; future variants must take that path too"
    )]
    match err {
        GvlSchemaError::InvalidJson(reason) => GvlParseError::InvalidJson(reason),
        GvlSchemaError::MissingRequiredField(field) => GvlParseError::MissingRequiredField(field),
        other => GvlParseError::InvalidJson(other.to_string()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Purpose {
    pub id: u8,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecialPurpose {
    pub id: u32,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Feature {
    pub id: u32,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecialFeature {
    pub id: u8,
    pub name: String,
}

/// One vendor's GVL declaration. The four ID spaces are separate:
/// `purposes` and `special_features` have wire caps (24 and 12
/// positions, `u8` fits); `special_purposes` and `features` are
/// GVL-declared with no wire cap and stay `u32`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vendor {
    pub id: u16,
    pub name: String,
    pub purposes: BTreeSet<u8>,
    pub leg_int_purposes: BTreeSet<u8>,
    pub flex_purposes: BTreeSet<u8>,
    pub special_purposes: BTreeSet<u32>,
    pub features: BTreeSet<u32>,
    pub special_features: BTreeSet<u8>,
    /// Excludes the vendor from authorization once the deletion instant
    /// has PASSED (the record is still looked up by id — the map
    /// retains it). Parsed at validation; no second parse here.
    pub deleted_date: Option<OffsetDateTime>,
}

impl Vendor {
    /// A vendor whose `deletedDate` has PASSED has been removed from
    /// the current GVL revision. Per the IAB contract deletion applies
    /// AFTER that instant, so enforcement denies it from then on while
    /// a FUTURE-dated vendor stays authorized until the instant passes —
    /// matching the CMP deletion comparator (`deleted_date <= now`).
    #[must_use]
    pub fn is_deleted_at(&self, now: OffsetDateTime) -> bool {
        matches!(self.deleted_date, Some(deleted) if deleted <= now)
    }

    /// GVL-declaration leg of the consent path, restriction-first: a
    /// publisher restriction overrides the vendor's declared default
    /// basis.
    #[must_use]
    #[expect(
        clippy::match_same_arms,
        reason = "NotAllowed and the basis-switching suppression share a value \
                  but encode distinct IAB rules; merging the arms would blur \
                  the restriction-first contract"
    )]
    pub fn declares_purpose_for_consent(
        &self,
        p: u8,
        restriction: Option<PubRestrictionType>,
    ) -> bool {
        match restriction {
            Some(PubRestrictionType::NotAllowed) => false,
            Some(PubRestrictionType::RequireConsent) => {
                // Must have declared consent OR flex.
                self.purposes.contains(&p) || self.flex_purposes.contains(&p)
            }
            Some(PubRestrictionType::RequireLegitInterest) => {
                // LI required — the consent path is suppressed
                // regardless of the vendor's declaration.
                false
            }
            None => {
                // No restriction — the vendor's DEFAULT declared basis
                // governs. Per the IAB vendor-list contract every
                // flexible purpose ALSO has exactly one default in
                // `purposes` XOR `legIntPurposes` (validator-enforced);
                // absent a publisher restriction ONLY that default
                // applies. `flex_purposes` is consulted ONLY when a
                // restriction SWITCHES the basis (the arms above), so
                // the consent leg here is the consent-default set only —
                // a consent-default flex purpose is NOT LI-authorized
                // (and vice versa) unless a restriction flips it. This
                // fails closed on the privacy boundary; §15 records it
                // as a deliberate divergence from prebid-server (which
                // ORs consent+LI with no restriction).
                self.purposes.contains(&p)
            }
        }
    }

    /// GVL-declaration leg of the legitimate-interest path,
    /// restriction-first.
    #[must_use]
    #[expect(
        clippy::match_same_arms,
        reason = "NotAllowed and the basis-switching suppression share a value \
                  but encode distinct IAB rules; merging the arms would blur \
                  the restriction-first contract"
    )]
    pub fn declares_purpose_for_li(&self, p: u8, restriction: Option<PubRestrictionType>) -> bool {
        match restriction {
            Some(PubRestrictionType::NotAllowed) => false,
            Some(PubRestrictionType::RequireLegitInterest) => {
                self.leg_int_purposes.contains(&p) || self.flex_purposes.contains(&p)
            }
            Some(PubRestrictionType::RequireConsent) => {
                // Consent required — the LI path is suppressed
                // regardless of declaration.
                false
            }
            // Default basis only absent a restriction; flex is switched
            // only by a pub restriction.
            None => self.leg_int_purposes.contains(&p),
        }
    }
}

/// The real storage; only `Gvl` methods reach in (the type stays
/// crate-private).
#[derive(Debug)]
pub(crate) struct GvlInner {
    pub gvl_specification_version: u32,
    pub vendor_list_version: u32,
    pub tcf_policy_version: u32,
    pub vendors: BTreeMap<u16, Vendor>,
    pub purposes: BTreeMap<u8, Purpose>,
    pub special_purposes: BTreeMap<u32, SpecialPurpose>,
    pub features: BTreeMap<u32, Feature>,
    pub special_features: BTreeMap<u8, SpecialFeature>,
}

/// Thin cheap-clone wrapper — `Clone` is a single refcount bump, not a
/// vendor-map copy.
#[derive(Debug, Clone)]
pub struct Gvl {
    inner: Arc<GvlInner>,
}

fn lower(validated: ValidatedGvl) -> GvlInner {
    fn narrow_set(ids: &[u32]) -> BTreeSet<u8> {
        // Ids outside the wire-capped range cannot survive lowering;
        // the validator's cross-reference checks already rejected them.
        ids.iter().filter_map(|&id| u8::try_from(id).ok()).collect()
    }
    let vendors = validated
        .vendors
        .into_iter()
        .map(|(id, vendor)| {
            (
                id,
                Vendor {
                    id,
                    name: vendor.name,
                    purposes: narrow_set(&vendor.purposes),
                    leg_int_purposes: narrow_set(&vendor.leg_int_purposes),
                    flex_purposes: narrow_set(&vendor.flexible_purposes),
                    special_purposes: vendor.special_purposes.iter().copied().collect(),
                    features: vendor.features.iter().copied().collect(),
                    special_features: narrow_set(&vendor.special_features),
                    deleted_date: vendor.deleted_date,
                },
            )
        })
        .collect();
    GvlInner {
        gvl_specification_version: validated.gvl_specification_version,
        vendor_list_version: validated.vendor_list_version,
        tcf_policy_version: validated.tcf_policy_version,
        vendors,
        purposes: validated
            .purposes
            .into_iter()
            .map(|(id, purpose)| {
                (
                    id,
                    Purpose {
                        id,
                        name: purpose.name,
                    },
                )
            })
            .collect(),
        special_purposes: validated
            .special_purposes
            .into_iter()
            .map(|(id, purpose)| {
                (
                    id,
                    SpecialPurpose {
                        id,
                        name: purpose.name,
                    },
                )
            })
            .collect(),
        features: validated
            .features
            .into_iter()
            .map(|(id, feature)| {
                (
                    id,
                    Feature {
                        id,
                        name: feature.name,
                    },
                )
            })
            .collect(),
        special_features: validated
            .special_features
            .into_iter()
            .map(|(id, feature)| {
                (
                    id,
                    SpecialFeature {
                        id,
                        name: feature.name,
                    },
                )
            })
            .collect(),
    }
}

impl Gvl {
    /// Lower a GVL blob through the shared validator. `spec_expected`
    /// mirrors `validate_gvl_json`: `GvlStore::get` passes
    /// `Some(slot_spec)` so a mismatched or unsupported spec is
    /// hard-rejected at runtime, exactly as strong as the build gate;
    /// hand-authored test fixtures pass `None` to skip only the spec
    /// cross-check.
    ///
    /// No per-vendor logging happens here: deleted-vendor observability
    /// is a build-time aggregate (a per-vendor runtime warn would
    /// re-emit hundreds of lines every time a version rotates through
    /// the parse cache — request-triggerable log amplification).
    ///
    /// # Errors
    ///
    /// `GvlParseError` mapping the shared validator's rejection.
    pub fn load_from_json(raw: &str, spec_expected: Option<u32>) -> Result<Gvl, GvlParseError> {
        let validated = validate_gvl_json(raw, spec_expected).map_err(map_schema_error)?;
        Ok(Gvl {
            inner: Arc::new(lower(validated)),
        })
    }

    /// Byte-slice entry point for the `include_bytes!` archive path —
    /// performs the one UTF-8 validation, then the shared parse.
    ///
    /// # Errors
    ///
    /// `GvlParseError::InvalidJson` for non-UTF-8 input, otherwise as
    /// [`Gvl::load_from_json`].
    pub fn load_from_json_bytes(
        raw: &[u8],
        spec_expected: Option<u32>,
    ) -> Result<Gvl, GvlParseError> {
        let text = str::from_utf8(raw)
            .map_err(|err| GvlParseError::InvalidJson(format!("not UTF-8: {err}")))?;
        Self::load_from_json(text, spec_expected)
    }

    /// A `Gvl` with `vendor_list_version = 0`, `tcf_policy_version = 0`,
    /// and every map empty. Test fixtures that don't need a vendor
    /// lookup use it; `GvlStore::get` uses the zero version as its
    /// parse-failure sentinel.
    #[must_use]
    pub fn empty() -> Gvl {
        Gvl {
            inner: Arc::new(GvlInner {
                gvl_specification_version: 0,
                vendor_list_version: 0,
                tcf_policy_version: 0,
                vendors: BTreeMap::new(),
                purposes: BTreeMap::new(),
                special_purposes: BTreeMap::new(),
                features: BTreeMap::new(),
                special_features: BTreeMap::new(),
            }),
        }
    }

    #[must_use]
    pub fn vendor(&self, id: u16) -> Option<&Vendor> {
        self.inner.vendors.get(&id)
    }

    #[must_use]
    pub fn purpose(&self, id: u8) -> Option<&Purpose> {
        self.inner.purposes.get(&id)
    }

    #[must_use]
    pub fn special_feature(&self, id: u8) -> Option<&SpecialFeature> {
        self.inner.special_features.get(&id)
    }

    /// Top-level special-purpose table lookup — enforcement checks a
    /// vendor's `special_purposes` id set against this table.
    #[must_use]
    pub fn special_purpose(&self, id: u32) -> Option<&SpecialPurpose> {
        self.inner.special_purposes.get(&id)
    }

    /// Top-level feature table lookup.
    #[must_use]
    pub fn feature(&self, id: u32) -> Option<&Feature> {
        self.inner.features.get(&id)
    }

    #[must_use]
    pub fn vendor_list_version(&self) -> u32 {
        self.inner.vendor_list_version
    }

    #[must_use]
    pub fn tcf_policy_version(&self) -> u32 {
        self.inner.tcf_policy_version
    }

    #[must_use]
    pub fn gvl_specification_version(&self) -> u32 {
        self.inner.gvl_specification_version
    }
}

/// Lazy-parse store over the baked archive: raw JSON stays as
/// `include_bytes!` data in .rodata (zero heap at boot) and parses on
/// first `get(spec, list)`, behind an access-recency LRU bounded by
/// ENTRY COUNT. Eager parsing of every bundled version would resident
/// tens of megabytes per isolate; count-bounding keeps the accounting
/// honest (there is no per-`Gvl` byte figure worth summing).
type ParseCell = Arc<OnceCell<Gvl>>;

pub struct GvlStore {
    /// Immutable index of `(spec, list)` → `&'static` raw bytes.
    /// Populated once; no per-entry allocation after boot.
    raw_index: HashMap<(u32, u32), &'static [u8]>,
    /// Holds `Arc<OnceCell<Gvl>>` so concurrent misses on the same key
    /// singleflight through `OnceCell::get_or_init`; the parse runs
    /// outside the lock on the shared cell. If a cell is evicted while
    /// its parse is in flight, a later miss re-parses into a fresh
    /// cell — a rare, correctness-neutral duplicate parse under
    /// eviction pressure (there is no byte accounting to corrupt).
    cache: Mutex<LruCache<(u32, u32), ParseCell>>,
    /// Store-scoped parse counter — the test hook behind the
    /// singleflight and pure-Arc-bump regressions (store-scoped so
    /// concurrent tests never pollute each other's deltas).
    #[cfg(test)]
    parse_count: AtomicUsize,
}

/// The `baked()` initialization body, factored out so the boot-heap
/// regression can measure a fresh construction directly (the
/// process-wide static initializes once per process).
fn build_baked_store() -> GvlStore {
    let mut raw_index = HashMap::new();
    for entry in auctioneer_config_data::gvl_archive() {
        raw_index.insert((entry.spec, entry.vendor_list_version), entry.raw);
    }
    let entries_nz = NonZeroUsize::new(GVL_PARSE_CACHE_MAX_ENTRIES).unwrap_or(NonZeroUsize::MIN);
    GvlStore {
        raw_index,
        cache: Mutex::new(LruCache::new(entries_nz)),
        #[cfg(test)]
        parse_count: AtomicUsize::new(0),
    }
}

impl GvlStore {
    /// Process-wide baked store. Populated from
    /// `auctioneer_config_data::gvl_archive()` on first call; no per-entry
    /// parsing at init.
    #[must_use]
    pub fn baked() -> &'static GvlStore {
        static STORE: OnceLock<GvlStore> = OnceLock::new();
        STORE.get_or_init(build_baked_store)
    }

    #[cfg(test)]
    pub(crate) fn fresh_baked_for_test() -> Self {
        build_baked_store()
    }

    /// Parse-on-demand, singleflight-within-cell, entry-count bounded.
    /// Cache hit → Arc refcount bump; miss → one JSON parse behind the
    /// shared cell. `None` when the `(spec, list)` isn't bundled. A
    /// parse failure on a bundled entry is a workspace-invariant
    /// violation (build.rs validated at compile time); the runtime logs
    /// and returns `None` rather than panicking (edge runtimes tolerate
    /// panics poorly).
    #[must_use]
    pub fn get(&self, spec: u32, list: u32) -> Option<Gvl> {
        let key = (spec, list);

        // Check raw_index FIRST so unknown versions never touch the
        // cache (an attacker walking (3,1),(3,2)… could otherwise evict
        // the hot set).
        let raw = *self.raw_index.get(&key)?;

        // Take-or-create the shared cell under the lock; the parse runs
        // outside it. `get_or_insert` also marks the entry
        // most-recently-used and performs any count-based eviction
        // atomically.
        let cell = {
            let mut cache = self.cache.lock().ok()?;
            Arc::clone(cache.get_or_insert(key, || Arc::new(OnceCell::new())))
        };

        let parsed = cell.get_or_init(|| {
            #[cfg(test)]
            self.parse_count.fetch_add(1, Ordering::Relaxed);
            // Pass the slot spec INTO runtime lowering — `Some(spec)` —
            // so a blob whose declared gvlSpecificationVersion doesn't
            // match the slot it was fetched into is hard-rejected at
            // runtime, as strong as the build gate. A reject → parse
            // error → the empty sentinel below → fail-closed miss.
            Gvl::load_from_json_bytes(raw, Some(spec)).unwrap_or_else(|err| {
                log::error!(target: "auctioneer::gvl",
                    "baked GVL entry ({spec}, {list}) failed schema/spec check: {err:?}");
                Gvl::empty()
            })
        });

        if parsed.vendor_list_version() == 0 {
            return None; // parse/spec-failure sentinel → miss
        }

        Some(parsed.clone())
    }

    /// Test-only constructor: pre-parsed `Gvl`s behind inert raw-index
    /// slots, so `get` serves them without touching a real archive.
    #[must_use]
    pub fn for_test(entries: Vec<(u32, u32, Gvl)>) -> Self {
        let capacity = NonZeroUsize::new(entries.len().max(GVL_PARSE_CACHE_MAX_ENTRIES))
            .unwrap_or(NonZeroUsize::MIN);
        let mut raw_index = HashMap::new();
        let mut cache = LruCache::new(capacity);
        for (spec, list, gvl) in entries {
            raw_index.insert((spec, list), &b""[..]);
            cache.put((spec, list), Arc::new(OnceCell::with_value(gvl)));
        }
        GvlStore {
            raw_index,
            cache: Mutex::new(cache),
            #[cfg(test)]
            parse_count: AtomicUsize::new(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use time::macros::datetime;

    /// Test-scoped counting allocator (per-thread, cumulative) backing
    /// the boot-heap regression. Delegates every operation to `System`.
    mod alloc_track {
        #![expect(
            unsafe_code,
            reason = "a GlobalAlloc impl is unavoidably unsafe; test-only \
                      measurement backing the startup-peak allocation bound"
        )]

        use std::alloc::{GlobalAlloc, Layout, System};
        use std::cell::Cell;

        thread_local! {
            static ALLOCATED: Cell<u64> = const { Cell::new(0) };
        }

        pub fn allocated_on_this_thread() -> u64 {
            ALLOCATED.try_with(Cell::get).unwrap_or(0)
        }

        fn count(bytes: usize) {
            // TLS teardown can make try_with fail; a lost count only
            // relaxes the measurement, never breaks the allocator.
            let _outcome = ALLOCATED.try_with(|cell| {
                cell.set(
                    cell.get()
                        .saturating_add(u64::try_from(bytes).unwrap_or(u64::MAX)),
                );
            });
        }

        pub struct CountingAllocator;

        // SAFETY: every operation delegates verbatim to `System`; the
        // counting side effect touches only a thread-local counter and
        // never the returned memory.
        unsafe impl GlobalAlloc for CountingAllocator {
            unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
                count(layout.size());
                // SAFETY: same layout contract as the caller's.
                unsafe { System.alloc(layout) }
            }

            unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
                // SAFETY: ptr/layout come from a matching System alloc.
                unsafe { System.dealloc(ptr, layout) }
            }

            unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
                count(layout.size());
                // SAFETY: same layout contract as the caller's.
                unsafe { System.alloc_zeroed(layout) }
            }

            unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
                count(new_size.saturating_sub(layout.size()));
                // SAFETY: ptr/layout come from a matching System alloc;
                // new_size is the caller's requested size.
                unsafe { System.realloc(ptr, layout, new_size) }
            }
        }
    }

    #[global_allocator]
    static COUNTING_ALLOC: alloc_track::CountingAllocator = alloc_track::CountingAllocator;

    /// Minimal valid spec-3 GVL with one vendor (id 8) and one deleted
    /// vendor (id 9, past-dated) — the loader fixtures.
    ///
    /// Raw-string `const`, not `serde_json::json!`: `auctioneer-privacy`
    /// has no `serde_json` dependency on purpose — the GVL/GCL parse
    /// path is delegated to `auctioneer-gvl-schema` as the one parse path,
    /// and `Cargo.toml` says so. `versioned_gvl` below also rewrites
    /// `"vendorListVersion": 42` by its literal spelling.
    const MINIMAL_GVL: &str = r#"{
        "gvlSpecificationVersion": 3,
        "tcfPolicyVersion": 5,
        "vendorListVersion": 42,
        "lastUpdated": "2026-07-01T00:00:00Z",
        "purposes": {
            "1": {"id": 1, "name": "Storage and access"},
            "2": {"id": 2, "name": "Limited advertising"},
            "7": {"id": 7, "name": "Measure advertising performance"}
        },
        "specialPurposes": {"1": {"id": 1, "name": "Security"}},
        "features": {"1": {"id": 1, "name": "Matching"}},
        "specialFeatures": {"1": {"id": 1, "name": "Precise geolocation"}},
        "vendors": {
            "8": {"id": 8, "name": "Vendor Eight", "purposes": [1, 2],
                   "legIntPurposes": [7], "specialPurposes": [1],
                   "features": [1], "specialFeatures": [1]},
            "9": {"id": 9, "name": "Deleted Vendor", "purposes": [1],
                   "deletedDate": "2026-01-15T00:00:00Z"}
        }
    }"#;

    /// A distinct minimal GVL per (version) for cache tests.
    fn versioned_gvl(version: u32) -> String {
        MINIMAL_GVL.replacen(
            "\"vendorListVersion\": 42",
            &format!("\"vendorListVersion\": {version}"),
            1,
        )
    }

    fn store_with_raw_versions(versions: &[u32]) -> GvlStore {
        let mut raw_index = HashMap::new();
        for &version in versions {
            let leaked: &'static [u8] =
                Box::leak(versioned_gvl(version).into_bytes().into_boxed_slice());
            raw_index.insert((3, version), leaked);
        }
        let entries_nz =
            NonZeroUsize::new(GVL_PARSE_CACHE_MAX_ENTRIES).unwrap_or(NonZeroUsize::MIN);
        GvlStore {
            raw_index,
            cache: Mutex::new(LruCache::new(entries_nz)),
            parse_count: AtomicUsize::new(0),
        }
    }

    // ───────────────────────── loader (9) ─────────────────────────

    #[test]
    fn empty_json_object_fails_missing_field() {
        let err = Gvl::load_from_json("{}", None).unwrap_err();
        assert!(matches!(err, GvlParseError::MissingRequiredField(_)));
    }

    #[test]
    fn valid_minimal_gvl_round_trips() {
        let gvl = Gvl::load_from_json(MINIMAL_GVL, None).unwrap();
        assert_eq!(gvl.vendor_list_version(), 42);
        assert_eq!(gvl.tcf_policy_version(), 5);
        assert_eq!(gvl.gvl_specification_version(), 3);
        assert!(gvl.purpose(1).is_some());
        assert!(gvl.special_feature(1).is_some());
    }

    #[test]
    fn unknown_vendor_id_returns_none() {
        let gvl = Gvl::load_from_json(MINIMAL_GVL, None).unwrap();
        assert!(gvl.vendor(4242).is_none());
    }

    #[test]
    fn known_vendor_id_returns_purpose_set() {
        let gvl = Gvl::load_from_json(MINIMAL_GVL, None).unwrap();
        let vendor = gvl.vendor(8).expect("vendor 8 present");
        assert_eq!(vendor.purposes, BTreeSet::from([1, 2]));
        assert_eq!(vendor.leg_int_purposes, BTreeSet::from([7]));
        assert_eq!(vendor.special_purposes, BTreeSet::from([1]));
    }

    #[test]
    fn deleted_vendor_still_looks_up_but_has_deleted_date() {
        let gvl = Gvl::load_from_json(MINIMAL_GVL, None).unwrap();
        let vendor = gvl.vendor(9).expect("deleted vendor still looked up");
        assert_eq!(
            vendor.deleted_date,
            Some(datetime!(2026-01-15 00:00:00 UTC))
        );
        assert!(vendor.is_deleted_at(datetime!(2026-07-01 00:00:00 UTC)));
        assert!(!vendor.is_deleted_at(datetime!(2026-01-01 00:00:00 UTC)));
    }

    #[test]
    fn load_emits_no_per_vendor_deleted_warning() {
        // Deleted observability is the build-time aggregate; the
        // runtime load path is silent (a per-vendor warn would amplify
        // hundreds of lines per cache-rotation cycle).
        testing_logger::setup();
        let gvl = Gvl::load_from_json(MINIMAL_GVL, None).unwrap();
        testing_logger::validate(|captured| {
            let deleted_warns = captured
                .iter()
                .filter(|line| {
                    line.level == log::Level::Warn && line.body.contains("marked deleted")
                })
                .count();
            assert_eq!(
                deleted_warns, 0,
                "no per-vendor deleted warns may be emitted"
            );
        });
        // Pair-assert the lookup contract is unchanged.
        let vendor = gvl.vendor(9).expect("record still present");
        assert!(vendor.is_deleted_at(datetime!(2026-07-01 00:00:00 UTC)));
    }

    #[test]
    fn baked_gvl_loads_without_panic() {
        let store = GvlStore::baked();
        let entry = auctioneer_config_data::gvl_archive()
            .first()
            .expect("archive has entries");
        assert!(
            store.get(entry.spec, entry.vendor_list_version).is_some(),
            "baked entry must parse"
        );
    }

    #[test]
    fn gvl_empty_has_zero_vendors_and_lookups_return_none() {
        let gvl = Gvl::empty();
        assert_eq!(gvl.vendor_list_version(), 0);
        assert_eq!(gvl.tcf_policy_version(), 0);
        assert!(gvl.vendor(1).is_none());
        assert!(gvl.purpose(1).is_none());
        assert!(gvl.special_feature(1).is_none());
    }

    #[test]
    fn runtime_lowering_agrees_with_validated_dto() {
        /// DTO `Vec<u32>` → the runtime's `BTreeSet<u8>` domain. Ids
        /// outside 1..=24 cannot survive lowering, so they are dropped
        /// here exactly as the lowering drops them.
        fn to_purpose_set(raw: &[u32]) -> BTreeSet<u8> {
            raw.iter().filter_map(|&p| u8::try_from(p).ok()).collect()
        }

        for entry in auctioneer_config_data::gvl_archive() {
            let raw_str = str::from_utf8(entry.raw)
                .expect("build.rs validated UTF-8 for every archive entry");
            // ONE authoritative validation, via the standalone crate.
            let dto = auctioneer_gvl_schema::validate_gvl_json(raw_str, Some(entry.spec)).unwrap();
            // Runtime lowering of the SAME bytes.
            let gvl = Gvl::load_from_json(raw_str, Some(entry.spec)).unwrap();
            // Identity agreement.
            assert_eq!(gvl.vendor_list_version(), dto.vendor_list_version);
            assert_eq!(gvl.tcf_policy_version(), dto.tcf_policy_version);
            // Content agreement: EVERY DTO vendor survives lowering
            // with the same purpose sets — the assertion that actually
            // proves runtime == validator.
            for (&dto_id, dto_vendor) in &dto.vendors {
                let lowered = gvl
                    .vendor(dto_id)
                    .unwrap_or_else(|| panic!("vendor {dto_id} lost in lowering"));
                assert_eq!(lowered.purposes, to_purpose_set(&dto_vendor.purposes));
                assert_eq!(
                    lowered.leg_int_purposes,
                    to_purpose_set(&dto_vendor.leg_int_purposes)
                );
            }
        }
    }

    // ─────────────────── lookup / policy selection (5) ───────────────────

    #[test]
    fn baked_gvl_selected_when_exact_pair_matches() {
        let store = GvlStore::baked();
        let entry = auctioneer_config_data::gvl_archive()
            .iter()
            .find(|entry| entry.spec == 3)
            .expect("spec-3 entry bundled");
        let gvl = store
            .get(entry.spec, entry.vendor_list_version)
            .expect("exact pair matches");
        assert_eq!(gvl.vendor_list_version(), entry.vendor_list_version);
    }

    #[test]
    fn unbaked_pair_returns_none_from_store() {
        // The lookup contract only — the fail-closed permissions
        // consequence is asserted at the resolver.
        assert!(GvlStore::baked().get(3, 999_999).is_none());
    }

    #[test]
    fn policy_version_3_selects_gvl_spec_2() {
        assert_eq!(gvl_spec_for_policy(3), 2);
        assert_eq!(gvl_spec_for_policy(2), 2);
    }

    #[test]
    fn policy_version_4_selects_gvl_spec_3() {
        assert_eq!(gvl_spec_for_policy(4), 3);
    }

    #[test]
    fn baked_pair_with_stale_policy_version_field_still_returns_some() {
        // The lookup is keyed only by (spec, list); it does NOT
        // cross-check the blob's tcf_policy_version against the TC
        // string. The fail-closed consequence of a mismatch is the
        // resolver's concern.
        let stale = MINIMAL_GVL.replacen("\"tcfPolicyVersion\": 5", "\"tcfPolicyVersion\": 99", 1);
        let gvl = Gvl::load_from_json(&stale, None).unwrap();
        let store = GvlStore::for_test(vec![(3, 42, gvl)]);
        let looked_up = store.get(3, 42).expect("lookup ignores policy version");
        assert_eq!(looked_up.tcf_policy_version(), 99);
    }

    #[test]
    fn v24_policy_version_selects_correct_gvl_spec() {
        // TCF v2.4-era strings carry tcfPolicyVersion 5 → spec-3, and
        // the baked archive serves that pair.
        assert_eq!(gvl_spec_for_policy(5), 3);
        let entry = auctioneer_config_data::gvl_archive()
            .iter()
            .find(|entry| entry.spec == 3 && entry.tcf_policy_version == 5)
            .expect("v2.4-era entry bundled");
        let gvl = GvlStore::baked()
            .get(3, entry.vendor_list_version)
            .expect("v2.4 pair served");
        assert_eq!(gvl.tcf_policy_version(), 5);
    }

    // ─────────────────── cache / singleflight (5) ───────────────────

    #[test]
    fn gvl_cache_evicts_lru_at_entry_cap() {
        let versions = [101, 102, 103, 104, 105];
        let store = store_with_raw_versions(&versions);
        for &version in &versions {
            assert!(store.get(3, version).is_some());
        }
        let cache = store.cache.lock().unwrap();
        assert_eq!(cache.len(), GVL_PARSE_CACHE_MAX_ENTRIES);
        assert!(
            !cache.contains(&(3, 101)),
            "the least-recently-used key must be evicted"
        );
    }

    #[test]
    fn gvl_cache_singleflights_concurrent_misses() {
        let store = store_with_raw_versions(&[201]);
        thread::scope(|scope| {
            let first = scope.spawn(|| store.get(3, 201).is_some());
            let second = scope.spawn(|| store.get(3, 201).is_some());
            assert!(first.join().unwrap_or(false));
            assert!(second.join().unwrap_or(false));
        });
        assert_eq!(
            store.parse_count.load(Ordering::Relaxed),
            1,
            "concurrent misses on one key parse exactly once"
        );
    }

    #[test]
    fn gvl_cache_hit_is_pure_arc_bump() {
        let store = store_with_raw_versions(&[301]);
        let first = store.get(3, 301).expect("cold get parses");
        assert_eq!(store.parse_count.load(Ordering::Relaxed), 1);
        let second = store.get(3, 301).expect("hot get hits");
        assert_eq!(
            store.parse_count.load(Ordering::Relaxed),
            1,
            "a hot get performs no parse"
        );
        assert!(
            Arc::ptr_eq(&first.inner, &second.inner),
            "a hot get returns the same shared GvlInner"
        );
    }

    #[test]
    fn gvl_store_unknown_version_does_not_touch_cache() {
        let store = store_with_raw_versions(&[401]);
        assert!(store.get(3, 999).is_none());
        assert_eq!(
            store.cache.lock().unwrap().len(),
            0,
            "an unbaked pair must not allocate a cache cell (cache-eviction DoS)"
        );
    }

    #[test]
    fn gvl_store_unknown_version_returns_none() {
        let store = store_with_raw_versions(&[501]);
        assert!(store.get(2, 501).is_none());
        assert!(store.get(3, 502).is_none());
    }

    // ───────────────────── resource bounds (4) ─────────────────────

    #[test]
    fn gvl_store_get_bounded_by_entry_cap() {
        let versions = [601, 602, 603, 604, 605, 606];
        let store = store_with_raw_versions(&versions);
        for &version in &versions {
            assert!(store.get(3, version).is_some());
        }
        assert!(
            store.cache.lock().unwrap().len() <= GVL_PARSE_CACHE_MAX_ENTRIES,
            "the parse cache is bounded by entry count"
        );
    }

    #[test]
    fn gvl_store_boot_heap_under_startup_peak_max_bytes() {
        // Boot builds only the raw index over &'static pointers — zero
        // parses. Measured against a fresh construction so the
        // assertion holds regardless of which test initialized the
        // process-wide static first.
        let before = alloc_track::allocated_on_this_thread();
        let store = build_baked_store();
        let allocated = alloc_track::allocated_on_this_thread().saturating_sub(before);
        drop(store);
        assert!(
            allocated <= GVL_STARTUP_PEAK_MAX_BYTES,
            "boot allocated {allocated} bytes, over the {GVL_STARTUP_PEAK_MAX_BYTES}-byte cap"
        );
    }

    /// Cold-parse LATENCY is not asserted here: it is a
    /// production-hardware p95 target, and a debug-build wall-clock
    /// reading says nothing about it. `benches/gvl_cold_parse.rs` owns
    /// that measurement, over 40 iterations with percentiles.
    #[test]
    fn gvl_store_bundle_total_size_under_bundle_max_bytes() {
        let mut total: u64 = 0;
        for entry in auctioneer_config_data::gvl_archive() {
            total = total.saturating_add(u64::try_from(entry.raw.len()).unwrap_or(u64::MAX));
        }
        assert!(
            total <= GVL_BUNDLE_MAX_RAW_BYTES,
            "bundled raw archive is {total} bytes, over {GVL_BUNDLE_MAX_RAW_BYTES}"
        );
    }
}

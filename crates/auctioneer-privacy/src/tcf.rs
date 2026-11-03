//! TCF v2 consent string parsing — hand-rolled, hostile-input
//! hardened. `user.consent` is attacker-controlled request data, so
//! every allocation is reservation-bounded by a wire-derived cap and
//! all decode work is charged against an operation budget.
//!
//! Decision: HAND-ROLLED (the sole live candidate, `lib_tcstring
//! 0.5.0`, materializes range-encoded vendor sections as per-id
//! `Vec<u16>` with no allocation cap and parses the obsolete type-2
//! segment instead of rejecting it).
//!
//! Binary-schema source for the field taxonomy: IAB Europe
//! Transparency & Consent Framework v2 Technical Specifications,
//! "Consent string and Vendor List Formats v2" (TC String format
//! section), v2.4-era revision (May 2026 publication line):
//! <https://github.com/InteractiveAdvertisingBureau/GDPR-Transparency-and-Consent-Framework/blob/master/TCFv2/IAB%20Tech%20Lab%20-%20Consent%20string%20and%20vendor%20list%20formats%20v2.md>.
//!
//! Parse order over the raw input: (0) cap the WHOLE input at
//! [`MAX_TOTAL_INPUT_LEN`]; (1) split on the FIRST `.`;
//! (2) validate the trailing segment set — it must be exactly
//! `{Disclosed Vendors}` or `{Disclosed Vendors, Publisher TC}`,
//! every segment framing-valid under its own per-segment bounds;
//! (3) retain Disclosed Vendors membership from that same pass,
//! discarding Publisher TC contents; (4) cap the CORE substring at
//! [`MAX_INPUT_LEN`]; (5) base64url-decode the core; (6) cap the
//! decoded core at [`MAX_DECODED_LEN`]; then parse the core contents.
//! Where go-gdpr accepts a core-only or malformed-suffix string,
//! auctioneer rejects fail-closed; go-gdpr parity holds only for the
//! core segment's content semantics.

use std::collections::BTreeSet;
use std::mem;
use std::ops::RangeInclusive;

use time::macros::datetime;
use time::{OffsetDateTime, Time};

/// Byte cap on the CORE substring (before base64url decoding).
/// Intentional compatibility divergence: auctioneer caps consent strings
/// at 64 KiB, matching prebid-server's `HeaderMaxLen` convention.
/// Trailing segments have their own per-segment bounds. Real-world
/// TCF cores are ≤ ~2 KiB; publishers hitting this must trim.
pub const MAX_INPUT_LEN: usize = 0x1_0000;

/// A valid string has at most three segments: Core (0), Disclosed
/// Vendors (1, mandatory), Publisher TC (3). The trailing set must be
/// exactly `{DV}` or `{DV, PublisherTC}` (order-agnostic); more than
/// two trailing segments is rejected before any of them is decoded.
pub const MAX_TRAILING_SEGMENTS: usize = 2;

/// Byte cap on the ENTIRE raw input, separators included. Derived
/// rather than chosen: a legal string is one core plus at most
/// [`MAX_TRAILING_SEGMENTS`] trailing segments, each bounded by
/// [`MAX_INPUT_LEN`], with one `.` between neighbours. Enforced FIRST,
/// before the split, so a hostile suffix is never walked.
pub const MAX_TOTAL_INPUT_LEN: usize = MAX_INPUT_LEN
    .saturating_mul(MAX_TRAILING_SEGMENTS.saturating_add(1))
    .saturating_add(MAX_TRAILING_SEGMENTS);

/// Post-base64 decoded length ceiling (base64 → binary is 3/4 of
/// input size; `MAX_INPUT_LEN * 3 / 4` = 49 152 bytes).
pub const MAX_DECODED_LEN: usize = 0xC000;

/// Vendor ids are u16 storage per the IAB format.
pub const MAX_VENDOR_ID: u16 = u16::MAX;

/// Cap on the NUMBER OF ENCODED RANGE ENTRIES per range section.
/// `NumEntries` is a 12-bit wire field (max 4 095), so this is a
/// defensive assertion that fires only on decoder bugs, never on
/// adversarial input. `VendorBitSet` (fixed 8 KiB) absorbs any
/// encoded distribution regardless.
pub const MAX_RANGE_ENCODED_ENTRIES: u16 = 4_095;

/// Cap on `NumPubRestrictions`. Also a 12-bit wire field — same
/// defensive-assertion status as [`MAX_RANGE_ENCODED_ENTRIES`].
pub const MAX_PUB_RESTRICTION_ENCODED_ENTRIES: u16 = 4_095;

/// Cap on the DECODED list of restriction objects on
/// `TcfString::publisher_restrictions` (each restriction = purpose ×
/// type × vendor range). Intentional compatibility divergence: strict
/// wire fidelity would allow 4 095 records × 4 095 inner ranges
/// (~16 M-object memory blow-up); real-world publishers declare at
/// most tens of restrictions.
pub const MAX_PUBLISHER_RESTRICTION_LIST_LEN: usize = 2_048;

/// Default operation-budget ceiling (1 572 864). Charges: 1 per
/// base64url input byte, 1 per decoded bit read, 1 per decoded-list
/// push, the word count per accepted range entry, 1 per range entry
/// recorded for overlap detection, and — once per range section —
/// `n·⌈log2 n⌉ + n` for the sort and adjacent-pair scan that decide
/// disjointness (charged BEFORE the sort runs, so the ceiling bounds
/// the sort rather than trailing it).
///
/// The overlap charge depends only on a section's entry COUNT, never
/// on the order the entries were encoded in: identical ranges encoded
/// ascending, descending, or shuffled cost the same.
///
/// The reachable worst case under the caps: bits are charged as they
/// are DECLARED by the encoded fields, not per byte of segment, so the
/// bound comes from the widest section a string can declare. A range
/// section reads ≤ 4 095 entries × 34 bits ≈ 139 k bits and, on top of
/// that, charges ≤ 4 095 word-walk ops plus the overlap terms
/// (4 095 pushes + 4 095 × ⌈log2 4095⌉ = 12 sort ops + 4 095 scan ops
/// ≈ 61 k) — about 205 k per range section. A bitfield section instead
/// reads ≤ 65 535 bits. One core segment (two vendor sections plus
/// publisher restrictions, whose inner ranges share the same per-
/// section bound) with two trailing range segments reaches ≈ 1.2 M
/// with the ≤ 196 k input-byte charges included, which is why the
/// ceiling sits at 1 572 864: enough headroom that no legal string is
/// refused, tight enough that runaway decode work still trips it.
pub const DEFAULT_OP_CEILING: u64 = 0x18_0000;

/// `tcfPolicyVersion < 4` is only valid for strings created before
/// this instant (the TCF technical specification's policy-v3 cutoff;
/// IAB Europe's transition FAQ names a later 2023-11-20 grace end,
/// but the technical specification is the normative parser contract).
const POLICY_V3_CREATED_CUTOFF: OffsetDateTime = datetime!(2023-10-01 00:00:00 UTC);

/// Fixed-size bitset for vendor consent, LI, or disclosure. A `BTreeSet<u16>`
/// would allocate one tree node per vendor id, so a single compact
/// range entry like `1..=65535` in the encoded string would expand
/// into 65 535 tree nodes on decode — a memory blowup the
/// hostile-input caps are meant to prevent. `VendorBitSet` is a
/// fixed `[u64; 1024]` (8 KiB, `65_536 / 64 = 1024` words) whose
/// size does NOT depend on encoded cardinality.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VendorBitSet([u64; 1024]);

impl Default for VendorBitSet {
    fn default() -> Self {
        Self::new()
    }
}

impl VendorBitSet {
    /// The empty set.
    #[must_use]
    pub fn new() -> Self {
        Self([0; 1024])
    }

    /// Test-only constructor that bypasses `set`'s id-0 guard so the
    /// storage-layer zero check is provable on its own; the safe
    /// alternative to `unsafe` bit-poking under the workspace
    /// `unsafe_code = "deny"` lint. Takes the words by reference —
    /// the 8 KiB array by value trips the pass-by-value lint.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn from_raw_words(words: &[u64; 1024]) -> Self {
        Self(*words)
    }

    /// ID `0` is not a valid GVL vendor id; it is unrepresentable in
    /// the set. `contains(0)` is UNCONDITIONALLY `false` — the
    /// storage bit is never inspected. This is the safety net that
    /// makes `purpose_allowed_basic(.., 0, ..)` safe: bidders
    /// without a GVL id (`vendor_id` 0) CANNOT be granted consent by a
    /// malformed range entry that started at `0`.
    #[must_use]
    pub fn contains(&self, id: u16) -> bool {
        if id == 0 {
            return false;
        }
        // Total methods (`wrapping_div`/`wrapping_rem`/`wrapping_shl`)
        // plus `.get` keep this clear of the `integer_division`,
        // `arithmetic_side_effects`, and `indexing_slicing`
        // restriction lints. word = idx/64 ≤ 65535/64 = 1023 < 1024,
        // so the `None` arm is unreachable but fail-safe.
        let idx = usize::from(id);
        let word = idx.wrapping_div(64);
        let bit = u32::try_from(idx.wrapping_rem(64)).unwrap_or(0);
        match self.0.get(word) {
            Some(w) => (w & 1_u64.wrapping_shl(bit)) != 0,
            None => false,
        }
    }

    /// `set(0)` and any set covering id 0 is a silent no-op for id 0
    /// (the underlying bit is untouched even if the parser was
    /// tricked into invoking it). The PARSER MUST still reject range
    /// entries whose interval includes 0 or exceeds `MAX_VENDOR_ID`;
    /// this is defence in depth for the storage layer.
    pub fn set(&mut self, id: u16) {
        if id == 0 {
            return;
        }
        let idx = usize::from(id);
        // `.get_mut` + `wrapping_*` avoid `indexing_slicing` /
        // `integer_division` / `arithmetic_side_effects`. word ≤
        // 1023 < 1024, so `get_mut` is always `Some`; the `if let`
        // is fail-safe.
        let word = idx.wrapping_div(64);
        let bit = u32::try_from(idx.wrapping_rem(64)).unwrap_or(0);
        if let Some(w) = self.0.get_mut(word) {
            *w |= 1_u64.wrapping_shl(bit);
        }
    }

    /// Set every id in `start..=end` by filling whole `u64` words.
    /// Work is **O(words touched) ≤ 1024**, NOT O(range width) — a
    /// per-id loop would let a hostile `[1, 65535]` entry do 65 535
    /// mutations that the parser's operation counter never charged,
    /// so a stream of broad overlapping ranges could exceed the
    /// intended work ceiling without tripping it. The parser MUST
    /// charge the word count `(end/64 - start.max(1)/64 + 1)` to its
    /// counter and check the running total against the ceiling BEFORE
    /// calling `set_range` (validate-before-mutate); this method then
    /// does exactly that many word writes. `start.max(1)` guarantees
    /// bit 0 is never set even on a bypassed invocation.
    pub fn set_range(&mut self, start: u16, end: u16) {
        let lo = usize::from(start.max(1));
        let hi = usize::from(end);
        if hi < lo {
            return;
        }
        let first_word = lo.wrapping_div(64);
        let last_word = hi.wrapping_div(64);
        for word in first_word..=last_word {
            let base = word.wrapping_mul(64);
            let from = u32::try_from(lo.max(base).wrapping_sub(base)).unwrap_or(0);
            let word_top = base.wrapping_add(63);
            let to = u32::try_from(hi.min(word_top).wrapping_sub(base)).unwrap_or(63);
            if let Some(w) = self.0.get_mut(word) {
                *w |= Self::word_bits(from, to); // one word write
            }
        }
    }

    /// Inclusive bit mask for bits `from..=to` (`0 <= from <= to
    /// <= 63`). Lint-safe — `checked_shr`/`checked_shl` avoid
    /// `arithmetic_side_effects`, and `width == 64` (whole word) is
    /// handled without a 64-bit shift overflow.
    fn word_bits(from: u32, to: u32) -> u64 {
        let width = to.saturating_sub(from).saturating_add(1); // 1..=64
        let ones = u64::MAX
            .checked_shr(64_u32.saturating_sub(width))
            .unwrap_or(0);
        ones.checked_shl(from).unwrap_or(0)
    }
}

/// The word count `set_range(start, end)` will touch — the amount the
/// parser charges to its operation budget BEFORE mutating.
fn word_count(start: u16, end: u16) -> u64 {
    let first = u64::from(start.max(1)).wrapping_div(64);
    let last = u64::from(end).wrapping_div(64);
    last.wrapping_sub(first).saturating_add(1)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TcfString {
    pub tcf_policy_version: u8,
    pub vendor_list_version: u16,
    /// The core segment's `CmpId` (12 bits) — RETAINED. Consent
    /// resolution validates it against the bundled Global CMP List: a
    /// TC string whose CMP is absent from the GCL, or whose CMP was
    /// DELETED, makes consent UNUSABLE (fail-closed via
    /// `consent_usable = false` — per the TCF rule that only
    /// registered CMPs may create TC strings and IAB vendor guidance
    /// to discard strings from deleted CMPs).
    pub cmp_id: u16,
    /// `CmpVersion` (12 bits) — retained for diagnostics alongside
    /// `cmp_id`; not consulted by enforcement.
    pub cmp_version: u16,
    /// Decoded from the core segment's `Created` epoch-deciseconds
    /// field (36 bits). Retained for policy-version gating (Axis B)
    /// and staleness diagnostics.
    pub created: OffsetDateTime,
    /// Decoded from the core segment's `LastUpdated` field; retained
    /// for staleness diagnostics and the created/last-updated
    /// equality check (Axis C).
    pub last_updated: OffsetDateTime,
    pub purposes_consent: BTreeSet<u8>,
    pub purposes_li_transparency: BTreeSet<u8>,
    pub vendor_consent: VendorBitSet,
    pub vendor_li: VendorBitSet,
    /// Membership retained from the mandatory Disclosed Vendors
    /// segment. Disclosure is not consent or a legitimate-interest
    /// grant and does not participate in purpose authorization.
    pub disclosed_vendors: VendorBitSet,
    pub special_feature_optins: BTreeSet<u8>,
    pub purpose_one_treatment: bool,
    pub publisher_cc: [u8; 2],
    /// Publisher restrictions decoded from the core segment's
    /// publisher-restrictions section (this is NOT a separate
    /// `SegmentType` — it lives inside the core bitstream after
    /// `NumPubRestrictions`). The purpose enforcement matrix
    /// consults this via `check_pub_restriction(...)` and
    /// MAY override consent-vs-LI legal basis per vendor.
    /// **Stored as normalised inclusive vendor-id INTERVALS**, NOT
    /// one entry per vendor id. A single `RangeSection` with
    /// `[start=1, end=65535]` in the wire format produces exactly
    /// ONE `PublisherRestriction` entry with `vendor_range =
    /// 1..=65535`, not 65 535 entries. The cap is on ENCODED RANGE
    /// ENTRIES, not expanded cardinality.
    pub publisher_restrictions: Vec<PublisherRestriction>,
}

/// No `Copy` derive — `RangeInclusive<u16>` is not `Copy` in stable
/// Rust (its iterator state prevents it), and derive(Copy) would
/// fail to compile. `Clone` is enough for the `.iter()` walk in
/// `check_pub_restriction` below.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublisherRestriction {
    pub purpose_id: u8,
    pub restriction_type: PubRestrictionType,
    pub vendor_range: RangeInclusive<u16>,
}

impl PublisherRestriction {
    #[must_use]
    pub fn covers(&self, vendor_id: u16) -> bool {
        self.vendor_range.contains(&vendor_id)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PubRestrictionType {
    NotAllowed,
    RequireConsent,
    RequireLegitInterest,
}

impl TcfString {
    /// The IAB `CheckPubRestriction(purpose, type, vendor)` oracle.
    /// Returns true if the given restriction is declared for the
    /// given vendor/purpose combination.
    #[must_use]
    pub fn check_pub_restriction(
        &self,
        purpose_id: u8,
        restriction_type: PubRestrictionType,
        vendor_id: u16,
    ) -> bool {
        self.publisher_restrictions.iter().any(|r| {
            r.purpose_id == purpose_id
                && r.restriction_type == restriction_type
                && r.covers(vendor_id)
        })
    }

    /// Returns the first decoded restriction matching this (purpose, vendor).
    /// This is a record lookup, not an authorization decision: callers must
    /// detect conflicting types across records before selecting a legal basis.
    #[must_use]
    pub fn pub_restriction_for(
        &self,
        purpose_id: u8,
        vendor_id: u16,
    ) -> Option<PubRestrictionType> {
        self.publisher_restrictions
            .iter()
            .find(|r| r.purpose_id == purpose_id && r.covers(vendor_id))
            .map(|r| r.restriction_type)
    }
}

/// Which range-encoded section a range-validation error came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RangeSection {
    VendorConsent,
    VendorLegitimateInterest,
    /// The inner range list of a publisher-restriction record. These
    /// have NO section-level `MaxVendorId` (the field does not exist
    /// in TCF v2.3), so `VendorIdBeyondSegmentMax` never carries it.
    PublisherRestrictions,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TcfParseError {
    /// The core substring is not valid base64url (bad byte, an
    /// impossible length, or non-zero leftover bits in the final
    /// character).
    InvalidBase64,
    /// Either the WHOLE input exceeds [`MAX_TOTAL_INPUT_LEN`] bytes —
    /// checked first, before the input is split or walked — or the CORE
    /// substring exceeds [`MAX_INPUT_LEN`] bytes, checked after
    /// trailing-segment validation and before base64url decoding. Both
    /// carry the offending length.
    InputTooLong(usize),
    /// The decoded core exceeds [`MAX_DECODED_LEN`]. Defensive: a
    /// core within [`MAX_INPUT_LEN`] cannot decode past this.
    DecodedTooLong(usize),
    /// The core bitstream ended before the declared fields did.
    TruncatedCoreSegment,
    /// The core `Version` field is not 2.
    UnsupportedTcfVersion(u8),
    /// A structurally valid bit pattern with an impossible value — a
    /// 6-bit `PublisherCC` or `ConsentLanguage` letter outside `A..=Z`.
    InvalidBitfield,
    /// More than [`MAX_TRAILING_SEGMENTS`] trailing segments. Checked
    /// FIRST, before any per-segment content is inspected.
    TooManyTrailingSegments { actual: usize },
    /// An empty trailing segment (`..` or a trailing `.`).
    EmptyTrailingSegment,
    /// A trailing segment whose 3-bit type is neither 1 (Disclosed
    /// Vendors) nor 3 (Publisher TC). Type 0 in trailing position and
    /// the obsolete type 2 (Allowed Vendors / OOB — removed from the
    /// current format) land here.
    UnknownSegmentType { segment_type: u8 },
    /// A second type-1 segment, the moment the repeat is seen.
    DuplicateDisclosedVendorsSegment,
    /// A second type-3 segment, the moment the repeat is seen.
    DuplicatePublisherTcSegment,
    /// A type-1 segment that violates its per-segment bounds or its
    /// bitfield/range framing.
    MalformedDisclosedVendorsSegment { reason: &'static str },
    /// A type-3 segment that violates its per-segment bounds or the
    /// `57 + 2N`-bit Publisher TC layout.
    MalformedPublisherTcSegment { reason: &'static str },
    /// No type-1 segment present. The official TCF spec makes the
    /// Disclosed Vendors segment mandatory; checked LAST, after every
    /// trailing segment validated cleanly.
    MissingDisclosedVendorsSegment,
    /// `tcfPolicyVersion < 4` with `Created` on/after 2023-10-01 (the
    /// TCF technical specification's policy-v3 cutoff).
    PolicyVersionTooOldForCreated {
        created: OffsetDateTime,
        policy_version: u8,
    },
    /// `Created` and `LastUpdated` must hold the same value
    /// (IAB TCF v2.3 §5).
    CreatedLastUpdatedMismatch {
        created: OffsetDateTime,
        last_updated: OffsetDateTime,
    },
    /// A timestamp with a non-zero time-of-day component; `field` is
    /// `"created"` or `"lastUpdated"`.
    TimestampNotDayTruncated {
        field: &'static str,
        actual: OffsetDateTime,
    },
    /// A decoded timestamp outside the representable datetime range.
    /// Defensive: 36-bit deciseconds always land inside it.
    TimestampOutOfRange { field: &'static str },
    /// `IsServiceSpecific != 1`. Required unconditionally for a valid
    /// TCF v2.3 string, regardless of the publisher-restrictions
    /// section being empty.
    NotServiceSpecific,
    /// A non-zero bit inside the sub-byte terminal padding.
    NonZeroPadding,
    /// One or more WHOLE bytes beyond the last declared field —
    /// rejected regardless of their value.
    TrailingBytes,
    /// A range entry with a 0 endpoint. Zero is not a valid GVL
    /// vendor id; allowing it into any range would let a malformed
    /// string flip bit 0 in `VendorBitSet`, the sentinel that
    /// `purpose_allowed_basic(.., 0, ..)` relies on to deny bidders
    /// that lack a GVL id.
    InvalidVendorId { section: RangeSection, actual: u16 },
    /// A range entry ending past the section's declared
    /// `MaxVendorId`.
    VendorIdBeyondSegmentMax {
        section: RangeSection,
        actual: u16,
        max: u16,
    },
    /// A reversed range entry (`start > end`).
    RangeStartGreaterThanEnd {
        section: RangeSection,
        start: u16,
        end: u16,
    },
    /// Overlapping range entries within the same section.
    RangeEntriesOverlap { section: RangeSection },
    /// A range section declaring more than
    /// [`MAX_RANGE_ENCODED_ENTRIES`] entries. Defensive: the 12-bit
    /// wire field cannot carry more.
    TooManyRangeEntries { section: RangeSection, actual: u16 },
    /// `NumPubRestrictions` above
    /// [`MAX_PUB_RESTRICTION_ENCODED_ENTRIES`]. Defensive: the
    /// 12-bit wire field cannot carry more.
    TooManyPubRestrictions { actual: u16 },
    /// The decoded restriction list would exceed
    /// [`MAX_PUBLISHER_RESTRICTION_LIST_LEN`] objects.
    TooManyPublisherRestrictions,
    /// A restriction record's `PurposeId` outside 1..=24.
    InvalidPubRestrictionPurpose { actual: u8 },
    /// A restriction record with the reserved `RestrictionType` 3.
    UndefinedPubRestrictionType,
    /// A core `PurposeLITransparency` bit set for purposes 3..=6
    /// under `tcfPolicyVersion >= 4` (the TCF v2.2 prohibition; the
    /// bit is preserved under policy < 4).
    LiBitSetForConsentOnlyPurpose { purpose_id: u8 },
    /// The operation budget was exhausted before the parse finished.
    OpBudgetExceeded,
}

/// Running operation counter with a hard ceiling. Work is charged
/// BEFORE it happens (validate-before-mutate); on the charge that
/// would cross the ceiling, `used` clamps AT the ceiling and the
/// parse fails.
struct Budget {
    used: u64,
    ceiling: u64,
}

impl Budget {
    fn charge(&mut self, amount: u64) -> Result<(), TcfParseError> {
        let next = self.used.saturating_add(amount);
        if next > self.ceiling {
            self.used = self.ceiling;
            return Err(TcfParseError::OpBudgetExceeded);
        }
        self.used = next;
        Ok(())
    }
}

/// Why a bit-level read failed; callers map `Overrun` to their
/// segment's truncation error.
enum BitsError {
    Overrun,
    Budget,
}

/// MSB-first reader over a decoded segment. Every read charges its
/// width to the budget before touching the bytes.
struct BitReader<'bytes> {
    bytes: &'bytes [u8],
    bit_len: usize,
    cursor: usize,
}

impl<'bytes> BitReader<'bytes> {
    /// Callers cap `bytes` at [`MAX_DECODED_LEN`] first, so
    /// `len * 8` cannot overflow.
    fn new(bytes: &'bytes [u8]) -> Self {
        Self {
            bytes,
            bit_len: bytes.len().wrapping_mul(8),
            cursor: 0,
        }
    }

    fn remaining(&self) -> usize {
        self.bit_len.saturating_sub(self.cursor)
    }

    fn read_bits(&mut self, budget: &mut Budget, width: u32) -> Result<u64, BitsError> {
        if budget.charge(u64::from(width)).is_err() {
            return Err(BitsError::Budget);
        }
        // `u32` always fits `usize` on supported targets; the MAX
        // fallback fails safe as an overrun.
        let width_usize = usize::try_from(width).unwrap_or(usize::MAX);
        let end = match self.cursor.checked_add(width_usize) {
            Some(end) if end <= self.bit_len => end,
            Some(_) | None => return Err(BitsError::Overrun),
        };
        let mut value = 0_u64;
        for idx in self.cursor..end {
            let byte = self.bytes.get(idx.wrapping_div(8)).copied().unwrap_or(0);
            let shift = u32::try_from(7_usize.wrapping_sub(idx.wrapping_rem(8))).unwrap_or(0);
            value = value.wrapping_shl(1) | u64::from(byte.wrapping_shr(shift) & 1);
        }
        self.cursor = end;
        Ok(value)
    }

    /// Advances past `count` bits whose values are not retained
    /// (bitfield completeness checks). The charge still covers every
    /// skipped bit.
    fn skip_bits(&mut self, budget: &mut Budget, count: usize) -> Result<(), BitsError> {
        if budget
            .charge(u64::try_from(count).unwrap_or(u64::MAX))
            .is_err()
        {
            return Err(BitsError::Budget);
        }
        let end = match self.cursor.checked_add(count) {
            Some(end) if end <= self.bit_len => end,
            Some(_) | None => return Err(BitsError::Overrun),
        };
        self.cursor = end;
        Ok(())
    }

    /// Preserve the DV bitfield's all-at-once charge and truncation
    /// check before reading or mutating any membership bits.
    fn read_vendor_bitfield(
        &mut self,
        budget: &mut Budget,
        max_vendor_id: u16,
        set: &mut VendorBitSet,
    ) -> Result<(), BitsError> {
        let start = self.cursor;
        self.skip_bits(budget, usize::from(max_vendor_id))?;
        for id in 1..=max_vendor_id {
            let idx = start.wrapping_add(usize::from(id).wrapping_sub(1));
            let byte = self.bytes.get(idx.wrapping_div(8)).copied().unwrap_or(0);
            let shift = u32::try_from(7_usize.wrapping_sub(idx.wrapping_rem(8))).unwrap_or(0);
            if byte.wrapping_shr(shift) & 1 == 1 {
                set.set(id);
            }
        }
        Ok(())
    }
}

/// Why a base64url decode failed. `Parse` carries a budget failure
/// through unchanged; the other arms map per calling context (core →
/// `InvalidBase64`, trailing segment → `Malformed*Segment`).
enum DecodeFail {
    Parse(TcfParseError),
    /// A byte outside the base64url alphabet (`=` padding included —
    /// TC strings are unpadded).
    Char,
    /// `len % 4 == 1`: a lone trailing character cannot encode a
    /// byte.
    Length,
    /// Non-zero leftover bits in the final character.
    Leftover,
}

const fn b64url_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte.wrapping_sub(b'A')),
        b'a'..=b'z' => Some(byte.wrapping_sub(b'a').wrapping_add(26)),
        b'0'..=b'9' => Some(byte.wrapping_sub(b'0').wrapping_add(52)),
        b'-' => Some(62),
        b'_' => Some(63),
        _ => None,
    }
}

/// Bounded base64url decode. Callers enforce the raw-length cap
/// first, so the single up-front reservation is bounded by
/// [`MAX_DECODED_LEN`]; one op is charged per input byte.
fn decode_base64url(segment: &str, budget: &mut Budget) -> Result<Vec<u8>, DecodeFail> {
    let raw = segment.as_bytes();
    if raw.len().wrapping_rem(4) == 1 {
        return Err(DecodeFail::Length);
    }
    let mut out = Vec::with_capacity(raw.len().wrapping_mul(6).wrapping_div(8));
    let mut acc = 0_u32;
    let mut acc_bits = 0_u32;
    for &byte in raw {
        budget.charge(1).map_err(DecodeFail::Parse)?;
        let Some(value) = b64url_value(byte) else {
            return Err(DecodeFail::Char);
        };
        acc = acc.wrapping_shl(6) | u32::from(value);
        acc_bits = acc_bits.wrapping_add(6);
        if acc_bits >= 8 {
            acc_bits = acc_bits.wrapping_sub(8);
            let byte_out = u8::try_from(acc.wrapping_shr(acc_bits) & 0xFF).unwrap_or(0);
            out.push(byte_out);
        }
    }
    let leftover_mask = 1_u32.wrapping_shl(acc_bits).wrapping_sub(1);
    if acc & leftover_mask != 0 {
        return Err(DecodeFail::Leftover);
    }
    Ok(out)
}

/// ≤ 16-bit reads always fit; the 0 fallback fails closed.
fn to_u16(value: u64) -> u16 {
    u16::try_from(value).unwrap_or(0)
}

/// ≤ 8-bit reads always fit; the 0 fallback fails closed.
fn to_u8(value: u64) -> u8 {
    u8::try_from(value).unwrap_or(0)
}

fn core_read(r: &mut BitReader<'_>, budget: &mut Budget, width: u32) -> Result<u64, TcfParseError> {
    match r.read_bits(budget, width) {
        Ok(value) => Ok(value),
        Err(BitsError::Overrun) => Err(TcfParseError::TruncatedCoreSegment),
        Err(BitsError::Budget) => Err(TcfParseError::OpBudgetExceeded),
    }
}

fn dv_malformed(reason: &'static str) -> TcfParseError {
    TcfParseError::MalformedDisclosedVendorsSegment { reason }
}

fn dv_read(r: &mut BitReader<'_>, budget: &mut Budget, width: u32) -> Result<u64, TcfParseError> {
    match r.read_bits(budget, width) {
        Ok(value) => Ok(value),
        Err(BitsError::Overrun) => Err(dv_malformed("truncated segment")),
        Err(BitsError::Budget) => Err(TcfParseError::OpBudgetExceeded),
    }
}

fn pub_tc_malformed(reason: &'static str) -> TcfParseError {
    TcfParseError::MalformedPublisherTcSegment { reason }
}

fn pub_tc_read(
    r: &mut BitReader<'_>,
    budget: &mut Budget,
    width: u32,
) -> Result<u64, TcfParseError> {
    match r.read_bits(budget, width) {
        Ok(value) => Ok(value),
        Err(BitsError::Overrun) => Err(pub_tc_malformed("truncated segment")),
        Err(BitsError::Budget) => Err(TcfParseError::OpBudgetExceeded),
    }
}

/// Terminal-padding rule shared by every segment: after the last
/// declared field, `remaining >= 8` whole bits are smuggled data
/// (rejected regardless of value) and a partial final byte must be
/// all-zero — the minimum padding needed for byte alignment is the
/// only legal leftover.
fn check_segment_tail(
    r: &mut BitReader<'_>,
    budget: &mut Budget,
    whole_bytes: TcfParseError,
    non_zero_padding: TcfParseError,
) -> Result<(), TcfParseError> {
    let remaining = r.remaining();
    if remaining >= 8 {
        return Err(whole_bytes);
    }
    if remaining > 0 {
        // remaining ≤ 7 is in bounds, so only a budget failure can
        // surface from the read.
        let padding = match r.read_bits(budget, u32::try_from(remaining).unwrap_or(0)) {
            Ok(value) => value,
            Err(BitsError::Overrun | BitsError::Budget) => {
                return Err(TcfParseError::OpBudgetExceeded);
            }
        };
        if padding != 0 {
            return Err(non_zero_padding);
        }
    }
    Ok(())
}

/// Interval collector for overlap detection within one range section.
/// Bounded by the section's entry cap (≤ 4 095 four-byte pairs), whose
/// check runs before the first `push`, so the capacity is reserved once
/// and never grows.
///
/// Collect-then-`finalize` rather than insert-in-order: an ordered
/// tracker charges each entry the cost of the `Vec::insert` shift it
/// causes, which makes the charge — and therefore whether a string
/// parses at all — depend on the ENCODING ORDER of otherwise identical
/// entries. The IAB TCF v2 binary format imposes no ordering on range
/// entries, so a CMP that emits them descending encodes exactly as
/// valid a string as one that emits them ascending, and rejecting the
/// first is a wrong answer no matter which direction it fails in.
/// Appending every entry and sorting once at section end costs
/// `n + n·log2(n) + n` regardless of the order they arrived in.
struct SpanTracker(Vec<(u16, u16)>);

impl SpanTracker {
    /// `capacity` is the section's declared entry count, already
    /// checked against the section cap by the caller.
    fn with_capacity(capacity: u16) -> Self {
        Self(Vec::with_capacity(usize::from(capacity)))
    }

    /// Records `start..=end` for the section-end overlap check. Charges
    /// one op — the append — and nothing else; ordering work is charged
    /// by [`Self::finalize`].
    fn push(&mut self, start: u16, end: u16, budget: &mut Budget) -> Result<(), TcfParseError> {
        budget.charge(1)?;
        self.0.push((start, end));
        Ok(())
    }

    /// Sorts the collected spans and reports whether they are pairwise
    /// disjoint; `false` means the section overlaps somewhere.
    ///
    /// The sort and the adjacent-pair scan are charged BEFORE the sort
    /// runs, so the operation ceiling still bounds the work actually
    /// performed rather than merely the entry count.
    ///
    /// Behavioural note: because the verdict is computed once at
    /// section end, an overlap surfaces AFTER the whole section has
    /// been read, not at the offending entry. The error variant and
    /// message are unchanged; only the position in the stream at which
    /// the parse stops differs.
    fn finalize(&mut self, budget: &mut Budget) -> Result<bool, TcfParseError> {
        let len = u64::try_from(self.0.len()).unwrap_or(u64::MAX);
        // A section of 0 or 1 spans can neither sort nor overlap, and
        // `ilog2` is undefined at 0 — both return early, un-charged.
        let Some(log2) = len.checked_ilog2().filter(|_| len >= 2) else {
            return Ok(true);
        };
        // n·ceil(log2 n) for the sort plus n for the adjacent-pair
        // scan. `ilog2` floors, so round up: undercharging is what
        // would let the ceiling stop bounding the sort.
        let ceil_log2 = u64::from(log2).saturating_add(1);
        budget.charge(len.saturating_mul(ceil_log2).saturating_add(len))?;
        self.0.sort_unstable();
        Ok(self
            .0
            .windows(2)
            .all(|pair| match (pair.first(), pair.get(1)) {
                // Sorted by `(start, end)`, so the next span starts at
                // or after this one; they are disjoint exactly when it
                // starts strictly after this one ends.
                (Some(&(_, end)), Some(&(next_start, _))) => next_start > end,
                // `windows(2)` always yields pairs; unreachable, and
                // fail-closed if it ever were not.
                _ => false,
            }))
    }
}

/// Zero-endpoint and reversed-range validation shared by every range
/// section. The `MaxVendorId` bound is NOT here: publisher-restriction
/// inner ranges have no section-level maximum.
fn validate_range_entry(section: RangeSection, start: u16, end: u16) -> Result<(), TcfParseError> {
    if start == 0 || end == 0 {
        return Err(TcfParseError::InvalidVendorId { section, actual: 0 });
    }
    if start > end {
        return Err(TcfParseError::RangeStartGreaterThanEnd {
            section,
            start,
            end,
        });
    }
    Ok(())
}

/// Decodes one core vendor section (consent or LI) into `set`.
fn parse_vendor_section(
    r: &mut BitReader<'_>,
    budget: &mut Budget,
    set: &mut VendorBitSet,
    section: RangeSection,
) -> Result<(), TcfParseError> {
    let max_vendor_id = to_u16(core_read(r, budget, 16)?);
    if core_read(r, budget, 1)? == 1 {
        let num_entries = to_u16(core_read(r, budget, 12)?);
        if num_entries > MAX_RANGE_ENCODED_ENTRIES {
            return Err(TcfParseError::TooManyRangeEntries {
                section,
                actual: num_entries,
            });
        }
        let mut spans = SpanTracker::with_capacity(num_entries);
        for _ in 0..num_entries {
            let is_span = core_read(r, budget, 1)? == 1;
            let start = to_u16(core_read(r, budget, 16)?);
            let end = if is_span {
                to_u16(core_read(r, budget, 16)?)
            } else {
                start
            };
            validate_range_entry(section, start, end)?;
            if end > max_vendor_id {
                return Err(TcfParseError::VendorIdBeyondSegmentMax {
                    section,
                    actual: end,
                    max: max_vendor_id,
                });
            }
            // Validate-before-mutate: the word walk is charged (and
            // the ceiling checked) before `set_range` runs.
            budget.charge(word_count(start, end))?;
            // The tracker charges its own append.
            spans.push(start, end, budget)?;
            set.set_range(start, end);
        }
        if !spans.finalize(budget)? {
            return Err(TcfParseError::RangeEntriesOverlap { section });
        }
    } else {
        for id in 1..=max_vendor_id {
            if core_read(r, budget, 1)? == 1 {
                set.set(id);
            }
        }
    }
    Ok(())
}

/// Decodes the core publisher-restrictions section into the bounded
/// object list.
fn parse_pub_restrictions(
    r: &mut BitReader<'_>,
    budget: &mut Budget,
) -> Result<Vec<PublisherRestriction>, TcfParseError> {
    let declared = to_u16(core_read(r, budget, 12)?);
    if declared > MAX_PUB_RESTRICTION_ENCODED_ENTRIES {
        return Err(TcfParseError::TooManyPubRestrictions { actual: declared });
    }
    let mut restrictions = Vec::new();
    for _ in 0..declared {
        let purpose_id = to_u8(core_read(r, budget, 6)?);
        if purpose_id == 0 || purpose_id > 24 {
            return Err(TcfParseError::InvalidPubRestrictionPurpose { actual: purpose_id });
        }
        let restriction_type = match core_read(r, budget, 2)? {
            0 => PubRestrictionType::NotAllowed,
            1 => PubRestrictionType::RequireConsent,
            2 => PubRestrictionType::RequireLegitInterest,
            _ => return Err(TcfParseError::UndefinedPubRestrictionType),
        };
        let num_entries = to_u16(core_read(r, budget, 12)?);
        if num_entries > MAX_RANGE_ENCODED_ENTRIES {
            return Err(TcfParseError::TooManyRangeEntries {
                section: RangeSection::PublisherRestrictions,
                actual: num_entries,
            });
        }
        let mut spans = SpanTracker::with_capacity(num_entries);
        for _ in 0..num_entries {
            let is_span = core_read(r, budget, 1)? == 1;
            let start = to_u16(core_read(r, budget, 16)?);
            let end = if is_span {
                to_u16(core_read(r, budget, 16)?)
            } else {
                start
            };
            validate_range_entry(RangeSection::PublisherRestrictions, start, end)?;
            spans.push(start, end, budget)?;
            if restrictions.len() >= MAX_PUBLISHER_RESTRICTION_LIST_LEN {
                return Err(TcfParseError::TooManyPublisherRestrictions);
            }
            budget.charge(1)?; // the decoded-list push
            restrictions.push(PublisherRestriction {
                purpose_id,
                restriction_type,
                vendor_range: start..=end,
            });
        }
        if !spans.finalize(budget)? {
            return Err(TcfParseError::RangeEntriesOverlap {
                section: RangeSection::PublisherRestrictions,
            });
        }
    }
    Ok(restrictions)
}

fn decis_to_datetime(
    deciseconds: u64,
    field: &'static str,
) -> Result<OffsetDateTime, TcfParseError> {
    // A 36-bit decisecond count times 1e8 is far below `i128::MAX`,
    // so the wrapping multiply cannot wrap.
    let nanos = i128::from(deciseconds).wrapping_mul(100_000_000);
    OffsetDateTime::from_unix_timestamp_nanos(nanos)
        .map_err(|_source| TcfParseError::TimestampOutOfRange { field })
}

/// Both core timestamps must be UTC-day-truncated: encoders that
/// respect the spec always emit day-truncated deciseconds.
fn require_day_truncated(value: OffsetDateTime, field: &'static str) -> Result<(), TcfParseError> {
    if value.time() == Time::MIDNIGHT {
        Ok(())
    } else {
        Err(TcfParseError::TimestampNotDayTruncated {
            field,
            actual: value,
        })
    }
}

/// Reads a two-letter code (2 × 6-bit alphabet positions) as ASCII
/// uppercase bytes.
fn read_country_code(r: &mut BitReader<'_>, budget: &mut Budget) -> Result<[u8; 2], TcfParseError> {
    let hi = to_u8(core_read(r, budget, 6)?);
    let lo = to_u8(core_read(r, budget, 6)?);
    if hi > 25 || lo > 25 {
        return Err(TcfParseError::InvalidBitfield);
    }
    Ok([b'A'.wrapping_add(hi), b'A'.wrapping_add(lo)])
}

/// Reads a `width`-position id bitfield (position 1 = first bit) into
/// a set of the positions that are on.
fn read_id_set(
    r: &mut BitReader<'_>,
    budget: &mut Budget,
    width: u8,
) -> Result<BTreeSet<u8>, TcfParseError> {
    let mut set = BTreeSet::new();
    for id in 1..=width {
        if core_read(r, budget, 1)? == 1 {
            set.insert(id);
        }
    }
    Ok(set)
}

/// Validates a trailing Disclosed Vendors segment and retains its
/// bounded membership during the same walk.
fn validate_dv_framing(
    segment: &str,
    budget: &mut Budget,
    set: &mut VendorBitSet,
) -> Result<(), TcfParseError> {
    if segment.len() > MAX_INPUT_LEN {
        return Err(dv_malformed("segment longer than the 64 KiB raw cap"));
    }
    let bytes = match decode_base64url(segment, budget) {
        Ok(bytes) => bytes,
        Err(DecodeFail::Parse(err)) => return Err(err),
        Err(DecodeFail::Char) => return Err(dv_malformed("invalid base64url byte")),
        Err(DecodeFail::Length) => return Err(dv_malformed("truncated base64url quantum")),
        Err(DecodeFail::Leftover) => {
            return Err(dv_malformed("non-zero base64url leftover bits"));
        }
    };
    if bytes.len() > MAX_DECODED_LEN {
        return Err(dv_malformed("segment longer than the 48 KiB decoded cap"));
    }
    let mut r = BitReader::new(&bytes);
    let _segment_type: u64 = dv_read(&mut r, budget, 3)?;
    let max_vendor_id = to_u16(dv_read(&mut r, budget, 16)?);
    if dv_read(&mut r, budget, 1)? == 1 {
        let num_entries = to_u16(dv_read(&mut r, budget, 12)?);
        if num_entries > MAX_RANGE_ENCODED_ENTRIES {
            return Err(dv_malformed("range entry count above the 4095 cap"));
        }
        let mut spans = SpanTracker::with_capacity(num_entries);
        for _ in 0..num_entries {
            let is_span = dv_read(&mut r, budget, 1)? == 1;
            let start = to_u16(dv_read(&mut r, budget, 16)?);
            let end = if is_span {
                to_u16(dv_read(&mut r, budget, 16)?)
            } else {
                start
            };
            if start == 0 || end == 0 {
                return Err(dv_malformed("vendor id 0 in a range entry"));
            }
            if start > end {
                return Err(dv_malformed("range start greater than end"));
            }
            if end > max_vendor_id {
                return Err(dv_malformed("vendor id beyond the declared MaxVendorId"));
            }
            budget.charge(word_count(start, end))?;
            spans.push(start, end, budget)?;
            set.set_range(start, end);
        }
        // Same disjointness discipline the core vendor sections and
        // publisher restrictions enforce: a range section names each
        // vendor at most once, so an overlap (or a repeated single id)
        // is a malformed segment. Disclosed Vendors is mandatory, so
        // accepting a malformed one here would leave core consent usable
        // off a string that never decoded cleanly.
        if !spans.finalize(budget)? {
            return Err(dv_malformed("range entries overlap"));
        }
    } else {
        match r.read_vendor_bitfield(budget, max_vendor_id, set) {
            Ok(()) => {}
            Err(BitsError::Overrun) => return Err(dv_malformed("truncated segment")),
            Err(BitsError::Budget) => return Err(TcfParseError::OpBudgetExceeded),
        }
    }
    check_segment_tail(
        &mut r,
        budget,
        dv_malformed("whole trailing bytes after the declared fields"),
        dv_malformed("non-zero padding bits"),
    )
}

/// Framing validation for a trailing Publisher TC segment
/// (`SegmentType(3) + PubPurposesConsent(24) +
/// PubPurposesLITransparency(24) + NumCustomPurposes(6) + 2 × N
/// custom bits`). Contents are validated then discarded; publisher-TC
/// enforcement is a later capability.
fn validate_publisher_tc_framing(segment: &str, budget: &mut Budget) -> Result<(), TcfParseError> {
    if segment.len() > MAX_INPUT_LEN {
        return Err(pub_tc_malformed("segment longer than the 64 KiB raw cap"));
    }
    let bytes = match decode_base64url(segment, budget) {
        Ok(bytes) => bytes,
        Err(DecodeFail::Parse(err)) => return Err(err),
        Err(DecodeFail::Char) => return Err(pub_tc_malformed("invalid base64url byte")),
        Err(DecodeFail::Length) => return Err(pub_tc_malformed("truncated base64url quantum")),
        Err(DecodeFail::Leftover) => {
            return Err(pub_tc_malformed("non-zero base64url leftover bits"));
        }
    };
    if bytes.len() > MAX_DECODED_LEN {
        return Err(pub_tc_malformed(
            "segment longer than the 48 KiB decoded cap",
        ));
    }
    let mut r = BitReader::new(&bytes);
    let _segment_type: u64 = pub_tc_read(&mut r, budget, 3)?;
    let _pub_purposes_consent: u64 = pub_tc_read(&mut r, budget, 24)?;
    let _pub_purposes_li_transparency: u64 = pub_tc_read(&mut r, budget, 24)?;
    let num_custom_purposes = to_u8(pub_tc_read(&mut r, budget, 6)?);
    match r.skip_bits(budget, usize::from(num_custom_purposes).wrapping_mul(2)) {
        Ok(()) => {}
        Err(BitsError::Overrun) => return Err(pub_tc_malformed("truncated segment")),
        Err(BitsError::Budget) => return Err(TcfParseError::OpBudgetExceeded),
    }
    check_segment_tail(
        &mut r,
        budget,
        pub_tc_malformed("whole trailing bytes after the declared fields"),
        pub_tc_malformed("non-zero padding bits"),
    )
}

/// Parses the decoded core bitstream into a [`TcfString`]. The
/// vendor bitsets are built in the caller-owned slots so a budget
/// failure leaves the partially filled state observable
/// (validate-before-mutate proof); on success they move into the
/// returned value.
fn parse_core(
    bytes: &[u8],
    budget: &mut Budget,
    vendor_consent: &mut VendorBitSet,
    vendor_li: &mut VendorBitSet,
    disclosed_vendors: &mut VendorBitSet,
) -> Result<TcfString, TcfParseError> {
    let mut r = BitReader::new(bytes);
    let version = to_u8(core_read(&mut r, budget, 6)?);
    if version != 2 {
        return Err(TcfParseError::UnsupportedTcfVersion(version));
    }
    let created = decis_to_datetime(core_read(&mut r, budget, 36)?, "created")?;
    let last_updated = decis_to_datetime(core_read(&mut r, budget, 36)?, "lastUpdated")?;
    require_day_truncated(created, "created")?;
    require_day_truncated(last_updated, "lastUpdated")?;
    if created != last_updated {
        return Err(TcfParseError::CreatedLastUpdatedMismatch {
            created,
            last_updated,
        });
    }
    let cmp_id = to_u16(core_read(&mut r, budget, 12)?);
    let cmp_version = to_u16(core_read(&mut r, budget, 12)?);
    let _consent_screen: u64 = core_read(&mut r, budget, 6)?;
    // ConsentLanguage is two 6-bit alphabet positions, same encoding as
    // PublisherCC, so it takes the same validating reader: a position
    // above 25 is not a letter and the string is malformed. Nothing
    // downstream stores the language, but an unchecked read would let a
    // malformed string decode into usable consent.
    let _consent_language: [u8; 2] = read_country_code(&mut r, budget)?;
    let vendor_list_version = to_u16(core_read(&mut r, budget, 12)?);
    let tcf_policy_version = to_u8(core_read(&mut r, budget, 6)?);
    if tcf_policy_version < 4 && created >= POLICY_V3_CREATED_CUTOFF {
        return Err(TcfParseError::PolicyVersionTooOldForCreated {
            created,
            policy_version: tcf_policy_version,
        });
    }
    if core_read(&mut r, budget, 1)? != 1 {
        return Err(TcfParseError::NotServiceSpecific);
    }
    let _use_non_standard_stacks: u64 = core_read(&mut r, budget, 1)?;
    let special_feature_optins = read_id_set(&mut r, budget, 12)?;
    let purposes_consent = read_id_set(&mut r, budget, 24)?;
    let purposes_li_transparency = read_id_set(&mut r, budget, 24)?;
    if tcf_policy_version >= 4 {
        for purpose_id in 3..=6_u8 {
            if purposes_li_transparency.contains(&purpose_id) {
                return Err(TcfParseError::LiBitSetForConsentOnlyPurpose { purpose_id });
            }
        }
    }
    let purpose_one_treatment = core_read(&mut r, budget, 1)? == 1;
    let publisher_cc = read_country_code(&mut r, budget)?;
    parse_vendor_section(&mut r, budget, vendor_consent, RangeSection::VendorConsent)?;
    parse_vendor_section(
        &mut r,
        budget,
        vendor_li,
        RangeSection::VendorLegitimateInterest,
    )?;
    let publisher_restrictions = parse_pub_restrictions(&mut r, budget)?;
    check_segment_tail(
        &mut r,
        budget,
        TcfParseError::TrailingBytes,
        TcfParseError::NonZeroPadding,
    )?;
    Ok(TcfString {
        tcf_policy_version,
        vendor_list_version,
        cmp_id,
        cmp_version,
        created,
        last_updated,
        purposes_consent,
        purposes_li_transparency,
        vendor_consent: mem::take(vendor_consent),
        vendor_li: mem::take(vendor_li),
        disclosed_vendors: mem::take(disclosed_vendors),
        special_feature_optins,
        purpose_one_treatment,
        publisher_cc,
        publisher_restrictions,
    })
}

/// Reusable TCF v2 parser with an injectable operation ceiling. The
/// counter and the partially built vendor-consent and disclosure sets stay
/// observable after a failed parse so the validate-before-mutate
/// contract is provable from tests.
pub struct TcfParser {
    budget: Budget,
    vendor_consent: VendorBitSet,
    vendor_li: VendorBitSet,
    disclosed_vendors: VendorBitSet,
}

impl Default for TcfParser {
    fn default() -> Self {
        Self::new()
    }
}

impl TcfParser {
    /// Parser with [`DEFAULT_OP_CEILING`].
    #[must_use]
    pub fn new() -> Self {
        Self::with_op_ceiling(DEFAULT_OP_CEILING)
    }

    /// Parser with an injected operation ceiling (regression tests
    /// use a low ceiling to prove work stops at the budget).
    #[must_use]
    pub fn with_op_ceiling(op_ceiling: u64) -> Self {
        Self {
            budget: Budget {
                used: 0,
                ceiling: op_ceiling,
            },
            vendor_consent: VendorBitSet::new(),
            vendor_li: VendorBitSet::new(),
            disclosed_vendors: VendorBitSet::new(),
        }
    }

    /// Operations charged by the most recent [`Self::parse`] call.
    /// Clamps AT the ceiling when the budget was exhausted.
    #[must_use]
    pub fn ops_used(&self) -> u64 {
        self.budget.used
    }

    /// The vendor-consent bitset state left by the most recent
    /// [`Self::parse`] call: partially filled after a mid-section
    /// failure, empty after a success (the set moves into the
    /// returned [`TcfString`]).
    #[must_use]
    pub fn partial_vendor_consent(&self) -> &VendorBitSet {
        &self.vendor_consent
    }

    /// The disclosure bitset left by the most recent parse: possibly
    /// partial after failure, empty after success. Diagnostic state
    /// only, never usable consent; reset before every parse.
    #[must_use]
    pub fn partial_disclosed_vendors(&self) -> &VendorBitSet {
        &self.disclosed_vendors
    }

    /// Parses one TC string. See the module docs for the exact
    /// processing order over the raw input.
    ///
    /// # Errors
    ///
    /// Returns the FIRST defect in the deterministic order: the
    /// trailing-segment count, then each trailing segment left to
    /// right (empty → type → duplicate → framing), then the
    /// missing-Disclosed-Vendors check, then the core caps, decode,
    /// and content rules in wire order. All failures are fail-closed:
    /// consent becomes unusable, never a 400.
    pub fn parse(&mut self, input: &str) -> Result<TcfString, TcfParseError> {
        self.budget.used = 0;
        self.vendor_consent = VendorBitSet::new();
        self.vendor_li = VendorBitSet::new();
        self.disclosed_vendors = VendorBitSet::new();
        // The whole-input cap runs before the split: the per-segment
        // caps below apply only once the input has been walked, so
        // without this a multi-megabyte suffix would be scanned before
        // anything rejected it. A legal string is a core plus at most
        // MAX_TRAILING_SEGMENTS segments, each within MAX_INPUT_LEN,
        // plus their separating dots, so anything longer cannot parse.
        if input.len() > MAX_TOTAL_INPUT_LEN {
            return Err(TcfParseError::InputTooLong(input.len()));
        }
        let (core, trailing) = match input.split_once('.') {
            Some((head, tail)) => (head, Some(tail)),
            None => (input, None),
        };
        if let Some(tail) = trailing {
            // `take` short-circuits: one segment past the cap already
            // proves the string over-segmented, so a dot flood is
            // rejected without counting every dot in it.
            let actual = tail
                .split('.')
                .take(MAX_TRAILING_SEGMENTS.saturating_add(1))
                .count();
            if actual > MAX_TRAILING_SEGMENTS {
                return Err(TcfParseError::TooManyTrailingSegments { actual });
            }
        }
        let mut seen_dv = false;
        let mut seen_pub_tc = false;
        if let Some(tail) = trailing {
            for segment in tail.split('.') {
                let Some(&first) = segment.as_bytes().first() else {
                    return Err(TcfParseError::EmptyTrailingSegment);
                };
                let Some(type_code) = b64url_value(first) else {
                    return Err(TcfParseError::InvalidBase64);
                };
                match type_code.wrapping_shr(3) {
                    1 => {
                        if seen_dv {
                            return Err(TcfParseError::DuplicateDisclosedVendorsSegment);
                        }
                        seen_dv = true;
                        validate_dv_framing(
                            segment,
                            &mut self.budget,
                            &mut self.disclosed_vendors,
                        )?;
                    }
                    3 => {
                        if seen_pub_tc {
                            return Err(TcfParseError::DuplicatePublisherTcSegment);
                        }
                        seen_pub_tc = true;
                        validate_publisher_tc_framing(segment, &mut self.budget)?;
                    }
                    segment_type => {
                        return Err(TcfParseError::UnknownSegmentType { segment_type });
                    }
                }
            }
        }
        if !seen_dv {
            return Err(TcfParseError::MissingDisclosedVendorsSegment);
        }
        if core.len() > MAX_INPUT_LEN {
            return Err(TcfParseError::InputTooLong(core.len()));
        }
        let decoded = match decode_base64url(core, &mut self.budget) {
            Ok(bytes) => bytes,
            Err(DecodeFail::Parse(err)) => return Err(err),
            Err(DecodeFail::Char | DecodeFail::Length | DecodeFail::Leftover) => {
                return Err(TcfParseError::InvalidBase64);
            }
        };
        if decoded.len() > MAX_DECODED_LEN {
            return Err(TcfParseError::DecodedTooLong(decoded.len()));
        }
        parse_core(
            &decoded,
            &mut self.budget,
            &mut self.vendor_consent,
            &mut self.vendor_li,
            &mut self.disclosed_vendors,
        )
    }
}

/// Parses a TCF v2 TC string with the default operation ceiling.
///
/// # Errors
///
/// See [`TcfParser::parse`].
pub fn parse_tcf_v2(input: &str) -> Result<TcfString, TcfParseError> {
    TcfParser::new().parse(input)
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;

    use auctioneer_config::types::{
        EnforceAlgo, PurposeEnforcement, SpecialFeatureEnforcement, TriState,
    };
    use time::macros::datetime;

    use crate::testkit::{VendorSpec, gvl};
    use crate::{AuctionPermissions, BidderDecision, FetchDenial, PrivacyContext};

    use super::{
        MAX_PUBLISHER_RESTRICTION_LIST_LEN, PubRestrictionType, RangeSection, TcfParseError,
        TcfParser, VendorBitSet, parse_tcf_v2,
    };
    use crate::test_support::tc_encode::{
        BitWriter, CoreSpec, PubRestrictionSpec, RangeEntrySpec, VendorSectionSpec,
        dv_bitfield_segment, encode_core, minimal_dv_segment, valid_core_spec, with_minimal_dv,
        write_core,
    };
    use crate::test_support::tc_encode_segments::{dv_range_segment, publisher_tc_segment};

    /// Deciseconds since the UNIX epoch, all UTC-day-truncated:
    /// 2023-06-01 (policy-v3 era), 2023-09-30 (last valid policy-v3
    /// day), 2023-10-01 (the cutoff), 2023-10-15 and 2023-11-19 (the
    /// disputed FAQ grace window).
    const DS_2023_06_01: u64 = 16_855_776_000;
    const DS_2023_09_30: u64 = 16_960_320_000;
    const DS_2023_10_01: u64 = 16_961_184_000;
    const DS_2023_10_15: u64 = 16_973_280_000;
    const DS_2023_11_19: u64 = 17_003_520_000;

    fn fixture(name: &str) -> String {
        let path = format!("{}/tests/data/tcf_v2/{name}", env!("CARGO_MANIFEST_DIR"));
        fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("fixture {path} unreadable: {err}"))
            .trim_end()
            .to_owned()
    }

    fn assert_same_bidder_decision(actual: BidderDecision, expected: BidderDecision, case: &str) {
        // Exhaustive destructuring keeps new decision fields in this regression.
        let fields = |BidderDecision {
                          fetch_denial,
                          gdpr,
                          transmit_ufpd,
                          transmit_precise_geo,
                          transmit_tid,
                          ccpa,
                          lmt,
                          coppa,
                      }| {
            (
                fetch_denial,
                gdpr,
                transmit_ufpd,
                transmit_precise_geo,
                transmit_tid,
                ccpa,
                lmt,
                coppa,
            )
        };
        assert_eq!(fields(actual), fields(expected), "{case}");
    }

    fn full_consent_spec() -> CoreSpec {
        let mut spec = valid_core_spec();
        spec.purposes_consent = (1..=11).collect();
        spec.special_feature_optins = vec![1];
        spec.vendor_consent = VendorSectionSpec::Range {
            max_vendor_id: 6_000,
            entries: vec![RangeEntrySpec::Span(1, 6_000)],
        };
        spec
    }

    fn pub_restrictions_spec() -> CoreSpec {
        let mut spec = valid_core_spec();
        spec.vendor_consent = VendorSectionSpec::Bitfield {
            max_vendor_id: 900,
            set: vec![32, 887],
        };
        spec.pub_restrictions = vec![
            PubRestrictionSpec {
                purpose_id: 2,
                restriction_type: 0,
                entries: vec![RangeEntrySpec::Single(32)],
            },
            PubRestrictionSpec {
                purpose_id: 2,
                restriction_type: 1,
                entries: vec![RangeEntrySpec::Single(887)],
            },
            PubRestrictionSpec {
                purpose_id: 7,
                restriction_type: 2,
                entries: vec![RangeEntrySpec::Span(32, 40)],
            },
        ];
        spec
    }

    fn policy3_li4_spec() -> CoreSpec {
        let mut spec = valid_core_spec();
        spec.tcf_policy_version = 3;
        spec.created_ds = DS_2023_06_01;
        spec.last_updated_ds = DS_2023_06_01;
        spec.purposes_li_transparency = vec![4];
        spec
    }

    fn policy3_spec(created_ds: u64) -> CoreSpec {
        let mut spec = valid_core_spec();
        spec.tcf_policy_version = 3;
        spec.created_ds = created_ds;
        spec.last_updated_ds = created_ds;
        spec
    }

    #[test]
    fn valid_purpose_1_only_parses_purpose_1_and_no_vendors() {
        let raw = fixture("valid_purpose_1_only.txt");
        assert_eq!(raw, with_minimal_dv(&encode_core(&valid_core_spec())));
        let tcf = parse_tcf_v2(&raw).expect("valid fixture");
        assert_eq!(tcf.purposes_consent, BTreeSet::from([1]));
        assert!(tcf.purposes_li_transparency.is_empty());
        for vendor in [1_u16, 2, 100, u16::MAX] {
            assert!(!tcf.vendor_consent.contains(vendor));
        }
    }

    #[test]
    fn valid_full_consent_parses_all_purposes() {
        let raw = fixture("valid_full_consent.txt");
        assert_eq!(raw, with_minimal_dv(&encode_core(&full_consent_spec())));
        let tcf = parse_tcf_v2(&raw).expect("valid fixture");
        assert_eq!(tcf.purposes_consent, (1..=11).collect());
        assert!(tcf.special_feature_optins.contains(&1));
        assert!(tcf.vendor_consent.contains(1));
        assert!(tcf.vendor_consent.contains(2_500));
        assert!(tcf.vendor_consent.contains(6_000));
        assert!(!tcf.vendor_consent.contains(6_001));
    }

    #[test]
    fn purpose_one_treatment_flag_read() {
        let raw = fixture("valid_purpose_one_treatment.txt");
        let mut spec = valid_core_spec();
        spec.purpose_one_treatment = true;
        assert_eq!(raw, with_minimal_dv(&encode_core(&spec)));
        let tcf = parse_tcf_v2(&raw).expect("valid fixture");
        assert!(tcf.purpose_one_treatment);
        let baseline = parse_tcf_v2(&fixture("valid_purpose_1_only.txt")).expect("valid fixture");
        assert!(!baseline.purpose_one_treatment);
    }

    #[test]
    fn v1_string_rejected_as_unsupported() {
        let raw = fixture("unsupported_v1.txt");
        assert_eq!(
            parse_tcf_v2(&raw).unwrap_err(),
            TcfParseError::UnsupportedTcfVersion(1)
        );
    }

    #[test]
    fn truncated_core_rejected() {
        let raw = fixture("truncated_core.txt");
        let full_core = encode_core(&full_consent_spec());
        let head = full_core
            .get(..40)
            .expect("full-consent core exceeds 40 bytes");
        assert_eq!(raw, with_minimal_dv(head));
        assert_eq!(
            parse_tcf_v2(&raw).unwrap_err(),
            TcfParseError::TruncatedCoreSegment
        );
    }

    #[test]
    fn invalid_base64_rejected() {
        let raw = fixture("invalid_base64.txt");
        assert_eq!(raw, with_minimal_dv("not//valid==base64!!"));
        assert_eq!(
            parse_tcf_v2(&raw).unwrap_err(),
            TcfParseError::InvalidBase64
        );
    }

    #[test]
    fn empty_input_rejected() {
        // The deterministic order validates the trailing-segment set
        // before any core work; an empty input has zero trailing
        // segments, so the mandatory-DV rule is the first to fire.
        assert_eq!(
            parse_tcf_v2("").unwrap_err(),
            TcfParseError::MissingDisclosedVendorsSegment
        );
    }

    #[test]
    fn vendor_consent_range_encoding_decodes() {
        let mut spec = valid_core_spec();
        spec.vendor_consent = VendorSectionSpec::Range {
            max_vendor_id: 300,
            entries: vec![RangeEntrySpec::Single(7), RangeEntrySpec::Span(100, 200)],
        };
        let tcf = parse_tcf_v2(&with_minimal_dv(&encode_core(&spec))).expect("valid range section");
        for vendor in [7_u16, 100, 150, 200] {
            assert!(tcf.vendor_consent.contains(vendor));
        }
        for vendor in [6_u16, 8, 99, 201, 300] {
            assert!(!tcf.vendor_consent.contains(vendor));
        }
    }

    #[test]
    fn vendor_consent_bitfield_encoding_decodes() {
        let mut spec = valid_core_spec();
        spec.vendor_consent = VendorSectionSpec::Bitfield {
            max_vendor_id: 100,
            set: vec![2, 37, 100],
        };
        let tcf = parse_tcf_v2(&with_minimal_dv(&encode_core(&spec))).expect("valid bitfield");
        for vendor in [2_u16, 37, 100] {
            assert!(tcf.vendor_consent.contains(vendor));
        }
        for vendor in [1_u16, 36, 38, 99] {
            assert!(!tcf.vendor_consent.contains(vendor));
        }
    }

    #[test]
    fn purpose_li_transparency_decodes_separately_from_purpose_consent() {
        let mut spec = valid_core_spec();
        spec.purposes_consent = vec![2];
        spec.purposes_li_transparency = vec![7];
        let tcf = parse_tcf_v2(&with_minimal_dv(&encode_core(&spec))).expect("valid string");
        assert_eq!(tcf.purposes_consent, BTreeSet::from([2]));
        assert_eq!(tcf.purposes_li_transparency, BTreeSet::from([7]));
    }

    #[test]
    fn special_feature_1_optin_decodes() {
        let mut spec = valid_core_spec();
        spec.special_feature_optins = vec![1];
        let tcf = parse_tcf_v2(&with_minimal_dv(&encode_core(&spec))).expect("valid string");
        assert!(tcf.special_feature_optins.contains(&1));
        assert!(!tcf.special_feature_optins.contains(&2));
    }

    #[test]
    fn publisher_cc_two_char_iso_decoded() {
        let tcf =
            parse_tcf_v2(&with_minimal_dv(&encode_core(&valid_core_spec()))).expect("valid string");
        assert_eq!(tcf.publisher_cc, *b"DE");
    }

    #[test]
    fn pub_restriction_not_allowed_decoded() {
        let raw = fixture("valid_with_pub_restrictions.txt");
        assert_eq!(raw, with_minimal_dv(&encode_core(&pub_restrictions_spec())));
        let tcf = parse_tcf_v2(&raw).expect("valid fixture");
        assert!(tcf.check_pub_restriction(2, PubRestrictionType::NotAllowed, 32));
        assert_eq!(
            tcf.pub_restriction_for(2, 32),
            Some(PubRestrictionType::NotAllowed)
        );
    }

    #[test]
    fn pub_restriction_require_consent_decoded() {
        let tcf = parse_tcf_v2(&fixture("valid_with_pub_restrictions.txt")).expect("valid fixture");
        assert!(tcf.check_pub_restriction(2, PubRestrictionType::RequireConsent, 887));
        assert!(!tcf.check_pub_restriction(2, PubRestrictionType::RequireConsent, 32));
    }

    #[test]
    fn pub_restriction_require_legit_interest_decoded() {
        let tcf = parse_tcf_v2(&fixture("valid_with_pub_restrictions.txt")).expect("valid fixture");
        assert!(tcf.check_pub_restriction(7, PubRestrictionType::RequireLegitInterest, 32));
        assert!(tcf.check_pub_restriction(7, PubRestrictionType::RequireLegitInterest, 40));
        assert!(!tcf.check_pub_restriction(7, PubRestrictionType::RequireLegitInterest, 41));
    }

    #[test]
    fn check_pub_restriction_returns_false_when_absent() {
        let tcf = parse_tcf_v2(&fixture("valid_with_pub_restrictions.txt")).expect("valid fixture");
        assert!(!tcf.check_pub_restriction(3, PubRestrictionType::NotAllowed, 32));
        assert!(!tcf.check_pub_restriction(2, PubRestrictionType::NotAllowed, 33));
        assert_eq!(tcf.pub_restriction_for(5, 32), None);
    }

    #[test]
    fn axis_b_policy_v3_on_2023_09_30_accepted() {
        let raw = with_minimal_dv(&encode_core(&policy3_spec(DS_2023_09_30)));
        let tcf = parse_tcf_v2(&raw).expect("last valid policy-v3 day");
        assert_eq!(tcf.tcf_policy_version, 3);
        assert_eq!(tcf.created, datetime!(2023-09-30 00:00:00 UTC));
    }

    #[test]
    fn axis_b_policy_v3_on_2023_10_01_rejected() {
        let raw = with_minimal_dv(&encode_core(&policy3_spec(DS_2023_10_01)));
        assert_eq!(
            parse_tcf_v2(&raw).unwrap_err(),
            TcfParseError::PolicyVersionTooOldForCreated {
                created: datetime!(2023-10-01 00:00:00 UTC),
                policy_version: 3,
            }
        );
    }

    #[test]
    fn policy_v3_created_2023_10_15_rejected_per_tech_spec() {
        // Inside the IAB Europe FAQ's grace window; the TCF technical
        // specification is the normative parser contract, so auctioneer
        // rejects.
        let raw = with_minimal_dv(&encode_core(&policy3_spec(DS_2023_10_15)));
        assert_eq!(
            parse_tcf_v2(&raw).unwrap_err(),
            TcfParseError::PolicyVersionTooOldForCreated {
                created: datetime!(2023-10-15 00:00:00 UTC),
                policy_version: 3,
            }
        );
    }

    #[test]
    fn policy_v3_created_2023_11_19_rejected_per_tech_spec() {
        // Last day of the FAQ's grace window; still rejected under the
        // technical specification's cutoff.
        let raw = with_minimal_dv(&encode_core(&policy3_spec(DS_2023_11_19)));
        assert_eq!(
            parse_tcf_v2(&raw).unwrap_err(),
            TcfParseError::PolicyVersionTooOldForCreated {
                created: datetime!(2023-11-19 00:00:00 UTC),
                policy_version: 3,
            }
        );
    }

    #[test]
    fn created_equal_lastupdated_day_truncated_accepted() {
        let tcf =
            parse_tcf_v2(&with_minimal_dv(&encode_core(&valid_core_spec()))).expect("valid string");
        assert_eq!(tcf.created, datetime!(2024-02-01 00:00:00 UTC));
        assert_eq!(tcf.created, tcf.last_updated);
    }

    #[test]
    fn created_not_equal_lastupdated_rejected() {
        let mut spec = valid_core_spec();
        spec.last_updated_ds = spec.created_ds.wrapping_add(864_000); // one whole day
        assert_eq!(
            parse_tcf_v2(&with_minimal_dv(&encode_core(&spec))).unwrap_err(),
            TcfParseError::CreatedLastUpdatedMismatch {
                created: datetime!(2024-02-01 00:00:00 UTC),
                last_updated: datetime!(2024-02-02 00:00:00 UTC),
            }
        );
    }

    #[test]
    fn created_with_sub_day_component_rejected() {
        let mut spec = valid_core_spec();
        spec.created_ds = spec.created_ds.wrapping_add(5); // +0.5 s
        spec.last_updated_ds = spec.created_ds;
        assert!(matches!(
            parse_tcf_v2(&with_minimal_dv(&encode_core(&spec))).unwrap_err(),
            TcfParseError::TimestampNotDayTruncated {
                field: "created",
                ..
            }
        ));
    }

    #[test]
    fn lastupdated_with_sub_day_component_rejected() {
        // The day-truncation checks run before the equality check, so
        // the sub-day `LastUpdated` names its own field instead of
        // surfacing as a mismatch.
        let mut spec = valid_core_spec();
        spec.last_updated_ds = spec.created_ds.wrapping_add(5);
        assert!(matches!(
            parse_tcf_v2(&with_minimal_dv(&encode_core(&spec))).unwrap_err(),
            TcfParseError::TimestampNotDayTruncated {
                field: "lastUpdated",
                ..
            }
        ));
    }

    #[test]
    fn publisher_tc_contents_remain_opaque() {
        let raw = fixture("trailing_segments_ignored.txt");
        let mut spec = valid_core_spec();
        spec.purposes_consent = vec![1, 2];
        spec.vendor_consent = VendorSectionSpec::Bitfield {
            max_vendor_id: 3,
            set: vec![2, 3],
        };
        let core_seg = encode_core(&spec);
        assert_eq!(
            raw,
            format!(
                "{core_seg}.{}.{}",
                minimal_dv_segment(),
                publisher_tc_segment(0)
            )
        );
        let with_pub_tc = parse_tcf_v2(&raw).expect("full trailing set");
        let dv_only = parse_tcf_v2(&with_minimal_dv(&core_seg)).expect("core + DV");
        assert_eq!(with_pub_tc, dv_only);
    }

    #[test]
    fn missing_disclosed_vendors_segment_rejected() {
        let core_seg = encode_core(&valid_core_spec());
        assert_eq!(
            parse_tcf_v2(&core_seg).unwrap_err(),
            TcfParseError::MissingDisclosedVendorsSegment
        );
        let pub_tc_only = format!("{core_seg}.{}", publisher_tc_segment(0));
        assert_eq!(
            parse_tcf_v2(&pub_tc_only).unwrap_err(),
            TcfParseError::MissingDisclosedVendorsSegment
        );
    }

    #[test]
    fn disclosed_vendors_segment_any_position_detected() {
        let core = encode_core(&valid_core_spec());
        let pub_tc = publisher_tc_segment(2);
        for dv in [
            dv_bitfield_segment(u16::MAX, &[1, u16::MAX]),
            dv_range_segment(
                u16::MAX,
                &[RangeEntrySpec::Single(1), RangeEntrySpec::Single(u16::MAX)],
            ),
        ] {
            let dv_only = parse_tcf_v2(&format!("{core}.{dv}")).expect("DV alone");
            assert!(dv_only.disclosed_vendors.contains(1));
            assert!(dv_only.disclosed_vendors.contains(u16::MAX));
            for raw in [
                format!("{core}.{dv}.{pub_tc}"),
                format!("{core}.{pub_tc}.{dv}"),
            ] {
                assert_eq!(parse_tcf_v2(&raw).expect("either DV position"), dv_only);
            }
        }
    }

    #[test]
    fn dv_bitfield_retains_membership_without_core_grants() {
        let disclosed = [1, 63, 64, 66, u16::MAX];
        let dv = dv_bitfield_segment(u16::MAX, &disclosed);
        let raw = format!("{}.{dv}", encode_core(&valid_core_spec()));
        let tcf = parse_tcf_v2(&raw).expect("framing-valid DV bitfield");
        for id in 0..=u16::MAX {
            assert_eq!(tcf.disclosed_vendors.contains(id), disclosed.contains(&id));
        }
        assert_eq!(tcf.vendor_consent, VendorBitSet::new());
        assert_eq!(tcf.vendor_li, VendorBitSet::new());
    }

    #[test]
    fn dv_ranges_retain_membership_without_core_grants() {
        let dv = dv_range_segment(
            u16::MAX,
            &[
                RangeEntrySpec::Span(0xFFFE, u16::MAX),
                RangeEntrySpec::Span(63, 65),
                RangeEntrySpec::Single(1),
            ],
        );
        let raw = format!("{}.{dv}", encode_core(&valid_core_spec()));
        let tcf = parse_tcf_v2(&raw).expect("framing-valid DV range encoding");
        for id in 0..=u16::MAX {
            assert_eq!(
                tcf.disclosed_vendors.contains(id),
                id == 1 || (63..=65).contains(&id) || id >= 0xFFFE,
            );
        }
        assert_eq!(tcf.vendor_consent, VendorBitSet::new());
        assert_eq!(tcf.vendor_li, VendorBitSet::new());
    }

    #[test]
    fn dv_membership_does_not_widen_bidder_decision() {
        const CORE: &str = "fake_core";
        const ALIAS: &str = "fake_alias";
        const VENDOR: u16 = 32;
        let mut spec = valid_core_spec();
        spec.purposes_consent = vec![2];
        spec.purposes_li_transparency = vec![7];
        let gvl = gvl(
            u32::from(spec.vendor_list_version),
            &[VendorSpec {
                id: VENDOR,
                purposes: vec![2],
                leg_int_purposes: vec![7],
                special_features: vec![1],
                ..VendorSpec::default()
            }],
        );
        let purposes = (2..=10)
            .map(|id| {
                (
                    id,
                    PurposeEnforcement {
                        enforce_algo: EnforceAlgo::Full,
                        enforce_purpose: TriState::Enabled,
                        enforce_vendors: TriState::Enabled,
                        vendor_exceptions: Vec::new(),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let context = |tcf| {
            let mut ctx =
                PrivacyContext::for_test(Some(tcf), gvl.clone(), purposes.clone(), BTreeSet::new());
            ctx.p2_enforced = true;
            ctx.sf1_enforced = true;
            ctx.sf1_effective = Some(SpecialFeatureEnforcement {
                enforce: TriState::Enabled,
                vendor_exceptions: Vec::new(),
            });
            ctx.alias_gvl_ids = BTreeMap::from([(ALIAS.into(), u32::from(VENDOR))]);
            ctx
        };
        for (case, consent, li, geo, gates) in [
            ("no core grants", false, false, false, (false, false, false)),
            (
                "consent without geo",
                true,
                false,
                false,
                (true, true, false),
            ),
            ("LI with geo", false, true, true, (false, true, true)),
            ("consent with geo", true, false, true, (true, true, true)),
        ] {
            spec.vendor_consent = VendorSectionSpec::Bitfield {
                max_vendor_id: VENDOR,
                set: if consent { vec![VENDOR] } else { Vec::new() },
            };
            spec.vendor_li = VendorSectionSpec::Bitfield {
                max_vendor_id: VENDOR,
                set: if li { vec![VENDOR] } else { Vec::new() },
            };
            spec.special_feature_optins = if geo { vec![1] } else { Vec::new() };
            let core = encode_core(&spec);
            for (empty, populated) in [
                (
                    dv_bitfield_segment(VENDOR, &[]),
                    dv_bitfield_segment(VENDOR, &[VENDOR]),
                ),
                (
                    dv_range_segment(VENDOR, &[]),
                    dv_range_segment(VENDOR, &[RangeEntrySpec::Single(VENDOR)]),
                ),
            ] {
                let baseline = parse_tcf_v2(&format!("{core}.{empty}")).expect("empty DV");
                let disclosed = parse_tcf_v2(&format!("{core}.{populated}")).expect("populated DV");
                assert_eq!(baseline.disclosed_vendors, VendorBitSet::new());
                let mut with_dv = baseline.clone();
                with_dv.disclosed_vendors.set(VENDOR);
                assert_eq!(disclosed, with_dv, "only DV membership changes: {case}");
                let expected = context(baseline).bidder_decision(CORE, ALIAS);
                let (allow_bid_request, pass_id, pass_geo) = gates;
                let permissions = if allow_bid_request && pass_id && pass_geo {
                    AuctionPermissions::AllowAll
                } else {
                    AuctionPermissions::Partial {
                        allow_bid_request,
                        pass_id,
                        pass_geo,
                    }
                };
                assert_eq!(expected.gdpr, permissions, "{case}");
                assert_eq!(
                    expected.fetch_denial,
                    (!allow_bid_request).then_some(FetchDenial::Gdpr)
                );
                let actual = context(disclosed).bidder_decision(CORE, ALIAS);
                assert_same_bidder_decision(actual, expected, case);
            }
        }
    }

    #[test]
    fn dv_empty_membership_in_both_encodings() {
        for max in [0, u16::MAX] {
            for dv in [dv_bitfield_segment(max, &[]), dv_range_segment(max, &[])] {
                let raw = format!("{}.{dv}", encode_core(&valid_core_spec()));
                let tcf = parse_tcf_v2(&raw).expect("empty DV membership is legal");
                assert_eq!(tcf.disclosed_vendors, VendorBitSet::new());
                assert_eq!(tcf.vendor_consent, VendorBitSet::new());
                assert_eq!(tcf.vendor_li, VendorBitSet::new());
            }
        }
    }

    #[test]
    fn dv_parser_reuse_resets_state_after_failures_and_success() {
        let mut spec = valid_core_spec();
        spec.vendor_consent = VendorSectionSpec::Bitfield {
            max_vendor_id: 3,
            set: vec![3],
        };
        spec.vendor_li = VendorSectionSpec::Bitfield {
            max_vendor_id: 4,
            set: vec![4],
        };
        let core = encode_core(&spec);
        let dv = dv_range_segment(u16::MAX, &[RangeEntrySpec::Span(1, u16::MAX)]);
        let overlapping = dv_range_segment(
            u16::MAX,
            &[RangeEntrySpec::Span(1, u16::MAX), RangeEntrySpec::Single(1)],
        );
        let mut invalid_core = BitWriter::new();
        write_core(&mut invalid_core, &spec);
        invalid_core.push_bits(0, 16);
        let failures = [
            (
                format!("{core}.{overlapping}"),
                TcfParseError::MalformedDisclosedVendorsSegment {
                    reason: "range entries overlap",
                },
            ),
            (
                format!("{core}.{dv}.Y"),
                TcfParseError::MalformedPublisherTcSegment {
                    reason: "truncated base64url quantum",
                },
            ),
            (
                format!("{}.{dv}", invalid_core.into_base64url()),
                TcfParseError::TrailingBytes,
            ),
        ];
        let mut parser = TcfParser::new();
        for (raw, expected) in failures {
            assert_eq!(parser.parse(&raw).unwrap_err(), expected);
            assert!(parser.partial_disclosed_vendors().contains(u16::MAX));
            let fresh = format!(
                "{}.{}",
                encode_core(&valid_core_spec()),
                dv_bitfield_segment(2, &[2]),
            );
            let tcf = parser.parse(&fresh).expect("reuse after a failed parse");
            let mut expected_dv = VendorBitSet::new();
            expected_dv.set(2);
            assert_eq!(tcf.disclosed_vendors, expected_dv);
            assert_eq!(tcf.vendor_consent, VendorBitSet::new());
            assert_eq!(tcf.vendor_li, VendorBitSet::new());
            assert_eq!(parser.partial_disclosed_vendors(), &VendorBitSet::new());
            let empty = format!(
                "{}.{}",
                encode_core(&valid_core_spec()),
                dv_bitfield_segment(0, &[]),
            );
            let empty_tcf = parser.parse(&empty).expect("reuse after success");
            assert_eq!(empty_tcf.disclosed_vendors, VendorBitSet::new());
            assert_eq!(empty_tcf.vendor_consent, VendorBitSet::new());
            assert_eq!(empty_tcf.vendor_li, VendorBitSet::new());
        }
    }

    #[test]
    fn dv_segment_truncated_rejected() {
        let core_seg = encode_core(&valid_core_spec());
        // (a) a bitfield DV declaring MaxVendorId = 1000 but carrying
        // only 4 decoded bytes.
        let mut w = BitWriter::new();
        w.push_bits(1, 3);
        w.push_bits(1_000, 16);
        w.push_bit(false);
        w.push_bits(0, 12);
        let truncated_bitfield = format!("{core_seg}.{}", w.into_base64url());
        assert!(matches!(
            parse_tcf_v2(&truncated_bitfield).unwrap_err(),
            TcfParseError::MalformedDisclosedVendorsSegment { .. }
        ));
        // (b) a single-char segment whose 6 bits read as type 1 with
        // nothing after it — the first-char masquerade.
        let masquerade = format!("{core_seg}.I");
        assert!(matches!(
            parse_tcf_v2(&masquerade).unwrap_err(),
            TcfParseError::MalformedDisclosedVendorsSegment { .. }
        ));
    }

    #[test]
    fn dv_segment_duplicate_rejected() {
        let raw = format!(
            "{0}.{1}.{1}",
            encode_core(&valid_core_spec()),
            minimal_dv_segment()
        );
        assert_eq!(
            parse_tcf_v2(&raw).unwrap_err(),
            TcfParseError::DuplicateDisclosedVendorsSegment
        );
    }

    #[test]
    fn dv_segment_bad_range_rejected() {
        let core_seg = encode_core(&valid_core_spec());
        let bad_entry_sets = [
            vec![RangeEntrySpec::Span(20, 10)], // start > end
            vec![RangeEntrySpec::Span(0, 5)],   // id 0
            vec![RangeEntrySpec::Span(30, 50)], // beyond MaxVendorId 40
        ];
        for entries in bad_entry_sets {
            let dv = dv_range_segment(40, &entries);
            let raw = format!("{core_seg}.{dv}");
            assert!(matches!(
                parse_tcf_v2(&raw).unwrap_err(),
                TcfParseError::MalformedDisclosedVendorsSegment { .. }
            ));
        }
    }

    #[test]
    fn empty_trailing_segment_rejected() {
        let core_seg = encode_core(&valid_core_spec());
        let dv = minimal_dv_segment();
        assert_eq!(
            parse_tcf_v2(&format!("{core_seg}..{dv}")).unwrap_err(),
            TcfParseError::EmptyTrailingSegment
        );
        assert_eq!(
            parse_tcf_v2(&format!("{core_seg}.{dv}.")).unwrap_err(),
            TcfParseError::EmptyTrailingSegment
        );
    }

    #[test]
    fn unknown_segment_type_rejected() {
        let core_seg = encode_core(&valid_core_spec());
        // 'Q' = 0b010000: type 2, the obsolete Allowed Vendors / OOB
        // segment removed from the current format.
        assert_eq!(
            parse_tcf_v2(&format!("{core_seg}.QAAA")).unwrap_err(),
            TcfParseError::UnknownSegmentType { segment_type: 2 }
        );
        // 'o' = 0b101000: type 5, undefined.
        assert_eq!(
            parse_tcf_v2(&format!("{core_seg}.oAAA")).unwrap_err(),
            TcfParseError::UnknownSegmentType { segment_type: 5 }
        );
    }

    #[test]
    fn duplicate_publisher_tc_rejected() {
        // Exactly TWO framing-valid type-3 segments: the repeat fires
        // the moment the second type-3 is seen, BEFORE the final
        // missing-DV check — asserting the precedence order itself.
        let raw = format!(
            "{0}.{1}.{1}",
            encode_core(&valid_core_spec()),
            publisher_tc_segment(0)
        );
        assert_eq!(
            parse_tcf_v2(&raw).unwrap_err(),
            TcfParseError::DuplicatePublisherTcSegment
        );
    }

    #[test]
    fn publisher_tc_framing_accepted() {
        // NumCustomPurposes = 2: exactly 57 + 4 bits plus zero pad.
        let raw = format!(
            "{}.{}.{}",
            encode_core(&valid_core_spec()),
            minimal_dv_segment(),
            publisher_tc_segment(2)
        );
        // Publisher-TC contents are not retained on `TcfString` (the
        // struct has no such field); parsing just succeeds.
        parse_tcf_v2(&raw).expect("framing-valid Publisher TC");
    }

    #[test]
    fn publisher_tc_truncated_rejected() {
        // Declares NumCustomPurposes = 40 (needs 57 + 80 bits) but
        // carries only 8 decoded bytes.
        let mut w = BitWriter::new();
        w.push_bits(3, 3);
        w.push_bits(0, 24);
        w.push_bits(0, 24);
        w.push_bits(40, 6);
        w.push_bits(0, 7);
        let raw = format!(
            "{}.{}.{}",
            encode_core(&valid_core_spec()),
            minimal_dv_segment(),
            w.into_base64url()
        );
        assert!(matches!(
            parse_tcf_v2(&raw).unwrap_err(),
            TcfParseError::MalformedPublisherTcSegment { .. }
        ));
    }

    #[test]
    fn cmp_id_and_version_decoded_and_retained() {
        // CmpId 300 mirrors a registered Global CMP List id; the
        // retained fields feed the GCL authenticity check.
        let tcf =
            parse_tcf_v2(&with_minimal_dv(&encode_core(&valid_core_spec()))).expect("valid string");
        assert_eq!(tcf.cmp_id, 300);
        assert_eq!(tcf.cmp_version, 2);
    }

    #[test]
    fn vendor_id_zero_start_rejected() {
        let raw = fixture("vendor_range_starts_at_zero.txt");
        let mut spec = valid_core_spec();
        spec.vendor_consent = VendorSectionSpec::Range {
            max_vendor_id: 10,
            entries: vec![RangeEntrySpec::Span(0, 5)],
        };
        assert_eq!(raw, with_minimal_dv(&encode_core(&spec)));
        assert_eq!(
            parse_tcf_v2(&raw).unwrap_err(),
            TcfParseError::InvalidVendorId {
                section: RangeSection::VendorConsent,
                actual: 0,
            }
        );
    }

    #[test]
    fn vendor_id_zero_end_rejected() {
        let mut spec = valid_core_spec();
        spec.vendor_consent = VendorSectionSpec::Range {
            max_vendor_id: 10,
            entries: vec![RangeEntrySpec::Span(5, 0)],
        };
        assert_eq!(
            parse_tcf_v2(&with_minimal_dv(&encode_core(&spec))).unwrap_err(),
            TcfParseError::InvalidVendorId {
                section: RangeSection::VendorConsent,
                actual: 0,
            }
        );
    }

    #[test]
    fn vendor_id_beyond_segment_max_rejected() {
        let mut spec = valid_core_spec();
        spec.vendor_consent = VendorSectionSpec::Range {
            max_vendor_id: 200,
            entries: vec![RangeEntrySpec::Span(190, 250)],
        };
        assert_eq!(
            parse_tcf_v2(&with_minimal_dv(&encode_core(&spec))).unwrap_err(),
            TcfParseError::VendorIdBeyondSegmentMax {
                section: RangeSection::VendorConsent,
                actual: 250,
                max: 200,
            }
        );
    }

    #[test]
    fn bitset_contains_zero_is_false_after_bypass() {
        let mut words = [0_u64; 1024];
        words[0] = 0b11; // bit 0 (id 0) forced on alongside id 1
        let set = VendorBitSet::from_raw_words(&words);
        assert!(!set.contains(0));
        assert!(set.contains(1));
    }

    #[test]
    fn is_service_specific_zero_rejected_unconditionally() {
        let mut spec = valid_core_spec();
        spec.is_service_specific = false;
        assert_eq!(
            parse_tcf_v2(&with_minimal_dv(&encode_core(&spec))).unwrap_err(),
            TcfParseError::NotServiceSpecific
        );
        // A non-empty publisher-restrictions section must reject with
        // the SAME variant — the invariant is on the flag itself.
        let mut with_restrictions = pub_restrictions_spec();
        with_restrictions.is_service_specific = false;
        assert_eq!(
            parse_tcf_v2(&with_minimal_dv(&encode_core(&with_restrictions))).unwrap_err(),
            TcfParseError::NotServiceSpecific
        );
    }

    #[test]
    fn pub_restriction_purpose_0_rejected() {
        let mut spec = valid_core_spec();
        spec.pub_restrictions = vec![PubRestrictionSpec {
            purpose_id: 0,
            restriction_type: 1,
            entries: vec![RangeEntrySpec::Single(5)],
        }];
        assert_eq!(
            parse_tcf_v2(&with_minimal_dv(&encode_core(&spec))).unwrap_err(),
            TcfParseError::InvalidPubRestrictionPurpose { actual: 0 }
        );
    }

    #[test]
    fn pub_restriction_purpose_25_rejected() {
        let mut spec = valid_core_spec();
        spec.pub_restrictions = vec![PubRestrictionSpec {
            purpose_id: 25,
            restriction_type: 1,
            entries: vec![RangeEntrySpec::Single(5)],
        }];
        assert_eq!(
            parse_tcf_v2(&with_minimal_dv(&encode_core(&spec))).unwrap_err(),
            TcfParseError::InvalidPubRestrictionPurpose { actual: 25 }
        );
    }

    #[test]
    fn pub_restriction_type_3_rejected() {
        let mut spec = valid_core_spec();
        spec.pub_restrictions = vec![PubRestrictionSpec {
            purpose_id: 2,
            restriction_type: 3,
            entries: vec![RangeEntrySpec::Single(5)],
        }];
        assert_eq!(
            parse_tcf_v2(&with_minimal_dv(&encode_core(&spec))).unwrap_err(),
            TcfParseError::UndefinedPubRestrictionType
        );
    }

    #[test]
    fn pub_restrictions_exceeds_2048_rejected() {
        let raw = fixture("pub_restrictions_exceeds_2048_rejected.txt");
        let mut spec = valid_core_spec();
        let count = u16::try_from(MAX_PUBLISHER_RESTRICTION_LIST_LEN)
            .unwrap_or(0)
            .wrapping_add(1);
        spec.pub_restrictions = (1..=count)
            .map(|vendor| PubRestrictionSpec {
                purpose_id: 2,
                restriction_type: 1,
                entries: vec![RangeEntrySpec::Single(vendor)],
            })
            .collect();
        assert_eq!(raw, with_minimal_dv(&encode_core(&spec)));
        assert_eq!(
            parse_tcf_v2(&raw).unwrap_err(),
            TcfParseError::TooManyPublisherRestrictions
        );
    }

    #[test]
    fn purpose_3_li_bit_policy_gated() {
        let accepted_raw = fixture("core_policy3_purpose4_li_bit_accepted.txt");
        assert_eq!(
            accepted_raw,
            with_minimal_dv(&encode_core(&policy3_li4_spec()))
        );
        let accepted = parse_tcf_v2(&accepted_raw).expect("policy-v3 string keeps the LI bit");
        assert!(accepted.purposes_li_transparency.contains(&4));

        let rejected_raw = fixture("core_policy4_purpose4_li_bit_rejected.txt");
        let mut policy4 = valid_core_spec();
        policy4.purposes_li_transparency = vec![4];
        assert_eq!(rejected_raw, with_minimal_dv(&encode_core(&policy4)));
        assert_eq!(
            parse_tcf_v2(&rejected_raw).unwrap_err(),
            TcfParseError::LiBitSetForConsentOnlyPurpose { purpose_id: 4 }
        );

        // Purposes 3, 5, 6 via the same fixture family.
        for purpose_id in [3_u8, 5, 6] {
            let mut gated = valid_core_spec();
            gated.purposes_li_transparency = vec![purpose_id];
            assert_eq!(
                parse_tcf_v2(&with_minimal_dv(&encode_core(&gated))).unwrap_err(),
                TcfParseError::LiBitSetForConsentOnlyPurpose { purpose_id }
            );
            let mut ungated = policy3_li4_spec();
            ungated.purposes_li_transparency = vec![purpose_id];
            let kept = parse_tcf_v2(&with_minimal_dv(&encode_core(&ungated)))
                .expect("policy-v3 string keeps LI bits");
            assert!(kept.purposes_li_transparency.contains(&purpose_id));
        }
    }

    #[test]
    fn zero_padding_for_byte_alignment_accepted() {
        // The canonical core is 259 bits (213 fixed + two empty
        // 17-bit vendor sections + 12 restriction-count bits), so its
        // final byte carries 5 zero padding bits — the fixture
        // inherently exercises the legal-padding path.
        parse_tcf_v2(&with_minimal_dv(&encode_core(&valid_core_spec())))
            .expect("zero padding to the byte boundary is legal");
    }

    #[test]
    fn non_zero_padding_rejected() {
        let mut w = BitWriter::new();
        write_core(&mut w, &valid_core_spec());
        w.push_bit(true); // first padding bit non-zero
        assert_eq!(
            parse_tcf_v2(&with_minimal_dv(&w.into_base64url())).unwrap_err(),
            TcfParseError::NonZeroPadding
        );
    }

    #[test]
    fn whole_trailing_byte_rejected() {
        let mut w = BitWriter::new();
        write_core(&mut w, &valid_core_spec());
        w.push_bits(0, 5); // finish the byte with legal zero padding
        w.push_bits(0, 8); // one whole extra byte — all-zero, still rejected
        assert_eq!(
            parse_tcf_v2(&with_minimal_dv(&w.into_base64url())).unwrap_err(),
            TcfParseError::TrailingBytes
        );
    }
}

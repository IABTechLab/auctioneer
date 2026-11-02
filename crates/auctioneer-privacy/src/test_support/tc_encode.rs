//! Test-side TCF v2 TC-string encoder. Tests and fixtures state their
//! inputs as field values (`CoreSpec`) instead of opaque base64
//! literals; malformed inputs are built on the raw `BitWriter`.
//!
//! Available in unit-test builds and with the `test-support` feature.
//! Integration tests share this canonical implementation through the
//! privacy crate rather than compiling sibling source files. The encoder
//! itself is dependency-free.
//!
//! Kept as lint-hardened as library code (no unwrap/index/bare
//! arithmetic): the feature compiles outside `#[cfg(test)]`-attributed
//! scopes, where the test lint exemptions do not apply. Extra segment
//! builders live in the sibling `tc_encode_segments` module; date
//! constants live in the unit-test module that consumes them.

const B64URL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Deciseconds since the UNIX epoch for 2024-02-01T00:00:00Z, the
/// canonical valid `Created`/`LastUpdated` (day-truncated, after the
/// policy-v3 cutoff). Consumers read it back via
/// `valid_core_spec().created_ds`.
const DS_2024_02_01: u64 = 17_067_456_000;

/// 6-bit alphabet code for an ASCII uppercase letter (`b'A'` → 0).
#[must_use]
pub fn letter(c: u8) -> u8 {
    c.wrapping_sub(b'A')
}

/// MSB-first bit accumulator. `bytes()` zero-pads the final byte, so
/// "padding" from the parser's perspective is whatever the writer did
/// not explicitly push — tests craft non-zero padding by pushing the
/// pad bits themselves.
#[derive(Default)]
pub struct BitWriter {
    bytes: Vec<u8>,
    bit_len: usize,
}

impl BitWriter {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_bit(&mut self, bit: bool) {
        let byte_idx = self.bit_len.wrapping_div(8);
        if byte_idx == self.bytes.len() {
            self.bytes.push(0);
        }
        if bit {
            let offset = u32::try_from(self.bit_len.wrapping_rem(8)).unwrap_or(0);
            if let Some(byte) = self.bytes.get_mut(byte_idx) {
                *byte |= 0x80_u8.wrapping_shr(offset);
            }
        }
        self.bit_len = self.bit_len.wrapping_add(1);
    }

    /// Pushes the low `width` bits of `value`, most significant first.
    pub fn push_bits(&mut self, value: u64, width: u32) {
        for i in (0..width).rev() {
            self.push_bit(value.wrapping_shr(i) & 1 == 1);
        }
    }

    /// Base64url (no padding chars) of the accumulated bytes.
    #[must_use]
    pub fn into_base64url(self) -> String {
        base64url(&self.bytes)
    }
}

/// Base64url-encodes without `=` padding; leftover bits are zero.
fn base64url(bytes: &[u8]) -> String {
    let mut out = String::new();
    // `as_chunks` hands back fixed-size arrays, so the three bytes of a
    // full group are reachable by pattern rather than by fallible
    // indexing with an `unwrap_or(0)` that could never fire.
    let (chunks, rest) = bytes.as_chunks::<3>();
    for &[b0, b1, b2] in chunks {
        push_b64(&mut out, b0.wrapping_shr(2));
        push_b64(&mut out, b0.wrapping_shl(4) & 0x30 | b1.wrapping_shr(4));
        push_b64(&mut out, b1.wrapping_shl(2) & 0x3C | b2.wrapping_shr(6));
        push_b64(&mut out, b2 & 0x3F);
    }
    if let Some(&b0) = rest.first() {
        push_b64(&mut out, b0.wrapping_shr(2));
        match rest.get(1) {
            Some(&b1) => {
                push_b64(&mut out, b0.wrapping_shl(4) & 0x30 | b1.wrapping_shr(4));
                push_b64(&mut out, b1.wrapping_shl(2) & 0x3C);
            }
            None => push_b64(&mut out, b0.wrapping_shl(4) & 0x30),
        }
    }
    out
}

fn push_b64(out: &mut String, six_bits: u8) {
    let code = B64URL
        .get(usize::from(six_bits & 0x3F))
        .copied()
        .unwrap_or(b'A');
    out.push(char::from(code));
}

/// One encoded range-section entry.
#[derive(Clone, Copy)]
pub enum RangeEntrySpec {
    /// `IsARange = 0`, a single vendor id.
    Single(u16),
    /// `IsARange = 1`, an inclusive `(start, end)` interval.
    Span(u16, u16),
}

/// A vendor consent / vendor LI section of the core segment.
pub enum VendorSectionSpec {
    Bitfield {
        max_vendor_id: u16,
        set: Vec<u16>,
    },
    Range {
        max_vendor_id: u16,
        entries: Vec<RangeEntrySpec>,
    },
}

/// One publisher-restriction record (core segment).
pub struct PubRestrictionSpec {
    pub purpose_id: u8,
    /// On-wire 2-bit value: 0 `NotAllowed`, 1 `RequireConsent`,
    /// 2 `RequireLegitimateInterest`, 3 reserved/undefined.
    pub restriction_type: u8,
    pub entries: Vec<RangeEntrySpec>,
}

/// Every field of the core segment, in wire order.
pub struct CoreSpec {
    pub version: u8,
    pub created_ds: u64,
    pub last_updated_ds: u64,
    pub cmp_id: u16,
    pub cmp_version: u16,
    pub consent_screen: u8,
    /// Two 6-bit alphabet codes (see [`letter`]).
    pub consent_language: (u8, u8),
    pub vendor_list_version: u16,
    pub tcf_policy_version: u8,
    pub is_service_specific: bool,
    pub use_non_standard_stacks: bool,
    pub special_feature_optins: Vec<u8>,
    pub purposes_consent: Vec<u8>,
    pub purposes_li_transparency: Vec<u8>,
    pub purpose_one_treatment: bool,
    /// Two 6-bit alphabet codes.
    pub publisher_cc: (u8, u8),
    pub vendor_consent: VendorSectionSpec,
    pub vendor_li: VendorSectionSpec,
    pub pub_restrictions: Vec<PubRestrictionSpec>,
}

/// Canonical valid core: TCF v2, policy 4, day-truncated equal
/// timestamps, `IsServiceSpecific = 1`, purpose-1 consent only, no
/// vendors, no restrictions, publisher country `DE`, language `EN`.
#[must_use]
pub fn valid_core_spec() -> CoreSpec {
    CoreSpec {
        version: 2,
        created_ds: DS_2024_02_01,
        last_updated_ds: DS_2024_02_01,
        cmp_id: 300,
        cmp_version: 2,
        consent_screen: 1,
        consent_language: (letter(b'E'), letter(b'N')),
        vendor_list_version: 348,
        tcf_policy_version: 4,
        is_service_specific: true,
        use_non_standard_stacks: false,
        special_feature_optins: Vec::new(),
        purposes_consent: vec![1],
        purposes_li_transparency: Vec::new(),
        purpose_one_treatment: false,
        publisher_cc: (letter(b'D'), letter(b'E')),
        vendor_consent: VendorSectionSpec::Bitfield {
            max_vendor_id: 0,
            set: Vec::new(),
        },
        vendor_li: VendorSectionSpec::Bitfield {
            max_vendor_id: 0,
            set: Vec::new(),
        },
        pub_restrictions: Vec::new(),
    }
}

fn write_id_bitset(w: &mut BitWriter, width: u8, set: &[u8]) {
    for id in 1..=width {
        w.push_bit(set.contains(&id));
    }
}

/// Writes a `NumEntries(12)`-prefixed range-entry list (shared by the
/// core vendor sections and the segment builders in
/// `tc_encode_segments.rs`).
pub fn write_range_entries(w: &mut BitWriter, entries: &[RangeEntrySpec]) {
    w.push_bits(u64::try_from(entries.len()).unwrap_or(0), 12);
    for entry in entries {
        match *entry {
            RangeEntrySpec::Single(id) => {
                w.push_bit(false);
                w.push_bits(u64::from(id), 16);
            }
            RangeEntrySpec::Span(start, end) => {
                w.push_bit(true);
                w.push_bits(u64::from(start), 16);
                w.push_bits(u64::from(end), 16);
            }
        }
    }
}

fn write_vendor_section(w: &mut BitWriter, section: &VendorSectionSpec) {
    match section {
        VendorSectionSpec::Bitfield { max_vendor_id, set } => {
            w.push_bits(u64::from(*max_vendor_id), 16);
            w.push_bit(false);
            for id in 1..=*max_vendor_id {
                w.push_bit(set.contains(&id));
            }
        }
        VendorSectionSpec::Range {
            max_vendor_id,
            entries,
        } => {
            w.push_bits(u64::from(*max_vendor_id), 16);
            w.push_bit(true);
            write_range_entries(w, entries);
        }
    }
}

/// Writes the full core segment (no base64) into `w`.
pub fn write_core(w: &mut BitWriter, spec: &CoreSpec) {
    w.push_bits(u64::from(spec.version), 6);
    w.push_bits(spec.created_ds, 36);
    w.push_bits(spec.last_updated_ds, 36);
    w.push_bits(u64::from(spec.cmp_id), 12);
    w.push_bits(u64::from(spec.cmp_version), 12);
    w.push_bits(u64::from(spec.consent_screen), 6);
    w.push_bits(u64::from(spec.consent_language.0), 6);
    w.push_bits(u64::from(spec.consent_language.1), 6);
    w.push_bits(u64::from(spec.vendor_list_version), 12);
    w.push_bits(u64::from(spec.tcf_policy_version), 6);
    w.push_bit(spec.is_service_specific);
    w.push_bit(spec.use_non_standard_stacks);
    write_id_bitset(w, 12, &spec.special_feature_optins);
    write_id_bitset(w, 24, &spec.purposes_consent);
    write_id_bitset(w, 24, &spec.purposes_li_transparency);
    w.push_bit(spec.purpose_one_treatment);
    w.push_bits(u64::from(spec.publisher_cc.0), 6);
    w.push_bits(u64::from(spec.publisher_cc.1), 6);
    write_vendor_section(w, &spec.vendor_consent);
    write_vendor_section(w, &spec.vendor_li);
    w.push_bits(u64::try_from(spec.pub_restrictions.len()).unwrap_or(0), 12);
    for record in &spec.pub_restrictions {
        w.push_bits(u64::from(record.purpose_id), 6);
        w.push_bits(u64::from(record.restriction_type), 2);
        write_range_entries(w, &record.entries);
    }
}

/// Base64url text of the core segment alone (no trailing segments).
#[must_use]
pub fn encode_core(spec: &CoreSpec) -> String {
    let mut w = BitWriter::new();
    write_core(&mut w, spec);
    w.into_base64url()
}

/// Disclosed Vendors segment, bitfield encoding.
#[must_use]
pub fn dv_bitfield_segment(max_vendor_id: u16, set: &[u16]) -> String {
    let mut w = BitWriter::new();
    w.push_bits(1, 3);
    w.push_bits(u64::from(max_vendor_id), 16);
    w.push_bit(false);
    for id in 1..=max_vendor_id {
        w.push_bit(set.contains(&id));
    }
    w.into_base64url()
}

/// The canonical framing-valid minimal Disclosed Vendors segment:
/// `MaxVendorId = 1`, bitfield encoding, one vendor bit set,
/// zero-padded to the byte boundary.
#[must_use]
pub fn minimal_dv_segment() -> String {
    dv_bitfield_segment(1, &[1])
}

/// The fixture helper: appends the canonical framing-valid minimal
/// Disclosed Vendors segment to a core, making the string satisfy the
/// strict trailing-segment-set contract (a core-only string is
/// structurally invalid).
#[must_use]
pub fn with_minimal_dv(core: &str) -> String {
    let mut out = String::from(core);
    out.push('.');
    out.push_str(&minimal_dv_segment());
    out
}

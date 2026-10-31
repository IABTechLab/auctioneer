//! Prepaid debug capacity, not permission to capture or an allocation interceptor.
//!
//! These are conservative structural allowances, not independent measured
//! expansion ratios. Native projector/collector/engine allocation guards cover
//! dense nodes, Unicode/escaping, MIME parameters, tiny/empty bodies and concurrent
//! calls; whole-peak fit does not prove each allowance binds independently.
//! Body input is borrowed from already-funded request/response storage; none of
//! this charge authorizes a raw-body copy or a second batch of parser working sets.

use crate::bounded_json::NODE_RETAINED_BYTES;
use crate::budget::BudgetError;

pub const MAX_DEBUG_CALL_ROWS: u32 = 64;
pub const MAX_DEBUG_INPUT_BYTES: u64 = 64 * 1024;
pub const MAX_DEBUG_BODY_NODES: u32 = 4096;
pub const MAX_DEBUG_BODY_DEPTH: u32 = 64;
pub const MAX_DEBUG_SANITIZED_BODY_BYTES: u64 = 16 * 1024;
/// Includes record/container/key/string allocation CAPACITIES, not just lengths.
pub const MAX_DEBUG_RETAINED_BYTES: u64 = 64 * 1024;
pub const MAX_DEBUG_SERIALIZED_BYTES: u64 = 64 * 1024;

/// Owned decoded strings plus unescape scratch, allowing two capacities each.
/// The borrowed original input is NOT another allocation in this term.
pub const DEBUG_BODY_INPUT_FACTOR: u64 = 4;
/// Three conservative admitted-node working-set allowances. The current
/// projector borrows `RawValue` spans and streams bounded output; it does not
/// materialize three owned trees. Reuse the retained-Value ceiling to bound
/// possible node-associated scratch without confusing it with measured copies.
pub const DEBUG_BODY_NODE_COPIES: u64 = 3;
/// Two traversal frames per level, each allowed 16 eight-byte words.
pub const DEBUG_DEPTH_FRAME_BYTES: u64 = 256;
/// Old plus new capacity during geometric buffer growth (one plus two).
pub const DEBUG_GROWING_BUFFER_FACTOR: u64 = 3;

/// One owned MIME source and one construction/normalization working copy.
pub const DEBUG_MIME_SOURCE_FACTOR: u64 = 2;
/// `mime` 0.3.17's shortest parameter suffix is `;a=` (it accepts a final
/// empty unquoted value). This deliberately overcounts non-final parameters.
pub const DEBUG_MIME_MIN_PARAMETER_BYTES: u64 = 3;
/// Two Indexed ranges = four 64-bit words per parameter; old/new Vec
/// capacity overlap is at most three populations. Independent of JSON nodes.
pub const DEBUG_MIME_PARAMETER_PEAK_BYTES: u64 = 96;
/// Sixteen eight-byte words for the MIME source/parameter handles and iterators.
pub const DEBUG_MIME_FIXED_BYTES: u64 = 128;

/// One DTO copy plus two retained-capacity allowances for Value string/array
/// contents and growth. Captures must count their owned slots, not just text.
pub const DEBUG_CONVERSION_RETAINED_FACTOR: u64 = 3;
/// At most 32 fixed key/value pairs per call: call fields, request/response,
/// six allowed header-name/array pairs, grouping and optional body/error fields.
/// Repeated header values are contents charged by retained-capacity copies.
pub const DEBUG_ROW_VALUE_NODES: u64 = 64;
/// Outer object, three keys/values and the httpcalls grouping container.
pub const DEBUG_OUTER_VALUE_NODES: u64 = 8;
/// A one-byte control character can become six bytes (`\u0000`) in outer JSON.
pub const DEBUG_JSON_ESCAPE_FACTOR: u64 = 6;
/// Eight eight-byte slots for ordering, body sizes, counters and fit decisions.
pub const DEBUG_FIT_ROW_BYTES: u64 = 64;
pub const DEBUG_FIT_FIXED_BYTES: u64 = 64;

/// Conservative debug peak for one admitted auction, independent of signals.
///
/// All retained capture capacity coexists with one sequential body working set,
/// a separate MIME parser working set, DTO/Value conversion, escaped output and
/// fit bookkeeping. The escaped candidate funds six times retained contents
/// PLUS a full serialized cap for fixed metadata, with old/new writer overlap.
/// Request bodies are funded even if every response is empty. A zero call ceiling
/// keeps the fixed allowance (including empty-object assembly); only row-dependent
/// work narrows. Smaller installed body/node/depth caps can only reduce actual work.
///
/// # Errors
/// Returns [`BudgetError::BudgetArithmeticOverflow`] for an unrepresentable
/// product or sum. Today's bounded u32 call ceiling cannot trigger it, but every
/// helper and aggregate stays checked so future cap changes cannot wrap a charge.
pub fn debug_peak_bytes(max_outbound_requests: u32) -> Result<u64, BudgetError> {
    let rows = u64::from(max_outbound_requests.min(MAX_DEBUG_CALL_ROWS));
    let body_node_bytes = checked_mul(DEBUG_BODY_NODE_COPIES, NODE_RETAINED_BYTES)?;
    let value_nodes = checked_sum([
        checked_mul(rows, DEBUG_ROW_VALUE_NODES)?,
        DEBUG_OUTER_VALUE_NODES,
    ])?;
    let escaped_candidate = checked_sum([
        checked_mul(MAX_DEBUG_RETAINED_BYTES, DEBUG_JSON_ESCAPE_FACTOR)?,
        MAX_DEBUG_SERIALIZED_BYTES,
    ])?;
    checked_sum([
        MAX_DEBUG_RETAINED_BYTES,
        checked_mul(MAX_DEBUG_INPUT_BYTES, DEBUG_BODY_INPUT_FACTOR)?,
        checked_mul(u64::from(MAX_DEBUG_BODY_NODES), body_node_bytes)?,
        checked_mul(u64::from(MAX_DEBUG_BODY_DEPTH), DEBUG_DEPTH_FRAME_BYTES)?,
        checked_mul(MAX_DEBUG_SANITIZED_BODY_BYTES, DEBUG_GROWING_BUFFER_FACTOR)?,
        checked_mul(MAX_DEBUG_INPUT_BYTES, DEBUG_MIME_SOURCE_FACTOR)?,
        checked_mul(
            MAX_DEBUG_INPUT_BYTES.div_euclid(DEBUG_MIME_MIN_PARAMETER_BYTES),
            DEBUG_MIME_PARAMETER_PEAK_BYTES,
        )?,
        DEBUG_MIME_FIXED_BYTES,
        checked_mul(MAX_DEBUG_RETAINED_BYTES, DEBUG_CONVERSION_RETAINED_FACTOR)?,
        checked_mul(value_nodes, NODE_RETAINED_BYTES)?,
        checked_mul(escaped_candidate, DEBUG_GROWING_BUFFER_FACTOR)?,
        checked_mul(rows, DEBUG_FIT_ROW_BYTES)?,
        DEBUG_FIT_FIXED_BYTES,
    ])
}

fn checked_mul(a: u64, b: u64) -> Result<u64, BudgetError> {
    a.checked_mul(b)
        .ok_or(BudgetError::BudgetArithmeticOverflow)
}

fn checked_sum<const N: usize>(terms: [u64; N]) -> Result<u64, BudgetError> {
    terms.into_iter().try_fold(0_u64, |sum, bytes| {
        sum.checked_add(bytes)
            .ok_or(BudgetError::BudgetArithmeticOverflow)
    })
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_DEBUG_BODY_DEPTH, MAX_DEBUG_BODY_NODES, MAX_DEBUG_CALL_ROWS, MAX_DEBUG_INPUT_BYTES,
        MAX_DEBUG_RETAINED_BYTES, MAX_DEBUG_SANITIZED_BODY_BYTES, MAX_DEBUG_SERIALIZED_BYTES,
        checked_mul, checked_sum, debug_peak_bytes,
    };
    use crate::alloc_track::allocated_on_this_thread;
    use crate::budget::BudgetError;

    #[test]
    fn compiled_caps_match_the_wire_and_capture_contract() {
        assert_eq!(MAX_DEBUG_CALL_ROWS, 64);
        assert_eq!(MAX_DEBUG_INPUT_BYTES, 0x0001_0000);
        assert_eq!(MAX_DEBUG_BODY_NODES, 4_096);
        assert_eq!(MAX_DEBUG_BODY_DEPTH, 64);
        assert_eq!(MAX_DEBUG_SANITIZED_BODY_BYTES, 0x4000);
        assert_eq!(MAX_DEBUG_RETAINED_BYTES, 0x0001_0000);
        assert_eq!(MAX_DEBUG_SERIALIZED_BYTES, 0x0001_0000);
    }

    #[test]
    fn peak_matches_independent_capacity_derivation() {
        // Retention; decoded strings/unescape scratch; three node populations;
        // two traversal frames; growing body writer; MIME source, four-index
        // parameters with old/new Vec overlap, fixed MIME state; DTO and Value
        // copies; outer metadata; escaped writer old/new overlap; fit counters.
        let fixed = 0x0001_0000
            + 4 * 0x0001_0000
            + 3 * 384 * 4_096
            + 64 * 256
            + 3 * 0x4000
            + 2 * 0x0001_0000
            + 0x0001_0000_u64.div_euclid(3) * 96
            + 128
            + 3 * 0x0001_0000
            + 8 * 384
            + 3 * (6 * 0x0001_0000 + 0x0001_0000)
            + 64;
        let row = 64 * 384 + 64;
        assert_eq!(fixed, 8_916_128);
        assert_eq!(row, 24_640);
        assert_eq!(debug_peak_bytes(0).unwrap(), fixed);
        assert_eq!(debug_peak_bytes(1).unwrap(), fixed + row);
        assert_eq!(debug_peak_bytes(2).unwrap(), fixed + 2 * row);
        assert_eq!(debug_peak_bytes(63).unwrap(), 10_468_448);
        assert_eq!(debug_peak_bytes(64).unwrap(), 10_493_088);
        assert_eq!(debug_peak_bytes(65).unwrap(), 10_493_088);
        assert_eq!(debug_peak_bytes(u32::MAX).unwrap(), 10_493_088);
    }

    #[test]
    fn checked_helpers_refuse_product_and_aggregate_overflow() {
        let overflow = Err(BudgetError::BudgetArithmeticOverflow);
        assert_eq!(checked_mul(u64::MAX, 2), overflow);
        assert_eq!(checked_mul(u64::MAX.div_euclid(96) + 1, 96), overflow);
        assert_eq!(checked_sum([u64::MAX, 1]), overflow);
        assert_eq!(checked_sum([u64::MAX - 2, 1, 2]), overflow);
        assert_eq!(checked_mul(u64::MAX, 1), Ok(u64::MAX));
        assert_eq!(checked_mul(u64::MAX, 0), Ok(0));
        assert_eq!(checked_sum([u64::MAX - 2, 1, 1]), Ok(u64::MAX));
        assert_eq!(checked_sum([]), Ok(0));
    }

    #[test]
    fn deriving_and_refusing_arithmetic_allocate_nothing() {
        let before = allocated_on_this_thread();
        let peak = debug_peak_bytes(u32::MAX);
        let product = checked_mul(u64::MAX, 2);
        let sum = checked_sum([u64::MAX, 1]);
        let after = allocated_on_this_thread();
        assert_eq!(peak, Ok(10_493_088));
        assert_eq!(product, Err(BudgetError::BudgetArithmeticOverflow));
        assert_eq!(sum, Err(BudgetError::BudgetArithmeticOverflow));
        assert_eq!(after, before);
    }
}

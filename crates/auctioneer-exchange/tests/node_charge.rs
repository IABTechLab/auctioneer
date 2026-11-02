//! What a retained `serde_json::Value` tree costs per node, measured
//! against what the isolate budget charges for it.
//!
//! The budget bounds retained request memory with `max_parsed_nodes x
//! NODE_RETAINED_BYTES`, where the node count is SYNTACTIC — every
//! JSON value and every object key, counted as the parser produces
//! them. That charge is a claim about `serde_json::Value`
//! materialization specifically, so it is measured here, in the crate
//! whose per-bidder split retains those trees.
//!
//! The fixtures stress that claim with small map keys, deep arrays,
//! small strings, empty containers, and forests of nested arrays or
//! single-key objects. Every shape must fit with the same 20% margin.
//! Key and string CONTENTS are input copies bounded by the inbound
//! parse term, not by this one, so the fixtures keep them short.
//!
//! # Why Nesting Is Not Exempt
//!
//! A depth limit caps each chain, not the number of chains in a tree.
//! Minimum Vec/BTreeMap allocations therefore remain an input-scalable
//! cost. The former 64-byte charge underestimated the array forest by
//! about 2x and the object forest by about 5x; the latter now binds the
//! 384-byte charge. A larger budget must not hide an undercharge.

/// Test-scoped counting allocator (per-thread, live-bytes) backing the
/// retained-node charge. Delegates every operation to `System`.
mod alloc_track {
    #![expect(
        unsafe_code,
        reason = "a GlobalAlloc impl is unavoidably unsafe; test-only \
                  measurement backing the retained-node memory charge"
    )]

    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    thread_local! {
        static LIVE: Cell<i64> = const { Cell::new(0) };
    }

    /// Bytes currently held by this thread's live allocations. The
    /// difference across a parse is what the parse RETAINED, which is
    /// what the node charge bounds — a cumulative counter would also
    /// count transient parser buffers the tree does not keep.
    pub fn live_on_this_thread() -> i64 {
        LIVE.try_with(Cell::get).unwrap_or(0)
    }

    fn add(bytes: usize) {
        // TLS teardown can make try_with fail; a lost count only
        // relaxes the measurement, never breaks the allocator.
        let _outcome = LIVE.try_with(|cell| {
            cell.set(
                cell.get()
                    .saturating_add(i64::try_from(bytes).unwrap_or(i64::MAX)),
            );
        });
    }

    fn sub(bytes: usize) {
        let _outcome = LIVE.try_with(|cell| {
            cell.set(
                cell.get()
                    .saturating_sub(i64::try_from(bytes).unwrap_or(i64::MAX)),
            );
        });
    }

    pub struct CountingAllocator;

    // SAFETY: every operation delegates verbatim to `System`; the
    // counting side effect touches only a thread-local counter and
    // never the returned memory.
    unsafe impl GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            add(layout.size());
            // SAFETY: same layout contract as the caller's.
            unsafe { System.alloc(layout) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            sub(layout.size());
            // SAFETY: ptr/layout come from a matching System alloc.
            unsafe { System.dealloc(ptr, layout) }
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            add(layout.size());
            // SAFETY: same layout contract as the caller's.
            unsafe { System.alloc_zeroed(layout) }
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            add(new_size);
            sub(layout.size());
            // SAFETY: ptr/layout come from a matching System alloc;
            // new_size is the caller's requested size.
            unsafe { System.realloc(ptr, layout, new_size) }
        }
    }
}

#[global_allocator]
static COUNTING_ALLOC: alloc_track::CountingAllocator = alloc_track::CountingAllocator;

#[cfg(test)]
mod tests {
    use std::iter::repeat_n;
    use std::thread;

    use super::alloc_track::live_on_this_thread;
    use auctioneer_runtime::NODE_RETAINED_BYTES;
    use auctioneer_runtime::bounded_json::value_within;
    use serde_json::{Value, json};

    /// Syntactic nodes in a parsed tree, counted the way the parser
    /// charges them: every value, plus every object key.
    fn count_nodes(value: &Value) -> u64 {
        match value {
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => 1,
            Value::Array(items) => items.iter().map(count_nodes).fold(1, u64::saturating_add),
            // One for the object, then one key node and one value
            // subtree per entry.
            Value::Object(entries) => entries.iter().fold(1, |total, (_key, child)| {
                total.saturating_add(1).saturating_add(count_nodes(child))
            }),
        }
    }

    /// Bytes the parse of `body` retained, and the node count of the
    /// tree it produced.
    ///
    /// The measurement runs on a THREAD OF ITS OWN. The counter is
    /// thread-local and a fresh thread starts at zero, so the reading
    /// covers this parse and nothing else — no harness bookkeeping and
    /// no neighbouring test can land inside the window. Measuring on
    /// the harness's own thread made the result depend on whatever ran
    /// before it, which is not a property of the tree being measured.
    fn retained_cost(body: &[u8]) -> (u64, u64) {
        let owned = body.to_vec();
        thread::spawn(move || {
            let before = live_on_this_thread();
            let parsed = value_within(&owned, u32::MAX).expect("fixture parses within budget");
            let after = live_on_this_thread();
            let retained = u64::try_from(after.saturating_sub(before)).unwrap_or(0);
            let nodes = count_nodes(&parsed);
            // Count the nodes before the tree drops, so the reading and
            // the count describe the same value.
            drop(parsed);
            (retained, nodes)
        })
        .join()
        .expect("the measuring thread does not panic")
    }

    /// Small-key map flood: the maximum number of map entries per byte.
    fn small_key_map_flood() -> String {
        let entries: Vec<String> = (0_usize..2_000_usize)
            .map(|n| format!("\"k{n}\":0"))
            .collect();
        format!("{{{}}}", entries.join(","))
    }

    /// Deep array nesting at the depth cap: one node per level, the
    /// shape a byte cap alone does not bound. `serde_json` refuses
    /// past 128 levels of its own accord, so the fixture sits just
    /// inside that — the deepest tree the parser will actually build,
    /// which is what the charge has to cover.
    fn deep_array_nesting() -> String {
        let depth = 120_usize;
        let mut src = String::with_capacity(depth.saturating_mul(2).saturating_add(1));
        for _level in 0_usize..depth {
            src.push('[');
        }
        src.push('0');
        for _level in 0_usize..depth {
            src.push(']');
        }
        src
    }

    /// Many small strings: each one its own heap node.
    fn many_small_strings() -> String {
        let items: Vec<String> = (0_usize..2_000_usize)
            .map(|n| format!("\"s{n}\""))
            .collect();
        format!("[{}]", items.join(","))
    }

    /// Empty-container flood: the cheapest possible node in bytes, so
    /// the charge per node is at its most exposed.
    fn empty_container_flood() -> String {
        let items: Vec<&str> = (0_usize..2_000_usize)
            .map(|n| if n.trailing_zeros() > 0 { "{}" } else { "[]" })
            .collect();
        format!("[{}]", items.join(","))
    }

    fn nested_array_forest(depth: u8, width: usize) -> String {
        let mut tree = json!(0_i32);
        for _level in 0_u8..depth {
            tree = json!([tree]);
        }
        serde_json::to_string(&vec![tree; width]).expect("serialize array forest")
    }

    fn singleton_object_forest(depth: u8, width: usize) -> String {
        let mut tree = json!(0_i32);
        for _level in 0_u8..depth {
            tree = json!({"a": tree});
        }
        serde_json::to_string(&vec![tree; width]).expect("serialize object forest")
    }

    /// Single-entry object flood: each `{"a":0}` takes a whole
    /// 632-byte `BTreeMap` leaf for one entry, over the 3 syntactic
    /// nodes it costs. The shape the empty-container and dense-map
    /// fixtures between them miss.
    fn single_entry_object_flood() -> String {
        let items: Vec<&str> = repeat_n("{\"a\":0}", 20_000_usize).collect();
        format!("[{}]", items.join(","))
    }

    /// Single-item array flood: each `[0]` takes a `Vec` rounded up to
    /// capacity 4 for one item.
    fn single_item_array_flood() -> String {
        let items: Vec<&str> = repeat_n("[0]", 20_000_usize).collect();
        format!("[{}]", items.join(","))
    }

    /// Chains of single-entry objects, repeated: the worst bytes-per-node
    /// ratio any shape reaches, and the fixture that sets the charge.
    ///
    /// One level is an object plus its key — 2 syntactic nodes — and
    /// takes a full `BTreeMap` leaf, so the chain holds 632 bytes per 2
    /// nodes however long it runs. Repeating chains at the parser's own
    /// depth limit makes the node count unbounded at that ratio, which
    /// is what stops the depth limit from excusing this shape the way it
    /// excuses a single deep array.
    fn nested_single_entry_object_chains() -> String {
        let depth = 120_usize;
        let mut chain = String::from("0");
        for level in 0_usize..depth {
            // Distinct short keys, so no level is a repeat of another.
            let offset = u8::try_from(level.wrapping_rem(26)).unwrap_or(0);
            let key = char::from(b'a'.saturating_add(offset));
            chain = format!("{{\"{key}\":{chain}}}");
        }
        let chains: Vec<&str> = repeat_n(chain.as_str(), 2_000_usize).collect();
        format!("[{}]", chains.join(","))
    }

    /// The charge below is measured against a `BTreeMap`-backed
    /// `serde_json::Map`, which is what `serde_json` uses unless the
    /// `preserve_order` feature is on. That feature swaps in an
    /// `IndexMap`, which retains about twice as much for a key-dense
    /// tree — measured at 104.6 B/node against the 51.9 B/node a
    /// `BTreeMap` retains for the same 4001-node fixture. The node
    /// charge is derived from `BTreeMap` shapes, so keep its backing
    /// representation explicit even if a different map happens to fit.
    ///
    /// auctioneer never enables `preserve_order`, but Cargo unifies
    /// features across a workspace: any member that pulls a dependency
    /// wanting it turns it on for every crate in the build. That has
    /// happened once already (a template engine behind a CLI feature),
    /// and it surfaced as this guard failing with a number nobody could
    /// explain. Assert the precondition here so the next occurrence
    /// names itself instead.
    fn assert_btreemap_backed() {
        let parsed: Value = serde_json::from_str(r#"{"b":0,"a":0}"#).expect("the probe parses");
        let keys: Vec<&str> = parsed
            .as_object()
            .expect("the probe is an object")
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            ["a", "b"],
            "serde_json::Map kept insertion order, so `preserve_order` is \
             enabled and the map is IndexMap-backed. Every figure below is \
             measured against a BTreeMap; re-measure the charge before \
             trusting this guard. Find the feature with \
             `cargo tree --workspace -e features | grep -B14 'preserve_order'`."
        );
    }

    #[test]
    fn parsed_node_charge_guard() {
        assert_btreemap_backed();
        let fixtures = [
            ("small-key map flood", small_key_map_flood()),
            ("deep array nesting", deep_array_nesting()),
            ("many small strings", many_small_strings()),
            ("empty-container flood", empty_container_flood()),
            ("one singleton array", nested_array_forest(1_u8, 1_usize)),
            ("shallow array forest", nested_array_forest(4_u8, 16_usize)),
            (
                "nested array forest",
                nested_array_forest(100_u8, 128_usize),
            ),
            (
                "one singleton object",
                singleton_object_forest(1_u8, 1_usize),
            ),
            (
                "shallow object forest",
                singleton_object_forest(4_u8, 16_usize),
            ),
            (
                "nested object forest",
                singleton_object_forest(100_u8, 128_usize),
            ),
            ("single-item array flood", single_item_array_flood()),
            ("single-entry object flood", single_entry_object_flood()),
            (
                "nested single-entry object chains",
                nested_single_entry_object_chains(),
            ),
        ];

        let mut worst_milli = 0_u64;
        let mut violations = Vec::new();
        for (name, body) in &fixtures {
            let (retained, nodes) = retained_cost(body.as_bytes());
            let charged = nodes
                .checked_mul(NODE_RETAINED_BYTES)
                .expect("the charge does not overflow");
            let required = retained
                .checked_mul(6_u64)
                .expect("measurement plus margin fits")
                .div_ceil(5_u64);
            // Integer thousandths, so the diagnostic carries the real
            // per-node figure without a float conversion.
            let per_node_milli = retained
                .saturating_mul(1_000)
                .checked_div(nodes)
                .unwrap_or(0);

            worst_milli = worst_milli.max(per_node_milli);
            if required > charged {
                violations.push(format!(
                    "{name}: a {nodes}-node tree retained {retained} B \
                     ({required} B with 20% margin), over the \
                     {charged} B the budget charges at NODE_RETAINED_BYTES = \
                     {NODE_RETAINED_BYTES} ({per_node_milli} milli-B/node observed)"
                ));
            }
        }
        assert!(violations.is_empty(), "{}", violations.join("\n"));

        // The charge is derived from one shape, so that shape has to be
        // the one that binds. A fixture set whose worst ratio sits far
        // under the charge would pass the loop above while leaving the
        // charge unjustified — which is how the previous 64 survived.
        assert!(
            worst_milli > 300_000,
            "no fixture reached 300 milli-B/node (worst was {worst_milli}); the \
             object-chain shape the charge is derived from is no longer being \
             measured, so the charge is unsupported by this guard"
        );
        // The asymptote for a single-entry object chain is
        // `(632 + K) / 2` for a K-byte key: the 632-byte BTreeMap leaf
        // plus the key `String`'s own allocation, over the 2 syntactic
        // nodes the object and its key make. These fixtures use 1-byte
        // keys, so the bound is 316.5 and not the 632/2 = 316 the
        // leaf alone implies. A measurement above it means the leaf
        // size or the node counting changed and the derivation needs
        // redoing, not that the margin should absorb it.
        //
        // The ceiling is on the KEY LENGTH these fixtures use, not on
        // what the charge funds — `NODE_RETAINED_BYTES` is sound at
        // every K, see `bounded_json`'s derivation. A fixture with
        // longer keys legitimately measures higher and belongs with its
        // own bound rather than under this one.
        assert!(
            worst_milli <= 316_500,
            "a fixture retained {worst_milli} milli-B/node, past the 316_500 that \
             a 632-byte BTreeMap leaf plus a 1-byte key over 2 nodes implies. \
             Re-derive the charge from the new leaf size rather than widening \
             the margin"
        );
    }
}

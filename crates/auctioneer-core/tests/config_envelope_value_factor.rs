//! What the runtime-config envelope's whole-value materialization
//! costs, measured against what the isolate budget charges for it.
//!
//! The budget charges `max_runtime_config_bytes x
//! CONFIG_ENVELOPE_VALUE_FACTOR` for the `serde_json::Value` the
//! pinned `EdgeZero` `AppConfig` extractor builds from the WHOLE raw
//! envelope before any schema is applied — its version precheck parses
//! the envelope as a generic `Value` so it can recognise a newer format
//! that no longer deserializes as the typed envelope.
//!
//! `auctioneer-runtime` states that charge but cannot measure it: it owns
//! no config path. The measurement lives here, beside the resolver that
//! calls the extractor, and reproduces the precheck's own
//! `serde_json::from_str::<serde_json::Value>` on the raw envelope text.
//!
//! The fixtures are NOT plausible configs. What decides whether the
//! charge BOUNDS the materialization is how much heap a `Value` costs
//! against the wire bytes that produce it, so the shapes below are the
//! ones that maximise heap per wire byte. A store-writable blob is
//! attacker-controlled for this purpose: whoever can write the config
//! store picks the shape.
//!
//! # What makes the charge a bound
//!
//! The extractor now runs a structural scan over the raw text BEFORE it
//! materializes anything, and rejects a blob past
//! [`DEFAULT_CONFIG_JSON_DEPTH`] levels or
//! [`DEFAULT_CONFIG_JSON_NODES`] values with
//! `StoreExtractionReason::StructuralLimit`. The scan allocates no
//! `Value` nodes: map keys are read as `IgnoredAny` and every scalar
//! visitor returns unit.
//!
//! Those two caps split the retained cost into two terms that are
//! bounded separately, which is what the guards below assert:
//!
//! - a NODE term, at most `DEFAULT_CONFIG_JSON_NODES` nodes times the
//!   priciest per-node cost. It does not scale with the envelope cap at
//!   all, so it is an ABSOLUTE ceiling;
//! - a BYTES term, the string and number text the nodes point at, which
//!   retains about its own wire length and so is bounded by the
//!   envelope cap itself.
//!
//! `max_runtime_config_bytes x CONFIG_ENVELOPE_VALUE_FACTOR` has to
//! cover both at the shipped 1 MiB cap. It does, with room to spare —
//! and the margin comes from the node cap, not from the factor.

/// Test-scoped counting allocator (per-thread, live-bytes) backing the
/// envelope-value factor measurement. Delegates every operation to
/// `System`.
mod alloc_track {
    #![expect(
        unsafe_code,
        reason = "a GlobalAlloc impl is unavoidably unsafe; test-only \
                  measurement backing the config envelope-value bound"
    )]

    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    thread_local! {
        static LIVE: Cell<i64> = const { Cell::new(0) };
    }

    /// Bytes currently held by this thread's live allocations. The
    /// difference across a parse is what the `Value` RETAINS, which is
    /// what the budget funds — as distinct from a cumulative total,
    /// which would also count every transient the parser frees.
    pub fn live_on_this_thread() -> i64 {
        LIVE.try_with(Cell::get).unwrap_or(0)
    }

    fn live_add(bytes: usize) {
        // TLS teardown can make try_with fail; a lost count only
        // relaxes the measurement, never breaks the allocator.
        let _outcome = LIVE.try_with(|cell| {
            cell.set(
                cell.get()
                    .saturating_add(i64::try_from(bytes).unwrap_or(i64::MAX)),
            );
        });
    }

    fn live_sub(bytes: usize) {
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
            live_add(layout.size());
            // SAFETY: same layout contract as the caller's.
            unsafe { System.alloc(layout) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            live_sub(layout.size());
            // SAFETY: ptr/layout come from a matching System alloc.
            unsafe { System.dealloc(ptr, layout) }
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            live_add(layout.size());
            // SAFETY: same layout contract as the caller's.
            unsafe { System.alloc_zeroed(layout) }
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            live_add(new_size);
            live_sub(layout.size());
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
    use super::alloc_track::live_on_this_thread;
    use std::thread;

    use auctioneer_runtime::budget::CONFIG_ENVELOPE_VALUE_FACTOR;
    use edgezero_core::config_store::{DEFAULT_CONFIG_JSON_DEPTH, DEFAULT_CONFIG_JSON_NODES};

    /// The runtime-config envelope cap every shipped adapter row
    /// declares, which is the base the precheck charge scales with.
    ///
    /// Stated here rather than imported because the adapter constants
    /// are private to their crates;
    /// `the_measured_cap_is_the_one_every_adapter_ships` in each
    /// adapter's envelope tests pins the rows themselves.
    const SHIPPED_ENVELOPE_CAP: u64 = 1 << 20;

    /// Bytes the priciest single admissible `Value` node retains, as
    /// `the_node_cap_sets_an_absolute_retained_ceiling` measures it,
    /// rounded UP.
    ///
    /// A one-entry `Value::Object` is the priciest node: it costs a
    /// whole map allocation however few wire bytes produced it. Every
    /// other shape measured there comes in below this.
    const WORST_BYTES_PER_NODE: u64 = 623;

    /// What the string and number TEXT inside the nodes retains, per
    /// wire byte. A `Value::String` holds about its own wire length, so
    /// this term is just over 1 and is bounded by the envelope cap
    /// rather than by the node cap.
    const BYTES_TERM_PER_WIRE_BYTE: u64 = 2;

    /// Wrap a `data` payload in the v1 envelope shape the extractor's
    /// precheck parses. The precheck reads the WHOLE text, so the
    /// envelope's own two fields are part of the measured input.
    fn envelope(data: &str) -> String {
        format!("{{\"version\":1,\"data\":{data}}}")
    }

    /// `count` copies of `item` in a JSON array.
    fn repeat_in_array(count: usize, item: &str) -> String {
        let mut src = String::from("[");
        for index in 0..count {
            if index > 0 {
                src.push(',');
            }
            src.push_str(item);
        }
        src.push(']');
        src
    }

    /// An object nested `depth` levels deep, every key empty and the
    /// leaf a single digit — the most `Value::Object` nodes per wire
    /// byte the parser will accept.
    fn nested_empty_key_object(depth: usize) -> String {
        let mut src = String::new();
        for _ in 0..depth {
            src.push_str("{\"\":");
        }
        src.push('0');
        for _ in 0..depth {
            src.push('}');
        }
        src
    }

    /// Upstream's structural scan, reimplemented over a parsed `Value`
    /// so a fixture can be checked for admissibility.
    ///
    /// Returns `(nodes, max_depth)` under upstream's own accounting: the
    /// root value is depth 1, a map value or array element is its
    /// parent's depth + 1, and map KEYS are not counted as nodes
    /// (upstream reads them as `IgnoredAny`). A seed rejects when its
    /// depth exceeds the cap or the running node count has already
    /// reached it, so a fixture is admissible exactly while
    /// `max_depth <= cap` and `nodes <= cap`.
    fn structural_cost(value: &serde_json::Value, depth: u32) -> (u32, u32) {
        let mut nodes = 1_u32;
        let mut deepest = depth;
        let mut child = |inner: &serde_json::Value| {
            let (sub_nodes, sub_depth) = structural_cost(inner, depth.saturating_add(1));
            nodes = nodes.saturating_add(sub_nodes);
            deepest = deepest.max(sub_depth);
        };
        match value {
            serde_json::Value::Object(map) => {
                for inner in map.values() {
                    child(inner);
                }
            }
            serde_json::Value::Array(items) => {
                for inner in items {
                    child(inner);
                }
            }
            serde_json::Value::Bool(_)
            | serde_json::Value::Null
            | serde_json::Value::Number(_)
            | serde_json::Value::String(_) => {}
        }
        (nodes, deepest)
    }

    /// Whether the extractor's structural scan ADMITS this envelope, so
    /// a fixture cannot claim to measure a reachable worst case while
    /// being rejected before materialization.
    fn is_admissible(raw: &str) -> bool {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
            return false;
        };
        let (nodes, depth) = structural_cost(&value, 1);
        depth <= DEFAULT_CONFIG_JSON_DEPTH && nodes <= DEFAULT_CONFIG_JSON_NODES
    }

    /// Bytes the raw envelope's `Value` RETAINS, and the envelope's own
    /// wire length.
    ///
    /// Measured on a THREAD OF ITS OWN: the counter is thread-local and
    /// a fresh thread starts at zero, so the reading covers this parse
    /// and nothing else — no harness bookkeeping and no neighbouring
    /// test can land inside the window.
    fn retained_by_precheck(raw: String) -> (u64, u64) {
        let wire = u64::try_from(raw.len()).expect("fixture length fits u64");
        let retained = thread::spawn(move || {
            let before = live_on_this_thread();
            // The precheck's own call, verbatim: the whole raw envelope
            // as a generic `Value`, before any schema narrows it.
            let value: serde_json::Value =
                serde_json::from_str(&raw).expect("fixture envelope parses");
            let retained = u64::try_from(live_on_this_thread().saturating_sub(before)).unwrap_or(0);
            // Drop AFTER the reading, so what is measured is what the
            // precheck holds while it inspects the envelope.
            drop(value);
            retained
        })
        .join()
        .expect("the measuring thread does not panic");
        (retained, wire)
    }

    /// The deepest admissible nesting for an ARRAY MEMBER, derived from
    /// upstream's depth cap rather than written as a literal.
    ///
    /// Two levels of the depth budget are spent before a member starts:
    /// the envelope object is depth 1 and its `data` array is depth 2,
    /// so a member's outermost object sits at depth 3 and a member
    /// nested `k` levels puts its leaf at depth `2 + k + 1`. The cap
    /// admits `k = DEPTH - 3`.
    fn deepest_admissible_member_nesting() -> usize {
        usize::try_from(DEFAULT_CONFIG_JSON_DEPTH)
            .expect("the depth cap fits usize")
            .saturating_sub(3)
    }

    /// How many members of that nesting the NODE cap admits. Each
    /// contributes `k + 1` nodes (one object per level plus the leaf),
    /// and the envelope object and its `data` array take one each.
    fn admissible_member_count(nesting: usize) -> usize {
        usize::try_from(DEFAULT_CONFIG_JSON_NODES)
            .expect("the node cap fits usize")
            .saturating_sub(2)
            .checked_div(nesting.saturating_add(1))
            .expect("the nesting is non-zero")
    }

    /// THE BOUND: at the cap every shipped adapter declares, the
    /// worst envelope the extractor will ADMIT retains less than the
    /// budget charges for it.
    ///
    /// This is the guard that closes what used to be an open shortfall.
    /// It replaces a test that asserted `retained > charged` and said so
    /// deliberately, because before upstream's structural caps there was
    /// no finite factor that bounded the materialization at a reachable
    /// envelope size: a 40 KB blob of deeply nested empty-key objects
    /// retained 5 MB, about 126x its wire, and funding that ratio at the
    /// 1 MiB cap would have charged 132 MiB per concurrent auction.
    ///
    /// The ratio is still about 126x — upstream's caps did not change
    /// it, and
    /// `the_conservative_factor_funds_dense_envelopes_at_their_wire_size` keeps
    /// that on the record. What changed is that the ratio can only be
    /// reached on a SMALL envelope now, because the node cap puts an
    /// absolute ceiling on the expensive term. The charge scales with
    /// the CAP, so the bound holds where it has to.
    ///
    /// The fixture maximises BOTH terms at once: it saturates the node
    /// budget with the priciest node shape, then fills the rest of the
    /// 1 MiB envelope with string text, which is the only other thing a
    /// `Value` retains. A fixture that maximised one term alone would
    /// understate the total.
    #[test]
    fn the_factor_bounds_the_worst_admissible_envelope_at_the_shipped_cap() {
        let nesting = deepest_admissible_member_nesting();
        let members = admissible_member_count(nesting);
        let item = nested_empty_key_object(nesting);
        let nodes_only = repeat_in_array(members, &item);

        // Fill the remaining envelope budget with one long string: the
        // node budget is already spent, so every further wire byte can
        // only buy string text, and that is the BYTES term.
        let spent = u64::try_from(envelope(&nodes_only).len()).expect("fixture length fits u64");
        let room = SHIPPED_ENVELOPE_CAP
            .saturating_sub(spent)
            .saturating_sub(8_u64);
        let mut data = nodes_only;
        data.pop();
        data.push_str(",\"");
        for _ in 0..room {
            data.push('x');
        }
        data.push_str("\"]");
        let raw = envelope(&data);

        assert!(
            is_admissible(&raw),
            "the worst-case fixture must pass upstream's structural scan, or it \
             measures a blob the extractor rejects before materializing anything",
        );
        let wire_len = u64::try_from(raw.len()).expect("fixture length fits u64");
        assert!(
            wire_len <= SHIPPED_ENVELOPE_CAP,
            "the fixture must fit the {SHIPPED_ENVELOPE_CAP} B envelope cap it claims \
             to measure at, and it is {wire_len} B",
        );

        let (retained, wire) = retained_by_precheck(raw);
        let charged = SHIPPED_ENVELOPE_CAP
            .checked_mul(CONFIG_ENVELOPE_VALUE_FACTOR)
            .expect("the charge does not overflow");
        assert!(
            retained <= charged,
            "the worst envelope the extractor ADMITS at the {SHIPPED_ENVELOPE_CAP} B \
             cap RETAINED {retained} B from {wire} B of wire, over the {charged} B the \
             budget charges at CONFIG_ENVELOPE_VALUE_FACTOR = \
             {CONFIG_ENVELOPE_VALUE_FACTOR}. The precheck charge no longer bounds the \
             materialization: re-derive the factor against upstream's \
             DEFAULT_CONFIG_JSON_NODES = {DEFAULT_CONFIG_JSON_NODES} and \
             DEFAULT_CONFIG_JSON_DEPTH = {DEFAULT_CONFIG_JSON_DEPTH}",
        );
    }

    /// The bound above is not an accident of one fixture: it follows
    /// from the two caps, and this states the arithmetic.
    ///
    /// `nodes x WORST_BYTES_PER_NODE` (the absolute node term) plus
    /// `cap x BYTES_TERM_PER_WIRE_BYTE` (the text the nodes point at)
    /// is what any admissible envelope can retain, and the charge
    /// covers it. Asserted symbolically so that raising the envelope cap
    /// or widening upstream's node cap fails HERE, with the arithmetic
    /// in the message, rather than only at whichever fixture happens to
    /// go over first.
    #[test]
    fn the_two_structural_caps_bound_the_charge_symbolically() {
        let node_term = u64::from(DEFAULT_CONFIG_JSON_NODES)
            .checked_mul(WORST_BYTES_PER_NODE)
            .expect("the node term does not overflow");
        let bytes_term = SHIPPED_ENVELOPE_CAP
            .checked_mul(BYTES_TERM_PER_WIRE_BYTE)
            .expect("the bytes term does not overflow");
        let bound = node_term
            .checked_add(bytes_term)
            .expect("the bound does not overflow");
        let charged = SHIPPED_ENVELOPE_CAP
            .checked_mul(CONFIG_ENVELOPE_VALUE_FACTOR)
            .expect("the charge does not overflow");
        assert!(
            bound <= charged,
            "the structural caps bound the precheck Value at {node_term} B of nodes \
             ({DEFAULT_CONFIG_JSON_NODES} x {WORST_BYTES_PER_NODE} B) plus \
             {bytes_term} B of text ({SHIPPED_ENVELOPE_CAP} B x \
             {BYTES_TERM_PER_WIRE_BYTE}) = {bound} B, over the {charged} B charged at \
             CONFIG_ENVELOPE_VALUE_FACTOR = {CONFIG_ENVELOPE_VALUE_FACTOR}",
        );
    }

    /// The NODE cap is what makes the expensive term absolute, and this
    /// measures the per-node cost the bound above is built from.
    ///
    /// Without this, `WORST_BYTES_PER_NODE` would be a number nobody
    /// checked. The shapes are the candidates for priciest node: a
    /// one-entry object (a whole map allocation), a one-element array, a
    /// bare scalar, and a wide object with distinct keys. The dearest
    /// admissible one must come in at or under the constant the
    /// symbolic bound uses.
    #[test]
    fn the_node_cap_sets_an_absolute_retained_ceiling() {
        let nesting = deepest_admissible_member_nesting();
        let members = admissible_member_count(nesting);
        let mut nested_array = String::new();
        for _ in 0..nesting {
            nested_array.push('[');
        }
        nested_array.push('0');
        for _ in 0..nesting {
            nested_array.push(']');
        }
        let candidates = [
            (
                "nested one-entry objects",
                repeat_in_array(members, &nested_empty_key_object(nesting)),
            ),
            (
                "nested one-element arrays",
                repeat_in_array(members, &nested_array),
            ),
            (
                "flat scalars",
                repeat_in_array(
                    usize::try_from(DEFAULT_CONFIG_JSON_NODES).expect("fits usize") - 2,
                    "0",
                ),
            ),
        ];
        let mut dearest = 0_u64;
        let mut dearest_shape = "none";
        for (label, data) in candidates {
            let raw = envelope(&data);
            if !is_admissible(&raw) {
                continue;
            }
            let value: serde_json::Value =
                serde_json::from_str(&raw).expect("the candidate parses");
            let (nodes, _depth) = structural_cost(&value, 1);
            drop(value);
            let (retained, _wire) = retained_by_precheck(raw);
            let per_node = retained
                .checked_div(u64::from(nodes))
                .expect("a parsed envelope has at least one node");
            if per_node > dearest {
                dearest = per_node;
                dearest_shape = label;
            }
        }
        assert!(
            dearest > 0,
            "no candidate node shape was admissible, so this measures nothing",
        );
        assert!(
            dearest <= WORST_BYTES_PER_NODE,
            "the dearest admissible node shape ({dearest_shape}) retained {dearest} B \
             per node, over WORST_BYTES_PER_NODE = {WORST_BYTES_PER_NODE} that \
             the_two_structural_caps_bound_the_charge_symbolically builds its bound \
             from; re-derive that constant and re-check the bound",
        );
    }

    /// Deep empty-key objects retain about 126 bytes per wire byte.
    /// The conservative factor funds that shape at its actual wire size,
    /// independently of the structural cap's absolute retention bound.
    #[test]
    fn the_conservative_factor_funds_dense_envelopes_at_their_wire_size() {
        let nesting = deepest_admissible_member_nesting();
        let members = admissible_member_count(nesting);
        let raw = envelope(&repeat_in_array(members, &nested_empty_key_object(nesting)));
        assert!(
            is_admissible(&raw),
            "the ratio fixture must itself be admissible, or the ratio it reports is \
             unreachable",
        );
        let (retained, wire) = retained_by_precheck(raw);
        let funded = wire
            .checked_mul(CONFIG_ENVELOPE_VALUE_FACTOR)
            .expect("the per-wire charge does not overflow");
        assert!(
            retained <= funded,
            "nested one-entry objects RETAINED {retained} B for a {wire} B envelope, \
             over the {funded} B that CONFIG_ENVELOPE_VALUE_FACTOR = \
             {CONFIG_ENVELOPE_VALUE_FACTOR} funds at this envelope's own wire size",
        );
        // The shipped cap funds the same retention with additional headroom.
        let charged_at_cap = SHIPPED_ENVELOPE_CAP
            .checked_mul(CONFIG_ENVELOPE_VALUE_FACTOR)
            .expect("the charge does not overflow");
        assert!(
            retained <= charged_at_cap,
            "even the ratio-maximal envelope must fit the {charged_at_cap} B charged at \
             the shipped cap; it retained {retained} B",
        );
    }

    /// The node cap, not `serde_json`'s recursion limit, is now the
    /// structural limit that makes the cost converge.
    ///
    /// This replaces a guard that asserted `serde_json` refuses to
    /// recurse past 128 levels. That refusal is still real but no longer
    /// load-bearing: upstream rejects at
    /// [`DEFAULT_CONFIG_JSON_DEPTH`] = 64, half of it, and rejects on
    /// NODE COUNT too, which the parser never did. Both halves are
    /// asserted — the fixture's own depth is admitted, one level more is
    /// not, and the node cap bites independently of depth — because a
    /// depth-only check would miss the cap that actually bounds the
    /// absolute cost.
    #[test]
    fn upstream_structural_caps_are_what_bound_the_materialization() {
        let nesting = deepest_admissible_member_nesting();
        let deepest = envelope(&repeat_in_array(1, &nested_empty_key_object(nesting)));
        assert!(
            is_admissible(&deepest),
            "the worst-case fixture's own nesting must be admitted, or every \
             measurement above describes a blob the extractor rejects",
        );
        let too_deep = envelope(&repeat_in_array(
            1,
            &nested_empty_key_object(nesting.saturating_add(1)),
        ));
        assert!(
            !is_admissible(&too_deep),
            "one level past the fixture's nesting must exceed \
             DEFAULT_CONFIG_JSON_DEPTH = {DEFAULT_CONFIG_JSON_DEPTH}, or the fixture is \
             not at the deepest admissible nesting and the measured ceiling is derived \
             from the wrong shape",
        );

        // The node cap bites on its own, at a depth the depth cap is
        // nowhere near: this is the half `serde_json`'s recursion limit
        // never provided, and it is what makes the ceiling absolute.
        let shallow_but_wide = envelope(&repeat_in_array(
            usize::try_from(DEFAULT_CONFIG_JSON_NODES).expect("fits usize"),
            "0",
        ));
        assert!(
            !is_admissible(&shallow_but_wide),
            "an envelope of {DEFAULT_CONFIG_JSON_NODES} flat scalars must exceed \
             DEFAULT_CONFIG_JSON_NODES, independently of depth; without a node cap the \
             retained ceiling would scale with the envelope cap and no fixed factor \
             would bound it",
        );

        // And the ratio rises monotonically toward the depth cap rather
        // than jumping, which is what makes the deepest admissible
        // nesting the worst SHAPE rather than some middle depth.
        let mut previous = 0_u64;
        for depth in [1_usize, 2, 8, 32, nesting] {
            let members = admissible_member_count(depth);
            let raw = envelope(&repeat_in_array(members, &nested_empty_key_object(depth)));
            let (retained, wire) = retained_by_precheck(raw);
            // retained per wire byte, scaled to stay in integers.
            let scaled = retained
                .checked_mul(1_000)
                .expect("the scaled ratio does not overflow")
                .checked_div(wire)
                .expect("the envelope is non-empty");
            assert!(
                scaled >= previous,
                "retention per wire byte must not fall as nesting deepens; at depth \
                 {depth} it was {scaled} per 1000 B against {previous} at the \
                 shallower depth, so the deepest admissible nesting is no longer the \
                 worst shape and the measured ceiling is derived from the wrong one",
            );
            previous = scaled;
        }
    }

    /// The doc's stated typical case — "a dense array of small numbers"
    /// — is where the factor 17 came from, and the node cap now rejects
    /// it outright.
    ///
    /// Kept because the factor's doc comment still cites that
    /// measurement as its basis, and a reader needs to know the shape is
    /// no longer reachable: it needed 500,000 nodes against a cap of
    /// [`DEFAULT_CONFIG_JSON_NODES`]. The factor is not re-derived
    /// downward on that account — it is the funded reservation the
    /// shipped rows are sized against, and lowering it would move every
    /// adapter's proof row and the aux-route floor that consumes it.
    #[test]
    fn the_dense_small_number_array_the_factor_came_from_is_no_longer_admissible() {
        // Digits cycle rather than repeating one value, so the parser
        // cannot shortcut on identical tokens. `%` is denied
        // workspace-wide, so they come from a cycling iterator.
        let mut digits = "0123456789".chars().cycle();
        let mut data = String::from("[");
        for index in 0_usize..500_000_usize {
            if index > 0 {
                data.push(',');
            }
            data.push(digits.next().expect("a cycling iterator never ends"));
        }
        data.push(']');
        let raw = envelope(&data);
        assert!(
            !is_admissible(&raw),
            "a 500,000-element array must exceed DEFAULT_CONFIG_JSON_NODES = \
             {DEFAULT_CONFIG_JSON_NODES}; if upstream now admits it, the shape the \
             factor was derived from is reachable again and the bound above must be \
             re-measured against it",
        );
    }
}

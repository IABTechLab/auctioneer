//! Parser-time retained-node budget for untrusted JSON.
//!
//! A byte cap alone does not bound what a parse *retains*: `[[[[…]]]]`
//! and `{"a":{"a":…}}` both fit in a few KiB while expanding into one
//! heap node per nesting level. This module parses through a
//! `Deserializer` wrapper that counts syntactic nodes as they are
//! produced and aborts the moment the count passes `max_nodes`.
//!
//! Counting is **syntactic**, not "nodes the target type kept": every
//! JSON value and every object key counts, including subtrees the
//! target type discards. A typed target that ignores an unknown field
//! still pays for the whole subtree under it — that subtree is what the
//! parser had to walk, and under `serde_json`'s `Value` it is what a
//! `Value` target would have retained.
//!
//! The budget is therefore stable across entry points: the same bytes
//! cost the same whether they are parsed into a narrow struct or into
//! [`serde_json::Value`].
//!
//! # Enums
//!
//! Only serde's **externally tagged** representation reaches the
//! `EnumAccess` machinery, and it is counted shape by shape so a typed
//! enum target costs what the same bytes cost as a `Value`:
//! `{"Tag":1}` is three nodes, `{"Tag":[1,2]}` and `{"Tag":{"a":1}}`
//! are five, and the bare string `"Tag"` is one.
//!
//! The internally tagged, adjacently tagged and untagged
//! representations need no special handling: serde buffers them
//! through its own content model and they arrive as ordinary JSON
//! objects, maps and sequences, which the map and sequence accessors
//! already charge. They are covered by tests rather than by code.
//!
//! One shape is knowingly undercounted. A unit variant spelled as the
//! object `{"Tag":null}` costs three nodes as a `Value` but is charged
//! one, because it cannot be told apart from the bare string `"Tag"`
//! at the point the charge would be made. `serde_json` accepts only
//! `null` there, so the gap is a fixed two nodes on a fixed-size input
//! and carries no subtree.

use serde::de::{
    DeserializeOwned, DeserializeSeed, EnumAccess, Error as DeError, IgnoredAny, MapAccess,
    SeqAccess, VariantAccess, Visitor,
};
use serde::{Deserialize, Deserializer};
use serde_json::value::RawValue;
use std::cell::Cell;
use std::error::Error as StdError;
use std::fmt;
use std::marker::PhantomData;
use std::mem::{align_of, size_of};

/// Bytes charged against a memory budget per retained JSON node.
///
/// Covers container allocations, not just `Value`'s inline size. A
/// one-child `BTreeMap` still allocates a whole leaf, and a one-child
/// `Vec` retains its minimum capacity. A forest of single-key object
/// chains measured 8,106,496 retained bytes for 25,729 syntactic nodes
/// (315.073 bytes/node) on the native 64-bit target. The corresponding
/// array forest measured 127.040 bytes/node. Depth limits do not bound
/// the number of those chains that can coexist in one request.
///
/// Round the binding measurement plus 20% upward to 384 bytes/node.
/// `auctioneer-exchange/tests/node_charge.rs` guards the same margin for
/// every fixture, including nested forests. String/key contents are
/// input copies charged separately by the inbound-parse term. This
/// charge bounds retained `Value` trees, not typed `OpenRTB` objects or
/// all parser/transformation scratch.
pub const NODE_RETAINED_BYTES: u64 = 384;

/// The parse produced more syntactic nodes than the caller allowed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeBudgetExceeded {
    /// The `max_nodes` the parse was given. The parse aborts on the
    /// first node past it, so the true node count is not reported —
    /// counting it would mean walking the input the budget exists to
    /// avoid walking.
    pub budget: u32,
}

impl fmt::Display for NodeBudgetExceeded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "JSON node budget of {} exceeded", self.budget)
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "the remaining provided methods cannot be written on stable: \
              `provide` is nightly-only (error_generic_member_access), \
              `description` and `cause` are deprecated, and `type_id` is \
              sealed by `Any`"
)]
impl StdError for NodeBudgetExceeded {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        // A budget overrun has no underlying cause: it is the whole
        // failure.
        None
    }
}

/// Why a bounded parse failed.
#[derive(Debug)]
pub enum BoundedJsonError {
    /// The input was not valid JSON, or did not match the target type.
    Syntax(serde_json::Error),
    /// The node budget was exceeded before full input validation.
    NodeBudget(NodeBudgetExceeded),
    /// A target requested raw-value capture or an object key used a reserved
    /// `serde_json` raw-value marker, which this parser cannot safely forward.
    ///
    /// See [`UnbudgetableRawValue`].
    RawValue(UnbudgetableRawValue),
}

impl fmt::Display for BoundedJsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(err) => write!(f, "malformed JSON: {err}"),
            Self::NodeBudget(exceeded) => write!(f, "{exceeded}"),
            Self::RawValue(raw) => write!(f, "{raw}"),
        }
    }
}

/// A private `serde_json` raw-value protocol was requested or encountered
/// inside a budgeted typed parse.
///
/// `serde_json` implements `RawValue` through a PRIVATE newtype-struct
/// marker: `Box<RawValue>`'s `Deserialize` calls
/// `deserialize_newtype_struct` with the name
/// `$serde_json::private::RawValue`, and `serde_json` answers it by
/// capturing the value's raw byte span directly from its own
/// deserializer. That capture never re-enters this wrapper, so the
/// subtree inside it is charged NOTHING beyond the single node the
/// newtype struct itself cost — a budget of 7 admitted a 1.4 MB,
/// 200,001-node `ext` object.
///
/// The capture cannot be routed through the counter either: the raw
/// span is produced by `serde_json`'s own borrowed-string machinery
/// rather than by replaying the value through a `Deserializer`, so
/// there is no nested parse to wrap. Counting it would mean parsing the
/// captured span a SECOND time, which is work the budget exists to
/// prevent and which would charge a tree the caller never retains.
///
/// So the marker is refused in [`from_slice_within`], and the parse fails
/// closed. A reserved object key must also be refused: `Value` can interpret
/// it as an encoded string and start an unwrapped second parse without asking
/// for the private newtype. Ordinary string values do not activate that path.
///
/// Ingress uses [`count_json_nodes`] before its separately owned typed
/// deserialization: `BidRequest`, `Imp`, `Device` and `User` retain opaque
/// `Option<Box<RawValue>>` extensions that this wrapper must not accept.
/// Lexical counting alone does not validate a later decoder's private protocols.
#[derive(Debug)]
pub struct UnbudgetableRawValue {
    /// The private marker refused at the target or object-key boundary.
    name: &'static str,
}

impl UnbudgetableRawValue {
    /// The private `serde_json` marker name that was refused.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }
}

impl fmt::Display for UnbudgetableRawValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "a budgeted parse cannot capture `{}`: the raw span would be \
             retained without being charged against the node budget",
            self.name,
        )
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "the remaining provided methods cannot be written on stable: \
              `provide` is nightly-only (error_generic_member_access), \
              `description` and `cause` are deprecated, and `type_id` is \
              sealed by `Any` - identical to the `NodeBudgetExceeded` impl \
              above"
)]
impl StdError for UnbudgetableRawValue {
    /// No source: the refusal is raised by this parser, not wrapped
    /// around a failure from somewhere else. There is nothing
    /// underneath it to report.
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        None
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "the remaining provided methods cannot be written on stable: \
              `provide` is nightly-only (error_generic_member_access), \
              `description` and `cause` are deprecated, and `type_id` is \
              sealed by `Any`"
)]
impl StdError for BoundedJsonError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Syntax(err) => Some(err),
            Self::NodeBudget(exceeded) => Some(exceeded),
            Self::RawValue(raw) => Some(raw),
        }
    }
}

/// Sentinel `serde_json::Error` message marking a budget abort.
///
/// The counter aborts by raising a custom `serde` error, which
/// `serde_json` returns as its own opaque `Error`. The message is the
/// only channel that survives the round-trip, so it is matched on the
/// way out and re-raised as [`BoundedJsonError::NodeBudget`]. The
/// string is deliberately unlikely to collide with a real message.
const BUDGET_SENTINEL: &str = "auctioneer-runtime/bounded_json: node budget exceeded";

/// Sentinel `serde_json::Error` message marking a refused raw-value
/// capture. Travels out the same way [`BUDGET_SENTINEL`] does, and for
/// the same reason: a custom `serde` error's message is the only
/// channel that survives the round-trip through `serde_json::Error`.
const RAW_VALUE_SENTINEL: &str = "auctioneer-runtime/bounded_json: unbudgetable raw value";

/// The `serde_json` private raw-value marker a budgeted parse cannot honour.
///
/// In pinned `serde_json` 1.0.150, both `&RawValue` and `Box<RawValue>` call
/// `deserialize_newtype_struct` with this same name. The deserializer captures
/// a raw byte span instead of replaying the value. Object keys also reserve
/// this name because `Value` can interpret them as an encoded raw value.
/// Matched by name because `serde_json`'s raw-value token is private.
const RAW_VALUE_MARKERS: [&str; 1] = ["$serde_json::private::RawValue"];

/// Whether `name` is a `serde_json` raw-value capture marker.
fn is_raw_value_marker(name: &str) -> bool {
    RAW_VALUE_MARKERS.contains(&name)
}

/// Shared node counter for one parse.
///
/// `Cell` rather than an atomic: a `Deserializer` is driven from a
/// single thread, and the wrapper is re-created per nested value, so
/// every level must see the same count.
struct NodeCounter {
    seen: Cell<u32>,
    budget: u32,
    refused_raw_value: Cell<Option<&'static str>>,
}

impl NodeCounter {
    fn new(budget: u32) -> Self {
        Self {
            seen: Cell::new(0),
            budget,
            refused_raw_value: Cell::new(None),
        }
    }
}

trait NodeAccounting {
    fn charge<E: DeError>(&self) -> Result<(), E>;
    fn exceeded(&self) -> bool;
    fn limit(&self) -> u32;
    fn refuse_raw_value(&self, name: &'static str);
    fn raw_refusal(&self) -> Option<&'static str>;
}

impl NodeAccounting for NodeCounter {
    /// Charge one syntactic node. Fails once the count passes the
    /// budget — a parse that lands exactly on the budget succeeds.
    fn charge<E>(&self) -> Result<(), E>
    where
        E: DeError,
    {
        let next = self.seen.get().saturating_add(1);
        self.seen.set(next);
        if next > self.budget {
            return Err(E::custom(BUDGET_SENTINEL));
        }
        Ok(())
    }

    /// Whether the budget was passed during this parse.
    fn exceeded(&self) -> bool {
        self.seen.get() > self.budget
    }

    fn limit(&self) -> u32 {
        self.budget
    }

    fn refuse_raw_value(&self, name: &'static str) {
        self.refused_raw_value.set(Some(name));
    }

    fn raw_refusal(&self) -> Option<&'static str> {
        self.refused_raw_value.get()
    }
}

// Keep the original standard parser's fields, size and alignment. Static
// dispatch below must not introduce a trait-object pointer into its wrappers.
const _: () = {
    assert!(
        size_of::<NodeCounter>() == size_of::<(u32, u32, Option<&'static str>)>(),
        "standard node counter must retain its funded size"
    );
    assert!(
        align_of::<NodeCounter>() == align_of::<(u32, u32, Option<&'static str>)>(),
        "standard node counter must retain its funded alignment"
    );
};

struct SharedNodeCounter {
    standard: NodeCounter,
    overflowed: Cell<bool>,
}

impl SharedNodeCounter {
    fn new(budget: u32) -> Self {
        Self {
            standard: NodeCounter::new(budget),
            overflowed: Cell::new(false),
        }
    }

    fn observed_nodes(&self) -> u32 {
        self.standard.seen.get()
    }
}

impl NodeAccounting for SharedNodeCounter {
    fn charge<E: DeError>(&self) -> Result<(), E> {
        if self.standard.seen.get() == u32::MAX {
            self.overflowed.set(true);
            return Err(E::custom(BUDGET_SENTINEL));
        }
        self.standard.charge()
    }

    fn exceeded(&self) -> bool {
        self.overflowed.get() || self.standard.exceeded()
    }

    fn limit(&self) -> u32 {
        self.standard.limit()
    }

    fn refuse_raw_value(&self, name: &'static str) {
        self.standard.refuse_raw_value(name);
    }

    fn raw_refusal(&self) -> Option<&'static str> {
        self.standard.raw_refusal()
    }
}

const _: () = {
    assert!(
        size_of::<CountingDeserializer<'static, &'static (), NodeCounter>>()
            == size_of::<(&'static (), &'static NodeCounter, bool)>(),
        "standard deserializer must retain a thin counter reference"
    );
    assert!(
        size_of::<CountingVisitor<'static, (), NodeCounter>>() == size_of::<&'static NodeCounter>(),
        "standard visitor must retain a thin counter reference"
    );
};

/// `Deserializer` wrapper that charges one node per value it produces.
///
/// Every `deserialize_*` method charges, then forwards to the inner
/// deserializer with a [`CountingVisitor`] wrapping the caller's
/// visitor, so nested values reached through the sequence and map
/// accessors are charged too.
struct CountingDeserializer<'c, D, C> {
    inner: D,
    counter: &'c C,
    /// Whether this value charges a node itself. False only for an
    /// object key, which `CountingMapAccess::next_key_seed` has
    /// already charged.
    charge: bool,
}

/// `Visitor` wrapper that re-wraps the accessors handed to it, so the
/// values inside a sequence or map are charged as well.
struct CountingVisitor<'c, V, C> {
    inner: V,
    counter: &'c C,
}

/// `SeqAccess` wrapper charging each element.
struct CountingSeqAccess<'c, A, C> {
    inner: A,
    counter: &'c C,
}

/// `MapAccess` wrapper charging each key and each value.
struct CountingMapAccess<'c, A, C> {
    inner: A,
    counter: &'c C,
}

/// `EnumAccess` wrapper threading the counter into the variant tag.
struct CountingEnumAccess<'c, A, C> {
    inner: A,
    counter: &'c C,
}

/// `VariantAccess` wrapper charging the variant tag and the container
/// its payload arrives in.
struct CountingVariantAccess<'c, A, C> {
    inner: A,
    counter: &'c C,
}

/// `DeserializeSeed` wrapper threading the counter into a nested value.
struct CountingSeed<'c, S, C> {
    inner: S,
    counter: &'c C,
}

/// `DeserializeSeed` wrapper for an object key, whose node was already
/// charged by `CountingMapAccess::next_key_seed`.
struct KeySeed<'c, S, C> {
    inner: S,
    counter: &'c C,
}

struct MapKeySeed<'c, S, C> {
    inner: S,
    counter: &'c C,
}

struct MapKeyDeserializer<'c, D, C> {
    inner: CountingDeserializer<'c, D, C>,
}

struct MapKeyVisitor<'c, V, C> {
    inner: V,
    counter: &'c C,
}

macro_rules! forward_map_key {
    ($($method:ident $(<$($arg:ident: $ty:ty),+>)? ,)*) => {
        $(
            fn $method<V>(self, $($($arg: $ty,)+)? visitor: V) -> Result<V::Value, Self::Error>
            where
                V: Visitor<'de>,
            {
                self.inner.$method($($($arg,)+)? visitor)
            }
        )*
    };
}

macro_rules! guard_map_key {
    ($($method:ident,)*) => {
        $(
            fn $method<V>(self, visitor: V) -> Result<V::Value, Self::Error>
            where
                V: Visitor<'de>,
            {
                let counter = self.inner.counter;
                self.inner.$method(MapKeyVisitor { inner: visitor, counter })
            }
        )*
    };
}

#[expect(
    clippy::missing_trait_methods,
    reason = "Serde-private provided hooks are unnameable; every public method is forwarded"
)]
impl<'de, D: Deserializer<'de>, C: NodeAccounting> Deserializer<'de>
    for MapKeyDeserializer<'_, D, C>
{
    type Error = D::Error;

    guard_map_key!(
        deserialize_any,
        deserialize_str,
        deserialize_string,
        deserialize_identifier,
    );

    forward_map_key!(
        deserialize_bool,
        deserialize_i8,
        deserialize_i16,
        deserialize_i32,
        deserialize_i64,
        deserialize_i128,
        deserialize_u8,
        deserialize_u16,
        deserialize_u32,
        deserialize_u64,
        deserialize_u128,
        deserialize_f32,
        deserialize_f64,
        deserialize_char,
        deserialize_bytes,
        deserialize_byte_buf,
        deserialize_option,
        deserialize_unit,
        deserialize_seq,
        deserialize_map,
        deserialize_ignored_any,
        deserialize_unit_struct<name: &'static str>,
        deserialize_newtype_struct<name: &'static str>,
        deserialize_tuple<len: usize>,
        deserialize_tuple_struct<name: &'static str, len: usize>,
        deserialize_struct<name: &'static str, fields: &'static [&'static str]>,
        deserialize_enum<name: &'static str, variants: &'static [&'static str]>,
    );

    fn is_human_readable(&self) -> bool {
        self.inner.is_human_readable()
    }
}

impl<V, C: NodeAccounting> MapKeyVisitor<'_, V, C> {
    fn check<E: DeError>(&self, key: &str) -> Result<(), E> {
        if let Some(marker) = RAW_VALUE_MARKERS
            .iter()
            .copied()
            .find(|marker| *marker == key)
        {
            self.counter.refuse_raw_value(marker);
            return Err(E::custom(RAW_VALUE_SENTINEL));
        }
        Ok(())
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "JSON object keys are strings; non-string visitors retain Serde's default refusals"
)]
impl<'de, V: Visitor<'de>, C: NodeAccounting> Visitor<'de> for MapKeyVisitor<'_, V, C> {
    type Value = V::Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.expecting(formatter)
    }

    fn visit_str<E: DeError>(self, v: &str) -> Result<Self::Value, E> {
        self.check(v)?;
        self.inner.visit_str(v)
    }

    fn visit_borrowed_str<E: DeError>(self, v: &'de str) -> Result<Self::Value, E> {
        self.check(v)?;
        self.inner.visit_borrowed_str(v)
    }

    fn visit_string<E: DeError>(self, v: String) -> Result<Self::Value, E> {
        self.check(&v)?;
        self.inner.visit_string(v)
    }
}

/// Visitor that walks and charges an ignored subtree without building
/// it.
///
/// This is the reason `deserialize_ignored_any` cannot be forwarded:
/// the pinned `serde_json` consumes a skipped subtree inside a single
/// `deserialize_ignored_any` call, so a wrapper that charges once and
/// delegates would charge 1 for an arbitrarily large subtree. Charging
/// happens in [`CountingDeserializer::deserialize_ignored_any`], which
/// calls `deserialize_any` with this visitor and walks every node.
struct CountingIgnoredAny<'c, C> {
    counter: &'c C,
}

impl<'de, S: DeserializeSeed<'de>, C: NodeAccounting> DeserializeSeed<'de>
    for CountingSeed<'_, S, C>
{
    type Value = S::Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        self.inner.deserialize(CountingDeserializer {
            inner: deserializer,
            counter: self.counter,
            charge: true,
        })
    }
}

impl<'de, S: DeserializeSeed<'de>, C: NodeAccounting> DeserializeSeed<'de> for KeySeed<'_, S, C> {
    type Value = S::Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        // The key was already charged by `next_key_seed`. Wrap it
        // anyway so a key that is itself a structure (not possible in
        // JSON, but the trait allows it) still charges its children,
        // but suppress the charge for the key value itself.
        self.inner.deserialize(CountingDeserializer {
            inner: deserializer,
            counter: self.counter,
            charge: false,
        })
    }
}

impl<'de, S: DeserializeSeed<'de>, C: NodeAccounting> DeserializeSeed<'de>
    for MapKeySeed<'_, S, C>
{
    type Value = S::Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        // KeySeed also handles bare enum tags, which must remain ordinary values.
        self.inner.deserialize(MapKeyDeserializer {
            inner: CountingDeserializer {
                inner: deserializer,
                counter: self.counter,
                charge: false,
            },
        })
    }
}

impl<'de, A: SeqAccess<'de>, C: NodeAccounting> SeqAccess<'de> for CountingSeqAccess<'_, A, C> {
    type Error = A::Error;

    fn next_element_seed<S>(&mut self, seed: S) -> Result<Option<S::Value>, Self::Error>
    where
        S: DeserializeSeed<'de>,
    {
        self.inner.next_element_seed(CountingSeed {
            inner: seed,
            counter: self.counter,
        })
    }

    fn next_element<T>(&mut self) -> Result<Option<T>, Self::Error>
    where
        T: Deserialize<'de>,
    {
        self.next_element_seed(PhantomData)
    }

    fn size_hint(&self) -> Option<usize> {
        self.inner.size_hint()
    }
}

impl<'de, A: MapAccess<'de>, C: NodeAccounting> MapAccess<'de> for CountingMapAccess<'_, A, C> {
    type Error = A::Error;

    fn next_key_seed<S>(&mut self, seed: S) -> Result<Option<S::Value>, Self::Error>
    where
        S: DeserializeSeed<'de>,
    {
        // An object key is a syntactic node in its own right, and it is
        // charged here rather than through the wrapped deserializer:
        // a struct target hands field names to a visitor that never
        // re-enters `deserialize_*`, so charging downstream would miss
        // them. `MapKeySeed` suppresses the downstream charge so the
        // `Value` target — which does re-enter — is not double-charged.
        let outcome = self.inner.next_key_seed(MapKeySeed {
            inner: seed,
            counter: self.counter,
        })?;
        if outcome.is_some() {
            self.counter.charge::<Self::Error>()?;
        }
        Ok(outcome)
    }

    fn next_value_seed<S>(&mut self, seed: S) -> Result<S::Value, Self::Error>
    where
        S: DeserializeSeed<'de>,
    {
        self.inner.next_value_seed(CountingSeed {
            inner: seed,
            counter: self.counter,
        })
    }

    fn next_key<K>(&mut self) -> Result<Option<K>, Self::Error>
    where
        K: Deserialize<'de>,
    {
        self.next_key_seed(PhantomData)
    }

    fn next_value<V>(&mut self) -> Result<V, Self::Error>
    where
        V: Deserialize<'de>,
    {
        self.next_value_seed(PhantomData)
    }

    fn next_entry_seed<K, V>(
        &mut self,
        kseed: K,
        vseed: V,
    ) -> Result<Option<(K::Value, V::Value)>, Self::Error>
    where
        K: DeserializeSeed<'de>,
        V: DeserializeSeed<'de>,
    {
        match self.next_key_seed(kseed)? {
            Some(key) => Ok(Some((key, self.next_value_seed(vseed)?))),
            None => Ok(None),
        }
    }

    fn next_entry<K, V>(&mut self) -> Result<Option<(K, V)>, Self::Error>
    where
        K: Deserialize<'de>,
        V: Deserialize<'de>,
    {
        self.next_entry_seed(PhantomData, PhantomData)
    }

    fn size_hint(&self) -> Option<usize> {
        self.inner.size_hint()
    }
}

impl<'c, 'de, A: EnumAccess<'de>, C: NodeAccounting + 'c> EnumAccess<'de>
    for CountingEnumAccess<'c, A, C>
{
    type Error = A::Error;
    type Variant = CountingVariantAccess<'c, A::Variant, C>;

    fn variant_seed<S>(self, seed: S) -> Result<(S::Value, Self::Variant), Self::Error>
    where
        S: DeserializeSeed<'de>,
    {
        // The tag is not charged here. An externally tagged enum has
        // two shapes and they are indistinguishable at this point:
        //
        //   `{"Tag":payload}`  the tag is an object key, a node of its
        //                      own on top of the enclosing object
        //   `"Tag"`            a bare string unit variant, where the
        //                      tag IS the whole value and the single
        //                      node it costs is already charged by
        //                      `deserialize_enum`
        //
        // `serde_json` hands the first shape a `VariantAccess` and the
        // second a `UnitVariantAccess`, but both read the tag off
        // their own deserializer without re-entering this wrapper, so
        // neither the seed nor the counter can tell them apart.
        // Charging unconditionally here would double-count the bare
        // form; the charge is therefore made by the
        // `CountingVariantAccess` methods that only the object shape
        // can reach.
        //
        // `KeySeed` still wraps the seed: it suppresses a charge for
        // the tag itself while leaving any children charged, matching
        // how `CountingMapAccess::next_key_seed` treats an object key.
        let (value, variant) = self.inner.variant_seed(KeySeed {
            inner: seed,
            counter: self.counter,
        })?;
        Ok((
            value,
            CountingVariantAccess {
                inner: variant,
                counter: self.counter,
            },
        ))
    }

    fn variant<V>(self) -> Result<(V, Self::Variant), Self::Error>
    where
        V: Deserialize<'de>,
    {
        self.variant_seed(PhantomData)
    }
}

impl<'de, A: VariantAccess<'de>, C: NodeAccounting> VariantAccess<'de>
    for CountingVariantAccess<'_, A, C>
{
    type Error = A::Error;

    /// A unit variant is the one shape whose tag is not charged here.
    ///
    /// As the bare string `"Tag"` the whole value is a single node,
    /// already charged by `deserialize_enum`, so there is nothing to
    /// add. The object spelling `{"Tag":null}` is two nodes more — the
    /// tag key and the `null` — but it is indistinguishable from the
    /// bare form at this point (see `variant_seed`), and charging
    /// would break the bare form, which is the spelling `serde_json`
    /// emits and the only one a round-trip produces.
    ///
    /// The residual undercount is bounded rather than exploitable:
    /// `serde_json` accepts only `null` in that position, rejecting
    /// `{"Tag":{}}`, `{"Tag":[]}` and `{"Tag":null,"x":1}` outright,
    /// so the object spelling is a fixed-size input that is always
    /// short by exactly two nodes and can carry no subtree.
    fn unit_variant(self) -> Result<(), Self::Error> {
        self.inner.unit_variant()
    }

    fn newtype_variant_seed<S>(self, seed: S) -> Result<S::Value, Self::Error>
    where
        S: DeserializeSeed<'de>,
    {
        // Reaching a newtype payload proves the `{"Tag":payload}`
        // shape: the bare-string form rejects every non-unit variant.
        // Charge the tag key the enclosing object carries; the payload
        // charges itself through `CountingSeed`.
        self.counter.charge::<Self::Error>()?;
        self.inner.newtype_variant_seed(CountingSeed {
            inner: seed,
            counter: self.counter,
        })
    }

    fn newtype_variant<T>(self) -> Result<T, Self::Error>
    where
        T: Deserialize<'de>,
    {
        self.newtype_variant_seed(PhantomData)
    }

    fn tuple_variant<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        // `{"Tag":[…]}` — two nodes before any element: the tag key,
        // and the array itself. `serde_json` reads that array by
        // calling `deserialize_seq` on its own deserializer rather
        // than re-entering this wrapper, so the array is charged here;
        // the elements charge themselves through the wrapped visitor's
        // `visit_seq`.
        self.counter.charge::<Self::Error>()?;
        self.counter.charge::<Self::Error>()?;
        self.inner.tuple_variant(
            len,
            CountingVisitor {
                inner: visitor,
                counter: self.counter,
            },
        )
    }

    fn struct_variant<V>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        // `{"Tag":{…}}` — the tag key plus the inner object, for the
        // same reason as `tuple_variant`: `serde_json` reaches the
        // inner object through `deserialize_struct` on its own
        // deserializer, so it is charged here and its keys and values
        // charge themselves through the wrapped visitor's `visit_map`.
        self.counter.charge::<Self::Error>()?;
        self.counter.charge::<Self::Error>()?;
        self.inner.struct_variant(
            fields,
            CountingVisitor {
                inner: visitor,
                counter: self.counter,
            },
        )
    }
}

/// Generate the `deserialize_*` forwarders.
///
/// Each charges one node for the value about to be produced, then
/// forwards to the inner deserializer with the caller's visitor wrapped
/// so nested values are charged too. `deserialize_ignored_any` is NOT
/// generated here — it is implemented by hand below.
macro_rules! forward_counting {
    ($($method:ident $(<$len:ident: $lty:ty>)? ,)*) => {
        $(
            fn $method<V>(                self,
                $($len: $lty,)?
                visitor: V,
            ) -> Result<V::Value, Self::Error>
            where V: Visitor<'de>,
            {
                self.charge_self()?;
                self.inner.$method(
                    $($len,)?
                    CountingVisitor { inner: visitor, counter: self.counter },
                )
            }
        )*
    };
}

impl<D, C: NodeAccounting> CountingDeserializer<'_, D, C> {
    /// Charge the node for this value, unless it is an object key the
    /// map accessor already charged.
    fn charge_self<E>(&self) -> Result<(), E>
    where
        E: DeError,
    {
        if self.charge {
            self.counter.charge()?;
        }
        Ok(())
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "the only unimplemented provided methods are serde-private \
              (`__`-prefixed, #[doc(hidden)], with unnameable types in \
              their signatures); every public one is forwarded above"
)]
impl<'de, D: Deserializer<'de>, C: NodeAccounting> Deserializer<'de>
    for CountingDeserializer<'_, D, C>
{
    type Error = D::Error;

    forward_counting!(
        deserialize_any,
        deserialize_bool,
        deserialize_i8,
        deserialize_i16,
        deserialize_i32,
        deserialize_i64,
        deserialize_i128,
        deserialize_u8,
        deserialize_u16,
        deserialize_u32,
        deserialize_u64,
        deserialize_u128,
        deserialize_f32,
        deserialize_f64,
        deserialize_char,
        deserialize_str,
        deserialize_string,
        deserialize_bytes,
        deserialize_byte_buf,
        deserialize_option,
        deserialize_unit,
        deserialize_seq,
        deserialize_map,
        deserialize_identifier,
        deserialize_unit_struct<name: &'static str>,
        deserialize_tuple<len: usize>,
    );

    /// Charge the newtype struct, refusing `serde_json`'s private
    /// raw-value markers.
    ///
    /// Hand-written rather than generated, because this is the one
    /// forwarder whose NAME decides whether the inner deserializer will
    /// re-enter the wrapper. For an ordinary newtype struct it does, and
    /// the wrapped visitor charges the inner value. For the raw-value
    /// markers it does not: `serde_json` recognises the name and
    /// captures the value's raw byte span from its own deserializer, so
    /// forwarding charged one node for an arbitrarily large subtree.
    /// See [`UnbudgetableRawValue`] for why the capture is refused
    /// rather than counted.
    fn deserialize_newtype_struct<V>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        if is_raw_value_marker(name) {
            self.counter.refuse_raw_value(name);
            return Err(Self::Error::custom(RAW_VALUE_SENTINEL));
        }
        self.charge_self()?;
        self.inner.deserialize_newtype_struct(
            name,
            CountingVisitor {
                inner: visitor,
                counter: self.counter,
            },
        )
    }

    fn deserialize_tuple_struct<V>(
        self,
        name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.charge_self()?;
        self.inner.deserialize_tuple_struct(
            name,
            len,
            CountingVisitor {
                inner: visitor,
                counter: self.counter,
            },
        )
    }

    fn deserialize_struct<V>(
        self,
        name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.charge_self()?;
        self.inner.deserialize_struct(
            name,
            fields,
            CountingVisitor {
                inner: visitor,
                counter: self.counter,
            },
        )
    }

    fn deserialize_enum<V>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.charge_self()?;
        self.inner.deserialize_enum(
            name,
            variants,
            CountingVisitor {
                inner: visitor,
                counter: self.counter,
            },
        )
    }

    /// Walk a subtree the target type discards, charging every node.
    ///
    /// Forwarding to the inner `deserialize_ignored_any` would be
    /// wrong: the pinned `serde_json` skips the whole subtree inside
    /// that one call without re-entering the wrapper, so an
    /// arbitrarily large ignored subtree would cost a single node.
    /// Driving `deserialize_any` with a counting walker instead makes
    /// the skipped subtree cost exactly what the same bytes cost in a
    /// retained position.
    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.charge_self()?;
        self.inner.deserialize_any(CountingIgnoredAny {
            counter: self.counter,
        })?;
        // The subtree is consumed and discarded; hand the caller the
        // unit it expects from an ignored value.
        visitor.visit_unit()
    }

    fn is_human_readable(&self) -> bool {
        self.inner.is_human_readable()
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "the only unimplemented provided methods are serde-private \
              (`__`-prefixed, #[doc(hidden)], with unnameable types in \
              their signatures); every public one is forwarded above"
)]
impl<'de, V: Visitor<'de>, C: NodeAccounting> Visitor<'de> for CountingVisitor<'_, V, C> {
    type Value = V::Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.expecting(formatter)
    }

    // Scalars were charged by the `deserialize_*` that produced them;
    // the visitor only has to forward. Only the structured arms need
    // to re-wrap, so the children get charged.

    fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        self.inner.visit_bool(v)
    }

    fn visit_i8<E>(self, v: i8) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        self.inner.visit_i8(v)
    }

    fn visit_i16<E>(self, v: i16) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        self.inner.visit_i16(v)
    }

    fn visit_i32<E>(self, v: i32) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        self.inner.visit_i32(v)
    }

    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        self.inner.visit_i64(v)
    }

    fn visit_i128<E>(self, v: i128) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        self.inner.visit_i128(v)
    }

    fn visit_u8<E>(self, v: u8) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        self.inner.visit_u8(v)
    }

    fn visit_u16<E>(self, v: u16) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        self.inner.visit_u16(v)
    }

    fn visit_u32<E>(self, v: u32) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        self.inner.visit_u32(v)
    }

    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        self.inner.visit_u64(v)
    }

    fn visit_u128<E>(self, v: u128) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        self.inner.visit_u128(v)
    }

    fn visit_f32<E>(self, v: f32) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        self.inner.visit_f32(v)
    }

    fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        self.inner.visit_f64(v)
    }

    fn visit_char<E>(self, v: char) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        self.inner.visit_char(v)
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        self.inner.visit_str(v)
    }

    fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        self.inner.visit_borrowed_str(v)
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        self.inner.visit_string(v)
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        self.inner.visit_bytes(v)
    }

    fn visit_borrowed_bytes<E>(self, v: &'de [u8]) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        self.inner.visit_borrowed_bytes(v)
    }

    fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        self.inner.visit_byte_buf(v)
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        self.inner.visit_none()
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        self.inner.visit_unit()
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        // `Option` is a wrapper, not a node of its own: the
        // `deserialize_option` that produced it already charged, so
        // the inner value must not charge again.
        self.inner.visit_some(CountingDeserializer {
            inner: deserializer,
            counter: self.counter,
            charge: false,
        })
    }

    fn visit_newtype_struct<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Same reasoning as `visit_some`: the newtype wrapper is not a
        // separate JSON node.
        self.inner.visit_newtype_struct(CountingDeserializer {
            inner: deserializer,
            counter: self.counter,
            charge: false,
        })
    }

    fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        self.inner.visit_seq(CountingSeqAccess {
            inner: seq,
            counter: self.counter,
        })
    }

    fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        self.inner.visit_map(CountingMapAccess {
            inner: map,
            counter: self.counter,
        })
    }

    fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
    where
        A: EnumAccess<'de>,
    {
        self.inner.visit_enum(CountingEnumAccess {
            inner: data,
            counter: self.counter,
        })
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "the only unimplemented provided methods are serde-private \
              (`__`-prefixed, #[doc(hidden)], with unnameable types in \
              their signatures); every public one is forwarded above"
)]
impl<'de, C: NodeAccounting> Visitor<'de> for CountingIgnoredAny<'_, C> {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("any JSON value")
    }

    // Scalars: the node was charged by the caller before this walker
    // was handed to `deserialize_any`, so the leaves are free here.

    fn visit_bool<E>(self, _v: bool) -> Result<(), E>
    where
        E: DeError,
    {
        Ok(())
    }

    fn visit_i8<E>(self, _v: i8) -> Result<(), E>
    where
        E: DeError,
    {
        Ok(())
    }

    fn visit_i16<E>(self, _v: i16) -> Result<(), E>
    where
        E: DeError,
    {
        Ok(())
    }

    fn visit_i32<E>(self, _v: i32) -> Result<(), E>
    where
        E: DeError,
    {
        Ok(())
    }

    fn visit_i64<E>(self, _v: i64) -> Result<(), E>
    where
        E: DeError,
    {
        Ok(())
    }

    fn visit_i128<E>(self, _v: i128) -> Result<(), E>
    where
        E: DeError,
    {
        Ok(())
    }

    fn visit_u8<E>(self, _v: u8) -> Result<(), E>
    where
        E: DeError,
    {
        Ok(())
    }

    fn visit_u16<E>(self, _v: u16) -> Result<(), E>
    where
        E: DeError,
    {
        Ok(())
    }

    fn visit_u32<E>(self, _v: u32) -> Result<(), E>
    where
        E: DeError,
    {
        Ok(())
    }

    fn visit_u64<E>(self, _v: u64) -> Result<(), E>
    where
        E: DeError,
    {
        Ok(())
    }

    fn visit_u128<E>(self, _v: u128) -> Result<(), E>
    where
        E: DeError,
    {
        Ok(())
    }

    fn visit_f32<E>(self, _v: f32) -> Result<(), E>
    where
        E: DeError,
    {
        Ok(())
    }

    fn visit_f64<E>(self, _v: f64) -> Result<(), E>
    where
        E: DeError,
    {
        Ok(())
    }

    fn visit_char<E>(self, _v: char) -> Result<(), E>
    where
        E: DeError,
    {
        Ok(())
    }

    fn visit_str<E>(self, _v: &str) -> Result<(), E>
    where
        E: DeError,
    {
        Ok(())
    }

    fn visit_borrowed_str<E>(self, _v: &'de str) -> Result<(), E>
    where
        E: DeError,
    {
        Ok(())
    }

    fn visit_string<E>(self, _v: String) -> Result<(), E>
    where
        E: DeError,
    {
        Ok(())
    }

    fn visit_bytes<E>(self, _v: &[u8]) -> Result<(), E>
    where
        E: DeError,
    {
        Ok(())
    }

    fn visit_borrowed_bytes<E>(self, _v: &'de [u8]) -> Result<(), E>
    where
        E: DeError,
    {
        Ok(())
    }

    fn visit_byte_buf<E>(self, _v: Vec<u8>) -> Result<(), E>
    where
        E: DeError,
    {
        Ok(())
    }

    fn visit_none<E>(self) -> Result<(), E>
    where
        E: DeError,
    {
        Ok(())
    }

    fn visit_unit<E>(self) -> Result<(), E>
    where
        E: DeError,
    {
        Ok(())
    }

    fn visit_some<D>(self, deserializer: D) -> Result<(), D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(CountingIgnoredAny {
            counter: self.counter,
        })
    }

    fn visit_newtype_struct<D>(self, deserializer: D) -> Result<(), D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(CountingIgnoredAny {
            counter: self.counter,
        })
    }

    /// Every element is a node, and each is walked rather than skipped.
    fn visit_seq<A>(self, mut seq: A) -> Result<(), A::Error>
    where
        A: SeqAccess<'de>,
    {
        while seq
            .next_element_seed(IgnoredSubtree {
                counter: self.counter,
            })?
            .is_some()
        {}
        Ok(())
    }

    /// Every key AND every value is a node.
    fn visit_map<A>(self, mut map: A) -> Result<(), A::Error>
    where
        A: MapAccess<'de>,
    {
        while map
            .next_key_seed(IgnoredSubtree {
                counter: self.counter,
            })?
            .is_some()
        {
            map.next_value_seed(IgnoredSubtree {
                counter: self.counter,
            })?;
        }
        Ok(())
    }

    /// An externally-tagged enum reaches this walker as a one-entry
    /// object: the tag is a node, the payload is a subtree. Errors
    /// from the payload must propagate — swallowing them would let an
    /// over-budget payload through, since the budget abort travels as
    /// an error.
    fn visit_enum<A>(self, data: A) -> Result<(), A::Error>
    where
        A: EnumAccess<'de>,
    {
        let ((), variant) = data.variant_seed(IgnoredSubtree {
            counter: self.counter,
        })?;
        VariantAccess::newtype_variant_seed(
            variant,
            IgnoredSubtree {
                counter: self.counter,
            },
        )
    }
}

/// One node inside an ignored subtree: charge it, then keep walking.
struct IgnoredSubtree<'c, C> {
    counter: &'c C,
}

impl<'de, C: NodeAccounting> DeserializeSeed<'de> for IgnoredSubtree<'_, C> {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<(), D::Error>
    where
        D: Deserializer<'de>,
    {
        self.counter.charge()?;
        deserializer.deserialize_any(CountingIgnoredAny {
            counter: self.counter,
        })
    }
}

/// Count value-start and object-key tokens without constructing a tree.
/// Serde validates their complete syntax in `count_json_nodes`.
fn check_node_budget(bytes: &[u8], max_nodes: u32) -> Result<u32, NodeBudgetExceeded> {
    scan_node_budget(bytes, max_nodes).map(|(nodes, _depth)| nodes)
}

fn scan_node_budget(bytes: &[u8], max_nodes: u32) -> Result<(u32, u32), NodeBudgetExceeded> {
    let mut input = bytes.iter().copied().peekable();
    let mut seen = 0_u32;
    let mut open_containers = 0_u32;
    let mut deepest = 0_u32;
    while let Some(byte) = input.next() {
        if matches!(byte, b']' | b'}') {
            open_containers = open_containers.saturating_sub(1);
            if open_containers == 0 {
                break;
            }
            continue;
        }
        if !matches!(
            byte,
            b'{' | b'[' | b'"' | b'-' | b'0'..=b'9' | b't' | b'f' | b'n'
        ) {
            continue;
        }
        if seen == max_nodes {
            return Err(NodeBudgetExceeded { budget: max_nodes });
        }
        seen = seen
            .checked_add(1)
            .ok_or(NodeBudgetExceeded { budget: max_nodes })?;

        match byte {
            b'"' => {
                // Escaped quotes and punctuation inside strings are not nodes.
                while let Some(character) = input.next() {
                    match character {
                        b'\\' => {
                            let _escaped = input.next();
                        }
                        b'"' => break,
                        _ => {}
                    }
                }
            }
            b'{' | b'[' => {
                open_containers = open_containers
                    .checked_add(1)
                    .ok_or(NodeBudgetExceeded { budget: max_nodes })?;
                deepest = deepest.max(open_containers);
            }
            _ => {
                // Numbers, true, false and null each cost one node. Do not
                // convert numbers: RawValue accepts e.g. 1e400 verbatim.
                while input.peek().is_some_and(|next| {
                    !matches!(next, b',' | b']' | b'}' | b' ' | b'\t' | b'\r' | b'\n')
                }) {
                    let _part = input.next();
                }
            }
        }
        // A trailing second value is a syntax error, not part of this
        // root's budget. Serde's end() remains responsible for refusing it.
        if open_containers == 0 {
            break;
        }
    }
    Ok((seen, deepest))
}

/// Count syntactic JSON nodes and validate the complete input without
/// constructing a target value. Every object key and value counts, including
/// duplicate keys and opaque extensions. Numbers are not converted, so valid
/// raw numeric tokens such as `1e400` remain acceptable.
///
/// This is the initial ingress/account-discovery pass, not a typed allocation
/// bound. A later typed parse must use the same bytes and its prepaid ingress
/// receipt. Unlike [`from_slice_within`], it does not ask a target type for any
/// private `RawValue` markers.
/// Reserved raw-value object keys are counted as ordinary source syntax here;
/// this is not permission to decode them through an unbounded typed parser.
///
/// # Errors
/// Returns [`BoundedJsonError::NodeBudget`] as soon as the token cap is exceeded,
/// or [`BoundedJsonError::Syntax`] for malformed or trailing input within the cap.
pub fn count_json_nodes(bytes: &[u8], max_nodes: u32) -> Result<u32, BoundedJsonError> {
    let nodes = check_node_budget(bytes, max_nodes).map_err(BoundedJsonError::NodeBudget)?;
    let mut syntax = serde_json::Deserializer::from_slice(bytes);
    IgnoredAny::deserialize(&mut syntax).map_err(BoundedJsonError::Syntax)?;
    syntax.end().map_err(BoundedJsonError::Syntax)?;
    Ok(nodes)
}

/// Original shape of one field in a JSON root object, without its contents.
/// Typed-only callers must use `Unavailable`, never infer `Absent` from an
/// `Option` that may have normalized JSON null. Repeated keys are deliberately
/// not resolved by first/last-key precedence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum JsonFieldShape {
    Unavailable,
    Absent,
    Null,
    Object,
    NonObject,
    Duplicate,
}

/// Perform the same node admission and complete syntax validation as
/// [`count_json_nodes`], retaining only one root field's shape. A non-object
/// root returns `Unavailable`; this pass does not replace typed validation.
///
/// Values are skipped or borrowed during the existing syntax pass, never
/// retained or numerically converted. Escaped keys use Serde's input-bounded
/// parser scratch; neither keys nor raw values escape the call.
///
/// # Errors
/// Returns the same node-budget/syntax refusals as [`count_json_nodes`].
pub fn count_json_nodes_with_field(
    bytes: &[u8],
    max_nodes: u32,
    field: &str,
) -> Result<(u32, JsonFieldShape), BoundedJsonError> {
    let nodes = check_node_budget(bytes, max_nodes).map_err(BoundedJsonError::NodeBudget)?;
    let mut syntax = serde_json::Deserializer::from_slice(bytes);
    let shape = if bytes.iter().find(|byte| !byte.is_ascii_whitespace()) == Some(&b'{') {
        syntax
            .deserialize_map(RootField { field })
            .map_err(BoundedJsonError::Syntax)?
    } else {
        IgnoredAny::deserialize(&mut syntax).map_err(BoundedJsonError::Syntax)?;
        JsonFieldShape::Unavailable
    };
    syntax.end().map_err(BoundedJsonError::Syntax)?;
    Ok((nodes, shape))
}

struct RootField<'a> {
    field: &'a str,
}

#[expect(
    clippy::missing_trait_methods,
    reason = "deserialize_map only invokes visit_map; other JSON shapes retain Serde's type refusals"
)]
impl<'de> Visitor<'de> for RootField<'_> {
    type Value = JsonFieldShape;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON object")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut shape = JsonFieldShape::Absent;
        while let Some(selected) = map.next_key_seed(FieldKey { field: self.field })? {
            if selected {
                // Borrowing RawValue preserves opaque large numeric tokens and
                // validates nested syntax without allocating a second tree.
                let raw: &RawValue = map.next_value()?;
                shape = if shape == JsonFieldShape::Absent {
                    match raw.get().as_bytes().first() {
                        Some(b'n') => JsonFieldShape::Null,
                        Some(b'{') => JsonFieldShape::Object,
                        _ => JsonFieldShape::NonObject,
                    }
                } else {
                    JsonFieldShape::Duplicate
                };
            } else {
                let _ignored: IgnoredAny = map.next_value()?;
            }
        }
        Ok(shape)
    }
}

struct FieldKey<'a> {
    field: &'a str,
}

impl<'de> DeserializeSeed<'de> for FieldKey<'_> {
    type Value = bool;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(self)
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "deserialize_str routes borrowed and escaped JSON keys through visit_str"
)]
impl Visitor<'_> for FieldKey<'_> {
    type Value = bool;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON object key")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        Ok(v == self.field)
    }
}

/// Parse `bytes` into `T`, aborting if the input exceeds `max_nodes`
/// syntactic JSON nodes.
///
/// A node is any JSON value (object, array, string, number, boolean,
/// null) plus any object key. Values the target type discards are
/// counted exactly as if they had been retained, so the budget bounds
/// the parse rather than the result.
///
/// For a Value target, [`NODE_RETAINED_BYTES`] charges retained container
/// memory. Input copies and parser scratch need separate charges, and arbitrary
/// typed targets can retain more per node. `RawValue` targets are refused.
///
/// # Errors
///
/// [`BoundedJsonError::NodeBudget`] if the input has more than
/// `max_nodes` nodes, [`BoundedJsonError::Syntax`] if the input is not
/// valid JSON or does not match `T`, or [`BoundedJsonError::RawValue`] if `T`
/// requests a private raw-value capture marker or an object key uses a reserved
/// raw-value marker. Ordinary string values containing those names are allowed.
pub fn from_slice_within<T>(bytes: &[u8], max_nodes: u32) -> Result<T, BoundedJsonError>
where
    T: DeserializeOwned,
{
    let counter = NodeCounter::new(max_nodes);
    from_slice_counted(bytes, &counter)
}

fn from_slice_counted<T, C>(bytes: &[u8], counter: &C) -> Result<T, BoundedJsonError>
where
    T: DeserializeOwned,
    C: NodeAccounting,
{
    from_slice_seed_counted(bytes, counter, PhantomData::<T>)
}

fn from_slice_seed_counted<'de, S, C>(
    bytes: &'de [u8],
    counter: &C,
    seed: S,
) -> Result<S::Value, BoundedJsonError>
where
    S: DeserializeSeed<'de>,
    C: NodeAccounting,
{
    let mut inner = serde_json::Deserializer::from_slice(bytes);
    let counting = CountingDeserializer {
        inner: &mut inner,
        counter,
        charge: true,
    };

    let value = match seed.deserialize(counting) {
        Ok(value) => value,
        Err(err) => return Err(classify(counter, err)),
    };

    // `serde_json` does not check for trailing bytes unless asked, so
    // `{"a":1} garbage` would otherwise parse clean.
    if let Err(err) = inner.end() {
        return Err(classify(counter, err));
    }
    Ok(value)
}

/// Parse `bytes` into a [`serde_json::Value`] under the same budget as
/// [`from_slice_within`].
///
/// # Errors
///
/// As [`from_slice_within`].
pub fn value_within(bytes: &[u8], max_nodes: u32) -> Result<serde_json::Value, BoundedJsonError> {
    from_slice_within::<serde_json::Value>(bytes, max_nodes)
}

/// Remaining visits for the typed pass over one delivered proprietary response.
/// This is not an allocation descriptor or permission to decode arbitrary DTOs.
/// A sealed response session owns this non-cloneable ledger and its original caps.
#[derive(Debug)]
pub struct SharedDecodeBudget {
    nodes: u32,
    bytes: usize,
}

impl SharedDecodeBudget {
    #[must_use]
    pub const fn new(nodes: u32, bytes: usize) -> Self {
        Self { nodes, bytes }
    }

    #[must_use]
    pub const fn remaining_nodes(&self) -> u32 {
        self.nodes
    }

    #[must_use]
    pub const fn remaining_bytes(&self) -> usize {
        self.bytes
    }
}

/// Fixed classification for the proprietary-only source and typed helpers.
#[derive(Debug)]
pub enum ProprietaryJsonError {
    Decode(BoundedJsonError),
    ByteBudget,
    DepthBudget,
}

/// Validate a source under the existing node semantics and a fixed 64-container
/// guard. This must not narrow the standard [`from_slice_within`] operation.
///
/// # Errors
/// Refuses excessive source nodes/depth and malformed or trailing JSON.
pub fn check_proprietary_source(bytes: &[u8], max_nodes: u32) -> Result<u32, ProprietaryJsonError> {
    from_proprietary_source_within::<IgnoredAny>(bytes, max_nodes).map(|(nodes, _ignored)| nodes)
}

/// Lexically admit source nodes and at most 64 containers, then deserialize one
/// qualified source target and validate the end of input. Borrowed raw spans are
/// permitted here because the lexical scan charges their complete source shape;
/// they are still refused by the candidate [`from_slice_shared_within`] parser.
///
/// This operation replaces standalone source validation, not follows it. Its
/// caller must precharge three source-length byte-visit allowances: one lexical
/// scan, one structural deserialization and up to one source length of UTF-8
/// validation for disjoint borrowed raw captures in Serde's slice reader.
/// Aggregate candidate typed parsing is a separate second structural pass with
/// its own shared visit ledger. These are scan allowances, not instruction
/// counts. This helper neither
/// qualifies target allocations nor exposes any sealed bidder response bytes.
///
/// # Errors
/// Refuses excessive source nodes/depth before invoking the target, or returns
/// syntax/type/trailing-input failures without publishing a partial target.
pub fn from_proprietary_source_within<'de, T>(
    bytes: &'de [u8],
    max_nodes: u32,
) -> Result<(u32, T), ProprietaryJsonError>
where
    T: Deserialize<'de>,
{
    from_proprietary_source_seed_within(bytes, max_nodes, PhantomData::<T>)
}

/// Source admission for a seed carrying the private session ledger. This is the
/// same lexical scan and single structural parse as
/// [`from_proprietary_source_within`], not an additional validation pass.
/// Target allocation qualification and source/candidate work precharges remain
/// the caller's responsibility.
///
/// # Errors
/// Refuses source nodes/depth before invoking the seed, or syntax/type/trailing
/// input without publishing the seed's partial output.
pub fn from_proprietary_source_seed_within<'de, S>(
    bytes: &'de [u8],
    max_nodes: u32,
    seed: S,
) -> Result<(u32, S::Value), ProprietaryJsonError>
where
    S: DeserializeSeed<'de>,
{
    let (nodes, deepest) = scan_node_budget(bytes, max_nodes)
        .map_err(|error| ProprietaryJsonError::Decode(BoundedJsonError::NodeBudget(error)))?;
    if deepest > 64 {
        return Err(ProprietaryJsonError::DepthBudget);
    }
    let mut syntax = serde_json::Deserializer::from_slice(bytes);
    let target = seed
        .deserialize(&mut syntax)
        .map_err(|error| ProprietaryJsonError::Decode(BoundedJsonError::Syntax(error)))?;
    syntax
        .end()
        .map_err(|error| ProprietaryJsonError::Decode(BoundedJsonError::Syntax(error)))?;
    Ok((nodes, target))
}

/// Decode a candidate against the session's remaining typed visits. Full input
/// bytes are charged before deserialization; observed nodes remain spent on
/// success or failure. Target allocation qualification is the caller's duty.
/// The sealed bidder API must not expose this as a generic response decoder.
///
/// # Errors
/// Refuses byte/node exhaustion, syntax/types and unchargeable raw capture.
pub fn from_slice_shared_within<T>(
    bytes: &[u8],
    budget: &mut SharedDecodeBudget,
) -> Result<T, ProprietaryJsonError>
where
    T: DeserializeOwned,
{
    from_slice_shared_seed_within(bytes, budget, PhantomData::<T>)
}

/// Seeded counterpart of [`from_slice_shared_within`] for closed typed sessions.
/// Full byte admission precedes the seed and all observed visits stay spent,
/// including syntax/type/structural failures. This does not fund arbitrary DTOs.
///
/// # Errors
/// As [`from_slice_shared_within`]; trailing JSON data is always refused.
pub fn from_slice_shared_seed_within<'de, S>(
    bytes: &'de [u8],
    budget: &mut SharedDecodeBudget,
    seed: S,
) -> Result<S::Value, ProprietaryJsonError>
where
    S: DeserializeSeed<'de>,
{
    let Some(remaining_bytes) = budget.bytes.checked_sub(bytes.len()) else {
        budget.bytes = 0;
        return Err(ProprietaryJsonError::ByteBudget);
    };
    budget.bytes = remaining_bytes;
    let counter = SharedNodeCounter::new(budget.nodes);
    let decoded = from_slice_seed_counted(bytes, &counter, seed);
    // A refused visit exhausts the available nodes, including at u32::MAX.
    // Successful and failed targets both keep their observed visits spent.
    budget.nodes = budget.nodes.saturating_sub(counter.observed_nodes());
    // A recovering seed may replace the original error or return success.
    // Only parser-owned observations establish these sticky refusals; neither
    // seed output nor diagnostic text can erase or manufacture their provenance.
    if counter.exceeded() {
        return Err(ProprietaryJsonError::Decode(BoundedJsonError::NodeBudget(
            NodeBudgetExceeded {
                budget: counter.limit(),
            },
        )));
    }
    if let Some(name) = counter.raw_refusal() {
        return Err(ProprietaryJsonError::Decode(BoundedJsonError::RawValue(
            UnbudgetableRawValue { name },
        )));
    }
    decoded.map_err(ProprietaryJsonError::Decode)
}

#[cfg(test)]
mod proprietary_tests {
    use super::{
        BUDGET_SENTINEL, BoundedJsonError, ProprietaryJsonError, RAW_VALUE_SENTINEL,
        SharedDecodeBudget, SharedNodeCounter, check_proprietary_source, classify,
        from_proprietary_source_within, from_slice_counted, from_slice_shared_within,
        from_slice_within,
    };
    use serde::de::{Error as _, IgnoredAny};
    use serde::{Deserialize, Deserializer};
    use serde_json::json;
    use serde_json::value::RawValue;

    #[test]
    fn seeded_candidate_keeps_failures_spent_and_refuses_trailing_data() {
        use super::from_slice_shared_seed_within;
        use std::marker::PhantomData;
        let mut budget = SharedDecodeBudget::new(3, 12);
        let error =
            from_slice_shared_seed_within(b"true", &mut budget, PhantomData::<u64>).unwrap_err();
        typed_refusal(&error);
        assert_eq!((budget.remaining_nodes(), budget.remaining_bytes()), (2, 8));
        assert!(from_slice_shared_seed_within(b"true", &mut budget, PhantomData::<bool>).unwrap());
        assert_eq!((budget.remaining_nodes(), budget.remaining_bytes()), (1, 4));
        assert!(matches!(
            from_slice_shared_seed_within(b"0 1", &mut budget, PhantomData::<u64>),
            Err(ProprietaryJsonError::Decode(BoundedJsonError::Syntax(_)))
        ));
        assert_eq!((budget.remaining_nodes(), budget.remaining_bytes()), (0, 1));
    }

    fn node_refusal(error: &ProprietaryJsonError) {
        assert!(matches!(
            error,
            ProprietaryJsonError::Decode(BoundedJsonError::NodeBudget(_))
        ));
    }

    #[test]
    fn shared_seed_recovery_cannot_erase_parser_owned_structural_refusals() {
        use super::from_slice_shared_seed_within;
        use serde::de::DeserializeSeed;
        use std::cell::Cell;

        struct RecoveringSeed<'a> {
            raw_capture: bool,
            return_ok: bool,
            refused: &'a Cell<bool>,
        }

        impl<'de> DeserializeSeed<'de> for RecoveringSeed<'_> {
            type Value = ();

            fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
                let failed = if self.raw_capture {
                    Box::<RawValue>::deserialize(deserializer).is_err()
                } else {
                    bool::deserialize(deserializer).is_err()
                };
                self.refused.set(failed);
                if self.return_ok {
                    Ok(())
                } else {
                    Err(D::Error::custom(
                        "fixed replacement without internal markers",
                    ))
                }
            }
        }

        for raw_capture in [false, true] {
            for return_ok in [false, true] {
                let nodes = if raw_capture { 8 } else { 0 };
                let mut budget = SharedDecodeBudget::new(nodes, 4);
                let refused = Cell::new(false);
                let error = from_slice_shared_seed_within(
                    b"true",
                    &mut budget,
                    RecoveringSeed {
                        raw_capture,
                        return_ok,
                        refused: &refused,
                    },
                )
                .expect_err("seed results cannot override sticky structural provenance");
                assert!(refused.get(), "the parser must refuse before seed recovery");
                if raw_capture {
                    assert!(matches!(
                        error,
                        ProprietaryJsonError::Decode(BoundedJsonError::RawValue(_))
                    ));
                } else {
                    node_refusal(&error);
                }
                assert_eq!(budget.remaining_nodes(), nodes);
                assert_eq!(budget.remaining_bytes(), 0);
            }
        }
    }

    fn typed_refusal(error: &ProprietaryJsonError) {
        assert!(matches!(
            error,
            ProprietaryJsonError::Decode(BoundedJsonError::Syntax(_))
        ));
    }

    #[test]
    fn shared_visits_are_cumulative_even_after_the_tree_is_dropped() {
        let mut budget = SharedDecodeBudget::new(3, 64);
        let tree: Vec<bool> =
            from_slice_shared_within(b"[true]", &mut budget).expect("first candidate");
        assert_eq!(tree, [true]);
        drop(tree);
        assert_eq!(budget.remaining_nodes(), 1);
        assert_eq!(budget.remaining_bytes(), 58);
        let next: bool = from_slice_shared_within(b"false", &mut budget).expect("last funded node");
        assert!(!next);
        assert_eq!(budget.remaining_nodes(), 0);
        assert_eq!(budget.remaining_bytes(), 53);
        node_refusal(
            &from_slice_shared_within::<bool>(b"true", &mut budget)
                .expect_err("no fresh node grant"),
        );
        assert_eq!(budget.remaining_nodes(), 0);
        assert_eq!(budget.remaining_bytes(), 49);
        from_slice_within::<bool>(b"true", 1).expect("standard standalone parser is unchanged");
        let counter = SharedNodeCounter::new(u32::MAX);
        counter.standard.seen.set(u32::MAX);
        let spoof = serde_json::Error::custom(BUDGET_SENTINEL);
        assert!(matches!(
            classify(&counter, spoof),
            BoundedJsonError::Syntax(_)
        ));
        assert!(matches!(
            from_slice_counted::<bool, _>(b"true", &counter)
                .expect_err("the refused visit cannot wrap"),
            BoundedJsonError::NodeBudget(_)
        ));
    }

    #[test]
    fn shared_typed_failure_spends_visits_and_preserves_the_next_candidate() {
        let mut budget = SharedDecodeBudget::new(2, 9);
        let invalid = serde_json::to_vec(&json!("bad")).expect("typed-failure fixture");
        typed_refusal(
            &from_slice_shared_within::<u64>(&invalid, &mut budget)
                .expect_err("invalid integer target"),
        );
        assert_eq!(budget.remaining_nodes(), 1);
        assert_eq!(budget.remaining_bytes(), 4);
        assert!(from_slice_shared_within::<bool>(b"true", &mut budget).expect("valid sibling"));
        assert_eq!(budget.remaining_nodes(), 0);
        assert_eq!(budget.remaining_bytes(), 0);

        for sentinel in [BUDGET_SENTINEL, RAW_VALUE_SENTINEL] {
            let input = serde_json::to_vec(&json!(sentinel)).expect("sentinel fixture");
            let bytes = input.len().checked_add(4).expect("fixture byte allowance");
            let mut remaining = SharedDecodeBudget::new(2, bytes);
            typed_refusal(
                &from_slice_shared_within::<u64>(&input, &mut remaining)
                    .expect_err("source text cannot establish refusal provenance"),
            );
            assert_eq!(remaining.remaining_nodes(), 1);
            assert!(
                from_slice_shared_within::<bool>(b"true", &mut remaining)
                    .expect("sentinel input cannot poison its sibling")
            );
            assert_eq!(remaining.remaining_nodes(), 0);
            assert_eq!(remaining.remaining_bytes(), 0);
        }
    }

    #[test]
    fn shared_syntax_failure_keeps_all_bytes_and_observed_nodes_spent() {
        let mut budget = SharedDecodeBudget::new(4, 10);
        // Deliberately malformed source: a JSON value cannot express this input.
        typed_refusal(
            &from_slice_shared_within::<Vec<bool>>(b"[true,", &mut budget)
                .expect_err("unfinished array"),
        );
        assert_eq!(budget.remaining_nodes(), 2);
        assert_eq!(budget.remaining_bytes(), 4);
        assert!(
            from_slice_shared_within::<bool>(b"true", &mut budget).expect("funded valid sibling")
        );
        assert_eq!(budget.remaining_nodes(), 1);
        assert_eq!(budget.remaining_bytes(), 0);
    }

    #[test]
    fn shared_byte_refusal_happens_before_any_target_nodes_and_cannot_rearm() {
        let mut budget = SharedDecodeBudget::new(8, 3);
        assert!(matches!(
            from_slice_shared_within::<bool>(b"true", &mut budget)
                .expect_err("byte cap before target"),
            ProprietaryJsonError::ByteBudget
        ));
        assert_eq!(budget.remaining_nodes(), 8);
        assert_eq!(budget.remaining_bytes(), 0);
        assert!(matches!(
            from_slice_shared_within::<u64>(b"0", &mut budget)
                .expect_err("failure cannot restore bytes"),
            ProprietaryJsonError::ByteBudget
        ));
        assert_eq!(budget.remaining_bytes(), 0);
    }

    #[test]
    fn proprietary_depth_is_63_64_65_without_changing_standard_depth() {
        for depth in [63_u32, 64, 65] {
            let mut value = json!(null);
            for _ in 0..depth {
                value = json!([value]);
            }
            let bytes = serde_json::to_vec(&value).expect("depth fixture");
            let result = check_proprietary_source(&bytes, 256);
            if depth <= 64 {
                assert_eq!(
                    result.expect("fixed guard is inclusive"),
                    depth.checked_add(1).expect("fixture nodes")
                );
            } else {
                assert!(matches!(
                    result.expect_err("one over proprietary depth"),
                    ProprietaryJsonError::DepthBudget
                ));
            }
            from_slice_within::<IgnoredAny>(&bytes, 256).expect("standard guard is not narrowed");
        }
    }

    #[test]
    fn proprietary_source_counts_duplicate_unknown_fields_and_refuses_trailing_syntax() {
        // Exact duplicate keys are the subject; json! would erase one occurrence.
        let source = br#"{"a":0,"a":[1,2]}"#;
        assert_eq!(
            check_proprietary_source(source, 7).expect("all source nodes counted"),
            7
        );
        node_refusal(&check_proprietary_source(source, 6).expect_err("one over node cap"));
        typed_refusal(
            &check_proprietary_source(b"{} trailing", 8).expect_err("whole source syntax"),
        );
    }

    #[test]
    fn proprietary_source_borrows_raw_value_without_building_a_tree() {
        let bytes = serde_json::to_vec(&json!({"fake_payload":[true, null]}))
            .expect("borrowed source fixture");
        let (nodes, raw) = from_proprietary_source_within::<&RawValue>(&bytes, 5)
            .expect("one borrowed source target");
        assert_eq!(nodes, 5);
        assert_eq!(raw.get().as_bytes(), bytes);
        assert_eq!(raw.get().as_ptr(), bytes.as_ptr());

        // Raw numeric spelling must not be converted to an intermediate f64.
        let (number_nodes, number) =
            from_proprietary_source_within::<&RawValue>(b"1e400", 1).expect("opaque numeric token");
        assert_eq!(number_nodes, 1);
        assert_eq!(number.get(), "1e400");
    }

    #[test]
    fn proprietary_source_escaped_and_duplicate_keys_keep_source_semantics() {
        #[derive(Debug, Deserialize)]
        struct BorrowedSource<'a> {
            #[serde(borrow)]
            candidate: &'a RawValue,
        }

        // Escaped field spelling and duplicate unknown keys are the input under
        // test; constructing a Value would normalize or discard them.
        let bytes = br#"{"candi\u0064ate":{"fake":1},"fake_unknown":null,"fake_unknown":null}"#;
        let (nodes, source) = from_proprietary_source_within::<BorrowedSource<'_>>(bytes, 9)
            .expect("escaped recognized field and duplicate unknowns");
        assert_eq!(nodes, 9);
        assert_eq!(
            source.candidate.get(),
            serde_json::to_string(&json!({"fake": 1_i32})).unwrap()
        );
        node_refusal(
            &from_proprietary_source_within::<BorrowedSource<'_>>(bytes, 8)
                .expect_err("unknown duplicate values still spend source nodes"),
        );

        let duplicate = br#"{"candidate":{},"candi\u0064ate":{}}"#;
        typed_refusal(
            &from_proprietary_source_within::<BorrowedSource<'_>>(duplicate, 5)
                .expect_err("recognized duplicate keys retain typed refusal"),
        );
    }

    #[test]
    fn proprietary_source_refuses_before_admission_or_rolls_back_the_target() {
        #[derive(Debug)]
        struct UnreachableTarget;

        impl<'de> Deserialize<'de> for UnreachableTarget {
            #[expect(
                clippy::panic_in_result_fn,
                reason = "the test sentinel detects target construction before source admission"
            )]
            fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                panic!("source limits must be checked before target deserialization");
            }
        }

        node_refusal(
            &from_proprietary_source_within::<UnreachableTarget>(b"true", 0)
                .expect_err("source node refusal before target"),
        );
        let mut nested = json!(null);
        for _ in 0..65_usize {
            nested = json!([nested]);
        }
        let deep = serde_json::to_vec(&nested).expect("depth refusal fixture");
        assert!(matches!(
            from_proprietary_source_within::<UnreachableTarget>(&deep, 256)
                .expect_err("source depth refusal before target"),
            ProprietaryJsonError::DepthBudget
        ));

        // Neither malformed input nor a valid prefix with trailing input may
        // publish its borrowed target to the caller.
        for invalid in [b"{\"fake\":".as_slice(), b"{} trailing"] {
            typed_refusal(
                &from_proprietary_source_within::<&RawValue>(invalid, 8)
                    .expect_err("whole-source failure discards the target"),
            );
        }
    }
}

/// Decide whether a `serde_json` failure was the budget abort or a
/// genuine syntax/type error.
///
/// The counter is consulted first: only a parse that actually passed
/// its budget can have raised the sentinel, so an input that happens to
/// contain the sentinel text cannot be mistaken for one.
fn classify<C: NodeAccounting>(counter: &C, err: serde_json::Error) -> BoundedJsonError {
    if counter.exceeded() && err.to_string().contains(BUDGET_SENTINEL) {
        return BoundedJsonError::NodeBudget(NodeBudgetExceeded {
            budget: counter.limit(),
        });
    }
    // Serde type errors can quote arbitrary input, including our marker.
    // Only an observed raw-value request or reserved object key establishes refusal.
    if let Some(name) = counter.raw_refusal() {
        return BoundedJsonError::RawValue(UnbudgetableRawValue { name });
    }
    BoundedJsonError::Syntax(err)
}

/// The raw byte-string JSON fixtures in this module are literal on
/// purpose, and none of them may become `serde_json::json!`.
///
/// Three reasons, all of which apply somewhere below:
///
/// - **The input is the unit under test.** `from_slice_within` and
///   `value_within` take `&[u8]`, and what is asserted is the
///   *syntactic node count of those exact bytes* — 14 for `FIXTURE`, 3
///   for `{"New":1}`, 5 for `{"Pair":[1,2]}`, and so on, each
///   hand-counted in the assertion's own comment. `json!` renders its
///   own spacing, so a conversion would silently re-point every one of
///   those numbers at a different byte string.
/// - **Several are `const`s of `&'static [u8]`.** `json!` yields a
///   `Value`; the `const` would have to become a call site doing
///   `json!(…).to_string().into_bytes()`.
/// - **Several are deliberately malformed** (`{"known": }`, `{"a":[1,2`,
///   `{"Unit":{}}`) so the parser must reject them. `json!` cannot
///   express input that does not parse — it would not compile.
///
/// The `format!` probes are the same case as the first: they vary one
/// dimension (nesting depth, element count) to prove the node charge
/// moves one-for-one with it, which is a claim about the rendered text.
#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use serde_json::Value;
    use serde_json::value::RawValue;
    use std::collections::BTreeMap;
    use std::fmt::Write as _;

    #[derive(Debug, Deserialize)]
    struct OneKnownField {
        known: u32,
    }

    #[derive(Debug, Deserialize)]
    struct RawField {
        extra: Box<RawValue>,
    }

    #[test]
    fn root_field_inspection_preserves_shape_without_normalizing_null() {
        for (input, expected, nodes) in [
            (serde_json::json!({}), JsonFieldShape::Absent, 1_u32),
            (serde_json::json!({"ext": null}), JsonFieldShape::Null, 3),
            (serde_json::json!({"ext": {}}), JsonFieldShape::Object, 3),
            (
                serde_json::json!({"ext": {"nested": [null, {}]}}),
                JsonFieldShape::Object,
                7,
            ),
            (serde_json::json!({"ext": []}), JsonFieldShape::NonObject, 3),
            (
                serde_json::json!({"ext": true}),
                JsonFieldShape::NonObject,
                3,
            ),
            (
                serde_json::json!({"ext": 1_i32}),
                JsonFieldShape::NonObject,
                3,
            ),
            (
                serde_json::json!({"ext": "{}"}),
                JsonFieldShape::NonObject,
                3,
            ),
            (
                serde_json::json!({"nested": {"ext": null}}),
                JsonFieldShape::Absent,
                5,
            ),
            (serde_json::json!([{}]), JsonFieldShape::Unavailable, 2),
            (serde_json::json!(null), JsonFieldShape::Unavailable, 1),
        ] {
            let bytes = serde_json::to_vec(&input).unwrap();
            assert_eq!(
                count_json_nodes_with_field(&bytes, nodes, "ext").unwrap(),
                (nodes, expected),
                "{input}"
            );
            assert!(matches!(
                count_json_nodes_with_field(&bytes, nodes - 1, "ext"),
                Err(BoundedJsonError::NodeBudget(_))
            ));
        }
        assert_eq!(size_of::<JsonFieldShape>(), 1);
    }

    #[test]
    fn root_field_inspection_decodes_keys_and_never_chooses_a_duplicate() {
        // Value would erase duplicate keys and original key escaping.
        for (bytes, expected, nodes) in [
            (
                b"{\"\\u0065xt\":null}".as_slice(),
                JsonFieldShape::Null,
                3_u32,
            ),
            (
                b"{\"ext\":null,\"\\u0065xt\":{}}".as_slice(),
                JsonFieldShape::Duplicate,
                5,
            ),
            (
                b"{\"ext\":{},\"ext\":null}".as_slice(),
                JsonFieldShape::Duplicate,
                5,
            ),
            (
                b"{\"ext\":1e400,\"ignored\":1e400}".as_slice(),
                JsonFieldShape::NonObject,
                5,
            ),
            (b"1e400".as_slice(), JsonFieldShape::Unavailable, 1),
        ] {
            assert_eq!(count_json_nodes(bytes, nodes).unwrap(), nodes);
            assert_eq!(
                count_json_nodes_with_field(bytes, nodes, "ext").unwrap(),
                (nodes, expected)
            );
        }
    }

    #[test]
    fn root_field_inspection_validates_unselected_content_and_trailing_input() {
        for bytes in [
            b"{\"ext\":{},\"ignored\":[1e]}".as_slice(),
            b"{\"ext\":null,\"ext\":}".as_slice(),
            b"{\"ext\":{}} []".as_slice(),
            b"{\"ext\":{\"bad\":}}".as_slice(),
            b"{\"\\q\":0}".as_slice(),
            b"[1e]".as_slice(),
            b"true false".as_slice(),
        ] {
            assert!(matches!(
                count_json_nodes_with_field(bytes, 100, "ext"),
                Err(BoundedJsonError::Syntax(_))
            ));
        }
        assert!(matches!(
            count_json_nodes_with_field(b"{\"ext\":{},\"ignored\":[1e]}", 1, "ext"),
            Err(BoundedJsonError::NodeBudget(_))
        ));
    }

    #[test]
    fn bounded_json_raw_numbers_preserve_syntax_without_numeric_conversion() {
        // Value/json! cannot represent this valid opaque numeric token.
        let bytes = br#"{"extra":1e400}"#;
        let plain: RawField = serde_json::from_slice(bytes).expect("valid raw number");
        assert_eq!(
            count_json_nodes(bytes, 3).expect("three syntax nodes fit"),
            3
        );
        assert_eq!(plain.extra.get(), "1e400");
        assert!(matches!(
            from_slice_within::<RawField>(bytes, 3),
            Err(BoundedJsonError::RawValue(_))
        ));
        assert_eq!(
            count_json_nodes(b"1e400", 1).expect("one raw numeric token fits"),
            1
        );
        let scalar: Box<RawValue> = serde_json::from_slice(b"1e400").expect("valid raw number");
        assert_eq!(scalar.get(), "1e400");
        assert!(matches!(
            value_within(bytes, 3),
            Err(BoundedJsonError::Syntax(_))
        ));
    }

    #[test]
    fn raw_value_marker_in_a_type_error_is_not_a_refusal() {
        let input = serde_json::to_vec(RAW_VALUE_SENTINEL).expect("string serializes");
        assert!(matches!(
            from_slice_within::<u32>(&input, 10),
            Err(BoundedJsonError::Syntax(_))
        ));
    }

    #[test]
    fn bounded_json_raw_fields_use_syntactic_not_synthetic_nodes() {
        for (value, nodes) in [
            (serde_json::json!({"extra": {}}), 3_u32),
            (
                serde_json::json!({"extra": {"deep": [{}, null, true]}}),
                8_u32,
            ),
        ] {
            let bytes = serde_json::to_vec(&value).expect("fixture serializes");
            assert_eq!(exact_node_count::<Value>(&bytes), nodes);
            assert_eq!(
                count_json_nodes(&bytes, nodes).expect("exact cap fits"),
                nodes
            );
            let parsed: RawField = serde_json::from_slice(&bytes).expect("preflighted typed parse");
            let retained: Value = serde_json::from_str(parsed.extra.get()).expect("raw JSON");
            assert_eq!(retained, value["extra"]);
            assert!(matches!(
                count_json_nodes(&bytes, nodes - 1),
                Err(BoundedJsonError::NodeBudget(_))
            ));
        }
    }

    #[test]
    fn bounded_json_token_counts_cover_scalars_escaping_and_container_keys() {
        for (value, nodes) in [
            (serde_json::json!(null), 1_u32),
            (serde_json::json!(true), 1),
            (serde_json::json!(false), 1),
            (serde_json::json!(-12.5e-2_f64), 1),
            (serde_json::json!("\\\"[]{}true,false:0\n\t\u{2603}"), 1),
            (
                serde_json::json!([null, true, false, -1_i32, 1.25_f64, "[]{}\\\"", {}, []]),
                9,
            ),
            (
                serde_json::json!({"": "", "\\\"escaped": [], "nested": {"[": [true, false]}}),
                11,
            ),
        ] {
            let bytes = serde_json::to_vec(&value).expect("fixture serializes");
            assert_eq!(exact_node_count::<Value>(&bytes), nodes);
            assert_eq!(count_json_nodes(&bytes, nodes).unwrap(), nodes);
            assert!(matches!(
                count_json_nodes(&bytes, nodes - 1),
                Err(BoundedJsonError::NodeBudget(_))
            ));
        }
        // Preserve duplicate keys in the input instead of collapsing them in json!.
        let duplicate_keys = br#"{"same":0,"same":1}"#;
        assert_eq!(exact_node_count::<Value>(duplicate_keys), 5);
        assert_eq!(count_json_nodes(duplicate_keys, 5).unwrap(), 5);
        assert!(matches!(
            count_json_nodes(duplicate_keys, 4),
            Err(BoundedJsonError::NodeBudget(_))
        ));
    }

    #[test]
    fn bounded_json_token_preflight_never_replaces_syntax_validation() {
        for bytes in [
            b"[1e]".as_slice(),
            b"[1 2]".as_slice(),
            b"{\"a\":truefalse}".as_slice(),
            b"\"unterminated\\".as_slice(),
            b"\"bad\\q\"".as_slice(),
            b"{} []".as_slice(),
        ] {
            assert!(matches!(
                count_json_nodes(bytes, 100),
                Err(BoundedJsonError::Syntax(_))
            ));
        }
    }

    #[test]
    fn bounded_json_trailing_tokens_do_not_spend_the_root_budget() {
        for bytes in [
            b"{} []".as_slice(),
            b"true false".as_slice(),
            b"\"a\" \"b\"".as_slice(),
        ] {
            assert!(matches!(
                count_json_nodes(bytes, 1),
                Err(BoundedJsonError::Syntax(_))
            ));
            assert!(matches!(
                value_within(bytes, 1),
                Err(BoundedJsonError::Syntax(_))
            ));
        }
    }

    /// The fixture both boundary tests measure: one retained field and
    /// a nested subtree the typed target discards.
    const FIXTURE: &[u8] = br#"{"known":1,"extra":{"a":[1,2,3],"b":{"c":4}}}"#;

    /// Count the syntactic nodes in `bytes`: the smallest budget that
    /// parses is the node count. Probes upward from zero, so it also
    /// proves the budget is monotonic — every budget below the answer
    /// must fail and every budget at or above it must succeed.
    fn exact_node_count<T>(bytes: &[u8]) -> u32
    where
        T: DeserializeOwned,
    {
        const CEILING: u32 = 1_000;

        for budget in 0..=CEILING {
            match from_slice_within::<T>(bytes, budget) {
                Ok(_) => return budget,
                Err(BoundedJsonError::NodeBudget(exceeded)) => {
                    assert_eq!(exceeded.budget, budget, "error reports its own budget");
                }
                Err(err) => panic!("unexpected parse failure at budget {budget}: {err}"),
            }
        }
        panic!("input needs more than {CEILING} nodes; raise the probe ceiling")
    }

    #[test]
    fn bounded_json_counts_nested_skipped_subtrees_at_exact_boundary() {
        let exact = exact_node_count::<OneKnownField>(FIXTURE);

        // AT the budget: Ok.
        let at = from_slice_within::<OneKnownField>(FIXTURE, exact)
            .expect("the exact budget must parse");
        assert_eq!(at.known, 1);

        // ONE node over the budget: NodeBudget, not Syntax.
        let one_short = exact.checked_sub(1).expect("fixture has at least one node");
        match from_slice_within::<OneKnownField>(FIXTURE, one_short) {
            Err(BoundedJsonError::NodeBudget(exceeded)) => {
                assert_eq!(exceeded.budget, one_short);
            }
            other => panic!("one node over budget should be NodeBudget, got {other:?}"),
        }

        // The skipped subtree must dominate the count. If
        // `deserialize_ignored_any` were delegated, the whole `extra`
        // subtree would cost 1 node and `exact` would be about 4.
        assert!(
            exact >= 10,
            "skipped subtree undercounted: exact={exact}, expected >= 10"
        );
    }

    #[test]
    fn bounded_json_counts_keys_and_ignored_any_via_both_entry_points() {
        let typed = exact_node_count::<OneKnownField>(FIXTURE);
        let untyped = exact_node_count::<Value>(FIXTURE);

        assert_eq!(
            typed, untyped,
            "typed and Value entry points must agree on the syntactic count"
        );

        // `value_within` is the `Value` entry point and must agree with
        // `from_slice_within::<Value>` at the boundary.
        value_within(FIXTURE, untyped).expect("parses at the exact budget");
        let one_short = untyped
            .checked_sub(1)
            .expect("fixture has at least one node");
        assert!(matches!(
            value_within(FIXTURE, one_short),
            Err(BoundedJsonError::NodeBudget(_))
        ));

        // Hand-count the shape to pin the accounting down:
        //   outer object                          1
        //   key "known" + value 1                 2
        //   key "extra" + its object              2
        //   key "a" + array + 3 elements          5
        //   key "b" + object + key "c" + value 4  4
        assert_eq!(typed, 14, "syntactic node count for the fixture");
    }

    /// A deeply nested subtree the target discards must cost one node
    /// per level. Delegating `deserialize_ignored_any` to the pinned
    /// `serde_json` would charge 1 regardless of depth, so the count
    /// would not move with `depth` at all.
    #[test]
    fn bounded_json_charges_ignored_subtrees_per_level_not_per_call() {
        let count_at = |depth: usize| -> u32 {
            let nested = format!("{}1{}", "[".repeat(depth), "]".repeat(depth));
            let bytes = format!(r#"{{"known":1,"extra":{nested}}}"#);
            exact_node_count::<OneKnownField>(bytes.as_bytes())
        };

        let shallow = count_at(4);
        let deep = count_at(64);

        assert_eq!(
            deep.checked_sub(shallow)
                .expect("deeper input counts more nodes"),
            60,
            "each extra nesting level must cost exactly one node \
             (shallow={shallow}, deep={deep})"
        );
    }

    #[test]
    fn bounded_json_syntax_errors_are_syntax_not_budget() {
        // Malformed input, with a budget far larger than it needs, so
        // the only possible failure is a syntax error.
        match from_slice_within::<OneKnownField>(br#"{"known": }"#, 1_000) {
            Err(BoundedJsonError::Syntax(_)) => {}
            other => panic!("malformed JSON should be Syntax, got {other:?}"),
        }

        // Truncated input is a syntax error too.
        match value_within(br#"{"a":[1,2"#, 1_000) {
            Err(BoundedJsonError::Syntax(_)) => {}
            other => panic!("truncated JSON should be Syntax, got {other:?}"),
        }

        // Well-formed but wrong type for the target is also Syntax.
        match from_slice_within::<OneKnownField>(br#"{"known":"not a number"}"#, 1_000) {
            Err(BoundedJsonError::Syntax(_)) => {}
            other => panic!("type mismatch should be Syntax, got {other:?}"),
        }

        // Trailing bytes after a complete value are a syntax error,
        // not a silently-accepted parse.
        match value_within(br#"{"known":1} trailing"#, 1_000) {
            Err(BoundedJsonError::Syntax(_)) => {}
            other => panic!("trailing bytes should be Syntax, got {other:?}"),
        }

        // A payload that embeds the sentinel text is still a normal
        // parse, not a spoofed budget error.
        let spoof = format!(r#"{{"known":1,"s":"{BUDGET_SENTINEL}"}}"#);
        value_within(spoof.as_bytes(), 1_000).expect("sentinel text is just a string value");
    }

    /// The four externally tagged variant shapes.
    #[derive(Debug, Deserialize, PartialEq, Eq)]
    enum Tagged {
        Unit,
        New(u32),
        Pair(u32, u32),
        Fields { a: u32 },
    }

    /// Internally tagged: the tag shares the payload's object.
    #[derive(Debug, Deserialize, PartialEq, Eq)]
    #[serde(tag = "kind")]
    enum Internally {
        Fields { a: u32 },
    }

    /// Adjacently tagged: tag and content are sibling keys.
    #[derive(Debug, Deserialize, PartialEq, Eq)]
    #[serde(tag = "kind", content = "body")]
    enum Adjacently {
        New(u32),
        Fields { a: u32 },
    }

    /// Untagged: the variant is chosen by shape alone.
    #[derive(Debug, Deserialize, PartialEq, Eq)]
    #[serde(untagged)]
    enum Untagged {
        Fields { a: u32 },
    }

    /// Assert a typed target and the `Value` target agree on `bytes`,
    /// that the agreed count parses, and that one node less is refused
    /// as a budget overrun rather than a syntax error. Returns the
    /// count so the caller can pin the exact number.
    fn assert_entry_points_agree<T>(bytes: &[u8], label: &str) -> u32
    where
        T: DeserializeOwned + fmt::Debug,
    {
        let typed = exact_node_count::<T>(bytes);
        let untyped = exact_node_count::<Value>(bytes);
        assert_eq!(
            typed,
            untyped,
            "{label}: typed and Value entry points disagree \
             (typed={typed}, value={untyped}) for {}",
            String::from_utf8_lossy(bytes)
        );

        // AT the exact budget both entry points parse.
        from_slice_within::<T>(bytes, typed)
            .unwrap_or_else(|err| panic!("{label}: exact budget {typed} must parse: {err}"));
        value_within(bytes, untyped)
            .unwrap_or_else(|err| panic!("{label}: Value at {untyped} must parse: {err}"));

        // ONE BELOW the exact budget both refuse, as NodeBudget.
        let short = typed.checked_sub(1).expect("every shape has >= 1 node");
        match from_slice_within::<T>(bytes, short) {
            Err(BoundedJsonError::NodeBudget(exceeded)) => {
                assert_eq!(exceeded.budget, short, "{label}: error reports its budget");
            }
            other => panic!("{label}: typed at {short} should be NodeBudget, got {other:?}"),
        }
        match value_within(bytes, short) {
            Err(BoundedJsonError::NodeBudget(_)) => {}
            other => panic!("{label}: Value at {short} should be NodeBudget, got {other:?}"),
        }
        typed
    }

    /// Each externally tagged shape charges its real syntactic nodes.
    ///
    /// Before the variant tag was charged, `{"New":1}` parsed under a
    /// budget of 2 while the `Value` entry point needed 3, and the
    /// tuple and struct shapes were short by two — their payload
    /// container was reached through `serde_json`'s own deserializer
    /// and never charged.
    #[test]
    fn bounded_json_externally_tagged_variants_agree_with_value() {
        // Bare-string unit variant: the tag is the whole value.
        assert_eq!(
            assert_entry_points_agree::<Tagged>(br#""Unit""#, "bare unit"),
            1,
            "a bare-string unit variant is one node"
        );

        // Newtype: enclosing object + tag key + payload.
        assert_eq!(
            assert_entry_points_agree::<Tagged>(br#"{"New":1}"#, "newtype"),
            3,
            "newtype variant: object + tag + payload"
        );

        // Tuple: enclosing object + tag key + array + two elements.
        assert_eq!(
            assert_entry_points_agree::<Tagged>(br#"{"Pair":[1,2]}"#, "tuple"),
            5,
            "tuple variant: object + tag + array + 2 elements"
        );

        // Struct: enclosing object + tag key + inner object + key +
        // value.
        assert_eq!(
            assert_entry_points_agree::<Tagged>(br#"{"Fields":{"a":1}}"#, "struct"),
            5,
            "struct variant: object + tag + inner object + key + value"
        );
    }

    /// A tuple or struct variant payload must cost one node per
    /// element, not a flat charge for the container.
    ///
    /// Charging only the container would leave the count flat as the
    /// payload grows, which is the amplification the budget exists to
    /// stop.
    #[test]
    fn bounded_json_variant_payloads_scale_with_their_size() {
        #[derive(Debug, Deserialize)]
        enum Wide {
            Items(Vec<u32>),
        }

        // `len` is a `u32` so the expected payload can be built
        // without a widening cast: the elements are `u32` themselves.
        let count_at = |len: u32| -> u32 {
            let expected = (0..len).collect::<Vec<u32>>();
            let elems = expected
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(",");
            let bytes = format!(r#"{{"Items":[{elems}]}}"#);
            let exact = assert_entry_points_agree::<Wide>(bytes.as_bytes(), "wide newtype");

            // The budget must bound the parse without truncating it:
            // confirm the whole payload is still delivered at the
            // exact budget, so the count above is the cost of a real
            // parse rather than of an early abort.
            let Wide::Items(items) = from_slice_within::<Wide>(bytes.as_bytes(), exact)
                .expect("the exact budget parses");
            assert_eq!(
                items, expected,
                "the full payload must survive the bounded parse"
            );
            exact
        };

        let small = count_at(4);
        let large = count_at(40);
        assert_eq!(
            large.checked_sub(small).expect("more elements cost more"),
            36,
            "each extra element must cost exactly one node \
             (small={small}, large={large})"
        );
    }

    /// The other three serde representations arrive as ordinary JSON
    /// objects and are already charged by the map accessor.
    ///
    /// They are asserted rather than special-cased: if serde ever
    /// routed one of them through `EnumAccess`, these would fail.
    #[test]
    fn bounded_json_other_enum_representations_agree_with_value() {
        // Internally tagged: one object, tag key + value, field + value.
        assert_eq!(
            assert_entry_points_agree::<Internally>(
                br#"{"kind":"Fields","a":1}"#,
                "internally tagged"
            ),
            5,
            "internally tagged: object + 2 keys + 2 values"
        );

        // Adjacently tagged newtype: object + 2 keys + 2 values.
        assert_eq!(
            assert_entry_points_agree::<Adjacently>(
                br#"{"kind":"New","body":1}"#,
                "adjacently tagged newtype"
            ),
            5,
            "adjacently tagged newtype: object + 2 keys + 2 values"
        );

        // Adjacently tagged struct: the content is a nested object.
        assert_eq!(
            assert_entry_points_agree::<Adjacently>(
                br#"{"kind":"Fields","body":{"a":1}}"#,
                "adjacently tagged struct"
            ),
            7,
            "adjacently tagged struct: + inner object, key and value"
        );

        // Untagged: indistinguishable from the bare struct it wraps.
        assert_eq!(
            assert_entry_points_agree::<Untagged>(br#"{"a":1}"#, "untagged"),
            3,
            "untagged: object + key + value"
        );
    }

    /// The object spelling of a unit variant is the one shape the
    /// typed path undercounts, and the gap is bounded.
    ///
    /// `serde_json` accepts only `null` in that position, so the shape
    /// is a fixed-size input short by exactly two nodes; it cannot
    /// carry a subtree. This test pins both halves of that claim, so
    /// the exception cannot silently widen.
    #[test]
    fn bounded_json_object_spelled_unit_variant_undercount_is_bounded() {
        const OBJECT_UNIT: &[u8] = br#"{"Unit":null}"#;

        let typed = exact_node_count::<Tagged>(OBJECT_UNIT);
        let untyped = exact_node_count::<Value>(OBJECT_UNIT);
        assert_eq!(typed, 1, "the object spelling is charged one node");
        assert_eq!(untyped, 3, "as a Value it is object + key + null");
        assert_eq!(count_json_nodes(OBJECT_UNIT, 3).unwrap(), 3);
        assert!(matches!(
            count_json_nodes(OBJECT_UNIT, 2),
            Err(BoundedJsonError::NodeBudget(_))
        ));
        assert_eq!(
            untyped.checked_sub(typed).expect("Value counts more"),
            2,
            "the undercount is exactly two nodes"
        );

        // The gap cannot grow: no payload other than `null` parses, so
        // no subtree can hide behind it.
        for payload in [
            &br#"{"Unit":{}}"#[..],
            &br#"{"Unit":[]}"#[..],
            &br#"{"Unit":{"a":[1,2,3]}}"#[..],
            &br#"{"Unit":null,"x":1}"#[..],
        ] {
            match from_slice_within::<Tagged>(payload, 1_000) {
                Err(BoundedJsonError::Syntax(_)) => {}
                other => panic!(
                    "{} must be refused as Syntax, got {other:?}",
                    String::from_utf8_lossy(payload)
                ),
            }
        }

        // The bare spelling, which is what `serde_json` emits, is
        // exact.
        assert_eq!(
            exact_node_count::<Tagged>(br#""Unit""#),
            exact_node_count::<Value>(br#""Unit""#),
            "the bare spelling agrees exactly"
        );
    }

    /// A variant payload at the budget edge must abort as a budget
    /// overrun, not be mistaken for malformed input.
    #[test]
    fn bounded_json_variant_overrun_is_budget_not_syntax() {
        const INPUT: &[u8] = br#"{"Fields":{"a":1}}"#;
        for budget in 0..exact_node_count::<Tagged>(INPUT) {
            match from_slice_within::<Tagged>(INPUT, budget) {
                Err(BoundedJsonError::NodeBudget(exceeded)) => {
                    assert_eq!(exceeded.budget, budget);
                }
                other => panic!("budget {budget} should be NodeBudget, got {other:?}"),
            }
        }
    }

    /// A target field asking for a `RawValue` capture.
    ///
    /// `Box<RawValue>` is the spelling that reaches `serde_json`'s
    /// private newtype-struct marker, and it is the spelling
    /// `BidRequest`, `Imp`, `Device` and `User` all use for their `ext`
    /// fields — so this is the shape a future caller would route
    /// through the budget.
    #[derive(Deserialize)]
    struct RawExt {
        ext: Option<Box<RawValue>>,
    }

    impl RawExt {
        /// The captured span's length, for a row that has to say what a
        /// clean parse would have retained.
        ///
        /// Also what keeps the field live: a struct whose field is
        /// never read needs a `dead_code` exemption, and the point of
        /// these fixtures is the field's TYPE, so reading it is both
        /// honest and cheaper than an attribute.
        fn captured_len(&self) -> usize {
            self.ext.as_ref().map_or(0, |raw| raw.get().len())
        }
    }

    /// A raw-value capture in a sequence position.
    #[derive(Deserialize)]
    struct RawSeq {
        items: Vec<Box<RawValue>>,
    }

    impl RawSeq {
        /// How many captures the sequence held. Keeps the field read,
        /// for the reason [`RawExt::captured_len`] gives.
        fn captures(&self) -> usize {
            self.items.len()
        }
    }

    /// A raw-value capture in a map-value position.
    #[derive(Deserialize)]
    struct RawMap {
        m: BTreeMap<String, Box<RawValue>>,
    }

    impl RawMap {
        /// How many captures the map held. Keeps the field read, for
        /// the reason [`RawExt::captured_len`] gives.
        fn captures(&self) -> usize {
            self.m.len()
        }
    }

    /// An object of `entries` distinct keys, rendered as text.
    ///
    /// Built by `format!` rather than `serde_json::json!` for the
    /// reason the module's fixture note gives: what these rows assert
    /// is the node count of the exact rendered bytes, and `json!`
    /// renders its own spacing. A `Map` would also sort the keys,
    /// which is a second way the bytes could drift from the count.
    fn wide_object(entries: u32) -> String {
        let mut text = String::from("{\"a0\":0");
        for index in 1..entries {
            // `write!` into the buffer rather than `push_str(&format!())`:
            // the latter allocates a throwaway `String` per entry, which on
            // a 100_000-entry fixture is 100_000 allocations the row does
            // not need. Infallible for a `String` sink.
            write!(text, ",\"a{index}\":{index}").expect("writing to a String never fails");
        }
        text.push('}');
        text
    }

    /// A `RawValue` capture cannot smuggle an unbudgeted subtree past
    /// the counter.
    ///
    /// This is the bypass the refusal exists for. `serde_json` answers
    /// its own private newtype-struct marker by capturing the value's
    /// raw byte span from its own deserializer, without re-entering the
    /// counting wrapper — so forwarding the marker charged ONE node for
    /// the whole subtree. Against the unfixed parser this fixture
    /// parsed clean at a budget of 7, retaining a 1.4 MB,
    /// 200,001-node object.
    ///
    /// The budget is 7 and not 0, so the row cannot pass merely because
    /// nothing fits: 7 nodes comfortably funds the wrapper object and
    /// its key, which is what makes the refusal attributable to the
    /// marker rather than to the budget.
    #[test]
    fn a_raw_value_capture_is_refused_rather_than_reparsed_unbudgeted() {
        // 100_000 entries → 200_001 syntactic nodes inside `ext`.
        let body = format!("{{\"ext\":{}}}", wide_object(100_000));
        // Discriminated on the ERROR rather than the whole `Result`:
        // `RawExt` holds a `RawValue` and cannot derive `Debug`, and
        // deriving it would make the fixture claim a capability the
        // type under test does not have.
        // Mapped to the captured LENGTH rather than discarded, so a
        // regression reports what the clean parse retained instead of a
        // bare `Ok`.
        let refusal =
            from_slice_within::<RawExt>(body.as_bytes(), 7).map(|parsed| parsed.captured_len());
        match refusal {
            Err(BoundedJsonError::RawValue(raw)) => {
                assert_eq!(
                    raw.name(),
                    "$serde_json::private::RawValue",
                    "the refusal names the private marker it refused",
                );
            }
            other => panic!(
                "a raw-value capture must be refused, not captured unbudgeted. A clean \
                 parse here is the defect: {} bytes of `ext` retained at budget 7. Got \
                 {other:?}",
                body.len(),
            ),
        }
    }

    /// The refusal covers every position a capture can occupy, not just
    /// a direct struct field.
    ///
    /// A guard that checked only the field spelling would leave the
    /// sequence and map routes open, which is the same shape of gap the
    /// marker itself was: counted in one place and not another.
    #[test]
    fn a_raw_value_capture_is_refused_in_sequence_and_map_positions() {
        let subtree = wide_object(100_000);

        let in_seq = format!("{{\"items\":[{subtree}]}}");
        assert!(
            matches!(
                from_slice_within::<RawSeq>(in_seq.as_bytes(), 7).map(|parsed| parsed.captures()),
                Err(BoundedJsonError::RawValue(_)),
            ),
            "a capture reached through a sequence element is refused too",
        );

        let in_map = format!("{{\"m\":{{\"k\":{subtree}}}}}");
        assert!(
            matches!(
                from_slice_within::<RawMap>(in_map.as_bytes(), 7).map(|parsed| parsed.captures()),
                Err(BoundedJsonError::RawValue(_)),
            ),
            "a capture reached through a map value is refused too",
        );
    }

    fn assert_raw_capture_refused<'de, T: Deserialize<'de>>(bytes: &'de [u8]) {
        let counter = NodeCounter::new(7);
        let error = from_slice_seed_counted(bytes, &counter, PhantomData::<T>)
            .map(|_| ())
            .expect_err("raw capture must be refused");
        match error {
            BoundedJsonError::RawValue(raw) => {
                assert_eq!(raw.name(), "$serde_json::private::RawValue");
            }
            other @ (BoundedJsonError::Syntax(_) | BoundedJsonError::NodeBudget(_)) => {
                panic!("expected raw-value refusal, got {other:?}")
            }
        }
        // The newtype guard must refuse before raw capture reaches MapKey.
        assert_eq!(counter.seen.get(), 0);
    }

    #[test]
    fn borrowed_raw_value_capture_is_refused_before_charging() {
        let body = wide_object(8);
        let borrowed: &RawValue = serde_json::from_slice(body.as_bytes()).unwrap();
        assert_eq!(borrowed.get(), body);
        assert_raw_capture_refused::<&RawValue>(body.as_bytes());
    }

    #[test]
    fn owned_raw_value_capture_is_refused_before_charging() {
        let body = wide_object(8);
        let owned: Box<RawValue> = serde_json::from_slice(body.as_bytes()).unwrap();
        assert_eq!(owned.get(), body);
        assert_raw_capture_refused::<Box<RawValue>>(body.as_bytes());
    }

    fn fake_raw_marker_wire() -> Vec<u8> {
        let hidden = serde_json::json!([0_i32, 0_i32, 0_i32, 0_i32, 0_i32, 0_i32, 0_i32, 0_i32]);
        serde_json::to_vec(&serde_json::json!({
            "$serde_json::private::RawValue": hidden.to_string(),
        }))
        .expect("synthetic raw marker wire object")
    }

    #[test]
    fn raw_marker_wire_key_is_refused_before_flatten_can_expand_it() {
        #[derive(Debug, Deserialize)]
        struct FlattenedValue {
            #[serde(flatten)]
            extra: Value,
        }

        let hidden = serde_json::json!([0_i32, 0_i32, 0_i32, 0_i32, 0_i32, 0_i32, 0_i32, 0_i32]);
        for (wire, expected, nodes) in [
            (fake_raw_marker_wire(), hidden.clone(), 3),
            (
                serde_json::to_vec(&serde_json::json!({
                    "extra": {"$serde_json::private::RawValue": hidden.to_string()},
                }))
                .unwrap(),
                serde_json::json!({"extra": hidden}),
                5,
            ),
        ] {
            // Flatten buffers Content before Value can perform an unwrapped parse.
            let plain: FlattenedValue = serde_json::from_slice(&wire).unwrap();
            assert_eq!(plain.extra, expected);
            assert_eq!(count_json_nodes(&wire, nodes).unwrap(), nodes);
            match from_slice_within::<FlattenedValue>(&wire, nodes) {
                Err(BoundedJsonError::RawValue(raw)) => {
                    assert_eq!(raw.name(), "$serde_json::private::RawValue");
                }
                other => panic!("flatten must not expand an encoded raw value: {other:?}"),
            }
            let mut budget = SharedDecodeBudget::new(nodes, wire.len());
            assert!(matches!(
                from_slice_shared_within::<FlattenedValue>(&wire, &mut budget),
                Err(ProprietaryJsonError::Decode(BoundedJsonError::RawValue(_))),
            ));
        }
    }

    #[test]
    fn raw_marker_wire_key_cannot_expand_a_value_past_node_budget() {
        let wire = fake_raw_marker_wire();
        // The wire has three nodes; its encoded array would materialize nine.
        assert_eq!(count_json_nodes(&wire, 3).unwrap(), 3);
        match from_slice_within::<Value>(&wire, 3) {
            Err(BoundedJsonError::RawValue(raw)) => {
                assert_eq!(raw.name(), "$serde_json::private::RawValue");
            }
            other => panic!("reserved wire key must refuse hidden expansion: {other:?}"),
        }
    }

    #[test]
    fn raw_marker_wire_key_cannot_expand_a_value_past_shared_budget() {
        let wire = fake_raw_marker_wire();
        let mut budget = SharedDecodeBudget::new(3, wire.len());
        let decoded = from_slice_shared_within::<Value>(&wire, &mut budget);
        assert_eq!(budget.remaining_bytes(), 0);
        match decoded {
            Err(ProprietaryJsonError::Decode(BoundedJsonError::RawValue(raw))) => {
                assert_eq!(raw.name(), "$serde_json::private::RawValue");
            }
            other => panic!("shared budget must refuse hidden expansion: {other:?}"),
        }
    }

    #[test]
    fn raw_marker_wire_key_guard_covers_nested_escaped_and_reordered_keys() {
        let marker = "$serde_json::private::RawValue";
        let hidden = serde_json::json!([0_i32, 0_i32, 0_i32, 0_i32]).to_string();
        for document in [
            serde_json::json!({"fake_nested": [{(marker): hidden.clone()}]}),
            serde_json::json!({(marker): hidden.clone(), "fake_after": 0_i32}),
            serde_json::json!({"!fake_before": 0_i32, (marker): hidden}),
        ] {
            let encoded = document.to_string();
            // Only the key spelling changes; json! constructs the wire document.
            let escaped = encoded.replacen(marker, "\\u0024serde_json::private::RawValue", 1);
            for wire in [encoded, escaped] {
                let nodes = count_json_nodes(wire.as_bytes(), 32).unwrap();
                assert!(matches!(
                    from_slice_within::<Value>(wire.as_bytes(), nodes),
                    Err(BoundedJsonError::RawValue(_)),
                ));
                let mut budget = SharedDecodeBudget::new(nodes, wire.len());
                assert!(matches!(
                    from_slice_shared_within::<Value>(wire.as_bytes(), &mut budget),
                    Err(ProprietaryJsonError::Decode(BoundedJsonError::RawValue(_))),
                ));
            }
        }
    }

    #[test]
    fn raw_marker_wire_key_guard_preserves_ordinary_string_values() {
        #[derive(Debug, Deserialize, PartialEq, Eq)]
        struct FakeWrapped(String);

        #[derive(Debug, Deserialize, PartialEq, Eq)]
        enum FakeTag {
            #[serde(rename = "$serde_json::private::RawValue")]
            Marker,
        }

        #[derive(Debug, Deserialize, PartialEq, Eq)]
        struct FakeObject {
            fake_marker: String,
        }

        let marker = "$serde_json::private::RawValue";
        let scalar = serde_json::to_vec(&serde_json::json!(marker)).unwrap();
        assert_eq!(from_slice_within::<String>(&scalar, 1).unwrap(), marker);
        assert_eq!(
            from_slice_within::<Option<String>>(&scalar, 1).unwrap(),
            Some(marker.to_owned()),
        );
        assert_eq!(
            from_slice_within::<FakeWrapped>(&scalar, 1).unwrap(),
            FakeWrapped(marker.to_owned()),
        );
        assert_eq!(
            from_slice_within::<FakeTag>(&scalar, 1).unwrap(),
            FakeTag::Marker,
        );
        let object = serde_json::to_vec(&serde_json::json!({"fake_marker": marker})).unwrap();
        assert_eq!(
            from_slice_within::<FakeObject>(&object, 3).unwrap(),
            FakeObject {
                fake_marker: marker.to_owned(),
            },
        );
        assert_eq!(
            from_slice_within::<Value>(&object, 3).unwrap(),
            serde_json::json!({"fake_marker": marker}),
        );
    }

    #[test]
    fn raw_marker_wire_key_guard_preserves_numeric_and_newtype_map_keys() {
        #[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
        struct FakeKey(u32);

        let marker = "$serde_json::private::RawValue";
        let wire = serde_json::to_vec(&serde_json::json!({
            "7": marker, "42": "fake_value",
        }))
        .unwrap();
        let numeric = from_slice_within::<BTreeMap<u32, String>>(&wire, 5).unwrap();
        assert_eq!(numeric.get(&7).map(String::as_str), Some(marker));
        assert_eq!(numeric.get(&42).map(String::as_str), Some("fake_value"));
        let wrapped = from_slice_within::<BTreeMap<FakeKey, String>>(&wire, 5).unwrap();
        assert_eq!(wrapped.get(&FakeKey(7)).map(String::as_str), Some(marker));
        assert_eq!(
            wrapped.get(&FakeKey(42)).map(String::as_str),
            Some("fake_value"),
        );
    }

    /// An ORDINARY newtype struct still parses and is still charged.
    ///
    /// Guards the refusal against passing for the wrong reason: a
    /// `deserialize_newtype_struct` that refused every name would
    /// satisfy the rows above while breaking every newtype target in
    /// the workspace.
    #[test]
    fn an_ordinary_newtype_struct_is_unaffected_by_the_raw_value_refusal() {
        #[derive(Debug, Deserialize, PartialEq, Eq)]
        struct Wrapped(u32);

        assert_eq!(
            from_slice_within::<Wrapped>(b"5", 4).ok(),
            Some(Wrapped(5)),
            "a newtype struct that is not a raw-value marker still parses, and still \
             carries its value through the wrapper",
        );
        // And is still CHARGED: a budget of 0 refuses it on the count,
        // which is how the row shows the forwarder kept its charge
        // rather than skipping it along with the refusal.
        assert!(
            matches!(
                from_slice_within::<Wrapped>(b"5", 0),
                Err(BoundedJsonError::NodeBudget(_)),
            ),
            "and is still charged a node, so budget 0 refuses it",
        );
    }
}
